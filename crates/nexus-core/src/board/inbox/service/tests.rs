//! Portiert aus `packages/core/tests/unit/board/InboxService.test.ts`.
//!
//! Die Fragen legt der Test unmittelbar über die Repositories an: `BoardQuestionService`
//! bringt erst P3-3, die Arbeitsliste liest nur die Zeilen.

use super::*;
use crate::board::database::NewBoard;
use crate::board::note_service::{AddNoteOptions, NewNoteText, TakeNoteOptions};
use crate::board::question_types::BoardQuestion;
use crate::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
use crate::testing::{question_fixture, BoardFixture, USER_ID};

/// Der Kollege, dem das fremde Board gehört
const OTHER_ID: &str = "22222222-2222-2222-2222-222222222222";

/// Umgebung mit eigenem Board „Sprint“ (ARC-0001 …) und einem fremden Board des Kollegen im
/// selben Projekt
async fn setup() -> (
    BoardFixture,
    i32,
    InboxService<crate::board::in_memory::InMemoryBoardDatabase>,
) {
    let f = BoardFixture::create().await;
    let project_id = f.project_id().await;
    let other = crate::board::types::BoardUser {
        id: OTHER_ID.into(),
        name: "Kollege".into(),
    };
    let foreign = crate::tx!(f.db, [other], |u| {
        u.users().upsert(&other).await?;
        u.boards()
            .insert(&NewBoard {
                project_id,
                owner_id: OTHER_ID.into(),
                name: "Fremd".into(),
                repo_path: None,
                created_at: "2026-10-13T10:00:00.000Z".into(),
            })
            .await
    })
    .await
    .expect("Board anlegen geht immer")
    .id;
    let inbox = InboxService::new(f.db.clone());
    (f, foreign, inbox)
}

/// Legt ein blockiertes Ticket mit diesem Grund an und liefert seine ID
async fn blocked(f: &BoardFixture, title: &str, reason: &str, board_id: i32) -> String {
    let ticket = f
        .tickets
        .create(
            board_id,
            &NewTicketInput::titled(title),
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen geht immer");
    f.tickets
        .update(
            &ticket.ticket.id,
            &TicketPatch {
                status: Some(TicketStatus::Blocked),
                blocked_reason: Some(Some(reason.to_string())),
                ..TicketPatch::default()
            },
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket ändern geht immer");
    ticket.ticket.id.clone()
}

/// Legt eine offene Frage mit Anlass, Ersteller und betroffenen Tickets an
async fn question(
    f: &BoardFixture,
    number: i32,
    title: &str,
    ticket_ids: &[&str],
    occasion: Option<&str>,
    created_by: &str,
    state: QuestionState,
) {
    let question = BoardQuestion {
        title: title.to_string(),
        occasion: occasion.map(ToString::to_string),
        ticket_ids: ticket_ids.iter().map(|id| id.to_string()).collect(),
        created_by_id: created_by.to_string(),
        state,
        ..question_fixture(&format!("ARC-F{number:04}"), f.project_id().await, number)
    };
    crate::tx!(f.db, [question], |u| {
        u.questions().insert(&question).await
    })
    .await
    .expect("Frage anlegen geht immer");
}

#[tokio::test]
async fn liefert_je_art_einen_eintrag_eigener_boards_nach_dringlichkeit_sortiert() {
    let (f, foreign, inbox) = setup().await;
    blocked(&f, "Fertig", "Human Interaction: ansehen", f.board_id).await;
    f.advance(1);
    blocked(&f, "Zugang", "Zugang fehlt", f.board_id).await;
    f.advance(1);
    blocked(&f, "Merge", "Merge: Konflikt", f.board_id).await;
    f.advance(1);
    blocked(
        &f,
        "Setup",
        "Einrichtung fehlgeschlagen: npm ci",
        f.board_id,
    )
    .await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                status: Some(TicketStatus::InPlanning),
                ..NewTicketInput::titled("Planen")
            },
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                status: Some(TicketStatus::InProgress),
                ..NewTicketInput::titled("Läuft")
            },
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    blocked(&f, "Fremd blockiert", "Merge: fremd", foreign).await;
    let note = f
        .notes
        .add(
            f.board_id,
            &NewNoteText {
                title: "Cache prüfen".into(),
                body: Some("Details dazu".into()),
            },
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
        .unwrap();
    let taken = f
        .notes
        .add(
            f.board_id,
            &NewNoteText::titled("Schon übernommen"),
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
        .unwrap();
    f.notes
        .take(taken.id, &f.actor, &TakeNoteOptions::default())
        .await
        .unwrap();

    let items = inbox.list(USER_ID, &InboxFilter::default()).await.unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| (item.kind.as_str(), item.title.as_str()))
            .collect::<Vec<_>>(),
        [
            ("merge", "Merge"),
            ("setup", "Setup"),
            ("accept", "Fertig"),
            ("blocked", "Zugang"),
            ("planning", "Planen"),
            ("note", "Cache prüfen"),
        ]
    );
    let erster = &items[0];
    assert_eq!(erster.key, "ticket:ARC-0003");
    assert_eq!(erster.ticket_id.as_deref(), Some("ARC-0003"));
    assert_eq!(erster.board_id, Some(f.board_id));
    assert_eq!(erster.board_name.as_deref(), Some("Sprint"));
    assert_eq!(erster.project_key, "ARC");
    assert_eq!(erster.detail.as_deref(), Some("Merge: Konflikt"));
    let letzter = &items[5];
    assert_eq!(letzter.key, format!("note:{}", note.id));
    assert_eq!(letzter.note_id, Some(note.id));
    assert_eq!(letzter.detail.as_deref(), Some("Details dazu"));
    assert_eq!(letzter.ticket_id, None);
}

#[tokio::test]
async fn zeigt_ein_ticket_mit_offener_frage_nur_uber_die_frage_und_zahlt_die_wartenden_tickets() {
    let (f, _foreign, inbox) = setup().await;
    blocked(&f, "Wartet A", "Zugang fehlt", f.board_id).await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                status: Some(TicketStatus::InPlanning),
                ..NewTicketInput::titled("Wartet B")
            },
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    let done = f
        .tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("Fertig"),
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    f.tickets
        .move_to(
            &done.ticket.id,
            TicketStatus::Done,
            &f.actor,
            &crate::board::ticket_service::MoveOptions {
                acceptance: Some("ok".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    question(
        &f,
        1,
        "Welcher Speicher?",
        &["ARC-0001", "ARC-0002", "ARC-0003"],
        Some("Planung"),
        USER_ID,
        QuestionState::Open,
    )
    .await;

    let items = inbox.list(USER_ID, &InboxFilter::default()).await.unwrap();
    assert_eq!(items.len(), 1);
    let frage = &items[0];
    assert_eq!(frage.key, "question:ARC-F0001");
    assert_eq!(frage.kind, InboxKind::Question);
    assert_eq!(frage.title, "Welcher Speicher?");
    assert_eq!(frage.detail.as_deref(), Some("Planung"));
    assert_eq!(frage.question_id.as_deref(), Some("ARC-F0001"));
    assert_eq!(frage.waiting_tickets, 2);
    assert_eq!(frage.board_id, Some(f.board_id));
}

#[tokio::test]
async fn ubergeht_fragen_die_den_benutzer_nicht_betreffen_und_entschiedene_fragen() {
    let (f, foreign, inbox) = setup().await;
    f.tickets
        .create(
            foreign,
            &NewTicketInput::titled("Fremd"),
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    // Die fremde Frage hängt an einem Ticket eines fremden Boards und kommt vom Kollegen
    question(
        &f,
        1,
        "Fremde Frage",
        &["ARC-0001"],
        None,
        OTHER_ID,
        QuestionState::Open,
    )
    .await;
    question(
        &f,
        2,
        "Selbst gestellt",
        &[],
        None,
        USER_ID,
        QuestionState::Open,
    )
    .await;
    question(
        &f,
        3,
        "Verworfen",
        &[],
        None,
        USER_ID,
        QuestionState::Discarded,
    )
    .await;

    let items = inbox.list(USER_ID, &InboxFilter::default()).await.unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| (item.key.as_str(), item.board_id))
            .collect::<Vec<_>>(),
        [("question:ARC-F0002", None)]
    );
}

#[tokio::test]
async fn filtert_nach_board_und_lehnt_fremde_boards_ab() {
    let (f, foreign, inbox) = setup().await;
    let second = f
        .boards
        .create("ARC", "Zweites", &f.actor)
        .await
        .unwrap()
        .id;
    blocked(&f, "Eins", "Zugang fehlt", f.board_id).await;
    blocked(&f, "Zwei", "Zugang fehlt", second).await;
    f.notes
        .add(
            second,
            &NewNoteText::titled("Notiz zwei"),
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
        .unwrap();

    let alle = inbox.list(USER_ID, &InboxFilter::default()).await.unwrap();
    assert_eq!(
        alle.iter()
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>(),
        ["Eins", "Zwei", "Notiz zwei"]
    );
    let nur_zweites = inbox
        .list(
            USER_ID,
            &InboxFilter {
                board_id: Some(second),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        nur_zweites
            .iter()
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>(),
        ["Zwei", "Notiz zwei"]
    );
    assert_eq!(
        inbox
            .list(
                USER_ID,
                &InboxFilter {
                    board_id: Some(foreign),
                }
            )
            .await
            .unwrap_err()
            .code(),
        Some("board-not-owned")
    );
}

#[tokio::test]
async fn liefert_ohne_eigene_boards_eine_leere_liste_und_zahlt_je_art() {
    let (f, _foreign, inbox) = setup().await;
    assert!(inbox
        .list(OTHER_ID, &InboxFilter::default())
        .await
        .unwrap()
        .is_empty());
    blocked(&f, "Eins", "Zugang fehlt", f.board_id).await;
    let list = InboxService::<crate::board::in_memory::InMemoryBoardDatabase>::summarize(
        inbox.list(USER_ID, &InboxFilter::default()).await.unwrap(),
    );
    assert_eq!(list.total, 1);
    assert_eq!(
        list.counts,
        InboxCounts {
            merge: 0,
            setup: 0,
            question: 0,
            accept: 0,
            blocked: 1,
            planning: 0,
            note: 0,
        }
    );
}
