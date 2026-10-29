//! Portiert aus `packages/core/tests/unit/board/TicketTimelineService.test.ts`.
//!
//! Die Fragen und ihre Verlaufseinträge legt der Test unmittelbar über die Repositories an:
//! `BoardQuestionService` bringt erst P3-3, die Zeitleiste liest nur die Zeilen.

use super::*;
use crate::actors::AuthorKind;
use crate::board::basics::BoardActor;
use crate::board::planning_service::PlanInput;
use crate::board::question_types::{BoardQuestion, BoardQuestionHistoryEntry};
use crate::board::ticket_service::{ClaimOptions, NewTicketInput, WriteOptions};
use crate::board::ticket_step_edits::StepGuard;
use crate::board::ticket_timeline::TimelineActorType;
use crate::board::types::HistoryEntry;
use crate::board::types::TicketStep;
use crate::testing::{BoardFixture, USER_ID};

/// Die Namen der Dispatch-Konfiguration aus TypeScript
fn names() -> TimelineAgentNames {
    TimelineAgentNames {
        dispatcher_name: "dispatcher".into(),
        agent_prefix: "junior-".into(),
    }
}

/// Der Mensch, der das Ticket anlegt und die Frage entscheidet
fn human() -> BoardActor {
    BoardActor {
        user_id: USER_ID.into(),
        name: "Florian".into(),
    }
}

/// Schreibt einen Eintrag in den Verlauf des Tickets (das macht sonst der Fragen-Dienst)
async fn add_history(f: &BoardFixture, field: &str, to: Option<&str>, note: &str, by_name: &str) {
    let entry = HistoryEntry {
        ticket_id: "ARC-0001".into(),
        at: f.clock.iso(),
        by_id: USER_ID.into(),
        by_name: by_name.to_string(),
        field: field.to_string(),
        from: None,
        to: to.map(ToString::to_string),
        note: Some(note.to_string()),
    };
    crate::tx!(f.db, [entry], |u| { u.history().insert(&[entry]).await })
        .await
        .expect("Verlauf schreiben geht immer");
}

/// Schreibt einen Eintrag in den Verlauf einer Frage
async fn add_question_history(
    f: &BoardFixture,
    question_id: &str,
    entry_type: BoardQuestionHistoryType,
    text: &str,
    by_name: &str,
    by_kind: AuthorKind,
) {
    let entry = BoardQuestionHistoryEntry {
        question_id: question_id.to_string(),
        at: f.clock.iso(),
        by_id: USER_ID.into(),
        by_name: by_name.to_string(),
        by_kind: Some(by_kind),
        entry_type,
        text: text.to_string(),
    };
    crate::tx!(f.db, [entry], |u| {
        u.question_history().insert(&[entry]).await
    })
    .await
    .expect("Verlauf der Frage schreiben geht immer");
}

/// Setzt die betroffenen Tickets einer Frage neu (das `setTickets` des Fragen-Dienstes)
async fn set_tickets(f: &BoardFixture, question_id: &str, ticket_ids: &[&str]) {
    let question_id = question_id.to_string();
    let ticket_ids: Vec<String> = ticket_ids.iter().map(|id| id.to_string()).collect();
    crate::tx!(f.db, [question_id, ticket_ids], |u| {
        let question = u
            .questions()
            .by_id(&question_id, true)
            .await?
            .expect("die Frage wurde angelegt");
        u.questions()
            .update(&BoardQuestion {
                ticket_ids: ticket_ids.clone(),
                rev: question.rev + 1,
                ..question
            })
            .await
    })
    .await
    .expect("Frage ändern geht immer");
}

#[tokio::test]
async fn liefert_alle_ereignisse_eines_tickets_in_zeitfolge_mit_person_oder_agent() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                steps: Some(vec![TicketStep {
                    text: "Test".into(),
                    done: false,
                    detail: None,
                }]),
                ..NewTicketInput::titled("A")
            },
            &human(),
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    f.advance(1);
    f.planning
        .plan(
            "ARC-0001",
            &PlanInput {
                plan: Some("Plan".into()),
                ..PlanInput::default()
            },
            &f.actor,
        )
        .await
        .unwrap();
    f.advance(1);
    f.tickets
        .claim(
            "ARC-0001",
            "junior-ARC-0001",
            &f.actor,
            &ClaimOptions::default(),
        )
        .await
        .unwrap();
    f.advance(1);
    f.tickets
        .set_step("ARC-0001", 1, true, &f.actor, &StepGuard::default())
        .await
        .unwrap();
    f.advance(1);
    f.comments
        .add("ARC-0001", "Zwischenstand", &human(), false)
        .await
        .unwrap();
    f.advance(1);
    f.open_question(1, "Welcher Speicher?", &["ARC-0001"]).await;
    add_history(
        &f,
        "question",
        Some("open"),
        "Frage gestellt: ARC-F0001 „Welcher Speicher?“",
        "claude",
    )
    .await;
    add_question_history(
        &f,
        "ARC-F0001",
        BoardQuestionHistoryType::Created,
        "Welcher Speicher?",
        "claude",
        AuthorKind::Session,
    )
    .await;
    f.advance(1);
    f.answer_question("ARC-F0001").await;
    add_history(
        &f,
        "question",
        Some("answered"),
        "Entschieden: ARC-F0001 „Welcher Speicher?“",
        "Florian",
    )
    .await;
    add_question_history(
        &f,
        "ARC-F0001",
        BoardQuestionHistoryType::Decided,
        "SQL",
        "Florian",
        AuthorKind::Human,
    )
    .await;

    let timeline = TicketTimelineService::new(f.db.clone(), Some(names()))
        .get("arc-0001")
        .await
        .unwrap();
    assert_eq!(timeline.ticket_id, "ARC-0001");
    let events = &timeline.events;
    let mut zeiten: Vec<&str> = events.iter().map(|e| e.at.as_str()).collect();
    let gegeben = zeiten.clone();
    zeiten.sort_unstable();
    assert_eq!(gegeben, zeiten);

    assert_eq!(events[0].event_type, "field:status");
    assert_eq!(events[0].kind, TimelineKind::Status);
    assert_eq!(events[0].note, Some(Some("angelegt".into())));
    assert_eq!(events[0].actor.name, "Florian");
    assert_eq!(events[0].actor.actor_type, TimelineActorType::Person);

    let suche = |event_type: &str| {
        events
            .iter()
            .find(|event| event.event_type == event_type)
            .unwrap_or_else(|| panic!("{event_type} fehlt in der Zeitleiste"))
    };
    let plan = suche("field:plan");
    assert_eq!(plan.kind, TimelineKind::Content);
    assert_eq!(plan.actor.name, "claude");
    assert_eq!(plan.actor.actor_type, TimelineActorType::Agent);
    let claim = suche("field:assignee");
    assert_eq!(claim.kind, TimelineKind::Claim);
    assert_eq!(claim.to, Some(Some("junior-ARC-0001".into())));
    assert_eq!(suche("field:step").kind, TimelineKind::Content);
    let comment = suche("comment");
    assert_eq!(comment.kind, TimelineKind::Comment);
    assert_eq!(comment.text.as_deref(), Some("Zwischenstand"));

    let created = suche("question:created");
    assert_eq!(created.kind, TimelineKind::Question);
    assert_eq!(created.actor.actor_type, TimelineActorType::Agent);
    assert_eq!(
        created
            .reference
            .as_ref()
            .and_then(|r| r.question_id.as_deref()),
        Some("ARC-F0001")
    );
    assert!(created
        .text
        .as_deref()
        .is_some_and(|text| text.contains("Welcher Speicher?")));
    let decided = suche("question:decided");
    assert_eq!(decided.kind, TimelineKind::Question);
    assert_eq!(decided.actor.name, "Florian");
    assert_eq!(decided.actor.actor_type, TimelineActorType::Person);
    assert_eq!(
        decided
            .reference
            .as_ref()
            .and_then(|r| r.question_id.as_deref()),
        Some("ARC-F0001")
    );

    // Der Zustandswechsel der Frage steht schon über die Frage in der Zeitleiste, nicht
    // doppelt aus dem Ticketverlauf
    assert!(!events
        .iter()
        .any(|event| event.event_type == "field:question"));
}

#[tokio::test]
async fn behalt_frage_eintrage_des_ticketverlaufs_wenn_die_frage_nicht_mehr_verknupft_ist() {
    let f = BoardFixture::create().await;
    for titel in ["A", "B"] {
        f.tickets
            .create(
                f.board_id,
                &NewTicketInput::titled(titel),
                &f.actor,
                &WriteOptions::default(),
            )
            .await
            .unwrap();
    }
    f.open_question(1, "Q", &["ARC-0001"]).await;
    add_history(
        &f,
        "question",
        Some("open"),
        "Frage gestellt: ARC-F0001 „Q“",
        "claude",
    )
    .await;
    add_question_history(
        &f,
        "ARC-F0001",
        BoardQuestionHistoryType::Created,
        "Q",
        "claude",
        AuthorKind::Session,
    )
    .await;
    f.answer_question("ARC-F0001").await;
    set_tickets(&f, "ARC-F0001", &["ARC-0002"]).await;

    let timeline = TicketTimelineService::new(f.db.clone(), None)
        .get("ARC-0001")
        .await
        .unwrap();
    let frage = timeline
        .events
        .iter()
        .find(|event| event.event_type == "field:question")
        .expect("der Eintrag des Ticketverlaufs bleibt");
    assert_eq!(frage.kind, TimelineKind::Question);
    assert_eq!(
        frage
            .reference
            .as_ref()
            .and_then(|r| r.question_id.as_deref()),
        Some("ARC-F0001")
    );
    assert!(!timeline
        .events
        .iter()
        .any(|event| event.event_type == "question:created"));
}

#[tokio::test]
async fn meldet_ein_unbekanntes_ticket() {
    let f = BoardFixture::create().await;
    let fehler = TicketTimelineService::new(f.db.clone(), None)
        .get("ARC-0099")
        .await
        .unwrap_err();
    assert_eq!(fehler.code(), Some("ticket-not-found"));
}
