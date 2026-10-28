//! Offene Fragen sperren ihre Tickets.
//! Portiert aus `packages/core/tests/unit/board/QuestionTicketBlocking.test.ts`.
//!
//! Die Fragen entstehen hier unmittelbar über die Repositories ([`BoardFixture::open_question`]),
//! weil `BoardQuestionService` erst P3-3 bringt. Die beiden Fälle jener Datei, die am Dienst
//! hängen (`next-planning` übergeht Tickets mit offener Frage und `questions.setTickets`),
//! kommen mit P3-3 dazu.

use super::{add, create};
use crate::board::basics::TicketStatus;
use crate::board::database::BoardDatabase;
use crate::board::question_types::BoardQuestionHistoryType;
use crate::board::ticket_ordering::BoardColumn;
use crate::board::ticket_service::{
    ClaimOptions, MoveOptions, NewTicketInput, TicketListFilter, TicketPatch, WriteOptions,
};
use crate::testing::BoardFixture;
use crate::tx;

/// ARC-0001 „Login“ (mit Abnahme) und ARC-0002 „Export“; offene Frage ARC-F0001 betrifft ARC-0001
async fn setup() -> BoardFixture {
    let f = BoardFixture::create().await;
    create(
        &f,
        NewTicketInput {
            acceptance: Some("geht".into()),
            ..NewTicketInput::titled("Login")
        },
    )
    .await
    .unwrap();
    add(&f, "Export").await;
    f.open_question(1, "Welcher Speicher?", &["ARC-0001"]).await;
    f
}

#[tokio::test]
async fn zeigt_offene_fragen_an_ticket_und_board_ansicht_das_ticket_bleibt_in_seiner_spalte() {
    let f = setup().await;

    let login = f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(login.view.column, BoardColumn::Backlog);
    assert!(!login.view.waiting);
    assert_eq!(
        login
            .view
            .waiting_on_questions
            .iter()
            .map(|question| (question.id.as_str(), question.title.as_str()))
            .collect::<Vec<_>>(),
        [("ARC-F0001", "Welcher Speicher?")]
    );
    assert!(f
        .tickets
        .get("ARC-0002")
        .await
        .unwrap()
        .view
        .waiting_on_questions
        .is_empty());

    let view = f.views.get(f.board_id).await.unwrap();
    assert_eq!(
        view.columns[0]
            .tickets
            .iter()
            .map(|card| (
                card.id.as_str(),
                card.waiting_on_questions
                    .iter()
                    .map(|question| question.id.as_str())
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        [("ARC-0001", vec!["ARC-F0001"]), ("ARC-0002", vec![])]
    );
    assert_eq!(
        f.tickets
            .list(&TicketListFilter {
                board_ids: vec![f.board_id],
                ..TicketListFilter::default()
            })
            .await
            .unwrap()
            .iter()
            .find(|view| view.ticket.id == "ARC-0001")
            .unwrap()
            .waiting_on_questions
            .len(),
        1
    );
}

#[tokio::test]
async fn sperrt_in_arbeit_erledigt_und_ubernehmen_ausser_mit_force() {
    let f = setup().await;

    let error = f
        .tickets
        .move_to(
            "ARC-0001",
            TicketStatus::InProgress,
            &f.actor,
            &MoveOptions::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("question-open"));
    assert!(error.to_string().contains("ARC-F0001"));
    // Die Meldung bleibt neutral; den --force-Hinweis ergänzt erst die CLI
    assert!(!error.to_string().contains("--force"));

    assert_eq!(
        f.tickets
            .move_to(
                "ARC-0001",
                TicketStatus::Done,
                &f.actor,
                &MoveOptions::default()
            )
            .await
            .unwrap_err()
            .code(),
        Some("question-open")
    );
    assert_eq!(
        f.tickets
            .claim("ARC-0001", "claude", &f.actor, &ClaimOptions::default())
            .await
            .unwrap_err()
            .code(),
        Some("question-open")
    );
    assert_eq!(
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
            .unwrap_err()
            .code(),
        Some("question-open")
    );

    assert_eq!(
        f.tickets
            .move_to(
                "ARC-0001",
                TicketStatus::Planned,
                &f.actor,
                &MoveOptions::default()
            )
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::Planned
    );
    assert_eq!(
        f.tickets
            .claim(
                "ARC-0001",
                "claude",
                &f.actor,
                &ClaimOptions {
                    force: true,
                    ..ClaimOptions::default()
                }
            )
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::InProgress
    );
    f.tickets
        .release("ARC-0001", &f.actor, Some(TicketStatus::Planned))
        .await
        .unwrap();

    f.answer_question("ARC-F0001").await;

    assert!(f
        .tickets
        .get("ARC-0001")
        .await
        .unwrap()
        .view
        .waiting_on_questions
        .is_empty());
    assert_eq!(
        f.tickets
            .claim("ARC-0001", "claude", &f.actor, &ClaimOptions::default())
            .await
            .unwrap()
            .ticket
            .status,
        TicketStatus::InProgress
    );
}

#[tokio::test]
async fn entfernt_beim_loschen_eines_tickets_dessen_bezuge_an_fragen_und_schreibt_den_verlauf() {
    let f = setup().await;

    f.tickets.delete("ARC-0001", &f.actor).await.unwrap();

    let (question, history) = tx!(f.db, |u| {
        let question = u.questions().by_id("ARC-F0001", false).await?.unwrap();
        let history = u.question_history().list("ARC-F0001").await?;
        Ok((question, history))
    })
    .await
    .unwrap();

    assert!(question.ticket_ids.is_empty());
    assert_eq!(question.rev, 2);
    let last = history.last().unwrap();
    assert_eq!(last.entry_type, BoardQuestionHistoryType::TicketDeleted);
    assert_eq!(
        last.text,
        "Ticket ARC-0001 „Login“ gelöscht, Bezug entfernt."
    );
    assert_eq!(last.by_kind, None);
    assert_eq!(last.by_name, "claude");
}

#[tokio::test]
async fn entfernt_bezuge_auch_beim_loschen_eines_ganzen_boards() {
    let f = setup().await;

    f.boards.delete(f.board_id, true, &f.actor).await.unwrap();

    let question = tx!(f.db, |u| { u.questions().by_id("ARC-F0001", false).await })
        .await
        .unwrap()
        .unwrap();
    assert!(question.ticket_ids.is_empty());
}

#[tokio::test]
async fn raumt_beim_loschen_eines_boards_ohne_offene_tickets_die_bezuge_erledigter_tickets_auf() {
    let f = BoardFixture::create().await;
    create(
        &f,
        NewTicketInput {
            status: Some(TicketStatus::Done),
            acceptance: Some("geht".into()),
            ..NewTicketInput::titled("Fertig")
        },
    )
    .await
    .unwrap();
    f.open_question(1, "Betrifft", &["ARC-0001"]).await;

    f.boards.delete(f.board_id, false, &f.actor).await.unwrap();

    let (question, history) = tx!(f.db, |u| {
        let question = u.questions().by_id("ARC-F0001", false).await?.unwrap();
        let history = u.question_history().list("ARC-F0001").await?;
        Ok((question, history))
    })
    .await
    .unwrap();

    assert!(question.ticket_ids.is_empty());
    assert_eq!(question.rev, 2);
    assert_eq!(
        history.last().unwrap().text,
        "Ticket ARC-0001 „Fertig“ gelöscht, Bezug entfernt."
    );
}
