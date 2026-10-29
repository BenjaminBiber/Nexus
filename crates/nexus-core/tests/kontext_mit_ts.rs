//! Gegenprobe zum Hook-Kontext: Der Text, den `ContextBuilder` einspeist, steht im
//! Hook-Vertrag und landet in jeder Claude-Session. Er muss in Rust Zeichen für Zeichen
//! derselbe sein wie in TypeScript.
//!
//! Die Vergleichstexte in `tests/fixtures/context/*.txt` hat
//! `tests/fixtures/context/make.mjs` mit `packages/core/dist` erzeugt (siehe Kopf des
//! Skripts). Hier werden dieselben drei Zustände auf der Datenbank im Speicher aufgebaut
//! und mit den Dateien verglichen.

use std::sync::Arc;

use chrono::FixedOffset;
use nexus_core::board::basics::{BoardActor, TicketStatus};
use nexus_core::board::comment_service::CommentService;
use nexus_core::board::question_service::NewBoardQuestionInput;
use nexus_core::board::ticket_service::{NewTicketInput, TicketPatch, TicketService, WriteOptions};
use nexus_core::board::types::TicketStep;
use nexus_core::hooks::{ContextBuilder, ContextKind};
use nexus_core::testing::BoardFixture;

/// Die Zeitzone, in der `make.mjs` die Texte erzeugt hat
fn utc() -> FixedOffset {
    FixedOffset::east_opt(0).expect("UTC ist eine gültige Zone")
}

/// Der erwartete Text eines Zustands
fn erwartet(name: &str) -> String {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/context")
        .join(format!("{name}.txt"));
    std::fs::read_to_string(&file)
        .unwrap_or_else(|_| panic!("die Vergleichsdatei {} gehört ins Repo", file.display()))
}

/// Erbauer mit Standard-Workflow, ohne Marken und in UTC – wie in `make.mjs`
fn builder(
    f: &BoardFixture,
) -> ContextBuilder<nexus_core::board::in_memory::InMemoryBoardDatabase> {
    ContextBuilder::new(
        Arc::clone(&f.db),
        TicketService::new(Arc::clone(&f.db), f.clock.clone()),
        CommentService::new(Arc::clone(&f.db), f.clock.clone()),
        None,
        f.clock.clone(),
    )
    .in_zone(utc())
}

/// Der Mensch, dessen Kommentare eingespeist werden
fn human() -> BoardActor {
    BoardActor {
        user_id: nexus_core::testing::USER_ID.into(),
        name: "Florian".into(),
    }
}

#[tokio::test]
async fn ohne_ticket_steht_derselbe_text_wie_in_typescript() {
    let f = BoardFixture::create().await;
    let context = builder(&f)
        .build(ContextKind::SessionStart, &[], None)
        .await
        .unwrap();
    assert_eq!(context.text.unwrap(), erwartet("ohne-ticket"));
}

#[tokio::test]
async fn mit_ungelesenen_kommentaren_steht_derselbe_text_wie_in_typescript() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                spec: Some("001-x".into()),
                steps: Some(vec![
                    TicketStep {
                        text: "A".into(),
                        done: true,
                        detail: None,
                    },
                    TicketStep {
                        text: "B".into(),
                        done: false,
                        detail: None,
                    },
                ]),
                ..NewTicketInput::titled("Login")
            },
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    f.tickets
        .update(
            "ARC-0001",
            &TicketPatch {
                status: Some(TicketStatus::InProgress),
                ..TicketPatch::default()
            },
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    f.comments
        .add("ARC-0001", "Bitte prüfen", &human(), false)
        .await
        .unwrap();
    f.advance(5);
    f.comments
        .add("ARC-0001", "Zweite Zeile\nmit Umbruch", &human(), false)
        .await
        .unwrap();

    let context = builder(&f)
        .build(ContextKind::SessionStart, &["ARC-0001".to_string()], None)
        .await
        .unwrap();
    assert_eq!(context.text.unwrap(), erwartet("mit-kommentaren"));
    assert_eq!(context.comment_ids.len(), 2);
}

#[tokio::test]
async fn mit_offener_frage_steht_derselbe_text_wie_in_typescript() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("Login"),
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    f.questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                ticket_ids: Some(vec!["ARC-0001".into()]),
                ..NewBoardQuestionInput::titled("Welcher Speicher?")
            },
            &f.agent,
        )
        .await
        .unwrap();

    let context = builder(&f)
        .build(ContextKind::SessionStart, &["ARC-0001".to_string()], None)
        .await
        .unwrap();
    assert_eq!(context.text.unwrap(), erwartet("mit-frage"));
}
