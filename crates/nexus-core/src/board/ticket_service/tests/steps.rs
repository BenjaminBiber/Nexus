//! Schritte anlegen, bearbeiten, löschen und verschieben.
//! Portiert aus `packages/core/tests/unit/board/TicketStepEditing.test.ts`.

use super::{add, create};
use crate::board::ticket_service::{AddStepOptions, NewTicketInput, TicketDetail};
use crate::board::ticket_step_edits::{StepChange, StepGuard};
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

/// Ein Ticket mit den beiden Schritten
async fn with_steps(f: &BoardFixture, steps: Vec<TicketStep>) {
    create(
        f,
        NewTicketInput {
            steps: Some(steps),
            ..NewTicketInput::titled("A")
        },
    )
    .await
    .unwrap();
}

/// Die Texte der Schritte einer Detailfassung
fn texts(detail: &TicketDetail) -> Vec<&str> {
    detail.steps.iter().map(|step| step.text.as_str()).collect()
}

#[tokio::test]
async fn hangt_schritte_an_auch_ohne_schritte_und_fugt_an_position_ein() {
    let f = BoardFixture::create().await;
    add(&f, "A").await;

    let first = f
        .tickets
        .add_step(
            "ARC-0001",
            "  Test schreiben ",
            &f.actor,
            &AddStepOptions {
                detail: Some("Erwartet: rot".into()),
                ..AddStepOptions::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(first.view.ticket.rev, 2);
    assert_eq!(first.view.steps_total, 1);
    assert_eq!(
        first.steps,
        [TicketStep {
            text: "Test schreiben".into(),
            done: false,
            detail: Some("Erwartet: rot".into()),
        }]
    );
    let entry = first.history.last().unwrap();
    assert_eq!(entry.field, "step");
    assert_eq!(entry.from, None);
    assert_eq!(entry.to.as_deref(), Some("1: Test schreiben"));
    assert_eq!(entry.note.as_deref(), Some("Schritt hinzugefügt"));

    f.tickets
        .add_step("arc-1", "Committen", &f.actor, &AddStepOptions::default())
        .await
        .unwrap();
    let inserted = f
        .tickets
        .add_step(
            "ARC-0001",
            "Umsetzen",
            &f.actor,
            &AddStepOptions {
                at: Some(2),
                ..AddStepOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        texts(&inserted),
        ["Test schreiben", "Umsetzen", "Committen"]
    );
    assert_eq!(inserted.view.ticket.rev, 4);

    let error = f
        .tickets
        .add_step(
            "ARC-0001",
            "X",
            &f.actor,
            &AddStepOptions {
                at: Some(5),
                ..AddStepOptions::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("step-not-found"));
    assert!(error.to_string().contains("1 bis 4"));

    for (text, code) in [("  ", "step-empty"), (&"x".repeat(1001), "value-too-long")] {
        assert_eq!(
            f.tickets
                .add_step("ARC-0001", text, &f.actor, &AddStepOptions::default())
                .await
                .unwrap_err()
                .code(),
            Some(code)
        );
    }
    assert_eq!(
        f.tickets
            .add_step("ARC-0099", "X", &f.actor, &AddStepOptions::default())
            .await
            .unwrap_err()
            .code(),
        Some("ticket-not-found")
    );
}

#[tokio::test]
async fn bearbeitet_text_und_anleitung_behalt_den_haken_und_entfernt_eine_leere_anleitung() {
    let f = BoardFixture::create().await;
    with_steps(&f, steps()).await;
    f.tickets
        .set_step("ARC-0001", 1, true, &f.actor, &StepGuard::default())
        .await
        .unwrap();

    let edited = f
        .tickets
        .edit_step(
            "ARC-0001",
            1,
            &StepChange {
                text: Some("Test zuerst".into()),
                detail: None,
            },
            &f.actor,
            &StepGuard::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        edited.steps[0],
        TicketStep {
            text: "Test zuerst".into(),
            done: true,
            detail: Some("Erwartet: rot".into()),
        }
    );
    let entry = edited.history.last().unwrap();
    assert_eq!(entry.from.as_deref(), Some("1: Test schreiben"));
    assert_eq!(entry.to.as_deref(), Some("1: Test zuerst"));
    assert_eq!(entry.note.as_deref(), Some("Schritt bearbeitet"));

    let detail = f
        .tickets
        .edit_step(
            "ARC-0001",
            2,
            &StepChange {
                text: None,
                detail: Some(Some("npm test".into())),
            },
            &f.actor,
            &StepGuard::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        detail.steps[1],
        TicketStep {
            text: "Umsetzen".into(),
            done: false,
            detail: Some("npm test".into()),
        }
    );
    assert_eq!(
        detail.history.last().unwrap().note.as_deref(),
        Some("Anleitung geändert")
    );

    let cleared = f
        .tickets
        .edit_step(
            "ARC-0001",
            1,
            &StepChange {
                text: None,
                detail: Some(Some(String::new())),
            },
            &f.actor,
            &StepGuard::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        cleared.steps[0],
        TicketStep {
            text: "Test zuerst".into(),
            done: true,
            detail: None,
        }
    );

    let same = f
        .tickets
        .edit_step(
            "ARC-0001",
            1,
            &StepChange {
                text: Some("Test zuerst".into()),
                detail: None,
            },
            &f.actor,
            &StepGuard::default(),
        )
        .await
        .unwrap();
    assert_eq!(same.view.ticket.rev, cleared.view.ticket.rev);

    assert_eq!(
        f.tickets
            .edit_step(
                "ARC-0001",
                3,
                &StepChange {
                    text: Some("X".into()),
                    detail: None
                },
                &f.actor,
                &StepGuard::default(),
            )
            .await
            .unwrap_err()
            .code(),
        Some("step-not-found")
    );
    assert_eq!(
        f.tickets
            .edit_step(
                "ARC-0001",
                1,
                &StepChange::default(),
                &f.actor,
                &StepGuard::default()
            )
            .await
            .unwrap_err()
            .code(),
        Some("nothing-changed")
    );
    assert_eq!(
        f.tickets
            .edit_step(
                "ARC-0001",
                1,
                &StepChange {
                    text: Some(" ".into()),
                    detail: None
                },
                &f.actor,
                &StepGuard::default(),
            )
            .await
            .unwrap_err()
            .code(),
        Some("step-empty")
    );
}

#[tokio::test]
async fn loscht_schritte_mit_verlauf_die_folgenden_rucken_nach() {
    let f = BoardFixture::create().await;
    with_steps(&f, steps()).await;

    let removed = f
        .tickets
        .remove_step("ARC-0001", 1, &f.actor, &StepGuard::default())
        .await
        .unwrap();

    assert_eq!(
        removed.steps,
        [TicketStep {
            text: "Umsetzen".into(),
            done: false,
            detail: None,
        }]
    );
    assert_eq!(removed.view.ticket.rev, 2);
    assert_eq!(removed.view.steps_total, 1);
    let entry = removed.history.last().unwrap();
    assert_eq!(entry.field, "step");
    assert_eq!(entry.from.as_deref(), Some("1: Test schreiben"));
    assert_eq!(entry.to, None);
    assert_eq!(entry.note.as_deref(), Some("Schritt gelöscht"));

    f.tickets
        .remove_step("ARC-0001", 1, &f.actor, &StepGuard::default())
        .await
        .unwrap();
    let error = f
        .tickets
        .remove_step("ARC-0001", 1, &f.actor, &StepGuard::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("step-not-found"));
    assert!(error.to_string().contains("keine Schritte"));
}

#[tokio::test]
async fn verschiebt_schritte_samt_haken_und_anleitung() {
    let f = BoardFixture::create().await;
    let mut all = steps();
    all.push(TicketStep {
        text: "Committen".into(),
        done: true,
        detail: None,
    });
    with_steps(&f, all).await;

    let moved = f
        .tickets
        .move_step("ARC-0001", 3, 1, &f.actor, &StepGuard::default())
        .await
        .unwrap();
    assert_eq!(texts(&moved), ["Committen", "Test schreiben", "Umsetzen"]);
    assert!(moved.steps[0].done);
    let entry = moved.history.last().unwrap();
    assert_eq!(entry.field, "step");
    assert_eq!(entry.from.as_deref(), Some("3: Committen"));
    assert_eq!(entry.to.as_deref(), Some("1: Committen"));
    assert_eq!(entry.note.as_deref(), Some("Schritt verschoben"));

    let down = f
        .tickets
        .move_step("ARC-0001", 2, 3, &f.actor, &StepGuard::default())
        .await
        .unwrap();
    assert_eq!(texts(&down), ["Committen", "Umsetzen", "Test schreiben"]);
    assert_eq!(down.steps[2].detail.as_deref(), Some("Erwartet: rot"));

    let same = f
        .tickets
        .move_step("ARC-0001", 2, 2, &f.actor, &StepGuard::default())
        .await
        .unwrap();
    assert_eq!(same.view.ticket.rev, down.view.ticket.rev);

    for (from, to) in [(1, 4), (0, 1)] {
        assert_eq!(
            f.tickets
                .move_step("ARC-0001", from, to, &f.actor, &StepGuard::default())
                .await
                .unwrap_err()
                .code(),
            Some("step-not-found")
        );
    }
}

#[tokio::test]
async fn pruft_erwarteten_text_und_version() {
    let f = BoardFixture::create().await;
    with_steps(&f, steps()).await;
    // Ein anderer verschiebt Schritt 2 nach oben; wer noch „1 = Test schreiben“ kennt, trifft
    // daneben
    f.tickets
        .move_step("ARC-0001", 2, 1, &f.actor, &StepGuard::default())
        .await
        .unwrap();

    let expected = StepGuard {
        rev: None,
        expect_text: Some("Test schreiben".into()),
    };
    let conflict = "ARC-0001: Schritt 1 ist inzwischen „Umsetzen“ – die Schritte wurden geändert.";
    let errors = [
        f.tickets
            .edit_step(
                "ARC-0001",
                1,
                &StepChange {
                    text: Some("X".into()),
                    detail: None,
                },
                &f.actor,
                &expected,
            )
            .await
            .map(|_| ())
            .unwrap_err(),
        f.tickets
            .remove_step("ARC-0001", 1, &f.actor, &expected)
            .await
            .map(|_| ())
            .unwrap_err(),
        f.tickets
            .move_step("ARC-0001", 1, 2, &f.actor, &expected)
            .await
            .map(|_| ())
            .unwrap_err(),
        f.tickets
            .set_step("ARC-0001", 1, true, &f.actor, &expected)
            .await
            .map(|_| ())
            .unwrap_err(),
        f.tickets
            .add_step(
                "ARC-0001",
                "Neu",
                &f.actor,
                &AddStepOptions {
                    at: Some(1),
                    expect_text: Some("Test schreiben".into()),
                    ..AddStepOptions::default()
                },
            )
            .await
            .map(|_| ())
            .unwrap_err(),
    ];
    for error in &errors {
        assert_eq!(error.code(), Some("conflict"));
        assert_eq!(error.to_string(), conflict);
    }

    let gone = f
        .tickets
        .remove_step(
            "ARC-0001",
            3,
            &f.actor,
            &StepGuard {
                rev: None,
                expect_text: Some("Weg".into()),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(gone.code(), Some("conflict"));
    assert!(gone.to_string().contains("gibt es inzwischen nicht mehr"));

    let unchanged = f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(texts(&unchanged), ["Umsetzen", "Test schreiben"]);

    // Passender Text (Rand-Leerraum egal) und passende Version gehen durch
    let removed = f
        .tickets
        .remove_step(
            "ARC-0001",
            2,
            &f.actor,
            &StepGuard {
                rev: Some(unchanged.view.ticket.rev),
                expect_text: Some(" Test schreiben ".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(texts(&removed), ["Umsetzen"]);

    let veraltet = StepGuard {
        rev: Some(unchanged.view.ticket.rev),
        expect_text: None,
    };
    assert_eq!(
        f.tickets
            .set_step("ARC-0001", 1, true, &f.actor, &veraltet)
            .await
            .unwrap_err()
            .code(),
        Some("conflict")
    );
    assert_eq!(
        f.tickets
            .add_step(
                "ARC-0001",
                "Neu",
                &f.actor,
                &AddStepOptions {
                    rev: Some(unchanged.view.ticket.rev),
                    ..AddStepOptions::default()
                }
            )
            .await
            .unwrap_err()
            .code(),
        Some("conflict")
    );

    let done = f
        .tickets
        .set_step(
            "ARC-0001",
            1,
            true,
            &f.actor,
            &StepGuard {
                rev: Some(removed.view.ticket.rev),
                expect_text: Some("Umsetzen".into()),
            },
        )
        .await
        .unwrap();
    assert!(done.steps[0].done);
}
