//! Ideen verstehen: Board-Kontext sammeln und mit Claude zu einem Entwurf auswerten
//! (`IdeaInterpreter`).
//!
//! Claude ist hier wirklich ein eigener Prozess – der Ersatz `nexus-fake-claude`. Sein Modus
//! und seine Antwort kommen über die Umgebung des Laufs
//! ([`IdeaInterpreter::with_env`]), nicht über die des Testprozesses.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use common::BoardBackendWithServices;
use nexus_core::actors::AuthorKind;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::note_service::{AddNoteOptions, NewNoteText};
use nexus_core::board::question_service::NewBoardQuestionInput;
use nexus_core::board::question_types::BoardQuestionActor;
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use nexus_core::dispatch::DispatchConfigStore;
use nexus_core::telegram::idea::{IdeaKind, MAX_TITLE};
use nexus_core::HarnessHome;
use nexus_server::board_runtime::{factory, BoardBackend, BoardBackendFuture, BoardRuntimeHolder};
use nexus_server::dispatch::claude_runner::ProcessClaudeRunner;
use nexus_server::telegram::{IdeaInterpreter, IdeaRequest};
use serde_json::{json, Value};

/// Alles, was ein Lauf braucht: Benutzerordner, Board-Laufzeit und der Interpreter
struct Fixture {
    _home_dir: tempfile::TempDir,
    boards: BoardRuntimeHolder,
    interpreter: IdeaInterpreter,
    /// Das eine eigene Board des Tests
    board_id: i32,
}

impl Fixture {
    /// Ein Projekt mit einem Board, zwei Tickets (eines erledigt), einer Notiz und einer Frage;
    /// Claude ist der Ersatz im gegebenen Modus. Die Antwort baut der Aufrufer, sobald die
    /// Nummer des Boards feststeht.
    async fn start(modus: &str, antwort: impl Fn(i32) -> String) -> Self {
        let home_dir = tempfile::tempdir().expect("Benutzerordner");
        let home = HarnessHome::at(home_dir.path());
        DispatchConfigStore::new(home.clone())
            .write(&json!({
                "claude": { "command": env!("CARGO_BIN_EXE_nexus-fake-claude") },
            }))
            .expect("dispatch.yaml schreiben");

        let backend = BoardBackendWithServices::create().await;
        let actor = backend.actor();
        let services = &backend.services;
        services
            .projects
            .create("NXS", "Nexus", &actor)
            .await
            .expect("Projekt");
        let board = services
            .boards
            .create("NXS", "Nexus", &actor)
            .await
            .expect("Board");
        services
            .boards
            .set_repo_path(board.board.id, Some("/repos/nexus"))
            .await
            .expect("Repo-Pfad");
        for (title, status) in [
            ("Board-Ansicht bauen", TicketStatus::Backlog),
            ("Alte Spalte weg", TicketStatus::Done),
        ] {
            services
                .tickets
                .create(
                    board.board.id,
                    &NewTicketInput {
                        labels: Some(vec!["web".into()]),
                        status: Some(status),
                        acceptance: Some("Man sieht es in der Ansicht.".into()),
                        ..NewTicketInput::titled(title)
                    },
                    &actor,
                    &WriteOptions::default(),
                )
                .await
                .expect("Ticket");
        }
        services
            .notes
            .add(
                board.board.id,
                &NewNoteText::titled("Cache der Board-Ansicht läuft nie ab"),
                &actor,
                &AddNoteOptions::default(),
            )
            .await
            .expect("Notiz");
        services
            .questions
            .create(
                "NXS",
                &NewBoardQuestionInput::titled("Auch für SQL Server?"),
                &BoardQuestionActor {
                    actor: actor.clone(),
                    kind: AuthorKind::Human,
                    session: None,
                },
            )
            .await
            .expect("Frage");

        let shared = backend.clone();
        let boards = BoardRuntimeHolder::new(factory(move || -> BoardBackendFuture {
            let backend = shared.clone();
            Box::pin(async move { Ok(backend as Arc<dyn BoardBackend>) })
        }));

        let mut env: BTreeMap<String, String> = BTreeMap::new();
        env.insert("FAKE_CLAUDE_MODE".into(), modus.into());
        env.insert("FAKE_CLAUDE_REPLY".into(), antwort(board.board.id));
        Self {
            _home_dir: home_dir,
            boards,
            interpreter: IdeaInterpreter::with_env(home, env),
            board_id: board.board.id,
        }
    }

    /// Die Anfrage der ersten Runde auf dem Board des Tests
    fn request(&self, input: &str) -> IdeaRequest {
        IdeaRequest {
            chat_id: 42,
            input: input.into(),
            active_board_id: Some(self.board_id),
            previous: None,
        }
    }
}

/// Eine vollständige Antwort von Claude, mit weiteren Feldern überschreibbar
fn antwort(board_id: i32, extra: Value) -> String {
    let mut item = json!({
        "summary": "Der Cache der Board-Ansicht läuft nie ab.",
        "target": { "kind": "note", "boardId": board_id, "ticketId": null },
        "title": "Cache der Board-Ansicht läuft nie ab",
        "details": "Fällt in der Ansicht auf; Vorschlag: Lebenszeit setzen.",
        "related": [{ "ticketId": "NXS-0001", "why": "dieselbe Ansicht" }],
        "duplicates": [],
        "questions": ["Auch für SQL Server?"],
        "confidence": 0.7,
    });
    for (key, value) in extra.as_object().cloned().unwrap_or_default() {
        item[key] = value;
    }
    format!("Hier ist der Entwurf: {item}")
}

/// Die Standardantwort zum Board
fn gueltige_antwort(board_id: i32) -> String {
    antwort(board_id, json!({}))
}

#[tokio::test]
async fn liefert_einen_geprueften_entwurf_aus_dem_board_kontext() {
    let f = Fixture::start("antwort", gueltige_antwort).await;

    let draft = f
        .interpreter
        .interpret(
            &ProcessClaudeRunner,
            &f.boards,
            &f.request("Der Cache der Board-Ansicht läuft nie ab."),
        )
        .await
        .expect("Entwurf");

    assert_eq!(draft.target.kind, IdeaKind::Note);
    assert_eq!(draft.target.board_id, f.board_id);
    assert_eq!(draft.target.ticket_id, None);
    assert_eq!(draft.title, "Cache der Board-Ansicht läuft nie ab");
    assert_eq!(
        draft
            .related
            .iter()
            .map(|found| found.ticket_id.as_str())
            .collect::<Vec<_>>(),
        ["NXS-0001"]
    );
    assert_eq!(draft.questions[0].text, "Auch für SQL Server?");
    assert_eq!(draft.confidence, 0.7);
    assert_eq!(
        draft.transcript,
        "Der Cache der Board-Ansicht läuft nie ab."
    );
    assert_eq!(draft.rounds, 1);
    assert!(draft.title.chars().count() <= MAX_TITLE);
}

#[tokio::test]
async fn sammelt_eigene_boards_offene_tickets_aehnliche_notizen_und_fragen() {
    let f = Fixture::start("antwort", |_| String::new()).await;

    let context = f
        .interpreter
        .context(
            &f.boards,
            Some(f.board_id),
            "Der Cache der Board-Ansicht läuft nie ab.",
        )
        .await
        .expect("Kontext");

    assert_eq!(context.boards.len(), 1);
    assert_eq!(context.boards[0].name, "Nexus");
    assert_eq!(context.boards[0].repo_path.as_deref(), Some("/repos/nexus"));
    assert_eq!(context.active_board_id, Some(f.board_id));
    // Das erledigte Ticket gehört nicht in den Kontext
    assert_eq!(
        context
            .tickets
            .iter()
            .map(|ticket| ticket.title.as_str())
            .collect::<Vec<_>>(),
        ["Board-Ansicht bauen"]
    );
    assert_eq!(context.tickets[0].labels, ["web"]);
    assert_eq!(context.notes.len(), 1);
    assert!(context.notes[0].similar, "die Notiz ähnelt der Eingabe");
    assert_eq!(
        context
            .questions
            .iter()
            .map(|question| question.title.as_str())
            .collect::<Vec<_>>(),
        ["Auch für SQL Server?"]
    );
}

#[tokio::test]
async fn zaehlt_die_verfeinerungsrunde_und_haengt_die_antwort_ans_transkript() {
    let f = Fixture::start("antwort", gueltige_antwort).await;
    let erster = f
        .interpreter
        .interpret(
            &ProcessClaudeRunner,
            &f.boards,
            &f.request("Der Cache der Board-Ansicht läuft nie ab."),
        )
        .await
        .expect("erster Entwurf");

    let zweiter = f
        .interpreter
        .interpret(
            &ProcessClaudeRunner,
            &f.boards,
            &IdeaRequest {
                input: "Ja, auch für SQL Server.".into(),
                previous: Some(erster),
                ..f.request("")
            },
        )
        .await
        .expect("zweiter Entwurf");

    assert_eq!(zweiter.rounds, 2);
    assert_eq!(
        zweiter.transcript,
        "Der Cache der Board-Ansicht läuft nie ab.\nJa, auch für SQL Server."
    );
}

#[tokio::test]
async fn haelt_je_chat_nur_einen_lauf_gleichzeitig() {
    let f = Fixture::start("antwort", gueltige_antwort).await;

    let eine = f.request("Eine Idee.");
    let andere = f.request("Noch eine Idee.");
    let (erster, zweiter) = tokio::join!(
        f.interpreter
            .interpret(&ProcessClaudeRunner, &f.boards, &eine),
        f.interpreter
            .interpret(&ProcessClaudeRunner, &f.boards, &andere),
    );

    let codes: Vec<Option<String>> = [&erster, &zweiter]
        .iter()
        .map(|result| match result {
            Ok(_) => None,
            Err(error) => error.code().map(str::to_string),
        })
        .collect();
    assert!(
        codes.contains(&Some("conflict".to_string())),
        "einer der beiden Läufe ist ein Konflikt: {codes:?}"
    );
    assert!(codes.contains(&None), "der andere liefert einen Entwurf");
}

#[tokio::test]
async fn meldet_kaputtes_json_als_verstaendlichen_fehler() {
    let f = Fixture::start("antwort", |_| {
        "Ich habe die Idee leider nicht verstanden.".into()
    })
    .await;

    let error = f
        .interpreter
        .interpret(&ProcessClaudeRunner, &f.boards, &f.request("Eine Idee."))
        .await
        .unwrap_err();

    assert_eq!(error.code(), Some("analysis-failed"));
    assert!(
        error.to_string().contains("nicht lesbar"),
        "{}",
        error.to_string()
    );
}

#[tokio::test]
async fn meldet_den_zeitablauf_als_verstaendlichen_fehler() {
    // Der Ersatz läuft im Modus `sleep` bis zum Zeitlimit; im Test ist es eine Sekunde
    let mut f = Fixture::start("sleep", |_| String::new()).await;
    f.interpreter = f.interpreter.with_timeout(Duration::from_secs(1));

    let error = f
        .interpreter
        .interpret(&ProcessClaudeRunner, &f.boards, &f.request("Eine Idee."))
        .await
        .unwrap_err();

    assert_eq!(error.code(), Some("analysis-failed"));
    assert!(
        error
            .to_string()
            .contains("nicht innerhalb von einer Sekunde verstanden"),
        "{error}"
    );
}
