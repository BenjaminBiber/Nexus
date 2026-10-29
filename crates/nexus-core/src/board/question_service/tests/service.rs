//! Anlegen, lesen, auflisten, ändern, verknüpfen, kommentieren und recherchieren;
//! portiert aus `packages/core/tests/unit/board/BoardQuestionService.test.ts`.

use std::sync::Arc;

use super::{any_rev, ask, ask_titled, ids, last_history};
use crate::actors::AuthorKind;
use crate::board::in_memory::InMemoryBoardDatabase;
use crate::board::question_service::{
    BoardQuestionChanges, BoardQuestionListFilter, BoardQuestionWriteOptions, NewBoardQuestionInput,
};
use crate::board::question_types::{BoardQuestionDecisionOutcome, BoardQuestionHistoryType};
use crate::board::services::BoardServices;
use crate::board::ticket_service::NewTicketInput;
use crate::board::ticket_service::WriteOptions;
use crate::clock::Clock;
use crate::questions::{DecisionInput, QuestionState, ResearchInput, ResearchOptionInput};
use crate::testing::BoardFixture;

/// Der Fehlercode, wenn sich die Frage nicht anlegen lässt
async fn create_err(f: &BoardFixture, input: NewBoardQuestionInput) -> Option<String> {
    ask(f, input)
        .await
        .expect_err("die Frage ist fehlerhaft")
        .code()
        .map(str::to_string)
}

/// Die gefilterte Liste des Projekts ARC
async fn listed(
    f: &BoardFixture,
    filter: BoardQuestionListFilter,
) -> crate::board::question_service::BoardQuestionList {
    f.questions.list("ARC", &filter, None).await.unwrap()
}

/// Der Fehlercode, wenn sich die Abhängigkeiten nicht setzen lassen
async fn depends_err(f: &BoardFixture, refs: Vec<String>) -> Option<String> {
    f.questions
        .set_depends("ARC-F0001", &refs, &f.agent, &any_rev())
        .await
        .expect_err("die Abhängigkeit ist fehlerhaft")
        .code()
        .map(str::to_string)
}

/// Der Fehlercode, wenn sich die Recherche nicht ergänzen lässt
async fn research_err(f: &BoardFixture, input: ResearchInput) -> Option<String> {
    f.questions
        .add_research("ARC-F0001", &input, &f.agent)
        .await
        .expect_err("die Recherche ist fehlerhaft")
        .code()
        .map(str::to_string)
}

/// Umgebung mit zwei Tickets (ARC-0001 „Login“, ARC-0002 „Export“) auf dem Board
async fn setup() -> BoardFixture {
    let f = BoardFixture::create().await;
    for title in ["Login", "Export"] {
        f.tickets
            .create(
                f.board_id,
                &NewTicketInput::titled(title),
                &f.actor,
                &WriteOptions::default(),
            )
            .await
            .expect("das Ticket lässt sich anlegen");
    }
    f
}

#[tokio::test]
async fn legt_eine_frage_mit_nummer_je_projekt_herkunft_tags_tickets_und_verlauf_an() {
    let f = setup().await;

    let detail = ask(
        &f,
        NewBoardQuestionInput {
            title: " Welcher Speicher? ".into(),
            description: Some("# Kontext".into()),
            tags: Some(vec!["speicher".into(), "Speicher".into(), "ui".into()]),
            occasion: Some("Grill".into()),
            ticket_ids: Some(vec!["arc-2".into(), "ARC-0001".into(), "ARC-0002".into()]),
            depends_on: None,
        },
    )
    .await
    .unwrap();

    let question = &detail.view.question;
    assert_eq!(question.id, "ARC-F0001");
    assert_eq!(question.number, 1);
    assert_eq!(detail.view.project_key, "ARC");
    assert_eq!(question.title, "Welcher Speicher?");
    assert_eq!(question.description, "# Kontext");
    assert_eq!(question.state, QuestionState::Open);
    assert_eq!(question.tags, ["speicher", "ui"]);
    assert_eq!(question.occasion.as_deref(), Some("Grill"));
    assert_eq!(question.origin_kind, AuthorKind::Session);
    assert_eq!(question.origin_name, "claude");
    assert_eq!(question.origin_session.as_deref(), Some("s-1"));
    assert_eq!(question.ticket_ids, ["ARC-0001", "ARC-0002"]);
    assert!(question.depends_on.is_empty());
    assert!(question.decision.is_none());
    assert_eq!(question.rev, 1);
    assert_eq!(question.created_at, "2026-10-13T10:00:00.000Z");
    assert!(detail.view.ready);
    assert!(!detail.view.researched);
    assert_eq!(detail.view.option_count, 0);
    assert!(detail.view.waiting_on.is_empty());
    assert!(detail.research.is_empty());
    assert!(detail.comments.is_empty());
    assert_eq!(
        detail
            .view
            .tickets
            .iter()
            .map(|ticket| (
                ticket.id.as_str(),
                ticket.title.as_str(),
                ticket.status.as_str(),
                ticket.board_id
            ))
            .collect::<Vec<_>>(),
        [
            ("ARC-0001", "Login", "backlog", f.board_id),
            ("ARC-0002", "Export", "backlog", f.board_id)
        ]
    );
    assert_eq!(detail.history.len(), 1);
    assert_eq!(
        detail.history[0].entry_type,
        BoardQuestionHistoryType::Created
    );
    assert_eq!(detail.history[0].text, "Frage angelegt");
    assert_eq!(detail.history[0].by_name, "claude");
    assert_eq!(detail.history[0].by_kind, Some(AuthorKind::Session));

    // Die Nummern zählen je Projekt weiter, auch für einen anderen Handelnden
    let second = f
        .questions
        .create("ARC", &NewBoardQuestionInput::titled("Zweite"), &f.human)
        .await
        .unwrap();
    assert_eq!(second.view.question.id, "ARC-F0002");
    // Die Schreibweise der Nummer ist beim Lesen egal
    assert_eq!(
        f.questions.get("arc-f1").await.unwrap().view.question.title,
        "Welcher Speicher?"
    );
}

#[tokio::test]
async fn pruft_titel_projekt_tickets_und_nummer_ohne_nummern_zu_verbrauchen() {
    let f = setup().await;
    f.projects.create("OTH", "Other", &f.actor).await.unwrap();
    let other = f.boards.create("OTH", "Fremd", &f.actor).await.unwrap();
    f.tickets
        .create(
            other.board.id,
            &NewTicketInput::titled("Fremd"),
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();

    assert_eq!(
        create_err(&f, NewBoardQuestionInput::titled(" "))
            .await
            .as_deref(),
        Some("title-empty")
    );
    assert_eq!(
        f.questions
            .create("XXX", &NewBoardQuestionInput::titled("A"), &f.agent)
            .await
            .unwrap_err()
            .code(),
        Some("project-not-found")
    );
    assert_eq!(
        create_err(
            &f,
            NewBoardQuestionInput {
                ticket_ids: Some(vec!["ARC-0099".into()]),
                ..NewBoardQuestionInput::titled("A")
            }
        )
        .await
        .as_deref(),
        Some("question-ticket-unknown")
    );
    assert_eq!(
        create_err(
            &f,
            NewBoardQuestionInput {
                ticket_ids: Some(vec!["OTH-0001".into()]),
                ..NewBoardQuestionInput::titled("A")
            }
        )
        .await
        .as_deref(),
        Some("question-ticket-other-project")
    );
    assert_eq!(
        create_err(&f, NewBoardQuestionInput::titled(&"x".repeat(401)))
            .await
            .as_deref(),
        Some("value-too-long")
    );
    assert_eq!(
        f.questions.get("ARC-F0009").await.unwrap_err().code(),
        Some("question-not-found")
    );
    assert_eq!(
        f.questions.get("ARC-0001").await.unwrap_err().code(),
        Some("question-id-invalid")
    );
    // Keiner der Fehler hat eine Nummer verbraucht
    assert_eq!(
        ask_titled(&f, "Nach Fehlern").await.view.question.id,
        "ARC-F0001"
    );
}

#[tokio::test]
async fn listet_fragen_eines_projekts_mit_zahlern_und_filtern_bereite_zuerst() {
    let f = setup().await;
    ask(
        &f,
        NewBoardQuestionInput {
            depends_on: Some(vec!["ARC-0001".into()]),
            ..NewBoardQuestionInput::titled("Alt, wartet")
        },
    )
    .await
    .unwrap();
    f.advance(1);
    ask(
        &f,
        NewBoardQuestionInput {
            tags: Some(vec!["ui".into()]),
            ticket_ids: Some(vec!["ARC-0002".into()]),
            ..NewBoardQuestionInput::titled("Neuer, bereit")
        },
    )
    .await
    .unwrap();

    let all = f
        .questions
        .list("ARC", &BoardQuestionListFilter::default(), None)
        .await
        .unwrap();
    assert_eq!(all.counts.open, 2);
    assert_eq!(all.counts.answered, 0);
    assert_eq!(all.counts.discarded, 0);
    assert_eq!(all.counts.mine, 0);
    assert_eq!(
        all.questions
            .iter()
            .map(|item| (
                item.view.question.id.as_str(),
                item.view.ready,
                item.view
                    .waiting_on
                    .iter()
                    .map(|waiting| (waiting.id.as_str(), waiting.state.as_str()))
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        [
            ("ARC-F0002", true, vec![]),
            ("ARC-F0001", false, vec![("ARC-0001", "backlog")])
        ]
    );

    assert_eq!(
        ids(&listed(
            &f,
            BoardQuestionListFilter {
                ticket: Some("arc-2".into()),
                ..BoardQuestionListFilter::default()
            }
        )
        .await
        .questions),
        ["ARC-F0002"]
    );
    assert_eq!(
        ids(&listed(
            &f,
            BoardQuestionListFilter {
                tag: Some("ui".into()),
                ..BoardQuestionListFilter::default()
            }
        )
        .await
        .questions),
        ["ARC-F0002"]
    );
    assert_eq!(
        ids(&listed(
            &f,
            BoardQuestionListFilter {
                text: Some("WARTET".into()),
                ..BoardQuestionListFilter::default()
            }
        )
        .await
        .questions),
        ["ARC-F0001"]
    );
    let answered = listed(
        &f,
        BoardQuestionListFilter {
            state: Some(QuestionState::Answered),
            ..BoardQuestionListFilter::default()
        },
    )
    .await;
    assert!(answered.questions.is_empty());
    // Die Zähler gelten für alle Fragen des Projekts, nicht für die gefilterte Liste
    assert_eq!(answered.counts.open, 2);
}

#[tokio::test]
async fn andert_titel_beschreibung_tags_und_anlass_mit_verlauf_und_version() {
    let f = setup().await;
    ask(
        &f,
        NewBoardQuestionInput {
            description: Some("Erst".into()),
            ..NewBoardQuestionInput::titled("Alt")
        },
    )
    .await
    .unwrap();

    let changed = f
        .questions
        .update(
            "ARC-F0001",
            &BoardQuestionChanges {
                title: Some("Neu".into()),
                description: Some("Zweit".into()),
                tags: Some(vec!["a".into()]),
                occasion: Some(Some("spontan".into())),
            },
            &f.human,
            &BoardQuestionWriteOptions::rev(1),
        )
        .await
        .unwrap();

    assert_eq!(changed.view.question.title, "Neu");
    assert_eq!(changed.view.question.description, "Zweit");
    assert_eq!(changed.view.question.tags, ["a"]);
    assert_eq!(changed.view.question.occasion.as_deref(), Some("spontan"));
    assert_eq!(changed.view.question.rev, 2);
    assert_eq!(
        changed.history[1..]
            .iter()
            .map(|entry| (
                entry.entry_type.as_str(),
                entry.text.as_str(),
                entry.by_kind.map(|kind| kind.as_str())
            ))
            .collect::<Vec<_>>(),
        [
            ("title", "Alt → Neu", Some("human")),
            ("description", "Erst", Some("human")),
            ("tags", "(keine) → a", Some("human")),
            ("occasion", "(keiner) → spontan", Some("human")),
        ]
    );
    assert_eq!(
        f.questions
            .update(
                "ARC-F0001",
                &BoardQuestionChanges {
                    title: Some("Zu spät".into()),
                    ..BoardQuestionChanges::default()
                },
                &f.human,
                &BoardQuestionWriteOptions::rev(1),
            )
            .await
            .unwrap_err()
            .code(),
        Some("conflict")
    );
    assert_eq!(
        f.questions
            .update(
                "ARC-F0001",
                &BoardQuestionChanges::default(),
                &f.human,
                &any_rev(),
            )
            .await
            .unwrap_err()
            .code(),
        Some("nothing-changed")
    );
    // Eine Änderung, die nichts ändert, schreibt nicht und lässt die Version stehen
    assert_eq!(
        f.questions
            .update(
                "ARC-F0001",
                &BoardQuestionChanges {
                    title: Some("Neu".into()),
                    ..BoardQuestionChanges::default()
                },
                &f.human,
                &any_rev(),
            )
            .await
            .unwrap()
            .view
            .question
            .rev,
        2
    );
}

#[tokio::test]
async fn setzt_tickets_und_abhangigkeiten_lehnt_unbekanntes_selbstbezug_und_kreise_ab() {
    let f = setup().await;
    ask_titled(&f, "A").await;
    ask(
        &f,
        NewBoardQuestionInput {
            depends_on: Some(vec!["ARC-F0001".into()]),
            ..NewBoardQuestionInput::titled("B")
        },
    )
    .await
    .unwrap();

    let linked = f
        .questions
        .set_tickets(
            "ARC-F0001",
            &["ARC-0002".to_string(), "arc-1".to_string()],
            &f.agent,
            &BoardQuestionWriteOptions::rev(1),
        )
        .await
        .unwrap();
    assert_eq!(linked.view.question.ticket_ids, ["ARC-0001", "ARC-0002"]);
    assert_eq!(linked.view.question.rev, 2);
    assert_eq!(
        last_history(&linked),
        (
            BoardQuestionHistoryType::Tickets,
            "(keine) → ARC-0001, ARC-0002"
        )
    );

    let deps = f
        .questions
        .set_depends("ARC-F0001", &["arc-2".to_string()], &f.agent, &any_rev())
        .await
        .unwrap();
    assert_eq!(deps.view.question.depends_on, ["ARC-0002"]);
    assert_eq!(
        deps.view
            .waiting_on
            .iter()
            .map(|waiting| (waiting.id.as_str(), waiting.state.as_str()))
            .collect::<Vec<_>>(),
        [("ARC-0002", "backlog")]
    );
    assert!(!deps.view.ready);
    assert_eq!(deps.view.question.rev, 3);
    assert_eq!(
        last_history(&deps),
        (BoardQuestionHistoryType::Depends, "(keine) → ARC-0002")
    );
    // Die Frage B wartet auf die Frage A
    assert_eq!(
        f.questions
            .get("ARC-F0002")
            .await
            .unwrap()
            .view
            .waiting_on
            .iter()
            .map(|waiting| (waiting.id.as_str(), waiting.state.as_str()))
            .collect::<Vec<_>>(),
        [("ARC-F0001", "open")]
    );

    assert_eq!(
        depends_err(&f, vec!["ARC-F0002".into()]).await.as_deref(),
        Some("dependency-cycle")
    );
    assert_eq!(
        depends_err(&f, vec!["ARC-F0001".into()]).await.as_deref(),
        Some("dependency-self")
    );
    assert_eq!(
        depends_err(&f, vec!["ARC-F0042".into()]).await.as_deref(),
        Some("dependency-unknown")
    );
    assert_eq!(
        depends_err(&f, vec!["quatsch".into()]).await.as_deref(),
        Some("reference-invalid")
    );
    assert_eq!(
        ask(
            &f,
            NewBoardQuestionInput {
                depends_on: Some(vec!["ARC-0042".into()]),
                ..NewBoardQuestionInput::titled("C")
            }
        )
        .await
        .unwrap_err()
        .code(),
        Some("dependency-unknown")
    );
    assert!(f
        .questions
        .set_tickets("ARC-F0001", &[], &f.agent, &any_rev())
        .await
        .unwrap()
        .view
        .question
        .ticket_ids
        .is_empty());
}

#[tokio::test]
async fn hangt_kommentare_an_und_erhoht_die_version() {
    let f = setup().await;
    ask_titled(&f, "A").await;

    let detail = f
        .questions
        .comment("ARC-F0001", " Wie wäre es mit SQL? ", &f.human)
        .await
        .unwrap();

    assert_eq!(detail.view.question.rev, 2);
    assert_eq!(
        detail
            .comments
            .iter()
            .map(|comment| (
                comment.body.as_str(),
                comment.author_name.as_str(),
                comment.author_kind.as_str()
            ))
            .collect::<Vec<_>>(),
        [("Wie wäre es mit SQL?", "Florian", "human")]
    );
    assert_eq!(
        f.questions
            .comment("ARC-F0001", "  ", &f.human)
            .await
            .unwrap_err()
            .code(),
        Some("comment-empty")
    );
}

#[tokio::test]
async fn erganzt_recherchen_mit_fortlaufenden_optionen_und_beschreibt_sie_im_verlauf() {
    let f = setup().await;
    ask_titled(&f, "A").await;

    f.questions
        .add_research(
            "ARC-F0001",
            &ResearchInput {
                context: Some("Heute: Datei".into()),
                options: Some(vec![
                    ResearchOptionInput::new("Datei", "bleibt"),
                    ResearchOptionInput {
                        pros: Some(vec!["geteilt".into()]),
                        recommended: Some(true),
                        ..ResearchOptionInput::new("DB", "SQL")
                    },
                ]),
            },
            &f.agent,
        )
        .await
        .unwrap();
    let subagent = crate::board::question_types::BoardQuestionActor {
        kind: AuthorKind::Subagent,
        ..f.agent.clone()
    };
    let detail = f
        .questions
        .add_research(
            "ARC-F0001",
            &ResearchInput::options(vec![ResearchOptionInput {
                cons: Some(vec!["flüchtig".into()]),
                ..ResearchOptionInput::new("Cache", "im Speicher")
            }]),
            &subagent,
        )
        .await
        .unwrap();

    assert_eq!(
        detail
            .research
            .iter()
            .map(|research| (
                research.kind.as_str(),
                research.by_name.as_str(),
                research
                    .options
                    .iter()
                    .map(|option| option.id.as_str())
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        [
            ("session", "claude", vec!["O1", "O2"]),
            ("subagent", "claude", vec!["O3"])
        ]
    );
    assert_eq!(detail.research[0].context, "Heute: Datei");
    assert!(detail.research[0].options[0].pros.is_empty());
    assert_eq!(detail.research[0].options[1].pros, ["geteilt"]);
    assert!(detail.research[0].options[1].recommended);
    assert!(detail.view.researched);
    // Gezählt werden die Optionen der neuesten Recherche
    assert_eq!(detail.view.option_count, 1);
    assert_eq!(detail.view.question.rev, 3);
    assert_eq!(
        detail
            .history
            .iter()
            .filter(|entry| entry.entry_type == BoardQuestionHistoryType::Research)
            .map(|entry| entry.text.as_str())
            .collect::<Vec<_>>(),
        ["2 Optionen (O1 bis O2), O2 empfohlen", "1 Option (O3)"]
    );

    assert_eq!(
        research_err(&f, ResearchInput::default()).await.as_deref(),
        Some("research-invalid")
    );
    assert_eq!(
        research_err(
            &f,
            ResearchInput::options(vec![
                ResearchOptionInput {
                    recommended: Some(true),
                    ..ResearchOptionInput::new("a", "b")
                },
                ResearchOptionInput {
                    recommended: Some(true),
                    ..ResearchOptionInput::new("c", "d")
                },
            ])
        )
        .await
        .as_deref(),
        Some("research-recommended-multiple")
    );
    assert_eq!(
        research_err(
            &f,
            ResearchInput::options(
                (0..6)
                    .map(|i| ResearchOptionInput::new(&format!("T{i}"), "x"))
                    .collect()
            )
        )
        .await
        .as_deref(),
        Some("research-invalid")
    );
}

/// `ist Teil des Dienst-Bündels` aus TypeScript: `BoardServices.questions` arbeitet auf
/// derselben Datenbank
#[tokio::test]
async fn ist_teil_des_dienst_bundels() {
    let f = BoardFixture::create().await;
    let services: BoardServices<InMemoryBoardDatabase> =
        BoardServices::new(f.db.clone(), f.clock.clone(), f.repo_paths.clone());

    let detail = services
        .questions
        .create(
            "ARC",
            &NewBoardQuestionInput::titled("Aus dem Bündel"),
            &f.agent,
        )
        .await
        .unwrap();

    assert_eq!(detail.view.question.id, "ARC-F0001");
    // Dieselbe Datenbank: der Dienst der Umgebung liest die Frage des Bündels
    assert_eq!(
        f.questions
            .get("ARC-F0001")
            .await
            .unwrap()
            .view
            .question
            .title,
        "Aus dem Bündel"
    );
    // Und auch das Bündel ohne lokale Repo-Pfade bringt die Fragen mit
    let memory: BoardServices<InMemoryBoardDatabase> = BoardServices::with_memory_paths(
        Arc::new(InMemoryBoardDatabase::new()),
        Clock::fixed("2026-10-13T10:00:00.000Z".parse().unwrap()),
    );
    assert_eq!(
        memory.questions.get("ARC-F0001").await.unwrap_err().code(),
        Some("question-not-found")
    );
    // Entscheidungen bleiben Menschen vorbehalten, auch über das Bündel
    assert_eq!(
        services
            .questions
            .answer(
                "ARC-F0001",
                &DecisionInput::text("frei", "x"),
                &f.agent,
                &any_rev()
            )
            .await
            .unwrap_err()
            .code(),
        Some("human-only")
    );
    let answered = services
        .questions
        .answer(
            "ARC-F0001",
            &DecisionInput::text("frei", "x"),
            &f.human,
            &any_rev(),
        )
        .await
        .unwrap();
    assert_eq!(
        answered.view.question.decision.unwrap().outcome,
        BoardQuestionDecisionOutcome::Answered
    );
}
