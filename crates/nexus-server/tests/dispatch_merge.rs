//! Merge erledigter Tickets, Halte-Gründe, Merge-Session und Zielbranch; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchMerge.test.ts`.

mod common;
mod dispatch_support;

use std::path::Path;
use std::sync::Arc;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::{MoveOptions, TicketPatch};
use nexus_core::dispatch::{markers, MergePhase, RunEnd, RunKind, RunRecord, RunResult};
use nexus_server::dispatch::merge_git::{MergeGit, RealMergeGit};
use nexus_server::dispatch::merge_result::MergeRetryOutcome;
use nexus_server::dispatch::merge_service::MergeService;
use nexus_server::dispatch::status::DispatchMergeInfo;
use nexus_server::dispatch::DispatchNoticeKind;
use serde_json::json;

use dispatch_support::{DispatchFixture, FakeMergeGit, MergeAnswer, OrderedMerge, OK_OUTPUT};

// ----------------------------------------------------------------------------------------
// Helfer
// ----------------------------------------------------------------------------------------

/// Ausgecheckter Branch des Hauptordners
fn main_branch(f: &DispatchFixture) -> String {
    f.repo.git_out(&["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Inhalt einer Datei
fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("Datei lesen")
}

/// Startet ein bereites Ticket, lässt den Agenten fertig werden und wertet aus
async fn done_by_agent(f: &DispatchFixture, title: &str) -> String {
    done_by_agent_with(f, title, "feature.txt", "neu").await
}

/// Wie [`done_by_agent`], aber mit eigener Datei und eigenem Inhalt
async fn done_by_agent_with(f: &DispatchFixture, title: &str, file: &str, content: &str) -> String {
    let id = f.ready(title, TicketPatch::default()).await;
    f.tick().await;
    f.finish_as_agent(&id, file, content).await;
    f.end(&id, OK_OUTPUT, Some(0));
    id
}

/// Hauptordner bekommt einen eigenen Stand der Datei (Konflikt mit dem Ticket)
fn commit_on_main(f: &DispatchFixture, file: &str, content: &str) {
    std::fs::write(f.repo.path().join(file), content).expect("Datei im Hauptordner");
    f.repo.run_git(&["add", file]);
    f.repo
        .run_git(&["commit", "-q", "-m", &format!("haupt {file}")]);
}

/// Ticket bis zur laufenden Merge-Session bringen (Konflikt in feature.txt)
async fn in_merge_session(f: &DispatchFixture, title: &str) -> String {
    let id = f.ready(title, TicketPatch::default()).await;
    f.tick().await;
    commit_on_main(f, "feature.txt", "haupt");
    f.finish_as_agent(&id, "feature.txt", "ticket").await;
    f.end(&id, OK_OUTPUT, Some(0));
    f.tick().await;
    id
}

/// Löst den Konflikt im Worktree des Tickets, wie es die Merge-Session täte: Stand des Tickets
/// behalten
async fn resolve_in_worktree(f: &DispatchFixture, id: &str, target: &str) {
    let worktree = f.ticket(id).await.worktree.expect("Worktree des Tickets");
    f.repo.run_git(&[
        "-C",
        &worktree,
        "merge",
        "-q",
        "-s",
        "ours",
        "--no-edit",
        target,
    ]);
}

/// Zwei Tickets derselben Spec in einem gemeinsamen Worktree, beide erledigt und ausgewertet
/// (Konflikt optional in a.txt)
async fn shared_done(f: &DispatchFixture, conflict: bool) -> (String, String) {
    let first = f
        .ready(
            "Erstes",
            TicketPatch {
                spec: Some(Some("001-x".into())),
                spec_task: Some(Some("T001".into())),
                touches: Some(vec!["a".into()]),
                ..TicketPatch::default()
            },
        )
        .await;
    let second = f
        .ready(
            "Zweites",
            TicketPatch {
                spec: Some(Some("001-x".into())),
                spec_task: Some(Some("T002".into())),
                touches: Some(vec!["b".into()]),
                ..TicketPatch::default()
            },
        )
        .await;
    f.tick().await;
    if conflict {
        commit_on_main(f, "a.txt", "haupt");
    }
    f.finish_as_agent(&first, "a.txt", "a").await;
    f.end(&first, OK_OUTPUT, Some(0));
    // Erst jetzt ist der Worktree frei: das zweite Ticket tritt bei
    assert_eq!(f.tick().await.started, vec![second.clone()]);
    f.finish_as_agent(&second, "b.txt", "b").await;
    f.end(&second, OK_OUTPUT, Some(0));
    (first, second)
}

/// Merge-Sessions im Lauf-Speicher
fn merge_runs(f: &DispatchFixture) -> Vec<RunRecord> {
    f.ctx
        .stores
        .runs
        .list()
        .expect("Läufe")
        .into_iter()
        .filter(|run| run.kind == RunKind::Merge)
        .collect()
}

/// Merge-Auftrag eines Tickets
fn merge_entry(f: &DispatchFixture, id: &str) -> nexus_core::dispatch::MergeEntry {
    f.ctx
        .stores
        .state
        .read()
        .expect("Zustand")
        .merges
        .get(id)
        .cloned()
        .unwrap_or_else(|| panic!("kein Merge-Auftrag für {id}"))
}

/// Keine offenen Merge-Aufträge mehr
fn no_merges(f: &DispatchFixture) -> bool {
    f.ctx
        .stores
        .state
        .read()
        .expect("Zustand")
        .merges
        .is_empty()
}

/// Kommentare, die mit diesem Anfang beginnen
async fn comments_starting(f: &DispatchFixture, id: &str, head: &str) -> Vec<String> {
    f.comments(id)
        .await
        .into_iter()
        .filter(|body| body.starts_with(head))
        .collect()
}

/// Letzter Kommentar des Tickets
async fn last_comment(f: &DispatchFixture, id: &str) -> String {
    f.comments(id)
        .await
        .pop()
        .expect("mindestens ein Kommentar")
}

/// Fixture mit gelenkter Git-Seite des Merges
async fn with_fake_git(patch: serde_json::Value) -> (DispatchFixture, Arc<FakeMergeGit>) {
    let git = FakeMergeGit::new();
    let merge = Arc::new(MergeService::with_git(git.clone()));
    let f = DispatchFixture::create_with(patch, Some(merge)).await;
    (f, git)
}

// ----------------------------------------------------------------------------------------
// Merge im Hauptordner
// ----------------------------------------------------------------------------------------

#[tokio::test]
async fn mergt_nach_erledigt_in_den_ausgecheckten_branch_und_raumt_worktree_und_branch_auf() {
    let f = DispatchFixture::create(json!({})).await;
    let id = done_by_agent(&f, "Login").await;
    let worktree = f.ticket(&id).await.worktree.expect("Worktree");

    let report = f.tick().await;
    assert_eq!(report.merged, vec![id.clone()]);
    assert_eq!(read(&f.repo.path().join("feature.txt")), "neu");
    assert_eq!(
        f.repo
            .git_out(&["log", "-1", "--format=%P"])
            .split(' ')
            .count(),
        2
    );
    assert_eq!(
        f.repo.git_out(&["log", "-1", "--format=%s"]),
        format!("Merge {id} „Login“ (ticket/{id}-login)")
    );
    assert!(!Path::new(&worktree).exists());
    assert_eq!(
        f.repo
            .git_out(&["branch", "--list", &format!("ticket/{id}-login")]),
        ""
    );
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.branch, None);
    assert_eq!(ticket.worktree, None);
    assert!(last_comment(&f, &id).await.starts_with(&format!(
        "{} in {} (",
        markers::MERGED,
        main_branch(&f)
    )));
    let notice = f.notices.last().expect("Meldung");
    assert_eq!(notice.kind, DispatchNoticeKind::Merged);
    assert_eq!(notice.ticket_id.as_deref(), Some(id.as_str()));
    assert!(no_merges(&f));
}

#[tokio::test]
async fn lokale_anderungen_in_anderen_dateien_halten_den_merge_nicht_an_und_bleiben_stehen() {
    let f = DispatchFixture::create(json!({})).await;
    let id = done_by_agent(&f, "Bericht").await;
    std::fs::write(f.repo.path().join("lokal.txt"), "x").expect("lokale Datei");

    assert_eq!(f.tick().await.merged, vec![id.clone()]);
    assert_eq!(read(&f.repo.path().join("feature.txt")), "neu");
    assert_eq!(read(&f.repo.path().join("lokal.txt")), "x");
    assert_eq!(f.repo.git_out(&["status", "--porcelain"]), "?? lokal.txt");
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.branch, None);
}

#[tokio::test]
async fn uberschneidende_lokale_anderungen_warten_einmal_kommentiert_und_mergen_danach_von_selbst()
{
    let f = DispatchFixture::create(json!({})).await;
    commit_on_main(&f, "cfg.yaml", "alt");
    let id = done_by_agent_with(&f, "Konfig", "cfg.yaml", "ticket").await;
    std::fs::write(f.repo.path().join("cfg.yaml"), "lokal").expect("lokale Änderung");
    let notices = f.notices.all().len();

    assert!(f.tick().await.merged.is_empty());
    let again = f.tick().await;
    assert!(again.merged.is_empty());
    assert!(again.errors.is_empty(), "{again:?}");
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.blocked_reason, None);
    assert!(Path::new(&ticket.worktree.expect("Worktree")).exists());
    assert_eq!(
        comments_starting(&f, &id, "[dispatch] Merge wartet auf den Hauptordner").await,
        vec![
            "[dispatch] Merge wartet auf den Hauptordner: lokale Änderungen in cfg.yaml, die der \
Branch ebenfalls ändert – gemergt wird automatisch, sobald das erledigt ist."
                .to_string()
        ]
    );
    assert_eq!(f.notices.all().len(), notices);
    assert_eq!(merge_entry(&f, &id).phase, MergePhase::Waiting);
    assert_eq!(read(&f.repo.path().join("cfg.yaml")), "lokal");

    f.repo.run_git(&["checkout", "--", "cfg.yaml"]);
    assert_eq!(f.tick().await.merged, vec![id.clone()]);
    assert_eq!(read(&f.repo.path().join("cfg.yaml")), "ticket");
}

#[tokio::test]
async fn vorgemerkte_anderungen_lassen_den_merge_warten() {
    let f = DispatchFixture::create(json!({})).await;
    let id = done_by_agent(&f, "Index").await;
    std::fs::write(f.repo.path().join("lokal.txt"), "x").expect("lokale Datei");
    f.repo.run_git(&["add", "lokal.txt"]);

    assert!(f.tick().await.merged.is_empty());
    assert_eq!(
        last_comment(&f, &id).await,
        "[dispatch] Merge wartet auf den Hauptordner: vorgemerkte Änderungen (git add) in \
lokal.txt – gemergt wird automatisch, sobald das erledigt ist."
    );
    let report = f.service.retry_merge(&id).await.expect("Erneut mergen");
    assert_eq!(report.result.outcome, MergeRetryOutcome::Waiting);
    assert_eq!(
        report.result.text,
        format!(
            "{id}: Merge wartet auf den Hauptordner: vorgemerkte Änderungen (git add) in \
lokal.txt – gemergt wird automatisch, sobald das erledigt ist."
        )
    );

    f.repo.run_git(&["commit", "-q", "-m", "lokal"]);
    assert_eq!(f.tick().await.merged, vec![id]);
}

#[tokio::test]
async fn laufende_git_aktion_im_hauptordner_lasst_den_merge_warten_statt_zu_blockieren() {
    let f = DispatchFixture::create(json!({})).await;
    let id = done_by_agent(&f, "Rebase").await;
    let marker = f.repo.path().join(".git").join("MERGE_HEAD");
    std::fs::write(&marker, f.repo.git_out(&["rev-parse", "HEAD"])).expect("Marker schreiben");

    assert!(f.tick().await.merged.is_empty());
    assert_eq!(f.ticket(&id).await.status, TicketStatus::Done);
    assert_eq!(
        last_comment(&f, &id).await,
        "[dispatch] Merge wartet auf den Hauptordner: Im Hauptordner läuft gerade ein Merge. – \
gemergt wird automatisch, sobald das erledigt ist."
    );

    std::fs::remove_file(&marker).expect("Marker entfernen");
    assert_eq!(f.tick().await.merged, vec![id]);
}

// ----------------------------------------------------------------------------------------
// Squash
// ----------------------------------------------------------------------------------------

#[tokio::test]
async fn squash_ergibt_einen_commit_je_ticket_mit_trailer_und_raumt_auf() {
    let f = DispatchFixture::create(json!({ "mergeStrategy": "squash" })).await;
    let id = done_by_agent(&f, "Login").await;
    let ticket = f.ticket(&id).await;
    let branch = ticket.branch.clone().expect("Branch");
    let tip = f.repo.git_out(&["rev-parse", &branch]);
    let before = f.repo.git_out(&["rev-parse", "HEAD"]);

    assert_eq!(f.tick().await.merged, vec![id.clone()]);
    assert_eq!(read(&f.repo.path().join("feature.txt")), "neu");
    assert_eq!(f.repo.git_out(&["log", "-1", "--format=%P"]), before);
    assert_eq!(
        f.repo.git_out(&["log", "-1", "--format=%s"]),
        format!("{id} „Login“ (squash von ticket/{id}-login)")
    );
    assert_eq!(
        f.repo.git_out(&[
            "log",
            "-1",
            "--format=%(trailers:key=Harness-Squashed,valueonly)",
        ]),
        format!("ticket/{id}-login {tip}")
    );
    assert!(!Path::new(&ticket.worktree.expect("Worktree")).exists());
    assert_eq!(f.repo.git_out(&["branch", "--list", &branch]), "");
    let after = f.ticket(&id).await;
    assert_eq!(after.status, TicketStatus::Done);
    assert_eq!(after.branch, None);
    assert_eq!(after.worktree, None);
    let merged = last_comment(&f, &id).await;
    assert!(
        merged.starts_with(&format!("{} in {} (", markers::MERGED, main_branch(&f)))
            && merged.contains("aufgeräumt"),
        "{merged}"
    );
}

#[tokio::test]
async fn squash_fasst_den_branch_nach_der_merge_session_zusammen() {
    let f = DispatchFixture::create(json!({ "mergeStrategy": "squash" })).await;
    let id = in_merge_session(&f, "Konflikt").await;
    let ticket = f.ticket(&id).await;
    let branch = ticket.branch.clone().expect("Branch");
    let session = f.run(&id);
    assert_eq!(session.kind, RunKind::Merge);
    assert_eq!(session.target.as_deref(), Some(main_branch(&f).as_str()));
    assert_eq!(f.repo.git_out(&["status", "--porcelain"]), "");

    resolve_in_worktree(&f, &id, &main_branch(&f)).await;
    f.end(&id, OK_OUTPUT, Some(0));
    let before = f.repo.git_out(&["rev-parse", "HEAD"]);
    assert_eq!(f.tick().await.merged, vec![id.clone()]);
    assert_eq!(f.repo.git_out(&["log", "-1", "--format=%P"]), before);
    assert_eq!(
        f.repo.git_out(&["log", "-1", "--format=%s"]),
        format!("{id} „Konflikt“ (squash von {branch})")
    );
    assert_eq!(read(&f.repo.path().join("feature.txt")), "ticket");
    assert_eq!(f.repo.git_out(&["branch", "--list", &branch]), "");
}

// ----------------------------------------------------------------------------------------
// Merge-Session
// ----------------------------------------------------------------------------------------

#[tokio::test]
async fn startet_bei_konflikt_eine_merge_session_und_mergt_danach() {
    let f = DispatchFixture::create(json!({})).await;
    let id = in_merge_session(&f, "Konflikt").await;
    let ticket = f.ticket(&id).await;
    let session = f.run(&id);
    assert_eq!(session.kind, RunKind::Merge);
    assert_eq!(session.target.as_deref(), Some(main_branch(&f).as_str()));
    assert_eq!(session.cwd, ticket.worktree.clone().expect("Worktree"));
    assert_eq!(Some(session.branch.clone()), ticket.branch);
    assert!(
        session.args.iter().any(|arg| arg == "Bash(git merge:*)"),
        "{:?}",
        session.args
    );
    assert!(
        session
            .prompt
            .contains(&format!("git merge {}", main_branch(&f))),
        "{}",
        session.prompt
    );
    assert!(
        last_comment(&f, &id).await.starts_with(&format!(
            "{} in feature.txt beim Merge in ",
            markers::MERGE_CONFLICT
        )),
        "{}",
        last_comment(&f, &id).await
    );
    let entry = merge_entry(&f, &id);
    assert_eq!(entry.phase, MergePhase::Session);
    assert!(entry.session_used);
    assert_eq!(f.repo.git_out(&["status", "--porcelain"]), "");

    resolve_in_worktree(&f, &id, &main_branch(&f)).await;
    f.end(&id, OK_OUTPUT, Some(0));
    assert_eq!(f.tick().await.merged, vec![id]);
    assert_eq!(read(&f.repo.path().join("feature.txt")), "ticket");
}

#[tokio::test]
async fn blockiert_wenn_die_merge_session_den_konflikt_nicht_lost() {
    let f = DispatchFixture::create(json!({})).await;
    let id = in_merge_session(&f, "Ungelöst").await;
    f.end(&id, OK_OUTPUT, Some(0));
    f.tick().await;

    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    let reason = ticket.blocked_reason.expect("Grund");
    assert!(
        reason.starts_with("Merge: Die Merge-Session konnte den Konflikt nicht lösen"),
        "{reason}"
    );
}

#[tokio::test]
async fn blockiert_wenn_der_eigene_merge_nicht_zuruckgesetzt_werden_kann() {
    let (f, git) = with_fake_git(json!({})).await;
    let id = done_by_agent(&f, "Abbruch").await;
    // Ein scheiterndes `git merge --abort` lässt sich im Test nicht zuverlässig erzeugen; die
    // Git-Seite antwortet darum so, wie der Merger dann wirft
    git.answer_merge(MergeAnswer::AbortFailed { details: false });

    let report = f.tick().await;
    assert!(report.merged.is_empty());
    assert!(report.errors.is_empty(), "{report:?}");
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(
        ticket.blocked_reason.as_deref(),
        Some(
            "Merge: Der Hauptordner steht noch im Merge-Zustand: kaputt. Bitte von Hand \
aufräumen (git merge --abort)."
        )
    );
    assert!(no_merges(&f));
}

#[tokio::test]
async fn merge_session_endet_und_das_ticket_steht_nicht_mehr_auf_erledigt() {
    let f = DispatchFixture::create(json!({})).await;
    let id = in_merge_session(&f, "Zurück").await;
    f.backend
        .services
        .tickets
        .move_to(
            &id,
            TicketStatus::InProgress,
            &f.actor(),
            &MoveOptions {
                force: true,
                ..MoveOptions::default()
            },
        )
        .await
        .expect("zurück in Arbeit");
    f.end(&id, OK_OUTPUT, Some(0));
    f.tick().await;

    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::InProgress);
    assert_eq!(ticket.blocked_reason, None);
    assert!(no_merges(&f));
}

#[tokio::test]
async fn auftrag_in_phase_session_ohne_merge_lauf_gilt_als_ungelost() {
    let f = DispatchFixture::create(json!({})).await;
    let id = in_merge_session(&f, "Hängt").await;
    // Lauf verschwindet ohne Auswertung (z. B. Absturz zwischen Auswertung und Zustand)
    f.drop_run(&f.run(&id).id);
    f.tick().await;

    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(
        ticket.blocked_reason.as_deref(),
        Some("Merge: Die Merge-Session endete ohne Auswertung.")
    );
    assert!(no_merges(&f));
}

#[tokio::test]
async fn fachlicher_fehler_beim_auswerten_der_merge_session_blockiert() {
    let (f, git) = with_fake_git(json!({})).await;
    let id = in_merge_session(&f, "Kaputt").await;
    git.fail_contains("Git kaputt");
    f.end(&id, OK_OUTPUT, Some(0));
    f.tick().await;

    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(
        ticket.blocked_reason.as_deref(),
        Some("Merge: Die Merge-Session ließ sich nicht auswerten: Git kaputt")
    );
    assert!(no_merges(&f));
}

#[tokio::test]
async fn blockiert_wenn_der_zielbranch_nach_der_merge_session_gewechselt_wurde() {
    let f = DispatchFixture::create(json!({})).await;
    let id = in_merge_session(&f, "Ziel").await;
    let old = main_branch(&f);
    resolve_in_worktree(&f, &id, &old).await;
    f.repo.run_git(&["checkout", "-q", "-b", "anderer"]);
    f.end(&id, OK_OUTPUT, Some(0));

    let report = f.tick().await;
    assert!(report.merged.is_empty());
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(
        ticket.blocked_reason.as_deref(),
        Some(
            format!(
                "Merge: Zielbranch gewechselt (Konflikt gelöst gegen {old}, ausgecheckt ist \
anderer) – bitte Erneut mergen."
            )
            .as_str()
        )
    );
    assert_eq!(
        f.repo
            .git_out(&["log", "-1", "--format=%P"])
            .split(' ')
            .count(),
        1
    );
}

// ----------------------------------------------------------------------------------------
// Gemeinsamer Worktree
// ----------------------------------------------------------------------------------------

#[tokio::test]
async fn wartet_mit_dem_merge_bis_alle_tickets_des_gemeinsamen_worktrees_fertig_sind() {
    let f = DispatchFixture::create(json!({ "wip": 2, "joinWorktrees": "sameSpec" })).await;
    let first = f
        .ready(
            "Erstes",
            TicketPatch {
                spec: Some(Some("001-x".into())),
                spec_task: Some(Some("T001".into())),
                touches: Some(vec!["a".into()]),
                ..TicketPatch::default()
            },
        )
        .await;
    let second = f
        .ready(
            "Zweites",
            TicketPatch {
                spec: Some(Some("001-x".into())),
                spec_task: Some(Some("T002".into())),
                touches: Some(vec!["b".into()]),
                ..TicketPatch::default()
            },
        )
        .await;
    f.tick().await;
    f.finish_as_agent(&first, "a.txt", "a").await;
    f.end(&first, OK_OUTPUT, Some(0));

    let waiting = f.tick().await;
    assert!(waiting.merged.is_empty());
    assert_eq!(waiting.started, vec![second.clone()]);
    assert_eq!(
        f.ticket(&second).await.branch,
        f.ticket(&first).await.branch
    );
    // Zum Zeitpunkt der Auswertung war das zweite Ticket noch nicht beigetreten
    assert_eq!(
        last_comment(&f, &first).await,
        "[dispatch] Merge wartet auf weitere Tickets derselben Spec – der Branch wird gemergt, \
sobald alle Tickets des Worktrees fertig sind."
    );

    f.finish_as_agent(&second, "b.txt", "b").await;
    f.end(&second, OK_OUTPUT, Some(0));
    let mut merged = f.tick().await.merged;
    merged.sort();
    let mut both = vec![first, second];
    both.sort();
    assert_eq!(merged, both);
    assert!(f.repo.path().join("a.txt").exists() && f.repo.path().join("b.txt").exists());
}

#[tokio::test]
async fn kein_merge_solange_ein_ticket_des_gemeinsamen_worktrees_wieder_in_arbeit_ist() {
    let f = DispatchFixture::create(json!({ "wip": 2, "joinWorktrees": "sameSpec" })).await;
    let (first, second) = shared_done(&f, true).await;
    f.tick().await;
    let sessions = merge_runs(&f);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].ticket_id, first);

    // Während der Merge-Session geht das zweite Ticket zurück auf In Arbeit
    f.backend
        .services
        .tickets
        .move_to(
            &second,
            TicketStatus::InProgress,
            &f.actor(),
            &MoveOptions {
                force: true,
                ..MoveOptions::default()
            },
        )
        .await
        .expect("zurück in Arbeit");
    let head = f.repo.git_out(&["rev-parse", "HEAD"]);
    resolve_in_worktree(&f, &first, &main_branch(&f)).await;
    f.end(&first, OK_OUTPUT, Some(0));

    let report = f.tick().await;
    assert!(report.merged.is_empty());
    assert_eq!(f.repo.git_out(&["rev-parse", "HEAD"]), head);
    assert_eq!(f.ticket(&first).await.status, TicketStatus::Done);
    assert!(comments_starting(&f, &first, markers::MERGED)
        .await
        .is_empty());
    assert_eq!(merge_entry(&f, &first).phase, MergePhase::Waiting);
}

#[tokio::test]
async fn gemeinsamer_worktree_mit_konflikt_hat_genau_eine_merge_session() {
    let f = DispatchFixture::create(json!({ "wip": 2, "joinWorktrees": "sameSpec" })).await;
    let (first, second) = shared_done(&f, true).await;
    f.tick().await;
    let sessions = merge_runs(&f);
    assert_eq!(sessions.len(), 1);
    for id in [&first, &second] {
        let entry = merge_entry(&f, id);
        assert_eq!(entry.phase, MergePhase::Session);
        assert!(entry.session_used);
        assert_eq!(entry.target.as_deref(), Some(main_branch(&f).as_str()));
    }
    // Weiterer Durchlauf, solange die Session läuft: nichts Neues
    f.tick().await;
    assert_eq!(merge_runs(&f).len(), 1);

    // Ungelöst → beide erledigten Tickets des Worktrees blockiert
    f.end(&first, "", Some(1));
    f.tick().await;
    for id in [&first, &second] {
        let ticket = f.ticket(id).await;
        assert_eq!(ticket.status, TicketStatus::Blocked, "{id}");
        let reason = ticket.blocked_reason.expect("Grund");
        assert!(
            reason.starts_with("Merge: Die Merge-Session konnte den Konflikt nicht lösen"),
            "{reason}"
        );
    }
    assert!(no_merges(&f));
}

#[tokio::test]
async fn nach_erfolgreichem_merge_gehen_merge_blockierte_tickets_wieder_auf_erledigt() {
    let f = DispatchFixture::create(json!({ "wip": 2, "joinWorktrees": "sameSpec" })).await;
    let (first, second) = shared_done(&f, false).await;
    f.repo.run_git(&["checkout", "-q", "--detach"]);
    f.tick().await;
    assert_eq!(f.ticket(&first).await.status, TicketStatus::Blocked);
    assert_eq!(f.ticket(&second).await.status, TicketStatus::Blocked);
    f.repo.run_git(&["checkout", "-q", "-"]);

    let report = f.service.retry_merge(&first).await.expect("Erneut mergen");
    let mut merged = report.report.merged.clone();
    merged.sort();
    let mut both = vec![first.clone(), second.clone()];
    both.sort();
    assert_eq!(merged, both);
    assert_eq!(
        f.repo.git_out(&["log", "-1", "--format=%s"]),
        format!("Merge {first}, {second} (ticket/{first}-erstes)")
    );
    let ticket = f.ticket(&second).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.branch, None);
    let comment = last_comment(&f, &second).await;
    assert!(
        comment.starts_with(markers::MERGED) && comment.contains("wieder auf Erledigt"),
        "{comment}"
    );
}

// ----------------------------------------------------------------------------------------
// „Erneut mergen“
// ----------------------------------------------------------------------------------------

#[tokio::test]
async fn erneut_mergen_nur_fur_tickets_die_auf_einen_merge_warten() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Nichts", TicketPatch::default()).await;
    let error = f
        .service
        .retry_merge(&id)
        .await
        .expect_err("„Erneut mergen“ nicht möglich");
    assert_eq!(error.code(), Some("merge-not-possible"));
}

#[tokio::test]
async fn erneut_mergen_wird_abgelehnt_solange_die_merge_session_lauft() {
    let f = DispatchFixture::create(json!({})).await;
    let id = in_merge_session(&f, "Läuft").await;
    let error = f
        .service
        .retry_merge(&id)
        .await
        .expect_err("„Erneut mergen“ nicht möglich");
    assert_eq!(error.code(), Some("merge-not-possible"));
    assert_eq!(
        error.to_string(),
        format!("{id}: Die Merge-Session läuft noch.")
    );
    let entry = merge_entry(&f, &id);
    assert_eq!(entry.phase, MergePhase::Session);
    assert!(entry.session_used);
}

#[tokio::test]
async fn erneut_mergen_setzt_einen_hangenden_auftrag_in_phase_session_zuruck() {
    let f = DispatchFixture::create(json!({})).await;
    let id = in_merge_session(&f, "Hängt fest").await;
    f.drop_run(&f.run(&id).id);

    let report = f.service.retry_merge(&id).await.expect("Erneut mergen");
    assert_eq!(report.result.outcome, MergeRetryOutcome::Session);
    assert_eq!(
        report.result.text,
        format!("{id}: Merge-Konflikt – Merge-Session gestartet.")
    );
    // Erneuter Versuch: Konflikt, neue Merge-Session
    assert_eq!(merge_runs(&f).len(), 1);
    let entry = merge_entry(&f, &id);
    assert_eq!(entry.phase, MergePhase::Session);
    assert!(entry.session_used);
    assert_eq!(f.ticket(&id).await.status, TicketStatus::Done);
}

#[tokio::test]
async fn erneut_mergen_und_der_folgende_durchlauf_laufen_unter_derselben_sperre() {
    let merge = OrderedMerge::new();
    let f = DispatchFixture::create_with(json!({}), Some(merge.clone())).await;
    let id = done_by_agent(&f, "Sperre").await;
    f.repo.run_git(&["checkout", "-q", "--detach"]);
    f.tick().await;
    assert_eq!(f.ticket(&id).await.status, TicketStatus::Blocked);
    f.repo.run_git(&["checkout", "-q", "-"]);
    // In TypeScript wird die Attrappe erst hier gesetzt: die Durchläufe der Vorbereitung zählen
    // nicht mit
    merge.forget();

    // „Erneut mergen“ hält am Tor; der Durchlauf danach muss auf die Sperre warten
    let retried = {
        let service = f.service.clone();
        let id = id.clone();
        tokio::spawn(async move { service.retry_merge(&id).await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let timer = {
        let service = f.service.clone();
        tokio::spawn(async move { service.tick().await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    merge.release();
    let report = retried.await.expect("Aufgabe").expect("Erneut mergen");
    timer.await.expect("Aufgabe");

    assert_eq!(report.report.merged, vec![id]);
    assert_eq!(&merge.order()[..3], ["retry", "retry fertig", "merge-tick"]);
}
