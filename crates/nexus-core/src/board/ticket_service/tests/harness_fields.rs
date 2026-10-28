//! Harness-Felder am Ticket: Spec, Bereiche, Schnittstellen, Schritte, Sessions, Zielbranch.
//! Portiert aus `packages/core/tests/unit/board/TicketHarnessFields.test.ts`.

use super::{add, create, history, set};
use crate::board::basics::TicketStatus;
use crate::board::ticket_ordering::BoardColumn;
use crate::board::ticket_service::{
    ClaimOptions, NewTicketInput, TicketPatch, TicketSummary, WriteOptions,
};
use crate::board::ticket_step_edits::StepGuard;
use crate::board::types::TicketStep;
use crate::testing::BoardFixture;

/// Die zwei Schritte, mit denen die Tests arbeiten (`STEPS` in TypeScript)
fn steps() -> Vec<TicketStep> {
    vec![
        TicketStep {
            text: "Test schreiben".into(),
            done: false,
            detail: Some("Erwartet: rot".into()),
        },
        TicketStep {
            text: "Umsetzen".into(),
            done: false,
            detail: None,
        },
    ]
}

#[tokio::test]
async fn legt_ein_ticket_mit_spec_bereichen_schnittstellen_und_schritten_an() {
    let f = BoardFixture::create().await;

    let t = create(
        &f,
        NewTicketInput {
            spec: Some("001-login".into()),
            spec_task: Some("T001".into()),
            touches: Some(vec![" src/login ".into(), String::new()]),
            consumes: Some(vec!["GET /a".into()]),
            produces: Some(vec!["<Login>".into()]),
            steps: Some(steps()),
            ..NewTicketInput::titled("Login")
        },
    )
    .await
    .unwrap();

    assert_eq!(t.ticket.spec.as_deref(), Some("001-login"));
    assert_eq!(t.ticket.spec_task.as_deref(), Some("T001"));
    assert_eq!(t.ticket.branch, None);
    assert_eq!(t.ticket.worktree, None);
    assert_eq!(t.ticket.touches, ["src/login"]);
    assert_eq!(t.steps_done, 0);
    assert_eq!(t.steps_total, 2);
    assert!(t.sessions.is_empty());

    let detail = f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(detail.steps, steps());
    assert_eq!(detail.view.ticket.consumes, ["GET /a"]);
    assert_eq!(detail.view.ticket.produces, ["<Login>"]);
}

#[tokio::test]
async fn andert_branch_worktree_und_bereiche_mit_verlauf_und_ersetzt_schritte_nur_bei_anderung() {
    let f = BoardFixture::create().await;
    create(
        &f,
        NewTicketInput {
            steps: Some(steps()),
            ..NewTicketInput::titled("A")
        },
    )
    .await
    .unwrap();

    set(
        &f,
        "ARC-0001",
        TicketPatch {
            branch: Some(Some("ticket/ARC-0001-a".into())),
            worktree: Some(Some("C:/w/ARC-0001".into())),
            touches: Some(vec!["src".into()]),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();

    let entries = history(&f, "ARC-0001").await;
    for wanted in [
        ("branch", None, Some("ticket/ARC-0001-a")),
        ("worktree", None, Some("C:/w/ARC-0001")),
        ("touches", Some("[]"), Some(r#"["src"]"#)),
    ] {
        assert!(
            entries.iter().any(|(field, from, to)| field == wanted.0
                && from.as_deref() == wanted.1
                && to.as_deref() == wanted.2),
            "{wanted:?} fehlt im Verlauf {entries:?}"
        );
    }

    let mut mehr = steps();
    mehr.push(TicketStep {
        text: "Committen".into(),
        done: false,
        detail: None,
    });
    set(
        &f,
        "ARC-0001",
        TicketPatch {
            steps: Some(mehr),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    let after_steps = f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(after_steps.view.steps_total, 3);
    let last = after_steps.history.last().unwrap();
    assert_eq!(last.field, "steps");
    assert_eq!(last.from.as_deref(), Some("2 Schritte"));
    assert_eq!(last.to.as_deref(), Some("3 Schritte"));

    set(
        &f,
        "ARC-0001",
        TicketPatch {
            steps: Some(after_steps.steps.clone()),
            title: Some("A2".into()),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        f.tickets
            .get("ARC-0001")
            .await
            .unwrap()
            .history
            .iter()
            .filter(|entry| entry.field == "steps")
            .count(),
        1
    );

    set(
        &f,
        "ARC-0001",
        TicketPatch {
            branch: Some(None),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        f.tickets.get("ARC-0001").await.unwrap().view.ticket.branch,
        None
    );
}

#[tokio::test]
async fn pruft_schritte_und_langen() {
    let f = BoardFixture::create().await;
    let leer = TicketStep {
        text: "  ".into(),
        done: false,
        detail: None,
    };

    assert_eq!(
        create(
            &f,
            NewTicketInput {
                steps: Some(vec![leer]),
                ..NewTicketInput::titled("A")
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("step-empty")
    );
    assert_eq!(
        create(
            &f,
            NewTicketInput {
                spec: Some("x".repeat(201)),
                ..NewTicketInput::titled("A")
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("value-too-long")
    );
    assert_eq!(
        create(
            &f,
            NewTicketInput {
                steps: Some(vec![TicketStep {
                    text: "x".repeat(1001),
                    done: false,
                    detail: None,
                }]),
                ..NewTicketInput::titled("A")
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("value-too-long")
    );

    add(&f, "B").await;
    assert_eq!(
        set(
            &f,
            "ARC-0001",
            TicketPatch {
                worktree: Some(Some("w".repeat(501))),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("value-too-long")
    );
}

#[tokio::test]
async fn hakt_schritte_ab_und_zuruck_erhoht_dabei_die_version_und_schreibt_den_verlauf() {
    let f = BoardFixture::create().await;
    create(
        &f,
        NewTicketInput {
            steps: Some(steps()),
            ..NewTicketInput::titled("A")
        },
    )
    .await
    .unwrap();

    let done = f
        .tickets
        .set_step("ARC-0001", 1, true, &f.actor, &StepGuard::default())
        .await
        .unwrap();
    assert_eq!(done.view.steps_done, 1);
    assert_eq!(done.view.steps_total, 2);
    assert_eq!(done.view.ticket.rev, 2);
    assert!(done.steps[0].done);
    let entry = done.history.last().unwrap();
    assert_eq!(entry.field, "step");
    assert_eq!(entry.from.as_deref(), Some("1: offen"));
    assert_eq!(entry.to.as_deref(), Some("1: erledigt"));
    assert_eq!(entry.note.as_deref(), Some("Test schreiben"));

    assert_eq!(
        f.tickets
            .set_step("ARC-0001", 1, true, &f.actor, &StepGuard::default())
            .await
            .unwrap()
            .view
            .ticket
            .rev,
        2
    );
    let back = f
        .tickets
        .set_step("ARC-0001", 1, false, &f.actor, &StepGuard::default())
        .await
        .unwrap();
    assert_eq!(back.view.steps_done, 0);
    assert_eq!(back.view.ticket.rev, 3);

    let error = f
        .tickets
        .set_step("ARC-0001", 3, true, &f.actor, &StepGuard::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("step-not-found"));
    assert!(error.to_string().contains("1 bis 2"));

    add(&f, "Ohne").await;
    let error = f
        .tickets
        .set_step("ARC-0002", 1, true, &f.actor, &StepGuard::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("step-not-found"));
    assert!(error.to_string().contains("keine Schritte"));
}

#[tokio::test]
async fn verknupft_sessions_uber_claim_und_link_session_und_findet_die_tickets_einer_session() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;
    add(&f, "B").await;

    let claimed = f
        .tickets
        .claim(
            "ARC-0001",
            "claude",
            &f.actor,
            &ClaimOptions {
                session_id: Some("s-1".into()),
                ..ClaimOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(claimed.sessions, ["s-1"]);

    assert!(f.tickets.link_session("ARC-0002", "s-1").await.unwrap());
    assert!(!f.tickets.link_session("arc-2", "s-1").await.unwrap());
    assert_eq!(f.tickets.get("ARC-0002").await.unwrap().view.ticket.rev, 1);
    assert_eq!(
        f.tickets.tickets_of_session("s-1").await.unwrap(),
        ["ARC-0001", "ARC-0002"]
    );
    assert_eq!(
        f.tickets
            .link_session("ARC-0002", " ")
            .await
            .unwrap_err()
            .code(),
        Some("session-invalid")
    );
    assert_eq!(
        f.tickets
            .link_session("ARC-0002", &"s".repeat(101))
            .await
            .unwrap_err()
            .code(),
        Some("value-too-long")
    );
}

#[tokio::test]
async fn pruft_vor_dem_start_dieselben_sperren_wie_in_arbeit() {
    let f = BoardFixture::create().await;
    add(&f, "Basis").await;
    create(
        &f,
        NewTicketInput {
            depends_on: Some(vec!["ARC-0001".into()]),
            ..NewTicketInput::titled("Folge")
        },
    )
    .await
    .unwrap();

    assert_eq!(
        f.tickets
            .check_start("ARC-0002", false)
            .await
            .unwrap_err()
            .code(),
        Some("dependency-open")
    );
    assert_eq!(
        f.tickets
            .check_start("ARC-0002", true)
            .await
            .unwrap()
            .ticket
            .id,
        "ARC-0002"
    );
    assert_eq!(
        f.tickets
            .check_start("ARC-0001", false)
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::Backlog
    );

    f.open_question(1, "Offen?", &["ARC-0001"]).await;
    assert_eq!(
        f.tickets
            .check_start("ARC-0001", false)
            .await
            .unwrap_err()
            .code(),
        Some("question-open")
    );
}

#[tokio::test]
async fn liefert_kurzangaben_zu_bekannten_tickets_und_ubergeht_unbekannte() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;
    create(
        &f,
        NewTicketInput {
            status: Some(TicketStatus::Planned),
            ..NewTicketInput::titled("B")
        },
    )
    .await
    .unwrap();

    let wanted = ["ARC-0002", "ARC-0099", "T-0001", "kaputt", "ARC-0001"]
        .map(String::from)
        .to_vec();
    assert_eq!(
        f.tickets.summaries(&wanted).await.unwrap(),
        [
            TicketSummary {
                id: "ARC-0002".into(),
                title: "B".into(),
                status: TicketStatus::Planned,
                board_id: f.board_id,
            },
            TicketSummary {
                id: "ARC-0001".into(),
                title: "A".into(),
                status: TicketStatus::Backlog,
                board_id: f.board_id,
            },
        ]
    );
    assert!(f.tickets.summaries(&[]).await.unwrap().is_empty());
}

#[tokio::test]
async fn zeigt_fortschritt_und_sessions_auf_den_karten_des_boards() {
    let f = BoardFixture::create().await;
    create(
        &f,
        NewTicketInput {
            steps: Some(steps()),
            ..NewTicketInput::titled("A")
        },
    )
    .await
    .unwrap();
    f.tickets
        .set_step("ARC-0001", 2, true, &f.actor, &StepGuard::default())
        .await
        .unwrap();
    f.tickets.link_session("ARC-0001", "s-7").await.unwrap();

    let view = f.views.get(f.board_id).await.unwrap();
    let card = &view
        .columns
        .iter()
        .find(|column| column.id == BoardColumn::Backlog)
        .unwrap()
        .tickets[0];

    assert_eq!(card.steps_done, 1);
    assert_eq!(card.steps_total, 2);
    assert_eq!(card.sessions, ["s-7"]);
}

#[tokio::test]
async fn legt_ein_ticket_mit_zielbranch_an_und_andert_ihn_vor_dem_start_mit_verlauf() {
    let f = BoardFixture::create().await;

    assert_eq!(
        create(
            &f,
            NewTicketInput {
                target_branch: Some(" release/1 ".into()),
                ..NewTicketInput::titled("A")
            }
        )
        .await
        .unwrap()
        .ticket
        .target_branch
        .as_deref(),
        Some("release/1")
    );

    set(
        &f,
        "ARC-0001",
        TicketPatch {
            target_branch: Some(Some("release/2".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    let t = f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(t.view.ticket.target_branch.as_deref(), Some("release/2"));
    assert!(history(&f, "ARC-0001").await.contains(&(
        "targetBranch".to_string(),
        Some("release/1".to_string()),
        Some("release/2".to_string())
    )));

    set(
        &f,
        "ARC-0001",
        TicketPatch {
            target_branch: Some(Some(String::new())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        f.tickets
            .get("ARC-0001")
            .await
            .unwrap()
            .view
            .ticket
            .target_branch,
        None
    );
}

#[tokio::test]
async fn lehnt_ungultige_zielbranches_und_anderungen_nach_dem_start_ab() {
    let f = BoardFixture::create().await;

    assert_eq!(
        create(
            &f,
            NewTicketInput {
                target_branch: Some("ticket/ARC-0009-x".into()),
                ..NewTicketInput::titled("A")
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("target-branch-invalid")
    );
    create(
        &f,
        NewTicketInput {
            target_branch: Some("release/1".into()),
            ..NewTicketInput::titled("B")
        },
    )
    .await
    .unwrap();
    set(
        &f,
        "ARC-0001",
        TicketPatch {
            branch: Some(Some("ticket/ARC-0001-b".into())),
            worktree: Some(Some("C:/w/ARC-0001".into())),
            ..TicketPatch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        set(
            &f,
            "ARC-0001",
            TicketPatch {
                target_branch: Some(Some("release/2".into())),
                ..TicketPatch::default()
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("target-branch-locked")
    );
    // gleicher Wert ist keine Änderung und bleibt erlaubt
    f.tickets
        .update(
            "ARC-0001",
            &TicketPatch {
                target_branch: Some(Some("release/1".into())),
                title: Some("B2".into()),
                ..TicketPatch::default()
            },
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
}
