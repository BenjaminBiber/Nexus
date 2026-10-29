//! Entscheiden, verwerfen und wieder öffnen;
//! portiert aus `packages/core/tests/unit/board/BoardQuestionDecision.test.ts`.

use super::{any_rev, ids, last_history};
use crate::board::question_service::{
    BoardQuestionListFilter, BoardQuestionWriteOptions, NewBoardQuestionInput,
};
use crate::board::question_types::{
    BoardQuestionActor, BoardQuestionDecisionOutcome, BoardQuestionHistoryType,
};
use crate::board::ticket_service::{NewTicketInput, WriteOptions};
use crate::questions::{DecisionInput, QuestionState, ResearchInput, ResearchOptionInput};
use crate::testing::BoardFixture;

/// Der Fehlercode, wenn die Entscheidung nicht erlaubt ist
async fn answer_err(
    f: &BoardFixture,
    input: DecisionInput,
    actor: &BoardQuestionActor,
    options: BoardQuestionWriteOptions,
) -> Option<String> {
    f.questions
        .answer("ARC-F0001", &input, actor, &options)
        .await
        .expect_err("die Entscheidung ist nicht erlaubt")
        .code()
        .map(str::to_string)
}

/// Ticket ARC-0001 mit Frage ARC-F0001 und zwei Recherchen (O1, O2 und O3); Version der Frage
/// danach: 3
async fn researched() -> BoardFixture {
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
    f.questions
        .add_research(
            "ARC-F0001",
            &ResearchInput::options(vec![
                ResearchOptionInput::new("Datei", "JSON"),
                ResearchOptionInput {
                    recommended: Some(true),
                    ..ResearchOptionInput::new("DB", "SQL")
                },
            ]),
            &f.agent,
        )
        .await
        .unwrap();
    f.questions
        .add_research(
            "ARC-F0001",
            &ResearchInput::options(vec![ResearchOptionInput::new("Cache", "RAM")]),
            &f.agent,
        )
        .await
        .unwrap();
    f
}

#[tokio::test]
async fn beantwortet_mit_einer_option_der_neuesten_recherche_und_schreibt_verlauf_an_frage_und_ticket(
) {
    let f = researched().await;

    let detail = f
        .questions
        .answer(
            "ARC-F0001",
            &DecisionInput::option("O3", "schnell genug"),
            &f.human,
            &BoardQuestionWriteOptions::rev(3),
        )
        .await
        .unwrap();

    assert_eq!(detail.view.question.state, QuestionState::Answered);
    assert_eq!(detail.view.question.rev, 4);
    assert!(!detail.view.ready);
    let decision = detail
        .view
        .question
        .decision
        .as_ref()
        .expect("die Frage ist entschieden");
    assert_eq!(decision.outcome, BoardQuestionDecisionOutcome::Answered);
    assert_eq!(decision.option.as_deref(), Some("O3"));
    assert_eq!(decision.text, None);
    assert_eq!(decision.reason, "schnell genug");
    assert_eq!(decision.by_name, "Florian");
    assert_eq!(decision.by_id, f.human.actor.user_id);
    assert_eq!(decision.at, "2026-10-13T10:00:00.000Z");
    assert_eq!(
        last_history(&detail),
        (
            BoardQuestionHistoryType::Decided,
            "Option O3 „Cache“ gewählt. Begründung: schnell genug"
        )
    );

    let ticket = f.tickets.get("ARC-0001").await.unwrap();
    let entry = ticket.history.last().expect("das Ticket hat Verlauf");
    assert_eq!(entry.field, "question");
    assert_eq!(entry.from.as_deref(), Some("open"));
    assert_eq!(entry.to.as_deref(), Some("answered"));
    assert_eq!(
        entry.note.as_deref(),
        Some("Frage entschieden: ARC-F0001 „Welcher Speicher?“")
    );
    assert_eq!(entry.by_name, "Florian");
}

#[tokio::test]
async fn entscheidet_ohne_begrundung_der_verlauf_nennt_dann_keine() {
    let f = researched().await;

    let answered = f
        .questions
        .answer(
            "ARC-F0001",
            &DecisionInput::option("O3", "  "),
            &f.human,
            &any_rev(),
        )
        .await
        .unwrap();

    let decision = answered
        .view
        .question
        .decision
        .as_ref()
        .expect("die Frage ist entschieden");
    assert_eq!(decision.option.as_deref(), Some("O3"));
    assert_eq!(decision.reason, "");
    assert_eq!(last_history(&answered).1, "Option O3 „Cache“ gewählt.");
}

#[tokio::test]
async fn verlangt_mensch_offene_frage_eindeutige_wahl_und_eine_option_der_neuesten_recherche() {
    let f = researched().await;

    assert_eq!(
        answer_err(&f, DecisionInput::option("O3", "x"), &f.agent, any_rev())
            .await
            .as_deref(),
        Some("human-only")
    );
    // O1 steht nicht in der neuesten Recherche
    assert_eq!(
        answer_err(&f, DecisionInput::option("O1", "x"), &f.human, any_rev())
            .await
            .as_deref(),
        Some("decision-option-unknown")
    );
    assert_eq!(
        answer_err(
            &f,
            DecisionInput {
                option: Some("O3".into()),
                text: Some("beides".into()),
                reason: Some("x".into()),
            },
            &f.human,
            any_rev()
        )
        .await
        .as_deref(),
        Some("decision-ambiguous")
    );
    assert_eq!(
        answer_err(
            &f,
            DecisionInput::text("frei", "x"),
            &f.human,
            BoardQuestionWriteOptions::rev(1)
        )
        .await
        .as_deref(),
        Some("conflict")
    );

    let answered = f
        .questions
        .answer(
            "ARC-F0001",
            &DecisionInput::text("Erst CSV", "reicht"),
            &f.human,
            &any_rev(),
        )
        .await
        .unwrap();
    assert_eq!(
        last_history(&answered).1,
        "Antwort: Erst CSV. Begründung: reicht"
    );
    assert_eq!(
        answer_err(&f, DecisionInput::text("nochmal", "x"), &f.human, any_rev())
            .await
            .as_deref(),
        Some("question-state-invalid")
    );
    assert_eq!(
        ids(&f
            .questions
            .list(
                "ARC",
                &BoardQuestionListFilter {
                    state: Some(QuestionState::Answered),
                    ..BoardQuestionListFilter::default()
                },
                None
            )
            .await
            .unwrap()
            .questions),
        ["ARC-F0001"]
    );
}

#[tokio::test]
async fn verwirft_mit_pflichtbegrundung_und_offnet_wieder_mit_der_fruheren_entscheidung_im_verlauf()
{
    let f = researched().await;

    assert_eq!(
        f.questions
            .discard("ARC-F0001", Some(" "), &f.human, &any_rev())
            .await
            .unwrap_err()
            .code(),
        Some("decision-reason-missing")
    );

    let discarded = f
        .questions
        .discard("ARC-F0001", Some("doppelt"), &f.human, &any_rev())
        .await
        .unwrap();
    assert_eq!(discarded.view.question.state, QuestionState::Discarded);
    let decision = discarded
        .view
        .question
        .decision
        .as_ref()
        .expect("die Frage ist verworfen");
    assert_eq!(decision.outcome, BoardQuestionDecisionOutcome::Discarded);
    assert_eq!(decision.reason, "doppelt");
    assert_eq!(decision.option, None);
    assert_eq!(decision.text, None);
    assert_eq!(last_history(&discarded).1, "Verworfen. Begründung: doppelt");

    let counts = f
        .questions
        .list("ARC", &BoardQuestionListFilter::default(), None)
        .await
        .unwrap()
        .counts;
    assert_eq!(
        (counts.open, counts.answered, counts.discarded, counts.mine),
        (0, 0, 1, 0)
    );

    assert_eq!(
        f.questions
            .reopen("ARC-F0001", None, &f.agent, &any_rev())
            .await
            .unwrap_err()
            .code(),
        Some("human-only")
    );
    let reopened = f
        .questions
        .reopen("ARC-F0001", Some("neue Lage"), &f.human, &any_rev())
        .await
        .unwrap();
    assert_eq!(reopened.view.question.state, QuestionState::Open);
    assert!(reopened.view.question.decision.is_none());
    assert!(reopened.view.ready);
    assert_eq!(
        last_history(&reopened).1,
        "Wieder geöffnet. Grund: neue Lage. Frühere Entscheidung: Verworfen. Begründung: doppelt (Florian)"
    );
    assert_eq!(
        f.questions
            .reopen("ARC-F0001", None, &f.human, &any_rev())
            .await
            .unwrap_err()
            .code(),
        Some("question-state-invalid")
    );

    let history = f.tickets.get("ARC-0001").await.unwrap().history;
    assert_eq!(
        history[history.len() - 2..]
            .iter()
            .map(|entry| (
                entry.from.as_deref(),
                entry.to.as_deref(),
                entry.note.as_deref()
            ))
            .collect::<Vec<_>>(),
        [
            (
                Some("open"),
                Some("discarded"),
                Some("Frage verworfen: ARC-F0001 „Welcher Speicher?“")
            ),
            (
                Some("discarded"),
                Some("open"),
                Some("Frage wieder geöffnet: ARC-F0001 „Welcher Speicher?“")
            ),
        ]
    );
}

#[tokio::test]
async fn kurzt_den_verlaufseintrag_am_ticket_auf_200_zeichen() {
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
                ..NewBoardQuestionInput::titled(&"L".repeat(300))
            },
            &f.agent,
        )
        .await
        .unwrap();
    f.questions
        .discard("ARC-F0001", Some("zu lang"), &f.human, &any_rev())
        .await
        .unwrap();

    let note = f
        .tickets
        .get("ARC-0001")
        .await
        .unwrap()
        .history
        .last()
        .expect("das Ticket hat Verlauf")
        .note
        .clone()
        .expect("die Notiz steht am Eintrag");
    assert_eq!(crate::text::utf16_len(&note), 200);
    assert!(note.ends_with('…'));
}
