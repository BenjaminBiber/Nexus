//! Merge erledigter Tickets, Halte-Gründe, Merge-Session und Zielbranch; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchMerge.test.ts`.

mod common;
mod dispatch_support;

use std::path::Path;
use std::sync::Arc;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::{MoveOptions, TicketPatch};
use nexus_core::dispatch::{markers, MergePhase, RunKind, RunRecord};
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

// ----------------------------------------------------------------------------------------
// Abschluss und Abweisungen
// ----------------------------------------------------------------------------------------

#[tokio::test]
async fn abbruch_nach_dem_merge_wird_vom_nachsten_durchlauf_abgeschlossen() {
    let f = DispatchFixture::create(json!({})).await;
    let id = done_by_agent(&f, "Abbruch").await;
    let ticket = f.ticket(&id).await;
    let branch = ticket.branch.clone().expect("Branch");
    let worktree = ticket.worktree.clone().expect("Worktree");
    // Wie ein Verbindungsabriss: der Merge ist gelaufen, Branch und Worktree sind weg, nur das
    // Leeren am Ticket fehlt noch
    f.repo.run_git(&[
        "merge",
        "-q",
        "--no-ff",
        "-m",
        &format!("Merge {id} „Abbruch“ ({branch})"),
        &branch,
    ]);
    f.repo
        .run_git(&["worktree", "remove", "--force", &worktree]);
    f.repo.run_git(&["branch", "-D", &branch]);
    let merge = f.repo.git_out(&["log", "-1", "--format=%h"]);

    let report = f.tick().await;
    assert_eq!(report.merged, vec![id.clone()]);
    let after = f.ticket(&id).await;
    assert_eq!(after.status, TicketStatus::Done);
    assert_eq!(after.branch, None);
    assert_eq!(after.worktree, None);
    assert_eq!(after.blocked_reason, None);
    assert!(last_comment(&f, &id).await.starts_with(&format!(
        "{} in {} ({merge}).",
        markers::MERGED,
        main_branch(&f)
    )));
    // Kein zweiter Merge-Commit
    assert_eq!(f.repo.git_out(&["log", "-1", "--format=%h"]), merge);
    assert!(no_merges(&f));
}

#[tokio::test]
async fn gemergt_kommentar_meldet_unvollstandiges_aufraumen() {
    let f = DispatchFixture::create(json!({})).await;
    let id = done_by_agent(&f, "Rest").await;
    let worktree = f.ticket(&id).await.worktree.expect("Worktree");
    std::fs::write(Path::new(&worktree).join("rest.txt"), "x").expect("Rest im Worktree");

    assert_eq!(f.tick().await.merged, vec![id.clone()]);
    let comment = last_comment(&f, &id).await;
    assert!(comment.starts_with(markers::MERGED), "{comment}");
    assert!(
        comment.contains("Aufräumen unvollständig:\n")
            && comment.contains("ungesicherte Änderungen"),
        "{comment}"
    );
    assert!(!comment.contains("aufgeräumt"), "{comment}");
}

#[tokio::test]
async fn ein_von_git_abgewiesener_merge_mit_uberschneidung_wartet_statt_zu_blockieren() {
    let (f, git) = with_fake_git(json!({})).await;
    commit_on_main(&f, "cfg.yaml", "alt");
    let id = done_by_agent_with(&f, "Sprache", "cfg.yaml", "ticket").await;
    // Die Änderung entsteht erst zwischen Vorprüfung und Merge
    git.write_before_merge("cfg.yaml", "lokal");
    git.answer_merge(MergeAnswer::Refused(
        "Fehler: lokale Änderungen würden überschrieben:\n\tcfg.yaml\nAbbruch".into(),
    ));

    f.tick().await;
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.blocked_reason, None);
    assert_eq!(
        last_comment(&f, &id).await,
        "[dispatch] Merge wartet auf den Hauptordner: lokale Änderungen in cfg.yaml, die der \
Branch ebenfalls ändert – gemergt wird automatisch, sobald das erledigt ist."
    );
    assert_eq!(merge_entry(&f, &id).phase, MergePhase::Waiting);
}

#[tokio::test]
async fn abgewiesener_merge_ohne_uberschneidung_blockiert_mit_den_dateien_der_git_meldung() {
    let (f, git) = with_fake_git(json!({})).await;
    let id = done_by_agent(&f, "Abweisung").await;
    git.answer_merge(MergeAnswer::Refused(
        "error: The following untracked working tree files would be overwritten by merge:\n\tx.txt\nAborting"
            .into(),
    ));

    f.tick().await;
    assert_eq!(
        f.ticket(&id).await.blocked_reason.as_deref(),
        Some("Merge: Git lehnt den Merge ab, er würde lokale Änderungen überschreiben: x.txt")
    );
    assert_eq!(
        last_comment(&f, &id).await,
        "[dispatch] Merge nicht möglich: Git lehnt den Merge ab, er würde lokale Änderungen \
überschreiben: x.txt"
    );
}

#[tokio::test]
async fn leere_konfliktliste_heisst_unbekannten_dateien() {
    let (f, git) = with_fake_git(json!({})).await;
    let id = done_by_agent(&f, "Leer").await;
    git.answer_merge(MergeAnswer::Conflict(Vec::new()));

    f.tick().await;
    let comment = last_comment(&f, &id).await;
    assert!(
        comment.starts_with(&format!(
            "{} in unbekannten Dateien beim Merge in ",
            markers::MERGE_CONFLICT
        )),
        "{comment}"
    );
}

#[tokio::test]
async fn git_meldung_ohne_dateiliste_steht_zusammengefasst_in_einer_zeile() {
    let (f, git) = with_fake_git(json!({})).await;
    let id = done_by_agent(&f, "Zeilen").await;
    git.answer_merge(MergeAnswer::Refused(
        "fatal: etwas\n  ging   schief\nHinweis: x".into(),
    ));

    f.tick().await;
    assert_eq!(
        f.ticket(&id).await.blocked_reason.as_deref(),
        Some("Merge: Git lehnt den Merge ab: fatal: etwas ging schief Hinweis: x")
    );
}

#[tokio::test]
async fn nicht_nutzbarer_repo_pfad_blockiert_mit_grund() {
    let f = DispatchFixture::create(json!({})).await;
    let id = done_by_agent(&f, "Pfad").await;
    let missing = f.dir().join("gibt-es-nicht");
    f.backend.set_repo_path(f.board.id, &missing);

    let report = f.tick().await;
    assert!(report.errors.is_empty(), "{report:?}");
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    let reason = ticket.blocked_reason.expect("Grund");
    assert!(
        reason.starts_with(&format!(
            "Merge: Repo-Pfad {} nicht nutzbar: ",
            missing.to_string_lossy()
        )),
        "{reason}"
    );
}

// ----------------------------------------------------------------------------------------
// Ausgeschaltet, Pause und Zeitfenster
// ----------------------------------------------------------------------------------------

#[tokio::test]
async fn ausgeschaltet_oder_ausserhalb_des_zeitfensters_startet_keine_merge_session() {
    let (f, git) = with_fake_git(json!({})).await;
    let id = f.ready("Aus", TicketPatch::default()).await;
    f.tick().await;
    commit_on_main(&f, "feature.txt", "haupt");
    f.finish_as_agent(&id, "feature.txt", "ticket").await;
    f.end(&id, OK_OUTPUT, Some(0));
    f.service.set_enabled(false).await.expect("ausschalten");

    let before = git.merge_calls();
    f.tick().await;
    assert_eq!(git.merge_calls(), before + 1);
    assert!(merge_runs(&f).is_empty());
    let entry = merge_entry(&f, &id);
    assert_eq!(entry.phase, MergePhase::Pending);
    assert!(!entry.session_used);
    assert_eq!(entry.conflict_seen, Some(true));
    // Kein weiterer Merge-Versuch, solange der Konflikt gesehen ist
    f.tick().await;
    assert_eq!(git.merge_calls(), before + 1);

    // „Erneut mergen“ setzt den Merker zurück: genau ein neuer Versuch
    let report = f.service.retry_merge(&id).await.expect("Erneut mergen");
    assert_eq!(report.result.outcome, MergeRetryOutcome::Pending);
    assert!(
        report.result.text.contains("Merge-Session wartet"),
        "{}",
        report.result.text
    );
    assert_eq!(git.merge_calls(), before + 2);
    assert!(merge_runs(&f).is_empty());

    // Eingeschaltet, aber außerhalb des Zeitfensters: weiterhin kein Versuch
    f.service.set_enabled(true).await.expect("einschalten");
    f.set_local_time(12, 0);
    f.write_config(json!({ "window": { "from": "14:00", "to": "15:00" } }));
    f.tick().await;
    assert_eq!(git.merge_calls(), before + 2);
    assert!(merge_runs(&f).is_empty());

    // Fenster offen: Merge-Versuch und Merge-Session
    f.write_config(json!({ "window": null }));
    f.tick().await;
    assert_eq!(git.merge_calls(), before + 3);
    assert_eq!(merge_runs(&f).len(), 1);
}

#[tokio::test]
async fn wahrend_einer_pause_startet_keine_merge_session_danach_schon() {
    let (f, git) = with_fake_git(json!({})).await;
    let id = f.ready("Pause", TicketPatch::default()).await;
    f.tick().await;
    commit_on_main(&f, "feature.txt", "haupt");
    f.finish_as_agent(&id, "feature.txt", "ticket").await;
    f.end(&id, OK_OUTPUT, Some(0));
    f.service.pause(30).await.expect("pausieren");

    f.tick().await;
    assert!(merge_runs(&f).is_empty());
    let entry = merge_entry(&f, &id);
    assert_eq!(entry.phase, MergePhase::Pending);
    assert!(!entry.session_used);
    assert_eq!(entry.conflict_seen, Some(true));

    // Während der Pause kein weiterer Merge-Versuch im Hauptordner
    let before = git.merge_calls();
    f.tick().await;
    assert_eq!(git.merge_calls(), before);

    f.service.resume().await.expect("fortsetzen");
    f.tick().await;
    assert_eq!(merge_runs(&f).len(), 1);
}

// ----------------------------------------------------------------------------------------
// Zielbranch
// ----------------------------------------------------------------------------------------

/// Die Zielbranch-Seite von Git, wie der Test sie selbst benutzt
fn targets() -> RealMergeGit {
    RealMergeGit::new()
}

/// Pfad des Merge-Worktrees eines Zielbranches
fn merge_worktree(f: &DispatchFixture, target: &str) -> String {
    targets().merge_worktree_path(f.repo.path(), target)
}

/// Bereites Ticket mit Zielbranch starten (legt den Zielbranch an)
async fn started_with_target(f: &DispatchFixture, title: &str, target: &str) -> String {
    let id = f
        .ready(
            title,
            TicketPatch {
                target_branch: Some(Some(target.into())),
                ..TicketPatch::default()
            },
        )
        .await;
    f.tick().await;
    id
}

/// Commit direkt auf den Zielbranch über einen kurzzeitig geöffneten Merge-Worktree
fn commit_on_target(f: &DispatchFixture, target: &str, file: &str, content: &str) {
    let git = targets();
    let path = git
        .open_merge_worktree(f.repo.path(), target)
        .expect("Merge-Worktree öffnen");
    std::fs::write(Path::new(&path).join(file), content).expect("Datei im Zielbranch");
    f.repo.run_git(&["-C", &path, "add", file]);
    f.repo
        .run_git(&["-C", &path, "commit", "-q", "-m", &format!("ziel {file}")]);
    assert_eq!(git.close_merge_worktree(f.repo.path(), &path), None);
}

#[tokio::test]
async fn mergt_in_einen_nicht_ausgecheckten_zielbranch_ohne_den_hauptordner_anzufassen() {
    let f = DispatchFixture::create(json!({})).await;
    let before = f.repo.git_out(&["rev-parse", "HEAD"]);
    let id = started_with_target(&f, "Login", "release/1").await;
    let comments = f.comments(&id).await;
    assert!(
        comments.iter().any(
            |body| body.starts_with("[dispatch] Merge: Zielbranch release/1 neu angelegt von ")
        ),
        "{comments:?}"
    );
    // Kein zusätzliches Start-Ereignis: genau ein Start-Kommentar
    assert_eq!(
        comments
            .iter()
            .filter(|body| body.starts_with(markers::START))
            .count(),
        1
    );

    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));
    let worktree = f.ticket(&id).await.worktree.expect("Worktree");

    assert_eq!(f.tick().await.merged, vec![id.clone()]);
    assert_eq!(f.repo.git_out(&["rev-parse", "HEAD"]), before);
    assert!(!f.repo.path().join("feature.txt").exists());
    assert_eq!(f.repo.git_out(&["show", "release/1:feature.txt"]), "neu");
    assert_eq!(
        f.repo.git_out(&["log", "-1", "--format=%s", "release/1"]),
        format!("Merge {id} „Login“ (ticket/{id}-login)")
    );
    assert!(!Path::new(&merge_worktree(&f, "release/1")).exists());
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
    let comment = last_comment(&f, &id).await;
    assert!(
        comment.starts_with(&format!("{} in release/1 (", markers::MERGED))
            && comment.contains("aufgeräumt"),
        "{comment}"
    );
    let notice = f.notices.last().expect("Meldung");
    assert_eq!(notice.kind, DispatchNoticeKind::Merged);
    assert_eq!(notice.ticket_id.as_deref(), Some(id.as_str()));
    assert!(
        notice.text.starts_with("in release/1 gemergt"),
        "{}",
        notice.text
    );
    assert!(no_merges(&f));
}

#[tokio::test]
async fn squash_fasst_im_merge_worktree_in_den_zielbranch_zusammen() {
    let f = DispatchFixture::create(json!({ "mergeStrategy": "squash" })).await;
    let before = f.repo.git_out(&["rev-parse", "HEAD"]);
    let id = started_with_target(&f, "Login", "release/1").await;
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));
    let ticket = f.ticket(&id).await;
    let branch = ticket.branch.clone().expect("Branch");
    let tip = f
        .repo
        .git_out(&["rev-parse", &format!("refs/heads/{branch}")]);
    let target_before = f.repo.git_out(&["rev-parse", "refs/heads/release/1"]);

    assert_eq!(f.tick().await.merged, vec![id.clone()]);
    assert_eq!(
        f.repo
            .git_out(&["log", "-1", "--format=%P", "refs/heads/release/1"]),
        target_before
    );
    assert_eq!(
        f.repo
            .git_out(&["log", "-1", "--format=%s", "refs/heads/release/1"]),
        format!("{id} „Login“ (squash von {branch})")
    );
    assert_eq!(
        f.repo.git_out(&[
            "log",
            "-1",
            "--format=%(trailers:key=Harness-Squashed,valueonly)",
            "refs/heads/release/1",
        ]),
        format!("{branch} {tip}")
    );
    assert_eq!(
        f.repo
            .git_out(&["show", "refs/heads/release/1:feature.txt"]),
        "neu"
    );
    assert_eq!(f.repo.git_out(&["branch", "--list", &branch]), "");
    assert!(!Path::new(&ticket.worktree.expect("Worktree")).exists());
    assert!(!Path::new(&merge_worktree(&f, "release/1")).exists());
    assert_eq!(f.repo.git_out(&["rev-parse", "HEAD"]), before);
    assert!(!f.repo.path().join("feature.txt").exists());
    assert_eq!(f.repo.git_out(&["status", "--porcelain"]), "");
    let after = f.ticket(&id).await;
    assert_eq!(after.status, TicketStatus::Done);
    assert_eq!(after.branch, None);
    assert_eq!(after.worktree, None);
}

#[tokio::test]
async fn legt_einen_inzwischen_geloschten_zielbranch_beim_merge_neu_an() {
    let f = DispatchFixture::create(json!({})).await;
    let id = started_with_target(&f, "Weg", "release/1").await;
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));
    f.repo.run_git(&["branch", "-D", "release/1"]);

    assert_eq!(f.tick().await.merged, vec![id.clone()]);
    assert_eq!(f.repo.git_out(&["show", "release/1:feature.txt"]), "neu");
    assert!(f.comments(&id).await.contains(&format!(
        "[dispatch] Merge: Zielbranch release/1 neu angelegt von {}.",
        main_branch(&f)
    )));
}

#[tokio::test]
async fn blockiert_wenn_sich_der_zielbranch_nicht_anlegen_lasst() {
    let f = DispatchFixture::create(json!({})).await;
    let id = started_with_target(&f, "Kaputt", "release/1").await;
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));
    // release/1 weg, dafür ein Branch „release“: `git branch release/1` scheitert am Ref-Konflikt
    f.repo.run_git(&["branch", "-D", "release/1"]);
    f.repo.run_git(&["branch", "release"]);

    let report = f.tick().await;
    assert!(report.errors.is_empty(), "{report:?}");
    assert!(report.merged.is_empty());
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    let reason = ticket.blocked_reason.expect("Grund");
    assert!(
        reason.starts_with("Merge: Zielbranch release/1 nicht nutzbar: "),
        "{reason}"
    );
    assert!(no_merges(&f));
    assert!(f.tick().await.errors.is_empty());
}

#[tokio::test]
async fn konflikt_gesehen_und_dispatcher_aus_offnet_keinen_merge_worktree_je_durchlauf() {
    let (f, git) = with_fake_git(json!({})).await;
    let id = started_with_target(&f, "Wartet", "release/1").await;
    commit_on_target(&f, "release/1", "feature.txt", "haupt");
    f.finish_as_agent(&id, "feature.txt", "ticket").await;
    f.end(&id, OK_OUTPUT, Some(0));
    f.service.set_enabled(false).await.expect("ausschalten");

    f.tick().await;
    assert!(merge_runs(&f).is_empty());
    let entry = merge_entry(&f, &id);
    assert_eq!(entry.phase, MergePhase::Pending);
    assert_eq!(entry.conflict_seen, Some(true));
    let path = merge_worktree(&f, "release/1");
    assert!(!Path::new(&path).exists());

    let opened = git.open_calls();
    let merged = git.merge_calls();
    f.tick().await;
    f.tick().await;
    assert_eq!(git.open_calls(), opened);
    assert_eq!(git.merge_calls(), merged);
    assert!(!Path::new(&path).exists());
    assert_eq!(f.ticket(&id).await.status, TicketStatus::Done);
    let entry = merge_entry(&f, &id);
    assert_eq!(entry.phase, MergePhase::Pending);
    assert_eq!(entry.conflict_seen, Some(true));
}

#[tokio::test]
async fn nennt_den_zielbranch_des_tickets_im_dispatch_status() {
    let f = DispatchFixture::create(json!({})).await;
    let id = started_with_target(&f, "Wartet", "release/1").await;
    commit_on_target(&f, "release/1", "feature.txt", "haupt");
    f.finish_as_agent(&id, "feature.txt", "ticket").await;
    f.end(&id, OK_OUTPUT, Some(0));
    f.service.set_enabled(false).await.expect("ausschalten");
    f.tick().await;

    assert_eq!(merge_entry(&f, &id).target, None);
    let status = f
        .service
        .status(Some(f.board.id))
        .await
        .expect("Zustand des Dispatchers");
    assert_eq!(
        status.merges,
        vec![DispatchMergeInfo {
            ticket_id: id.clone(),
            phase: MergePhase::Pending,
            branch: format!("ticket/{id}-wartet"),
            target: Some("release/1".into()),
        }]
    );
}

#[tokio::test]
async fn ohne_zielbranch_am_ticket_steht_im_dispatch_status_kein_ziel() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Ohne", TicketPatch::default()).await;
    let entry = nexus_core::dispatch::MergeEntry {
        ticket_id: id.clone(),
        branch: "ticket/x".into(),
        phase: MergePhase::Waiting,
        session_used: true,
        wait_noted: true,
        target: Some(main_branch(&f)),
        conflict_seen: Some(false),
        held_by: None,
        since: "2026-10-22T08:00:00.000Z".into(),
    };
    f.ctx
        .stores
        .state
        .update(|state| {
            state.merges.insert(id.clone(), entry);
        })
        .await
        .expect("Zustand schreiben");

    let status = f
        .service
        .status(Some(f.board.id))
        .await
        .expect("Zustand des Dispatchers");
    assert_eq!(
        status.merges,
        vec![DispatchMergeInfo {
            ticket_id: id,
            phase: MergePhase::Waiting,
            branch: "ticket/x".into(),
            target: None,
        }]
    );
}

#[tokio::test]
async fn mergt_im_hauptordner_wenn_dort_der_zielbranch_ausgecheckt_ist() {
    let f = DispatchFixture::create(json!({})).await;
    let main = main_branch(&f);
    let id = started_with_target(&f, "Login", &main).await;
    assert!(
        !f.comments(&id)
            .await
            .iter()
            .any(|body| body.contains("neu angelegt")),
        "kein Anlegen des ausgecheckten Branches"
    );
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));
    let worktree = f.ticket(&id).await.worktree.expect("Worktree");

    assert_eq!(f.tick().await.merged, vec![id.clone()]);
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
    assert!(!Path::new(&merge_worktree(&f, &main)).exists());
    assert_eq!(
        f.repo
            .git_out(&["branch", "--list", &format!("ticket/{id}-login")]),
        ""
    );
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.branch, None);
    assert_eq!(ticket.worktree, None);
    let comment = last_comment(&f, &id).await;
    assert!(
        comment.starts_with(&format!("{} in {main} (", markers::MERGED))
            && comment.contains("aufgeräumt"),
        "{comment}"
    );
}

#[tokio::test]
async fn mergt_im_fremden_worktree_des_zielbranches_trotz_anderer_lokaler_anderungen() {
    let f = DispatchFixture::create(json!({})).await;
    let id = started_with_target(&f, "Fremd", "release/1").await;
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));
    let foreign = f.repo.base().join("fremd");
    let foreign_text = foreign.to_string_lossy().into_owned();
    f.repo
        .run_git(&["worktree", "add", "-q", &foreign_text, "release/1"]);
    std::fs::write(foreign.join("feature.txt"), "lokal").expect("lokale Änderung");
    let tip = f.repo.git_out(&["rev-parse", "release/1"]);

    assert!(f.tick().await.merged.is_empty());
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.blocked_reason, None);
    let comment = last_comment(&f, &id).await;
    assert!(
        comment.starts_with("[dispatch] Merge wartet auf den Worktree ")
            && comment.contains(
                "fremd: lokale Änderungen in feature.txt, die der Branch \
ebenfalls ändert"
            ),
        "{comment}"
    );
    assert_eq!(f.repo.git_out(&["rev-parse", "release/1"]), tip);
    assert!(!Path::new(&merge_worktree(&f, "release/1")).exists());

    // Ohne Überschneidung: gemergt wird im fremden Worktree, der samt lokaler Datei stehen bleibt
    std::fs::remove_file(foreign.join("feature.txt")).expect("lokale Änderung entfernen");
    std::fs::write(foreign.join("lokal.txt"), "x").expect("andere lokale Datei");
    assert_eq!(f.tick().await.merged, vec![id]);
    assert_eq!(read(&foreign.join("feature.txt")), "neu");
    assert_eq!(read(&foreign.join("lokal.txt")), "x");
    assert_eq!(f.repo.git_out(&["show", "release/1:feature.txt"]), "neu");
}

#[tokio::test]
async fn wartet_bei_liegengebliebenem_merge_worktree_mit_lokalen_anderungen() {
    let f = DispatchFixture::create(json!({})).await;
    let id = started_with_target(&f, "Liegen", "release/1").await;
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));
    let path = targets()
        .open_merge_worktree(f.repo.path(), "release/1")
        .expect("Merge-Worktree öffnen");
    std::fs::write(Path::new(&path).join("rest.txt"), "x").expect("Rest im Merge-Worktree");
    let tip = f.repo.git_out(&["rev-parse", "release/1"]);

    assert!(f.tick().await.merged.is_empty());
    let again = f.tick().await;
    assert!(again.merged.is_empty());
    assert!(again.errors.is_empty(), "{again:?}");
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.blocked_reason, None);
    let waits = comments_starting(&f, &id, "[dispatch] Merge wartet auf den Merge-Worktree").await;
    assert_eq!(waits.len(), 1, "{waits:?}");
    assert!(
        waits[0].contains(": liegengebliebene Änderungen in rest.txt – "),
        "{}",
        waits[0]
    );
    assert_eq!(f.repo.git_out(&["rev-parse", "release/1"]), tip);
    assert!(Path::new(&path).join("rest.txt").exists());

    // Sauber: der liegengebliebene Merge-Worktree wird wiederverwendet und danach entfernt
    std::fs::remove_file(Path::new(&path).join("rest.txt")).expect("Rest entfernen");
    assert_eq!(f.tick().await.merged, vec![id]);
    assert_eq!(f.repo.git_out(&["show", "release/1:feature.txt"]), "neu");
    assert!(!Path::new(&path).exists());
}

#[tokio::test]
async fn nennt_den_merge_worktree_wenn_sich_der_eigene_merge_dort_nicht_zurucksetzen_lasst() {
    let (f, git) = with_fake_git(json!({})).await;
    let id = started_with_target(&f, "Abbruch", "release/1").await;
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));
    git.answer_merge(MergeAnswer::AbortFailed { details: true });

    assert!(f.tick().await.errors.is_empty());
    let seen = git.last_merge_root();
    assert_ne!(seen, f.repo.path().to_string_lossy());
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(
        ticket.blocked_reason.as_deref(),
        Some(
            format!(
                "Merge: Der Merge-Worktree {seen} steht noch im Merge-Zustand: kaputt. Bitte \
dort von Hand aufräumen (git merge --abort)."
            )
            .as_str()
        )
    );
    // Der Ordner bleibt stehen, damit dort aufgeräumt werden kann
    assert!(Path::new(&seen).exists());
}

#[tokio::test]
async fn pruft_nach_der_merge_session_den_zielbranch_nicht_ein_gleichnamiges_tag() {
    let f = DispatchFixture::create(json!({})).await;
    let id = started_with_target(&f, "Mehrdeutig", "release/1").await;
    // Tag auf dem alten Stand des Zielbranches: im Ticket-Branch enthalten, der neue Stand nicht
    f.repo
        .run_git(&["tag", "release/1", "refs/heads/release/1"]);
    commit_on_target(&f, "release/1", "feature.txt", "haupt");
    f.finish_as_agent(&id, "feature.txt", "ticket").await;
    f.end(&id, OK_OUTPUT, Some(0));
    f.tick().await;
    let session = f.run(&id);
    assert_eq!(session.kind, RunKind::Merge);
    assert_eq!(session.target.as_deref(), Some("release/1"));

    // Die Merge-Session holt den Zielbranch nicht herein
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
async fn lost_konflikte_uber_die_merge_session_gegen_den_zielbranch() {
    let f = DispatchFixture::create(json!({})).await;
    let before = f.repo.git_out(&["rev-parse", "HEAD"]);
    let id = started_with_target(&f, "Konflikt", "release/1").await;
    commit_on_target(&f, "release/1", "feature.txt", "haupt");
    f.finish_as_agent(&id, "feature.txt", "ticket").await;
    f.end(&id, OK_OUTPUT, Some(0));
    f.tick().await;

    let ticket = f.ticket(&id).await;
    let session = f.run(&id);
    assert_eq!(session.kind, RunKind::Merge);
    assert_eq!(session.target.as_deref(), Some("release/1"));
    assert_eq!(session.cwd, ticket.worktree.clone().expect("Worktree"));
    assert_eq!(Some(session.branch.clone()), ticket.branch);
    assert!(
        session.prompt.contains("git merge release/1"),
        "{}",
        session.prompt
    );
    let entry = merge_entry(&f, &id);
    assert_eq!(entry.phase, MergePhase::Session);
    assert!(entry.session_used);
    assert_eq!(entry.target.as_deref(), Some("release/1"));
    let comment = last_comment(&f, &id).await;
    assert!(
        comment.starts_with(&format!(
            "{} in feature.txt beim Merge in release/1 ",
            markers::MERGE_CONFLICT
        )),
        "{comment}"
    );
    // Der abgebrochene Merge hinterlässt keinen Merge-Worktree
    assert!(!Path::new(&merge_worktree(&f, "release/1")).exists());

    // Die Merge-Session löst den Konflikt im Worktree; der Hauptordner bleibt auf seinem Branch
    resolve_in_worktree(&f, &id, "release/1").await;
    f.end(&id, OK_OUTPUT, Some(0));
    assert_eq!(f.tick().await.merged, vec![id.clone()]);
    assert_eq!(f.repo.git_out(&["show", "release/1:feature.txt"]), "ticket");
    assert_eq!(f.repo.git_out(&["rev-parse", "HEAD"]), before);
    assert!(last_comment(&f, &id)
        .await
        .starts_with(&format!("{} in release/1 ", markers::MERGED)));
}
