//! Kommentare, Notizen, Verlauf, Löschungen und Fragen auf SQL Server.
//!
//! Zwei Teile:
//!
//! * **Ohne Datenbank** (immer): das Feature-Gate der Notizen – portiert aus
//!   `packages/db/tests/MssqlFeatureGate.test.ts` – und der SQL-Text der Zugriffe
//!   (Sperr-Hinweis, Blöcke langer ID-Listen) über den [`FakeRunner`].
//! * **Mit Datenbank** (nur mit `HARNESS_TEST_DB`): Rundreisen je Repository gegen
//!   `AiHarness_Test`, analog zu den SQLite-Tests `Sqlite*Repository.test.ts`.
//!
//! Aufruf:
//! `cargo test -p nexus-db --test mssql_repos_notes_questions`
//! `HARNESS_TEST_DB='<json>' cargo test -p nexus-db --test mssql_repos_notes_questions -- --test-threads=1`
//!
//! Seriell, weil alle Tests mit Datenbank dieselbe Datenbank leeren.

mod common;

use common::test_db;

use nexus_core::actors::AuthorKind;
use nexus_core::board::basics::{NoteState, TicketStatus};
use nexus_core::board::database::{
    BoardDatabase, BoardQuestionFilter, BoardUnitOfWork, BoxFut, NewComment, NewNote,
    NewQuestionComment, NewQuestionResearch, NoteFilter,
};
use nexus_core::board::question_types::{
    BoardQuestion, BoardQuestionDecision, BoardQuestionDecisionOutcome, BoardQuestionHistoryEntry,
    BoardQuestionHistoryType, BoardQuestionOption,
};
use nexus_core::board::types::{Deletion, HistoryEntry, Note};
use nexus_core::questions::QuestionState;
use nexus_core::Result;
use nexus_db::mssql::testing::FakeRunner;
use nexus_db::mssql::{MsCell, MsRow, MssqlBoardDatabase, MssqlUnitOfWork};
use nexus_db::{ConnectOptions, ManagedBoardDatabase};

/// Zeitpunkt, mit dem die Tests schreiben
const TEST_NOW: &str = "2026-01-01T00:00:00.000Z";

// ---------------------------------------------------------------------------------------------
// Teil 1: ohne Datenbank
// ---------------------------------------------------------------------------------------------

/// Der Filter der Gate-Tests (ein Board, keine Quelle)
fn filter() -> NoteFilter {
    NoteFilter {
        board_id: 1,
        states: None,
        source: None,
    }
}

/// Eine Notiz, wie die Gate-Tests sie schreiben wollen
fn notiz() -> Note {
    Note {
        id: 1,
        board_id: 1,
        author_id: "a".to_string(),
        author_name: "A".to_string(),
        title: "T".to_string(),
        body: "B".to_string(),
        source: None,
        labels: Vec::new(),
        seen_count: 1,
        seen_in: Vec::new(),
        state: NoteState::Open,
        ticket_id: None,
        reason: None,
        created_at: TEST_NOW.to_string(),
    }
}

/// Dieselbe Notiz ohne ID (für `insert`)
fn neue_notiz() -> NewNote {
    let note = notiz();
    NewNote {
        board_id: note.board_id,
        author_id: note.author_id,
        author_name: note.author_name,
        title: note.title,
        body: note.body,
        source: note.source,
        labels: note.labels,
        seen_count: note.seen_count,
        seen_in: note.seen_in,
        state: note.state,
        ticket_id: note.ticket_id,
        reason: note.reason,
        created_at: note.created_at,
    }
}

#[tokio::test]
async fn verweigert_notizen_mit_hinweis_auf_das_skript() {
    let mut runner = FakeRunner::new();
    let mut uow =
        MssqlUnitOfWork::new(&mut runner, vec!["note-structure".to_string()], "AiHarness");
    let fehler = vec![
        uow.notes().list(&filter()).await.unwrap_err(),
        uow.notes().by_id(1, false).await.unwrap_err(),
        uow.notes().insert(&neue_notiz()).await.unwrap_err(),
        uow.notes().update(&notiz()).await.unwrap_err(),
    ];
    for error in &fehler {
        assert_eq!(error.code(), Some("board-schema-outdated"));
        assert!(
            error
                .to_string()
                .contains("packages/db/sql/005_note_structure.sql"),
            "Meldung ohne Skript: {error}"
        );
    }
    drop(uow);
    // Ein gesperrter Zugriff setzt gar keine Anweisung ab
    assert!(runner.texts.is_empty());
}

#[tokio::test]
async fn nennt_die_datenbank_und_verspricht_dass_die_ubrigen_befehle_weiterlaufen() {
    let mut runner = FakeRunner::new();
    let mut uow =
        MssqlUnitOfWork::new(&mut runner, vec!["note-structure".to_string()], "AiHarness");
    let meldung = uow.notes().list(&filter()).await.unwrap_err().to_string();
    assert!(meldung.contains("AiHarness"), "Meldung: {meldung}");
    assert!(meldung.contains("laufen weiter"), "Meldung: {meldung}");
}

#[tokio::test]
async fn fragt_notizen_ganz_normal_ab_sobald_die_erweiterung_da_ist() {
    let mut runner = FakeRunner::new();
    {
        let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), "AiHarness");
        assert_eq!(uow.notes().list(&filter()).await.unwrap(), Vec::new());
    }
    assert!(
        runner.texts[0].contains("board.Notes"),
        "{}",
        runner.texts[0]
    );
}

#[tokio::test]
async fn eine_leere_statusliste_ergibt_keine_notizen() {
    let mut runner = FakeRunner::new();
    {
        let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), "AiHarness");
        let filter = NoteFilter {
            states: Some(Vec::new()),
            ..filter()
        };
        assert_eq!(uow.notes().list(&filter).await.unwrap(), Vec::new());
    }
    assert!(runner.texts.is_empty());
}

#[tokio::test]
async fn sperrt_die_notiz_nur_beim_lesen_zum_andern() {
    let mut runner = FakeRunner::new();
    {
        let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), "AiHarness");
        assert!(uow.notes().by_id(1, true).await.unwrap().is_none());
        assert!(uow.notes().by_id(1, false).await.unwrap().is_none());
    }
    assert!(
        runner.texts[0].contains("WITH (UPDLOCK, ROWLOCK)"),
        "{}",
        runner.texts[0]
    );
    assert!(!runner.texts[1].contains("UPDLOCK"), "{}", runner.texts[1]);
}

/// Eine Zeile aus `board.Questions`, wie die SQL-Text-Tests sie brauchen
fn fragen_zeile() -> MsRow {
    let text = |value: &str| MsCell::Text(value.to_string());
    let zeit = MsCell::DateTime(
        chrono::NaiveDateTime::parse_from_str("2026-01-01T00:00:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
    );
    MsRow::from_cells([
        ("Id".to_string(), text("ARC-F0001")),
        ("ProjectId".to_string(), MsCell::Int(1)),
        ("Number".to_string(), MsCell::Int(1)),
        ("Title".to_string(), text("Wohin?")),
        ("Description".to_string(), text("")),
        ("State".to_string(), text("open")),
        ("Occasion".to_string(), MsCell::Null),
        ("OriginKind".to_string(), text("session")),
        ("OriginName".to_string(), text("agent")),
        ("OriginSession".to_string(), MsCell::Null),
        ("DecisionOutcome".to_string(), MsCell::Null),
        ("DecisionOption".to_string(), MsCell::Null),
        ("DecisionText".to_string(), MsCell::Null),
        ("DecisionReason".to_string(), MsCell::Null),
        ("DecidedById".to_string(), MsCell::Null),
        ("DecidedByName".to_string(), MsCell::Null),
        ("DecidedAt".to_string(), MsCell::Null),
        ("CreatedAt".to_string(), zeit.clone()),
        (
            "CreatedById".to_string(),
            MsCell::Guid(test_db::USER_ID.to_string()),
        ),
        ("UpdatedAt".to_string(), zeit),
        ("Rev".to_string(), MsCell::Int(1)),
    ])
}

#[tokio::test]
async fn sperrt_die_frage_beim_lesen_zum_andern_und_die_unterabfrage_nicht() {
    let mut runner = FakeRunner::with_rows(vec![fragen_zeile()]);
    {
        let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), "AiHarness");
        assert!(uow
            .questions()
            .by_id("ARC-F0001", true)
            .await
            .unwrap()
            .is_some());
    }
    assert!(
        runner.texts[0].contains("WITH (UPDLOCK, ROWLOCK)"),
        "{}",
        runner.texts[0]
    );
    // Tags, Tickets und Abhängigkeiten lesen über dieselbe Klausel – ohne Sperr-Hinweis
    for text in &runner.texts[1..] {
        assert!(text.contains("board.Question"), "{text}");
        assert!(!text.contains("UPDLOCK"), "{text}");
    }
}

#[tokio::test]
async fn liest_die_frage_ohne_sperr_hinweis_wenn_nicht_geandert_wird() {
    let mut runner = FakeRunner::with_rows(vec![fragen_zeile()]);
    {
        let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), "AiHarness");
        assert!(uow
            .questions()
            .by_id("ARC-F0001", false)
            .await
            .unwrap()
            .is_some());
    }
    for text in &runner.texts {
        assert!(!text.contains("UPDLOCK"), "{text}");
    }
}

#[tokio::test]
async fn leere_listen_im_filter_der_fragen_ergeben_keine_fragen() {
    let mut runner = FakeRunner::new();
    {
        let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), "AiHarness");
        for filter in [
            BoardQuestionFilter {
                ids: Some(Vec::new()),
                ..BoardQuestionFilter::default()
            },
            BoardQuestionFilter {
                states: Some(Vec::new()),
                ..BoardQuestionFilter::default()
            },
            BoardQuestionFilter {
                ticket_ids: Some(Vec::new()),
                ..BoardQuestionFilter::default()
            },
        ] {
            assert_eq!(uow.questions().list(&filter).await.unwrap(), Vec::new());
        }
    }
    assert!(runner.texts.is_empty());
}

#[tokio::test]
async fn teilt_lange_id_listen_in_blocke_unter_der_parametergrenze() {
    let ids: Vec<i64> = (1..=501).collect();
    let mut runner = FakeRunner::new();
    {
        let mut uow = MssqlUnitOfWork::new(&mut runner, Vec::new(), "AiHarness");
        uow.comments()
            .mark_read_by_agent(&ids, TEST_NOW)
            .await
            .unwrap();
    }
    assert_eq!(runner.texts.len(), 2);
    // 500 IDs + die Lesemarke je Block, dann der Rest
    assert_eq!(runner.bound[0].len(), 501);
    assert_eq!(runner.bound[1].len(), 2);
}

// ---------------------------------------------------------------------------------------------
// Teil 2: mit Datenbank
// ---------------------------------------------------------------------------------------------

/// Ein Projekt, ein Board und zwei Tickets in einer frisch geleerten Datenbank; liefert die
/// Verbindung samt Board-ID. `None`, solange es keine Test-Datenbank gibt.
///
/// Gesät wird mit rohem SQL: die Repositories für Benutzer, Projekte, Boards und Tickets
/// entstehen in einem anderen Ticket (P2-12).
async fn datenbank(test: &str) -> Option<(MssqlBoardDatabase, i32)> {
    let config = test_db::config_or_skip(test)?;
    test_db::reset(&config).await;
    let mut client = test_db::client(&config).await;
    let saat = format!(
        "INSERT INTO board.Users (Id, Name, CreatedAt) VALUES ('{user}', N'Test', '2026-01-01T00:00:00');
         INSERT INTO board.Projects ([Key], Name, CreatedAt, CreatedBy) VALUES (N'ARC', N'Arc', '2026-01-01T00:00:00', '{user}');
         DECLARE @p int = SCOPE_IDENTITY();
         INSERT INTO board.Boards (ProjectId, OwnerId, Name, CreatedAt) VALUES (@p, '{user}', N'Board', '2026-01-01T00:00:00');
         DECLARE @b int = SCOPE_IDENTITY();
         INSERT INTO board.Tickets (Id, ProjectId, BoardId, Number, Title, Status, Priority, SortOrder, NeedsPlanning, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
           VALUES (N'ARC-0001', @p, @b, 1, N'Eins', 'backlog', 'normal', 0, 0, '2026-01-01T00:00:00', '{user}', N'Test', '2026-01-01T00:00:00', 1),
                  (N'ARC-0002', @p, @b, 2, N'Zwei', 'backlog', 'normal', 1, 0, '2026-01-01T00:00:00', '{user}', N'Test', '2026-01-01T00:00:00', 1);
         SELECT @p AS ProjectId, @b AS BoardId;",
        user = test_db::USER_ID,
    );
    let zeilen = client
        .simple_query(saat)
        .await
        .expect("Saat")
        .into_results()
        .await
        .expect("Saat");
    let zeile = zeilen
        .into_iter()
        .flatten()
        .next()
        .expect("Saat ohne Zeile");
    let board_id: i32 = zeile.get("BoardId").expect("BoardId");
    let db = MssqlBoardDatabase::connect(&config, ConnectOptions::default())
        .await
        .expect("Verbindung zur Test-Datenbank");
    db.set_missing_features(Vec::new());
    Some((db, board_id))
}

#[tokio::test]
async fn schreibt_und_liest_kommentare_samt_lesemarken() {
    let Some((db, _board)) = datenbank("schreibt_und_liest_kommentare_samt_lesemarken").await
    else {
        return;
    };
    let neu = |ticket: &str, body: &str, gelesen: Option<&str>| NewComment {
        ticket_id: ticket.to_string(),
        author_id: test_db::USER_ID.to_string(),
        author_name: "Test".to_string(),
        body: body.to_string(),
        created_at: TEST_NOW.to_string(),
        read_by_agent_at: gelesen.map(str::to_string),
    };
    let ids = db
        .transaction(move |uow| -> BoxFut<'_, Result<Vec<i64>>> {
            let neu = neu;
            Box::pin(async move {
                let erster = uow
                    .comments()
                    .insert(&neu("ARC-0001", "eins", None))
                    .await?;
                let zweiter = uow
                    .comments()
                    .insert(&neu("ARC-0001", "zwei", Some(TEST_NOW)))
                    .await?;
                let dritter = uow
                    .comments()
                    .insert(&neu("ARC-0002", "drei", None))
                    .await?;
                Ok(vec![erster.id, zweiter.id, dritter.id])
            })
        })
        .await
        .unwrap();
    assert!(
        ids[0] < ids[1] && ids[1] < ids[2],
        "IDs zählen hoch: {ids:?}"
    );

    db.transaction(|uow| -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            let liste = uow.comments().list("ARC-0001").await?;
            assert_eq!(
                liste.iter().map(|c| c.body.as_str()).collect::<Vec<_>>(),
                ["eins", "zwei"]
            );
            assert_eq!(liste[0].author_id, test_db::USER_ID);
            assert_eq!(liste[0].created_at, TEST_NOW);
            assert_eq!(liste[0].read_by_agent_at, None);
            assert_eq!(liste[1].read_by_agent_at.as_deref(), Some(TEST_NOW));

            // Ungelesene über beide Tickets, in Reihenfolge des Anlegens
            let offen = uow
                .comments()
                .unread_by_agent(&["ARC-0001".to_string(), "ARC-0002".to_string()])
                .await?;
            assert_eq!(
                offen.iter().map(|c| c.body.as_str()).collect::<Vec<_>>(),
                ["eins", "drei"]
            );

            let gezahlt = uow.comments().count_by_board(1).await?;
            assert_eq!(gezahlt.get("ARC-0001"), Some(&2));
            assert_eq!(gezahlt.get("ARC-0002"), Some(&1));

            let offene_ids: Vec<i64> = offen.iter().map(|c| c.id).collect();
            uow.comments()
                .mark_read_by_agent(&offene_ids, "2026-02-01T12:00:00.000Z")
                .await?;
            assert!(uow
                .comments()
                .unread_by_agent(&["ARC-0001".to_string(), "ARC-0002".to_string()])
                .await?
                .is_empty());
            // Die ältere Lesemarke bleibt stehen
            let liste = uow.comments().list("ARC-0001").await?;
            assert_eq!(
                liste[0].read_by_agent_at.as_deref(),
                Some("2026-02-01T12:00:00.000Z")
            );
            assert_eq!(liste[1].read_by_agent_at.as_deref(), Some(TEST_NOW));
            Ok(())
        })
    })
    .await
    .unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
async fn schreibt_liest_und_andert_notizen_mit_filtern() {
    let Some((db, board_id)) = datenbank("schreibt_liest_und_andert_notizen_mit_filtern").await
    else {
        return;
    };
    db.transaction(move |uow| -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            let vorlage = |titel: &str, state: NoteState, source: Option<&str>| NewNote {
                board_id,
                author_id: test_db::USER_ID.to_string(),
                author_name: "Test".to_string(),
                title: titel.to_string(),
                body: "Befund".to_string(),
                source: source.map(str::to_string),
                labels: vec!["db".to_string(), "ui".to_string()],
                seen_count: 1,
                seen_in: Vec::new(),
                state,
                ticket_id: None,
                reason: None,
                created_at: TEST_NOW.to_string(),
            };
            let offen = uow
                .notes()
                .insert(&vorlage("Offen", NoteState::Open, Some("agent")))
                .await?;
            let verworfen = uow
                .notes()
                .insert(&vorlage("Verworfen", NoteState::Dropped, None))
                .await?;
            assert!(offen.id < verworfen.id);
            assert_eq!(offen.labels, ["db", "ui"]);

            // Ohne Statusliste kommt alles, mit Statusliste nur der genannte Stand
            let alle = uow
                .notes()
                .list(&NoteFilter {
                    board_id,
                    states: None,
                    source: None,
                })
                .await?;
            assert_eq!(alle.len(), 2);
            assert_eq!(alle[0].labels, ["db", "ui"]);
            assert_eq!(alle[0].author_id, test_db::USER_ID);
            assert_eq!(alle[0].created_at, TEST_NOW);
            let nur_offen = uow
                .notes()
                .list(&NoteFilter {
                    board_id,
                    states: Some(vec![NoteState::Open]),
                    source: None,
                })
                .await?;
            assert_eq!(
                nur_offen
                    .iter()
                    .map(|n| n.title.as_str())
                    .collect::<Vec<_>>(),
                ["Offen"]
            );
            // Die Quelle filtert zusätzlich
            let nach_quelle = uow
                .notes()
                .list(&NoteFilter {
                    board_id,
                    states: None,
                    source: Some("agent".to_string()),
                })
                .await?;
            assert_eq!(nach_quelle.len(), 1);

            // Lesen zum Ändern sperrt die Zeile und liefert dieselbe Notiz
            let gesperrt = uow.notes().by_id(offen.id, true).await?.expect("Notiz");
            assert_eq!(gesperrt, offen);
            let geandert = Note {
                title: "Neuer Titel".to_string(),
                seen_count: 3,
                seen_in: vec!["ARC-0002".to_string()],
                state: NoteState::Taken,
                ticket_id: Some("ARC-0002".to_string()),
                reason: Some("übernommen".to_string()),
                ..gesperrt
            };
            uow.notes().update(&geandert).await?;
            assert_eq!(
                uow.notes().by_id(offen.id, false).await?.expect("Notiz"),
                geandert
            );
            assert!(uow.notes().by_id(offen.id + 1000, false).await?.is_none());
            Ok(())
        })
    })
    .await
    .unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
async fn schreibt_und_liest_verlauf_und_loschprotokoll() {
    let Some((db, _board)) = datenbank("schreibt_und_liest_verlauf_und_loschprotokoll").await
    else {
        return;
    };
    db.transaction(|uow| -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            let eintrag = |field: &str, from: Option<&str>, to: Option<&str>| HistoryEntry {
                ticket_id: "ARC-0001".to_string(),
                at: TEST_NOW.to_string(),
                by_id: test_db::USER_ID.to_string(),
                by_name: "Test".to_string(),
                field: field.to_string(),
                from: from.map(str::to_string),
                to: to.map(str::to_string),
                note: None,
            };
            let eintrage = [
                eintrag("status", Some("backlog"), Some("planned")),
                eintrag("title", None, Some("Eins")),
            ];
            uow.history().insert(&eintrage).await?;
            let gelesen = uow.history().list("ARC-0001").await?;
            assert_eq!(gelesen, eintrage);
            assert!(uow.history().list("ARC-0002").await?.is_empty());

            let loschung = |ticket: &str, status: Option<TicketStatus>| Deletion {
                at: TEST_NOW.to_string(),
                by_id: test_db::USER_ID.to_string(),
                by_name: "Test".to_string(),
                ticket_id: ticket.to_string(),
                ticket_title: format!("Titel {ticket}"),
                from_status: status,
            };
            let erste = loschung("ARC-0009", Some(TicketStatus::Done));
            let zweite = loschung("ARC-0010", None);
            uow.deletions().insert(&erste).await?;
            uow.deletions().insert(&zweite).await?;
            assert_eq!(uow.deletions().list().await?, [erste, zweite]);
            Ok(())
        })
    })
    .await
    .unwrap();
    db.close().await.unwrap();
}

/// Eine Frage mit Tags, betroffenen Tickets und Abhängigkeiten
fn frage(project_id: i32, id: &str, number: i32) -> BoardQuestion {
    BoardQuestion {
        id: id.to_string(),
        project_id,
        number,
        title: "Wohin mit dem Cache?".to_string(),
        description: "Beschreibung".to_string(),
        state: QuestionState::Open,
        occasion: Some("beim Umbau".to_string()),
        origin_kind: AuthorKind::Session,
        origin_name: "agent".to_string(),
        origin_session: Some("s1".to_string()),
        tags: vec!["cache".to_string(), "db".to_string()],
        ticket_ids: vec!["ARC-0001".to_string(), "ARC-0002".to_string()],
        depends_on: vec!["ARC-0002".to_string()],
        decision: None,
        created_at: TEST_NOW.to_string(),
        created_by_id: test_db::USER_ID.to_string(),
        updated_at: TEST_NOW.to_string(),
        rev: 1,
    }
}

#[tokio::test]
async fn schreibt_liest_und_andert_fragen_samt_kindtabellen() {
    let Some((db, _board)) = datenbank("schreibt_liest_und_andert_fragen_samt_kindtabellen").await
    else {
        return;
    };
    db.transaction(|uow| -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            let erste = frage(1, "ARC-F0001", 1);
            let zweite = BoardQuestion {
                tags: Vec::new(),
                ticket_ids: Vec::new(),
                depends_on: Vec::new(),
                ..frage(1, "ARC-F0002", 2)
            };
            uow.questions().insert(&erste).await?;
            uow.questions().insert(&zweite).await?;

            assert_eq!(
                uow.questions().by_id("ARC-F0001", false).await?,
                Some(erste.clone())
            );
            assert_eq!(uow.questions().by_id("ARC-F0404", false).await?, None);
            // Sortiert nach Projekt und Nummer
            assert_eq!(
                uow.questions()
                    .list(&BoardQuestionFilter::default())
                    .await?,
                [erste.clone(), zweite.clone()]
            );
            // Filter nach Ticket, Zustand und ID
            assert_eq!(
                uow.questions()
                    .list(&BoardQuestionFilter {
                        ticket_ids: Some(vec!["ARC-0001".to_string()]),
                        ..BoardQuestionFilter::default()
                    })
                    .await?,
                vec![erste.clone()]
            );
            assert_eq!(
                uow.questions()
                    .list(&BoardQuestionFilter {
                        states: Some(vec![QuestionState::Answered]),
                        ..BoardQuestionFilter::default()
                    })
                    .await?,
                Vec::new()
            );
            assert_eq!(
                uow.questions()
                    .list(&BoardQuestionFilter {
                        ids: Some(vec!["ARC-F0002".to_string()]),
                        project_id: Some(1),
                        ..BoardQuestionFilter::default()
                    })
                    .await?,
                vec![zweite.clone()]
            );

            // Ändern schreibt die Kindtabellen neu und verlangt die Vorgängerversion
            let entschieden = BoardQuestion {
                state: QuestionState::Answered,
                tags: vec!["cache".to_string()],
                ticket_ids: vec!["ARC-0002".to_string()],
                depends_on: Vec::new(),
                decision: Some(BoardQuestionDecision {
                    outcome: BoardQuestionDecisionOutcome::Answered,
                    option: Some("O1".to_string()),
                    text: Some("So machen wir es".to_string()),
                    reason: "weil".to_string(),
                    by_id: test_db::USER_ID.to_string(),
                    by_name: "Test".to_string(),
                    at: "2026-02-01T12:00:00.000Z".to_string(),
                }),
                updated_at: "2026-02-01T12:00:00.000Z".to_string(),
                rev: 2,
                ..erste.clone()
            };
            uow.questions().update(&entschieden).await?;
            assert_eq!(
                uow.questions().by_id("ARC-F0001", true).await?,
                Some(entschieden.clone())
            );
            // Ein zweiter Schreibversuch mit derselben Rev ist ein Konflikt
            let fehler = uow.questions().update(&entschieden).await.unwrap_err();
            assert_eq!(fehler.code(), Some("conflict"));

            // Bezüge auf ein Ticket lesen und entfernen
            assert_eq!(
                uow.questions().ids_referencing_ticket("ARC-0002").await?,
                ["ARC-F0001"]
            );
            assert_eq!(
                uow.questions().remove_ticket("ARC-0002").await?,
                ["ARC-F0001"]
            );
            assert!(uow
                .questions()
                .ids_referencing_ticket("ARC-0002")
                .await?
                .is_empty());
            let danach = uow
                .questions()
                .by_id("ARC-F0001", false)
                .await?
                .expect("Frage");
            assert!(danach.ticket_ids.is_empty());
            // Die Frage selbst bleibt unverändert
            assert_eq!(danach.rev, entschieden.rev);
            Ok(())
        })
    })
    .await
    .unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
async fn schreibt_und_liest_recherchen_kommentare_und_verlauf_einer_frage() {
    let Some((db, _board)) =
        datenbank("schreibt_und_liest_recherchen_kommentare_und_verlauf_einer_frage").await
    else {
        return;
    };
    db.transaction(|uow| -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            uow.questions().insert(&frage(1, "ARC-F0001", 1)).await?;
            uow.questions()
                .insert(&BoardQuestion {
                    tags: Vec::new(),
                    ticket_ids: Vec::new(),
                    depends_on: Vec::new(),
                    ..frage(1, "ARC-F0002", 2)
                })
                .await?;

            let option = |id: &str, empfohlen: bool| BoardQuestionOption {
                id: id.to_string(),
                title: format!("Vorschlag {id}"),
                text: "Text".to_string(),
                pros: vec!["schnell".to_string()],
                cons: Vec::new(),
                recommended: empfohlen,
            };
            let neu = |options: Vec<BoardQuestionOption>| NewQuestionResearch {
                question_id: "ARC-F0001".to_string(),
                at: TEST_NOW.to_string(),
                by_id: test_db::USER_ID.to_string(),
                by_name: "Test".to_string(),
                kind: AuthorKind::Subagent,
                context: "Kontext".to_string(),
                options,
            };
            // Optionen kommen nach ihrer Nummer, nicht nach dem Text (O2 vor O10)
            let erste = uow
                .question_research()
                .insert(&neu(vec![option("O1", false), option("O2", true)]))
                .await?;
            let zweite = uow
                .question_research()
                .insert(&neu(vec![option("O10", false)]))
                .await?;
            assert!(erste.id < zweite.id);

            let gelesen = uow.question_research().list("ARC-F0001").await?;
            assert_eq!(gelesen, [erste.clone(), zweite.clone()]);
            assert_eq!(gelesen[0].options[1].id, "O2");
            assert!(gelesen[0].options[1].recommended);
            assert_eq!(gelesen[0].options[0].pros, ["schnell"]);
            assert!(uow.question_research().list("ARC-F0002").await?.is_empty());

            // Die Übersicht zählt alle Recherchen und die Optionen der neuesten
            let ubersicht = uow
                .question_research()
                .summaries(&["ARC-F0001".to_string(), "ARC-F0002".to_string()])
                .await?;
            let eintrag = ubersicht.get("ARC-F0001").expect("Übersicht");
            assert_eq!(eintrag.count, 2);
            assert_eq!(eintrag.option_count, 1);
            assert!(!ubersicht.contains_key("ARC-F0002"));
            assert!(uow.question_research().summaries(&[]).await?.is_empty());

            // Kommentare an der Frage, nur anhängen
            let kommentar = |body: &str| NewQuestionComment {
                question_id: "ARC-F0001".to_string(),
                author_id: test_db::USER_ID.to_string(),
                author_name: "Test".to_string(),
                author_kind: AuthorKind::Human,
                body: body.to_string(),
                created_at: TEST_NOW.to_string(),
            };
            let eins = uow.question_comments().insert(&kommentar("eins")).await?;
            let zwei = uow.question_comments().insert(&kommentar("zwei")).await?;
            assert!(eins.id < zwei.id);
            assert_eq!(
                uow.question_comments().list("ARC-F0001").await?,
                [eins, zwei]
            );
            assert!(uow.question_comments().list("ARC-F0002").await?.is_empty());

            // Verlauf der Frage, nur anhängen
            let verlauf = |art: BoardQuestionHistoryType, kind: Option<AuthorKind>, text: &str| {
                BoardQuestionHistoryEntry {
                    question_id: "ARC-F0001".to_string(),
                    at: TEST_NOW.to_string(),
                    by_id: test_db::USER_ID.to_string(),
                    by_name: "Test".to_string(),
                    by_kind: kind,
                    entry_type: art,
                    text: text.to_string(),
                }
            };
            let eintrage = [
                verlauf(
                    BoardQuestionHistoryType::Created,
                    Some(AuthorKind::Session),
                    "angelegt",
                ),
                verlauf(BoardQuestionHistoryType::TicketDeleted, None, "ARC-0002"),
            ];
            uow.question_history().insert(&eintrage).await?;
            assert_eq!(uow.question_history().list("ARC-F0001").await?, eintrage);
            assert!(uow.question_history().list("ARC-F0002").await?.is_empty());
            Ok(())
        })
    })
    .await
    .unwrap();
    db.close().await.unwrap();
}
