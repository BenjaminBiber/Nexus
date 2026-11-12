//! Der Worktree-Verwalter für den Dispatcher: Beitritt, Wiederherstellen, Aufräumen nach
//! dem Merge und Junctions.
//! Portiert aus `packages/core/tests/unit/worktrees/WorktreeManagerDispatch.test.ts`.

use std::fs;
use std::path::{Path, PathBuf};

use super::{code, finish_in, in_spec, start, start_with, touching, Env};
use crate::board::basics::TicketStatus;
use crate::board::ticket_service::{NewTicketInput, TicketPatch};
use crate::dispatch::git::{BranchMerger, MergeOutcome};
use crate::dispatch::merge_strategy::MergeStrategy;
use crate::test_support::link_dir;
use crate::worktrees::manager::{ResumeResult, StartOptions, StartResult};

/// Beitritt nur innerhalb derselben Spec (Dispatcher)
fn dispatch_join() -> StartOptions {
    StartOptions {
        join: true,
        join_only_same_spec: true,
        ..StartOptions::default()
    }
}

#[tokio::test]
async fn tritt_mit_join_only_same_spec_ohne_spec_nie_automatisch_bei() {
    let e = Env::new().await;
    e.ticket(touching("Erstes", &["a"])).await;
    e.ticket(touching("Zweites", &["b"])).await;

    let first = start(&e, "ARC-0001").await.unwrap();
    let second = start_with(&e, "ARC-0002", dispatch_join()).await.unwrap();

    assert!(!second.joined);
    assert_ne!(second.branch, first.branch);
}

#[tokio::test]
async fn tritt_mit_join_only_same_spec_derselben_spec_weiter_bei() {
    let e = Env::new().await;
    e.ticket(in_spec("Erstes", &["a"], "001-x", "T001")).await;
    e.ticket(in_spec("Zweites", &["b"], "001-x", "T002")).await;

    let first = start(&e, "ARC-0001").await.unwrap();

    assert_eq!(
        start_with(&e, "ARC-0002", dispatch_join())
            .await
            .unwrap()
            .branch,
        first.branch
    );
    assert_eq!(
        e.manager.tickets_of(&first.branch).unwrap(),
        ["ARC-0001", "ARC-0002"]
    );
}

#[tokio::test]
async fn join_candidate_nennt_den_branch_dem_start_beitreten_wurde_ohne_etwas_zu_andern() {
    let e = Env::new().await;
    e.ticket(in_spec("Erstes", &["a"], "001-x", "T001")).await;
    e.ticket(in_spec("Zweites", &["b"], "001-x", "T002")).await;
    e.ticket(touching("Ohne Spec", &["c"])).await;

    assert_eq!(
        e.manager.join_candidate("ARC-0002", true).await.unwrap(),
        None
    );
    let first = start(&e, "ARC-0001").await.unwrap();
    assert_eq!(
        e.manager.join_candidate("ARC-0002", true).await.unwrap(),
        Some(first.branch.clone())
    );
    assert_eq!(
        e.manager.join_candidate("ARC-0003", true).await.unwrap(),
        None
    );
    assert_eq!(
        e.manager.join_candidate("ARC-0001", true).await.unwrap(),
        None
    );
    assert_eq!(e.manager.tickets_of(&first.branch).unwrap(), ["ARC-0001"]);
    assert_eq!(e.f.tickets.get("ARC-0002").await.unwrap().branch, None);
}

#[tokio::test]
async fn legt_einen_fehlenden_worktree_aus_dem_branch_neu_an() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;
    let started = start(&e, "ARC-0001").await.unwrap();

    e.repo
        .git(&["worktree", "remove", "--force", &started.path]);
    e.manager.registry.mark_removed(&started.branch).unwrap();
    assert!(!Path::new(&started.path).exists());

    assert_eq!(
        e.manager.ensure_worktree("ARC-0001").await.unwrap(),
        started.path
    );
    assert!(Path::new(&started.path).exists());
    assert_eq!(
        e.manager
            .registry
            .read()
            .unwrap()
            .get(&started.branch)
            .unwrap()
            .removed,
        Some(false)
    );
    assert_eq!(
        e.manager.ensure_worktree("ARC-0001").await.unwrap(),
        started.path
    );
}

#[tokio::test]
async fn resume_nutzt_den_eigenen_worktree_und_legt_einen_fehlenden_ordner_neu_an() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;
    let started = start(&e, "ARC-0001").await.unwrap();

    assert_eq!(
        e.manager.resume("ARC-0001", &e.f.actor).await.unwrap(),
        ResumeResult::Path(started.path.clone())
    );

    e.repo
        .git(&["worktree", "remove", "--force", &started.path]);

    assert_eq!(
        e.manager.resume("ARC-0001", &e.f.actor).await.unwrap(),
        ResumeResult::Path(started.path.clone())
    );
    assert!(Path::new(&started.path).exists());
}

#[tokio::test]
async fn resume_stellt_den_worktree_eines_anderen_tickets_nicht_wieder_her() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;
    e.ticket(NewTicketInput::titled("Zweites")).await;
    let first = start(&e, "ARC-0001").await.unwrap();
    e.repo.git(&["worktree", "remove", "--force", &first.path]);
    e.set(
        "ARC-0002",
        TicketPatch {
            branch: Some(Some(first.branch.clone())),
            worktree: Some(Some(first.path.clone())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();

    let result = e.manager.resume("ARC-0002", &e.f.actor).await.unwrap();

    match &result {
        ResumeResult::Reset(text) => assert!(text.contains("gehört ARC-0001"), "{text}"),
        other => panic!("erwartet war ein Zurücksetzen, nicht {other:?}"),
    }
    assert!(!Path::new(&first.path).exists());
    let ticket = e.f.tickets.get("ARC-0002").await.unwrap();
    assert_eq!(ticket.branch, None);
    assert_eq!(ticket.worktree, None);
    assert_eq!(e.manager.tickets_of(&first.branch).unwrap(), ["ARC-0001"]);
}

#[tokio::test]
async fn resume_stellt_einen_aufgeraumten_worktree_nicht_wieder_her() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;
    let started = start(&e, "ARC-0001").await.unwrap();
    e.repo
        .git(&["worktree", "remove", "--force", &started.path]);
    e.manager.registry.mark_removed(&started.branch).unwrap();

    let result = e.manager.resume("ARC-0001", &e.f.actor).await.unwrap();

    match &result {
        ResumeResult::Reset(text) => assert!(text.contains("wurde aufgeräumt"), "{text}"),
        other => panic!("erwartet war ein Zurücksetzen, nicht {other:?}"),
    }
    assert!(!Path::new(&started.path).exists());
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.branch, None);
    assert_eq!(ticket.worktree, None);
    assert_ne!(e.repo.git(&["branch", "--list", &started.branch]), "");
}

#[tokio::test]
async fn ensure_worktree_lehnt_ungestartete_tickets_ab() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;

    assert_eq!(
        code(e.manager.ensure_worktree("ARC-0001").await),
        "ticket-not-started"
    );
}

#[tokio::test]
async fn raumt_nach_dem_merge_worktree_und_branch_auf_und_leert_die_ticket_felder() {
    let e = Env::new().await;
    let started = start_with_commit(&e, true).await;
    finish_in(&e, "ARC-0001", None, Path::new(&started.path))
        .await
        .unwrap();

    assert!(e.manager.is_ready(&started.branch).await.unwrap());

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed.as_deref(), Some(started.path.as_str()));
    assert_eq!(result.tickets, ["ARC-0001"]);
    assert_eq!(result.warnings, Vec::<String>::new());
    assert!(!Path::new(&started.path).exists());
    assert_eq!(e.repo.git(&["branch", "--list", &started.branch]), "");
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.branch, None);
    assert_eq!(ticket.worktree, None);
}

/// Hauptordner mit echtem node_modules (ignoriert) und Worktree, in dem ein Agent eine
/// Junction darauf angelegt hat
async fn with_junction(e: &Env) -> (StartResult, PathBuf) {
    fs::write(
        e.repo.path().join(".gitignore"),
        "node_modules/\nnode_modules\n",
    )
    .unwrap();
    e.repo.git(&["add", ".gitignore"]);
    e.repo.git(&["commit", "-q", "-m", "ignore"]);
    let modules = e.repo.path().join("node_modules");
    fs::create_dir_all(modules.join(".bin")).unwrap();
    fs::write(modules.join(".package-lock.json"), "{}").unwrap();
    fs::write(modules.join(".bin").join("tool"), "x").unwrap();
    e.ticket(NewTicketInput {
        acceptance: Some("ok".into()),
        ..NewTicketInput::titled("Erstes")
    })
    .await;
    let started = start(e, "ARC-0001").await.unwrap();
    link_dir(&modules, &Path::new(&started.path).join("node_modules"));
    (started, modules)
}

#[tokio::test]
async fn remove_merged_entfernt_eine_junction_im_worktree_nur_als_link() {
    let e = Env::new().await;
    let (started, modules) = with_junction(&e).await;
    let worktree = Path::new(&started.path);
    fs::write(worktree.join("neu.txt"), "x").unwrap();
    e.git_in(worktree, &["add", "neu.txt"]);
    e.git_in(worktree, &["commit", "-q", "-m", "neu"]);
    finish_in(&e, "ARC-0001", None, worktree).await.unwrap();
    e.repo
        .git(&["merge", "--no-ff", "--no-edit", &started.branch]);

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed.as_deref(), Some(started.path.as_str()));
    assert_eq!(result.warnings, Vec::<String>::new());
    assert!(!worktree.exists());
    assert_eq!(
        fs::read_to_string(modules.join(".package-lock.json")).unwrap(),
        "{}"
    );
    assert_eq!(
        fs::read_to_string(modules.join(".bin").join("tool")).unwrap(),
        "x"
    );
}

#[tokio::test]
async fn done_raumt_einen_fertigen_worktree_mit_junction_auf_ohne_das_ziel_anzufassen() {
    let e = Env::new().await;
    let (started, modules) = with_junction(&e).await;

    let done = finish_in(&e, "ARC-0001", None, e.repo.path())
        .await
        .unwrap();

    assert_eq!(done.removed, std::slice::from_ref(&started.path));
    assert!(!Path::new(&started.path).exists());
    assert_eq!(
        fs::read_to_string(modules.join(".package-lock.json")).unwrap(),
        "{}"
    );
    assert_eq!(
        fs::read_to_string(modules.join(".bin").join("tool")).unwrap(),
        "x"
    );
}

#[tokio::test]
async fn lasst_einen_worktree_mit_ungesicherten_anderungen_stehen_und_warnt() {
    let e = Env::new().await;
    e.ticket(NewTicketInput {
        acceptance: Some("ok".into()),
        ..NewTicketInput::titled("Erstes")
    })
    .await;
    let started = start(&e, "ARC-0001").await.unwrap();
    fs::write(Path::new(&started.path).join("offen.txt"), "x").unwrap();

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed, None);
    assert!(
        result.warnings[0].contains("ungesicherte Änderungen"),
        "{:?}",
        result.warnings
    );
    assert!(Path::new(&started.path).exists());
}

/// Startet ARC-0001 und committet eine Datei im Worktree; mit `merge` auch in den
/// Hauptordner gemergt
async fn start_with_commit(e: &Env, merge: bool) -> StartResult {
    e.ticket(NewTicketInput {
        acceptance: Some("ok".into()),
        ..NewTicketInput::titled("Erstes")
    })
    .await;
    let started = start(e, "ARC-0001").await.unwrap();
    let worktree = Path::new(&started.path);
    fs::write(worktree.join("neu.txt"), "x").unwrap();
    e.git_in(worktree, &["add", "neu.txt"]);
    e.git_in(worktree, &["commit", "-q", "-m", "neu"]);
    if merge {
        e.repo
            .git(&["merge", "--no-ff", "--no-edit", &started.branch]);
    }
    started
}

#[tokio::test]
async fn fasst_einen_ordner_nicht_an_der_kein_worktree_des_gemergten_branches_ist() {
    let e = Env::new().await;
    let started = start_with_commit(&e, true).await;
    // Der eingetragene Ordner gehört inzwischen zu einem anderen Branch (samt
    // Plugin-Einstellung)
    e.repo
        .git(&["worktree", "remove", "--force", &started.path]);
    e.repo
        .git(&["worktree", "add", "-q", "-b", "anderer", &started.path]);
    let settings = Path::new(&started.path).join(".claude");
    fs::create_dir_all(&settings).unwrap();
    fs::write(settings.join("settings.local.json"), "{}").unwrap();

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed, None);
    assert_eq!(
        result.warnings,
        [format!(
            "Der Ordner {} ist kein Worktree von Branch {}; er wird nicht angefasst. \
Bitte von Hand prüfen.",
            started.path, started.branch
        )]
    );
    assert!(settings.join("settings.local.json").exists());
    assert_ne!(e.repo.git(&["branch", "--list", &started.branch]), "");
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.branch.as_deref(), Some(started.branch.as_str()));
    assert_eq!(ticket.worktree.as_deref(), Some(started.path.as_str()));
}

#[tokio::test]
async fn raumt_einen_leeren_rest_ordner_eines_halb_entfernten_worktrees_weg() {
    let e = Env::new().await;
    let started = start_with_commit(&e, true).await;
    // Windows: `worktree remove` löscht Inhalt und Verwaltungseintrag, der gesperrte leere
    // Ordner bleibt stehen
    e.repo
        .git(&["worktree", "remove", "--force", &started.path]);
    fs::create_dir(&started.path).unwrap();

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed.as_deref(), Some(started.path.as_str()));
    assert_eq!(result.tickets, ["ARC-0001"]);
    assert_eq!(result.warnings, Vec::<String>::new());
    assert!(!Path::new(&started.path).exists());
    assert_eq!(
        e.manager
            .registry
            .read()
            .unwrap()
            .get(&started.branch)
            .unwrap()
            .removed,
        Some(true)
    );
    assert_eq!(e.repo.git(&["branch", "--list", &started.branch]), "");
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.branch, None);
    assert_eq!(ticket.worktree, None);
}

#[tokio::test]
async fn raumt_nach_einem_squash_worktree_und_branch_auf() {
    let e = Env::new().await;
    let started = start_with_commit(&e, false).await;
    finish_in(&e, "ARC-0001", None, Path::new(&started.path))
        .await
        .unwrap();

    let outcome = BranchMerger::new()
        .merge(
            e.repo.path(),
            &started.branch,
            Some("ARC-0001 (squash)"),
            MergeStrategy::Squash,
        )
        .unwrap();
    assert!(
        matches!(outcome, MergeOutcome::Merged { .. }),
        "{outcome:?}"
    );

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed.as_deref(), Some(started.path.as_str()));
    assert_eq!(result.tickets, ["ARC-0001"]);
    assert_eq!(result.warnings, Vec::<String>::new());
    assert!(!Path::new(&started.path).exists());
    assert_eq!(e.repo.git(&["branch", "--list", &started.branch]), "");
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.branch, None);
    assert_eq!(ticket.worktree, None);
}

#[tokio::test]
async fn raumt_nichts_auf_wenn_der_branch_nach_dem_squash_weitere_commits_bekam() {
    let e = Env::new().await;
    let started = start_with_commit(&e, false).await;
    BranchMerger::new()
        .merge(
            e.repo.path(),
            &started.branch,
            Some("ARC-0001 (squash)"),
            MergeStrategy::Squash,
        )
        .unwrap();
    let worktree = Path::new(&started.path);
    fs::write(worktree.join("danach.txt"), "x").unwrap();
    e.git_in(worktree, &["add", "danach.txt"]);
    e.git_in(worktree, &["commit", "-q", "-m", "danach"]);

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(
        result.warnings,
        [format!(
            "Branch {} ist nicht gemergt; Worktree und Branch bleiben.",
            started.branch
        )]
    );
    assert!(worktree.exists());
    assert_ne!(e.repo.git(&["branch", "--list", &started.branch]), "");
}

#[tokio::test]
async fn raumt_nichts_auf_solange_der_branch_nicht_im_hauptordner_gemergt_ist() {
    let e = Env::new().await;
    let started = start_with_commit(&e, false).await;

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed, None);
    assert_eq!(result.tickets, ["ARC-0001"]);
    assert_eq!(
        result.warnings,
        [format!(
            "Branch {} ist nicht gemergt; Worktree und Branch bleiben.",
            started.branch
        )]
    );
    assert!(Path::new(&started.path).exists());
    assert_ne!(e.repo.git(&["branch", "--list", &started.branch]), "");
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.branch.as_deref(), Some(started.branch.as_str()));
    assert_eq!(ticket.worktree.as_deref(), Some(started.path.as_str()));
}

#[tokio::test]
async fn behalt_die_ticket_felder_wenn_der_branch_nicht_geloscht_werden_kann() {
    let e = Env::new().await;
    let started = start_with_commit(&e, true).await;
    let second = e.repo.base().join("zweit");
    e.repo.git(&[
        "worktree",
        "add",
        "-q",
        "--force",
        &second.to_string_lossy(),
        &started.branch,
    ]);

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed.as_deref(), Some(started.path.as_str()));
    assert_eq!(result.warnings.len(), 1);
    assert!(
        result.warnings[0].contains("nicht gelöscht"),
        "{:?}",
        result.warnings
    );
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.branch.as_deref(), Some(started.branch.as_str()));
    assert_eq!(ticket.worktree.as_deref(), Some(started.path.as_str()));
}

#[tokio::test]
async fn entfernt_einen_gesperrten_worktree_nicht_mit_gewalt_sondern_warnt() {
    let e = Env::new().await;
    let started = start_with_commit(&e, true).await;
    e.repo.git(&["worktree", "lock", &started.path]);

    let result = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed, None);
    assert!(
        result.warnings[0].contains("nicht entfernt"),
        "{:?}",
        result.warnings
    );
    assert!(Path::new(&started.path).exists());
    assert!(Path::new(&started.path)
        .join(".claude")
        .join("settings.local.json")
        .exists());
    assert_ne!(e.repo.git(&["branch", "--list", &started.branch]), "");
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.branch.as_deref(), Some(started.branch.as_str()));
    assert_eq!(ticket.worktree.as_deref(), Some(started.path.as_str()));
    e.repo.git(&["worktree", "unlock", &started.path]);
}

#[tokio::test]
async fn entfernt_nie_den_hauptordner_selbst() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;
    e.repo.git(&["branch", "ticket/haupt"]);
    let path = if cfg!(windows) {
        e.repo.path().to_string_lossy().to_uppercase()
    } else {
        e.repo.path().to_string_lossy().into_owned()
    };
    e.manager
        .registry
        .add("ticket/haupt", &path, "ARC-0001")
        .unwrap();

    let result = e
        .manager
        .remove_merged("ticket/haupt", &e.f.actor, None)
        .await
        .unwrap();

    assert_eq!(result.removed, None);
    assert!(
        result.warnings[0].contains("Hauptordner"),
        "{:?}",
        result.warnings
    );
    assert!(e.repo.path().join(".git").exists());
    assert_ne!(e.repo.git(&["branch", "--list", "ticket/haupt"]), "");
}

#[tokio::test]
async fn ensure_worktree_lehnt_einen_vorhandenen_fremden_ordner_ab() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;
    let started = start(&e, "ARC-0001").await.unwrap();
    e.repo
        .git(&["worktree", "remove", "--force", &started.path]);
    fs::create_dir_all(&started.path).unwrap();

    assert_eq!(
        code(e.manager.ensure_worktree("ARC-0001").await),
        "worktree-mismatch"
    );

    fs::remove_dir_all(&started.path).unwrap();
    e.repo
        .git(&["worktree", "add", "-q", "-b", "anders", &started.path]);

    assert_eq!(
        code(e.manager.ensure_worktree("ARC-0001").await),
        "worktree-mismatch"
    );
}
