//! Start, Abschluss, Beitritt, Abnahme und Aufräumen am DB-Board.
//! Portiert aus `packages/core/tests/unit/worktrees/WorktreeManager.test.ts`.

use std::fs;
use std::path::Path;

use super::{code, finish, finish_in, in_spec, start, start_with, touching, Env};
use crate::board::basics::TicketStatus;
use crate::board::ticket_service::{NewTicketInput, TicketPatch};
use crate::dispatch::config::JoinWorktrees;
use crate::dispatch::git::TargetBranches;
use crate::test_support::TempRepo;
use crate::worktrees::manager::{RepoBoard, StartOptions};

#[tokio::test]
async fn legt_branch_und_worktree_an_setzt_in_arbeit_und_schaltet_das_plugin_ein() {
    let e = Env::new().await;
    e.ticket(touching("Login bauen", &["src/login"])).await;

    let result = start(&e, "ARC-0001").await.unwrap();

    assert_eq!(result.id, "ARC-0001");
    assert_eq!(result.branch, "ticket/ARC-0001-login-bauen");
    assert!(!result.joined);
    assert!(Path::new(&result.path).exists());
    assert!(Path::new(&result.path)
        .join(".claude")
        .join("settings.local.json")
        .exists());
    assert!(e
        .repo
        .git(&["branch", "--list"])
        .contains("ticket/ARC-0001-login-bauen"));
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.status, TicketStatus::InProgress);
    assert_eq!(ticket.branch.as_deref(), Some(result.branch.as_str()));
    assert_eq!(ticket.worktree.as_deref(), Some(result.path.as_str()));
    assert_eq!(
        e.manager
            .registry
            .read()
            .unwrap()
            .get(&result.branch)
            .unwrap()
            .tickets,
        ["ARC-0001"]
    );
}

#[tokio::test]
async fn verknupft_die_session_wenn_eine_angegeben_ist() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("A")).await;

    start_with(
        &e,
        "ARC-0001",
        StartOptions {
            session_id: Some("s-1".into()),
            ..StartOptions::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(e.f.tickets.get("ARC-0001").await.unwrap().sessions, ["s-1"]);
}

#[tokio::test]
async fn tritt_mit_in_einem_worktree_ohne_uberschneidung_bei_ohne_neuen_branch() {
    let e = Env::new().await;
    e.ticket(touching("Erstes", &["src/a"])).await;
    e.ticket(touching("Zweites", &["src/b"])).await;

    let first = start(&e, "ARC-0001").await.unwrap();
    let second = start_with(
        &e,
        "ARC-0002",
        StartOptions {
            in_ticket: Some("ARC-0001".into()),
            ..StartOptions::default()
        },
    )
    .await
    .unwrap();

    assert!(second.joined);
    assert_eq!(second.branch, first.branch);
    assert_eq!(second.path, first.path);
    assert_eq!(
        e.repo
            .git(&["branch", "--list", "ticket/*"])
            .split('\n')
            .count(),
        1
    );
    assert_eq!(
        e.manager
            .registry
            .read()
            .unwrap()
            .get(&first.branch)
            .unwrap()
            .tickets,
        ["ARC-0001", "ARC-0002"]
    );
}

#[tokio::test]
async fn tritt_mit_join_automatisch_nur_einem_offenen_worktree_derselben_spec_bei() {
    let e = Env::new().await;
    e.ticket(in_spec("Erstes", &["a"], "001-x", "T001")).await;
    e.ticket(in_spec("Anderes", &["b"], "002-y", "T001")).await;
    e.ticket(in_spec("Gleiches", &["c"], "001-x", "T002")).await;
    e.ticket(in_spec("Überschneidet", &["a/x"], "001-x", "T003"))
        .await;
    let joining = StartOptions {
        join: true,
        ..StartOptions::default()
    };

    let first = start_with(&e, "ARC-0001", joining.clone()).await.unwrap();

    assert!(
        !start_with(&e, "ARC-0002", joining.clone())
            .await
            .unwrap()
            .joined
    );
    let third = start_with(&e, "ARC-0003", joining.clone()).await.unwrap();
    assert!(third.joined);
    assert_eq!(third.branch, first.branch);
    let overlap = start_with(&e, "ARC-0004", joining).await.unwrap();
    assert!(!overlap.joined);
    assert_ne!(overlap.branch, first.branch);
}

#[tokio::test]
async fn ohne_join_tritt_start_keinem_worktree_derselben_spec_bei() {
    let e = Env::new().await;
    e.ticket(in_spec("Erstes", &["a"], "001-x", "T001")).await;
    e.ticket(in_spec("Gleiches", &["b"], "001-x", "T002")).await;

    let first = start(&e, "ARC-0001").await.unwrap();
    let second = start(&e, "ARC-0002").await.unwrap();

    assert!(!second.joined);
    assert_eq!(second.branch, "ticket/ARC-0002-gleiches");
    assert_ne!(second.path, first.path);
    assert_eq!(e.manager.tickets_of(&first.branch).unwrap(), ["ARC-0001"]);
}

#[tokio::test]
async fn warnt_bei_uberschneidung_mit_in_und_legt_einen_eigenen_worktree_an() {
    let e = Env::new().await;
    e.ticket(touching("Erstes", &["src"])).await;
    e.ticket(touching("Zweites", &["src/login"])).await;

    let first = start(&e, "ARC-0001").await.unwrap();
    let second = start_with(
        &e,
        "ARC-0002",
        StartOptions {
            in_ticket: Some("ARC-0001".into()),
            ..StartOptions::default()
        },
    )
    .await
    .unwrap();

    assert!(!second.joined);
    assert!(
        second
            .warning
            .as_deref()
            .unwrap_or("")
            .contains("überschneid"),
        "{:?}",
        second.warning
    );
    assert_ne!(second.path, first.path);
}

#[tokio::test]
async fn beachtet_die_sperren_vor_dem_anlegen_force_ubergeht_eine_offene_voraussetzung() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Basis")).await;
    e.ticket(NewTicketInput {
        depends_on: Some(vec!["ARC-0001".into()]),
        ..NewTicketInput::titled("Folge")
    })
    .await;

    assert_eq!(code(start(&e, "ARC-0002").await), "dependency-open");
    assert_eq!(e.repo.git(&["branch", "--list", "ticket/*"]), "");

    let forced = start_with(
        &e,
        "ARC-0002",
        StartOptions {
            force: true,
            ..StartOptions::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        e.f.tickets.get("ARC-0002").await.unwrap().status,
        TicketStatus::InProgress
    );
    assert!(Path::new(&forced.path).exists());
}

#[tokio::test]
async fn meldet_gestartete_tickets_fehlende_ziel_worktrees_vorhandene_branches_und_den_fehlenden_commit(
) {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;
    e.ticket(NewTicketInput::titled("Zweites")).await;

    assert_eq!(
        code(
            start_with(
                &e,
                "ARC-0002",
                StartOptions {
                    in_ticket: Some("ARC-0001".into()),
                    ..StartOptions::default()
                },
            )
            .await
        ),
        "host-not-started"
    );
    e.repo.git(&["branch", "ticket/ARC-0002-zweites"]);
    assert_eq!(code(start(&e, "ARC-0002").await), "branch-exists");
    start(&e, "ARC-0001").await.unwrap();
    assert_eq!(code(start(&e, "ARC-0001").await), "already-started");

    let empty = TempRepo::without_commit();
    let manager = e.manager_on(empty.path());
    e.ticket(NewTicketInput::titled("Drittes")).await;
    assert_eq!(
        code(
            manager
                .start(
                    "ARC-0003",
                    &StartOptions::default(),
                    empty.path(),
                    &e.f.actor
                )
                .await
        ),
        "no-commit"
    );
}

#[tokio::test]
async fn done_verlangt_eine_abnahme_setzt_erledigt_und_meldet_bereit_erst_wenn_alle_fertig_sind() {
    let e = Env::new().await;
    e.ticket(touching("Erstes", &["a"])).await;
    e.ticket(touching("Zweites", &["b"])).await;
    let first = start(&e, "ARC-0001").await.unwrap();
    start_with(
        &e,
        "ARC-0002",
        StartOptions {
            in_ticket: Some("ARC-0001".into()),
            ..StartOptions::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        code(finish(&e, "ARC-0001", None).await),
        "acceptance-missing"
    );
    let one = finish(&e, "ARC-0001", Some("Test grün")).await.unwrap();
    assert_eq!(one.id, "ARC-0001");
    assert!(!one.ready_to_merge);
    assert_eq!(one.warnings, Vec::<String>::new());
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.acceptance, "Test grün");

    let two = finish_in(&e, "ARC-0002", Some("ok"), Path::new(&first.path))
        .await
        .unwrap();

    assert!(two.ready_to_merge);
    assert!(
        e.manager
            .registry
            .read()
            .unwrap()
            .get(&first.branch)
            .unwrap()
            .ready_to_merge
    );
}

#[tokio::test]
async fn done_lasst_den_ticket_branch_unberuhrt() {
    let e = Env::new().await;
    e.ticket(NewTicketInput {
        spec: Some("docs/superpowers/specs/2026-10-22-x-design.md".into()),
        acceptance: Some("ok".into()),
        ..touching("Erstes", &["a"])
    })
    .await;
    let started = start(&e, "ARC-0001").await.unwrap();
    let head = e.repo.git(&["rev-parse", &started.branch]);

    let result = finish_in(&e, "ARC-0001", None, Path::new(&started.path))
        .await
        .unwrap();

    assert!(result.ready_to_merge);
    assert_eq!(result.warnings, Vec::<String>::new());
    assert_eq!(e.repo.git(&["rev-parse", &started.branch]), head);
    assert_eq!(
        e.git_in(
            Path::new(&started.path),
            &["diff", "--cached", "--name-only"]
        ),
        ""
    );
    assert_eq!(
        e.f.tickets.get("ARC-0001").await.unwrap().status,
        TicketStatus::Done
    );
}

#[tokio::test]
async fn gebundelt_halt_den_worktree_offen_solange_ein_ticket_derselben_spec_wartet() {
    let e = Env::bundled().await;
    e.ticket(in_spec("Erstes", &["a"], "001-x", "T001")).await;
    e.ticket(in_spec("Zweites", &["a"], "001-x", "T002")).await;
    let joining = StartOptions {
        join: true,
        ..StartOptions::default()
    };

    let first = start_with(&e, "ARC-0001", joining.clone()).await.unwrap();
    let done = finish(&e, "ARC-0001", Some("ok")).await.unwrap();

    assert!(!done.ready_to_merge);
    assert_eq!(done.removed, Vec::<String>::new());
    assert!(Path::new(&first.path).exists());
    assert!(!e.manager.is_ready(&first.branch).await.unwrap());

    let second = start_with(&e, "ARC-0002", joining).await.unwrap();
    assert!(second.joined);
    assert_eq!(second.branch, first.branch);
    assert!(
        finish_in(&e, "ARC-0002", Some("ok"), Path::new(&first.path))
            .await
            .unwrap()
            .ready_to_merge
    );
}

#[tokio::test]
async fn ein_worktree_je_ticket_ist_bereit_sobald_das_ticket_fertig_ist() {
    let e = Env::new().await;
    e.ticket(in_spec("Erstes", &["a"], "001-x", "T001")).await;
    e.ticket(in_spec("Zweites", &["b"], "001-x", "T002")).await;

    let first = start(&e, "ARC-0001").await.unwrap();

    assert!(
        finish(&e, "ARC-0001", Some("ok"))
            .await
            .unwrap()
            .ready_to_merge
    );
    assert!(e.manager.is_ready(&first.branch).await.unwrap());
}

#[tokio::test]
async fn ein_worktree_je_ticket_is_ready_pruft_jetzt_auch_nach_erledigt_von_hand() {
    let e = Env::new().await;
    e.ticket(NewTicketInput {
        acceptance: Some("ok".into()),
        ..NewTicketInput::titled("Erstes")
    })
    .await;
    let first = start(&e, "ARC-0001").await.unwrap();

    assert!(!e.manager.is_ready(&first.branch).await.unwrap());

    e.f.tickets
        .move_to(
            "ARC-0001",
            TicketStatus::Done,
            &e.f.actor,
            &Default::default(),
        )
        .await
        .unwrap();

    assert!(e.manager.is_ready(&first.branch).await.unwrap());
    assert!(
        e.manager
            .registry
            .read()
            .unwrap()
            .get(&first.branch)
            .unwrap()
            .ready_to_merge
    );
}

#[tokio::test]
async fn ein_vom_merge_blockiertes_ticket_gilt_als_fertig() {
    let e = Env::new().await;
    e.ticket(NewTicketInput {
        acceptance: Some("ok".into()),
        ..NewTicketInput::titled("Erstes")
    })
    .await;
    let first = start(&e, "ARC-0001").await.unwrap();

    e.set(
        "ARC-0001",
        TicketPatch {
            status: Some(TicketStatus::Blocked),
            blocked_reason: Some(Some(
                "Merge: lokale Änderungen im Hauptordner: a.txt".into(),
            )),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert!(e.manager.is_ready(&first.branch).await.unwrap());

    e.set(
        "ARC-0001",
        TicketPatch {
            status: Some(TicketStatus::Blocked),
            blocked_reason: Some(Some("Human Interaction: ansehen".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert!(!e.manager.is_ready(&first.branch).await.unwrap());
}

#[tokio::test]
async fn accept_nimmt_ein_human_interaction_ticket_ab_und_meldet_den_worktree_bereit() {
    let e = Env::new().await;
    e.ticket(NewTicketInput {
        acceptance: Some("Oberfläche geprüft".into()),
        ..NewTicketInput::titled("Ansehen")
    })
    .await;
    let started = start(&e, "ARC-0001").await.unwrap();

    assert_eq!(
        code(
            e.manager
                .accept("ARC-0001", None, e.repo.path(), &e.f.actor)
                .await
        ),
        "accept-not-possible"
    );

    e.set(
        "ARC-0001",
        TicketPatch {
            status: Some(TicketStatus::Blocked),
            blocked_reason: Some(Some("Human Interaction: bitte ansehen".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();

    let result = e
        .manager
        .accept("ARC-0001", None, e.repo.path(), &e.f.actor)
        .await
        .unwrap();

    assert_eq!(result.id, "ARC-0001");
    assert_eq!(result.branch.as_deref(), Some(started.branch.as_str()));
    assert!(result.ready_to_merge);
    assert_eq!(result.warnings, Vec::<String>::new());
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.blocked_reason, None);
    assert_eq!(ticket.branch.as_deref(), Some(started.branch.as_str()));
    // Der saubere, fertige Worktree ist danach aufgeräumt
    assert_eq!(result.removed, [started.path]);
}

#[tokio::test]
async fn raumt_fertige_saubere_worktrees_ausserhalb_des_aktuellen_ordners_auf() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;
    e.ticket(NewTicketInput::titled("Zweites")).await;
    e.ticket(NewTicketInput::titled("Drittes")).await;

    let first = start(&e, "ARC-0001").await.unwrap();
    let head = e.repo.git(&["rev-parse", "HEAD"]);
    let done = finish(&e, "ARC-0001", Some("ok")).await.unwrap();

    assert_eq!(done.removed, std::slice::from_ref(&first.path));
    assert!(!Path::new(&first.path).exists());
    assert!(e.repo.git(&["branch", "--list"]).contains(&first.branch));
    assert_eq!(e.repo.git(&["rev-parse", "HEAD"]), head);

    let second = start(&e, "ARC-0002").await.unwrap();
    fs::write(Path::new(&second.path).join("neu.txt"), "wichtig").unwrap();

    assert_eq!(
        finish(&e, "ARC-0002", Some("ok")).await.unwrap().removed,
        Vec::<String>::new()
    );
    assert!(Path::new(&second.path).join("neu.txt").exists());
}

#[tokio::test]
async fn legt_im_neuen_worktree_die_rules_fur_doku_und_board_regeln_an() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;

    let first = start(&e, "ARC-0001").await.unwrap();

    let rules = Path::new(&first.path).join(".claude").join("rules");
    assert!(rules.join("ai-harness-docs.md").exists());
    let board = fs::read_to_string(rules.join("ai-harness-board.md")).unwrap();
    assert!(board.ends_with("rules/board.md\n"), "{board}");
    assert!(board.contains('@'), "{board}");
    // Die Rules sind über `info/exclude` aus Git herausgenommen; offen bleibt nur die
    // Plugin-Einstellung, die der Harness nie in `info/exclude` einträgt (die Tests laufen
    // ohne die globale Git-Konfiguration des Rechners, die sie sonst ausnimmt)
    assert_eq!(
        e.git_in(
            Path::new(&first.path),
            &["status", "--porcelain", "--untracked-files=all"]
        ),
        "?? .claude/settings.local.json"
    );
}

#[tokio::test]
async fn gibt_im_neuen_worktree_den_import_der_doku_regeln_frei() {
    let e = Env::new().await;
    fs::write(e.claude_config_file(), "{\"projects\":{}}").unwrap();
    e.ticket(NewTicketInput::titled("Erstes")).await;

    let first = start(&e, "ARC-0001").await.unwrap();

    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(e.claude_config_file()).unwrap()).unwrap();
    assert_eq!(
        config["projects"][&first.path]["hasClaudeMdExternalIncludesApproved"],
        serde_json::Value::Bool(true)
    );
}

#[tokio::test]
async fn raumt_einen_worktree_auch_auf_wenn_die_rule_nicht_von_git_ausgenommen_ist() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("Erstes")).await;
    let first = start(&e, "ARC-0001").await.unwrap();
    // Projekt aus der Zeit vor der Rule: kein Eintrag in info/exclude
    let common_dir = e
        .repo
        .git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    fs::write(Path::new(&common_dir).join("info").join("exclude"), "").unwrap();

    let done = finish(&e, "ARC-0001", Some("ok")).await.unwrap();

    assert_eq!(done.removed, [first.path]);
}

#[tokio::test]
async fn done_eines_nie_gestarteten_tickets_meldet_keine_warnungen_und_keinen_branch() {
    let e = Env::new().await;
    e.ticket(NewTicketInput {
        spec: Some("docs/superpowers/specs/2026-10-22-x-design.md".into()),
        acceptance: Some("steht schon".into()),
        ..NewTicketInput::titled("Erstes")
    })
    .await;

    let result = finish(&e, "ARC-0001", None).await.unwrap();

    assert_eq!(result.warnings, Vec::<String>::new());
    assert_eq!(result.branch, None);
    assert_eq!(
        e.f.tickets.get("ARC-0001").await.unwrap().status,
        TicketStatus::Done
    );
}

#[tokio::test]
async fn lehnt_start_und_done_ab_wenn_das_ticket_auf_einem_anderen_board_liegt() {
    let e = Env::new().await;
    e.ticket(NewTicketInput {
        acceptance: Some("ok".into()),
        ..NewTicketInput::titled("Fremd")
    })
    .await;
    let other =
        e.f.boards
            .create("ARC", "Anderes", &e.f.actor)
            .await
            .unwrap();
    let repo_board = RepoBoard {
        id: other.id,
        name: other.name.clone(),
    };
    let foreign = StartOptions {
        repo_board: Some(repo_board.clone()),
        ..StartOptions::default()
    };

    let error = start_with(&e, "ARC-0001", foreign.clone())
        .await
        .expect_err("fremdes Board wird abgelehnt");
    let harness = error.harness().expect("erwartbarer Fehler");
    assert_eq!(harness.code, "board-repo-mismatch");
    assert!(
        harness.message.contains(&format!("Board {}", e.f.board_id))
            && harness.message.contains("Sprint")
            && harness.message.contains(&format!("Board {}", other.id))
            && harness.message.contains("Anderes")
            && harness.message.contains("nexus board boards repo"),
        "{}",
        harness.message
    );
    assert_eq!(e.repo.git(&["branch", "--list", "ticket/*"]), "");
    assert_eq!(
        code(
            e.manager
                .finish(
                    "ARC-0001",
                    &crate::worktrees::manager::FinishOptions {
                        repo_board: Some(repo_board),
                        ..Default::default()
                    },
                    e.repo.path(),
                    &e.f.actor
                )
                .await
        ),
        "board-repo-mismatch"
    );
    assert_ne!(
        e.f.tickets.get("ARC-0001").await.unwrap().status,
        TicketStatus::Done
    );

    let own = RepoBoard {
        id: e.f.board_id,
        name: "Sprint".into(),
    };
    assert!(
        !start_with(
            &e,
            "ARC-0001",
            StartOptions {
                repo_board: Some(own.clone()),
                ..StartOptions::default()
            }
        )
        .await
        .unwrap()
        .joined
    );
    assert_eq!(
        e.manager
            .finish(
                "ARC-0001",
                &crate::worktrees::manager::FinishOptions {
                    repo_board: Some(own),
                    ..Default::default()
                },
                e.repo.path(),
                &e.f.actor
            )
            .await
            .unwrap()
            .id,
        "ARC-0001"
    );
}

// ---------------------------------------------------------------------------------------------
// Zielbranch
// ---------------------------------------------------------------------------------------------

/// Ein Ticket mit Zielbranch
fn with_target(title: &str, touches: &[&str], target: &str) -> NewTicketInput {
    NewTicketInput {
        target_branch: Some(target.to_string()),
        ..touching(title, touches)
    }
}

#[tokio::test]
async fn zweigt_vom_zielbranch_ab_und_legt_ihn_bei_bedarf_an() {
    let e = Env::new().await;
    e.ticket(with_target("A", &[], "release/1")).await;

    let first = start(&e, "ARC-0001").await.unwrap();

    let notice = first.target_created.clone().unwrap_or_default();
    assert!(
        notice.starts_with("Zielbranch release/1 neu angelegt von ") && notice.ends_with('.'),
        "{notice}"
    );

    e.repo
        .git(&["commit", "--allow-empty", "-q", "-m", "nur auf main"]);
    e.ticket(with_target("B", &[], "release/1")).await;
    let second = start(&e, "ARC-0002").await.unwrap();

    assert_eq!(second.target_created, None);
    // Ticket-Branch steht auf release/1, der spätere Commit des Hauptordners fehlt darin
    assert_eq!(
        e.repo.git(&["rev-parse", &second.branch]),
        e.repo.git(&["rev-parse", "release/1"])
    );
    assert!(
        !e.repo
            .try_git(&["merge-base", "--is-ancestor", "HEAD", &second.branch])
            .0
    );
}

#[tokio::test]
async fn legt_einen_nur_als_origin_vorhandenen_zielbranch_von_dort_an() {
    let e = Env::new().await;
    e.repo
        .git(&["commit", "--allow-empty", "-q", "-m", "remote"]);
    e.repo
        .git(&["update-ref", "refs/remotes/origin/release/2", "HEAD"]);
    e.repo.git(&["reset", "-q", "--hard", "HEAD~1"]);
    e.ticket(with_target("A", &[], "release/2")).await;

    let first = start(&e, "ARC-0001").await.unwrap();

    assert_eq!(
        first.target_created.as_deref(),
        Some("Zielbranch release/2 neu angelegt von origin/release/2.")
    );
    assert_eq!(
        e.repo.git(&["rev-parse", &first.branch]),
        e.repo.git(&["rev-parse", "refs/remotes/origin/release/2"])
    );
}

#[tokio::test]
async fn ohne_zielbranch_bleibt_der_start_wie_bisher() {
    let e = Env::new().await;
    e.ticket(NewTicketInput::titled("A")).await;

    let first = start(&e, "ARC-0001").await.unwrap();

    assert_eq!(first.target_created, None);
    assert_eq!(
        e.repo.git(&["rev-parse", &first.branch]),
        e.repo.git(&["rev-parse", "HEAD"])
    );
}

#[tokio::test]
async fn tritt_nur_worktrees_mit_gleichem_zielbranch_bei() {
    let e = Env::new().await;
    e.ticket(with_target("A", &["a"], "release/1")).await;
    e.ticket(with_target("B", &["b"], "release/2")).await;
    start(&e, "ARC-0001").await.unwrap();

    let via_in = start_with(
        &e,
        "ARC-0002",
        StartOptions {
            in_ticket: Some("ARC-0001".into()),
            ..StartOptions::default()
        },
    )
    .await
    .unwrap();

    assert!(!via_in.joined);
    assert!(
        via_in
            .warning
            .as_deref()
            .unwrap_or("")
            .contains("anderen Zielbranch"),
        "{:?}",
        via_in.warning
    );
}

#[tokio::test]
async fn tritt_mit_join_nur_einem_worktree_derselben_spec_und_desselben_zielbranches_bei() {
    let e = Env::new().await;
    e.ticket(NewTicketInput {
        target_branch: Some("release/1".into()),
        ..in_spec("A", &["a"], "001-x", "T001")
    })
    .await;
    e.ticket(NewTicketInput {
        target_branch: Some("release/2".into()),
        ..in_spec("B", &["b"], "001-x", "T002")
    })
    .await;
    e.ticket(NewTicketInput {
        target_branch: Some("release/2".into()),
        ..in_spec("C", &["c"], "001-x", "T003")
    })
    .await;
    e.ticket(in_spec("D", &["d"], "001-x", "T004")).await;
    let joining = StartOptions {
        join: true,
        ..StartOptions::default()
    };

    let first = start_with(&e, "ARC-0001", joining.clone()).await.unwrap();
    let second = start_with(&e, "ARC-0002", joining.clone()).await.unwrap();

    assert!(!second.joined);
    assert_ne!(second.branch, first.branch);
    // gleiche Spec und gleicher Zielbranch: Beitritt zum zweiten, nicht zum ersten Worktree
    assert_eq!(
        e.manager.join_candidate("ARC-0003", false).await.unwrap(),
        Some(second.branch.clone())
    );
    let third = start_with(&e, "ARC-0003", joining.clone()).await.unwrap();
    assert!(third.joined);
    assert_eq!(third.branch, second.branch);
    // ohne Zielbranch tritt ein Ticket keinem Worktree mit Zielbranch bei
    assert_eq!(
        e.manager.join_candidate("ARC-0004", false).await.unwrap(),
        None
    );
    assert!(!start_with(&e, "ARC-0004", joining).await.unwrap().joined);
}

#[tokio::test]
async fn raumt_nach_merge_in_einen_zielbranch_ausserhalb_von_head_auf() {
    let e = Env::new().await;
    e.ticket(with_target("A", &[], "release/1")).await;
    let started = start(&e, "ARC-0001").await.unwrap();
    let worktree = Path::new(&started.path);
    fs::write(worktree.join("a.txt"), "a").unwrap();
    e.git_in(worktree, &["add", "a.txt"]);
    e.git_in(worktree, &["commit", "-q", "-m", "a"]);
    let merge_worktree = TargetBranches::new()
        .open_merge_worktree(e.repo.path(), "release/1")
        .unwrap();
    e.git_in(
        Path::new(&merge_worktree),
        &["merge", "--no-ff", "-q", "--no-edit", &started.branch],
    );

    let out = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, Some("release/1"))
        .await
        .unwrap();

    assert_eq!(out.warnings, Vec::<String>::new());
    assert_eq!(out.removed.as_deref(), Some(started.path.as_str()));
    assert_eq!(e.repo.git(&["branch", "--list", &started.branch]), "");
    assert!(!worktree.exists());
    let ticket = e.f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(ticket.branch, None);
    assert_eq!(ticket.worktree, None);
}

#[tokio::test]
async fn raumt_nichts_auf_solange_der_branch_nicht_im_zielbranch_steckt() {
    let e = Env::new().await;
    e.ticket(with_target("A", &[], "release/1")).await;
    let started = start(&e, "ARC-0001").await.unwrap();
    let worktree = Path::new(&started.path);
    fs::write(worktree.join("a.txt"), "a").unwrap();
    e.git_in(worktree, &["add", "a.txt"]);
    e.git_in(worktree, &["commit", "-q", "-m", "a"]);
    // Nur im Hauptordner gemergt, nicht in release/1
    e.repo
        .git(&["merge", "--no-ff", "-q", "--no-edit", &started.branch]);

    let out = e
        .manager
        .remove_merged(&started.branch, &e.f.actor, Some("release/1"))
        .await
        .unwrap();

    assert_eq!(out.removed, None);
    assert_eq!(out.tickets, ["ARC-0001"]);
    assert_eq!(out.warnings.len(), 1);
    assert!(
        out.warnings[0].contains("nicht gemergt"),
        "{:?}",
        out.warnings
    );
    assert!(e
        .repo
        .git(&["branch", "--list", &started.branch])
        .contains(&started.branch));
    assert!(worktree.exists());
}

/// Die Einstellung „ein Worktree je Ticket“ ist der Standard
#[test]
fn ein_worktree_je_ticket_ist_der_standard() {
    assert_eq!(JoinWorktrees::default(), JoinWorktrees::None);
}
