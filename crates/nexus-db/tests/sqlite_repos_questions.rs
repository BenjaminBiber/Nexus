//! Die vier Repositories der Fragen auf SQLite; portiert aus
//! `packages/db/tests/sqlite/SqliteQuestionRepository.test.ts`,
//! `SqliteQuestionCommentRepository.test.ts`, `SqliteQuestionHistoryRepository.test.ts` und
//! `SqliteQuestionResearchRepository.test.ts`.

mod common;

use common::temp_sqlite::{SeedTicket, TempSqlite, TEST_NOW, TEST_USER_ID};

use nexus_core::actors::AuthorKind;
use nexus_core::board::database::{
    BoardDatabase, BoardQuestionFilter, NewQuestionComment, NewQuestionResearch,
};
use nexus_core::board::question_types::{
    BoardQuestion, BoardQuestionComment, BoardQuestionDecision, BoardQuestionDecisionOutcome,
    BoardQuestionHistoryEntry, BoardQuestionHistoryType, BoardQuestionOption,
    BoardQuestionResearch, BoardQuestionResearchSummary,
};
use nexus_core::questions::QuestionState;
use nexus_core::Result;
use nexus_db::sqlite_params;

/// Ein zweiter Zeitpunkt, eine Stunde nach [`TEST_NOW`]
const LATER: &str = "2026-10-13T11:00:00.000Z";

/// Grundgerüst einer Frage; die Tests passen einzelne Felder danach an
fn question(project_id: i32) -> BoardQuestion {
    BoardQuestion {
        id: "ARC-F0001".into(),
        project_id,
        number: 1,
        title: "Welcher Speicher?".into(),
        description: String::new(),
        state: QuestionState::Open,
        occasion: None,
        origin_kind: AuthorKind::Session,
        origin_name: "claude".into(),
        origin_session: None,
        tags: Vec::new(),
        ticket_ids: Vec::new(),
        depends_on: Vec::new(),
        decision: None,
        created_at: TEST_NOW.into(),
        created_by_id: TEST_USER_ID.into(),
        updated_at: TEST_NOW.into(),
        rev: 1,
    }
}

/// Eine Option `O<n>` mit festen Texten
fn option(id: &str, recommended: bool) -> BoardQuestionOption {
    BoardQuestionOption {
        id: id.into(),
        title: format!("Variante {id}"),
        text: "Text äöü".into(),
        pros: vec!["schnell".into()],
        cons: Vec::new(),
        recommended,
    }
}

/// Texte der Liste in Lesereihenfolge
fn ids(questions: &[BoardQuestion]) -> Vec<String> {
    questions.iter().map(|q| q.id.clone()).collect()
}

/// Legt eine Frage mit Mindestfeldern über rohes SQL an (ohne das Repository)
async fn seed_question(temp: &TempSqlite, id: &str, project_id: i32, number: i32) -> Result<()> {
    temp.execute(
        "INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, CreatedAt, CreatedById, UpdatedAt, Rev)
         VALUES (@id, @projectId, @number, @title, 'open', 'session', 'claude', @now, @userId, @now, 1)",
        sqlite_params! {
            "id": id,
            "projectId": project_id,
            "number": number,
            "title": format!("Frage {number}"),
            "now": TEST_NOW,
            "userId": TEST_USER_ID,
        },
    )
    .await?;
    Ok(())
}

/// Legt die Frage über das Repository an
async fn insert(temp: &TempSqlite, question: &BoardQuestion) -> Result<()> {
    let question = question.clone();
    temp.db()
        .transaction(move |u| {
            let question = question.clone();
            Box::pin(async move { u.questions().insert(&question).await })
        })
        .await
}

/// Schreibt die Frage über das Repository
async fn update(temp: &TempSqlite, question: &BoardQuestion) -> Result<()> {
    let question = question.clone();
    temp.db()
        .transaction(move |u| {
            let question = question.clone();
            Box::pin(async move { u.questions().update(&question).await })
        })
        .await
}

/// Liest die Frage über das Repository
async fn by_id(temp: &TempSqlite, id: &str) -> Result<Option<BoardQuestion>> {
    let id = id.to_string();
    temp.db()
        .transaction(move |u| {
            let id = id.clone();
            Box::pin(async move { u.questions().by_id(&id, false).await })
        })
        .await
}

/// Liest die Fragen des Filters über das Repository
async fn list(temp: &TempSqlite, filter: BoardQuestionFilter) -> Result<Vec<BoardQuestion>> {
    temp.db()
        .transaction(move |u| {
            let filter = filter.clone();
            Box::pin(async move { u.questions().list(&filter).await })
        })
        .await
}

/// Temp-Datei mit Projekt `ARC`, zwei Tickets und einem zweiten Projekt `OTH`
async fn board_mit_tickets() -> (TempSqlite, i32, i32) {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Board").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-0001", &board, 1))
        .await
        .expect("Ticket 1");
    temp.seed_ticket(&SeedTicket::new("ARC-0002", &board, 2))
        .await
        .expect("Ticket 2");
    let other = temp.seed_project("OTH").await.expect("zweites Projekt");
    (temp, board.project_id, other)
}

// ---------------------------------------------------------------------------------------------
// Fragen
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn legt_eine_frage_mit_tags_tickets_und_abhangigkeiten_an() {
    let (temp, project_id, _) = board_mit_tickets().await;
    let mut q = question(project_id);
    q.tags = vec!["db".into(), "api".into()];
    q.ticket_ids = vec!["ARC-0002".into(), "ARC-0001".into()];
    q.depends_on = vec!["ARC-F0002".into(), "ARC-0001".into()];

    insert(&temp, &q).await.expect("anlegen");

    let mut erwartet = q.clone();
    erwartet.ticket_ids = vec!["ARC-0001".into(), "ARC-0002".into()];
    assert_eq!(
        by_id(&temp, "ARC-F0001").await.expect("lesen"),
        Some(erwartet.clone())
    );
    // forUpdate ist ein No-op und liefert dasselbe
    let gesperrt = temp
        .db()
        .transaction(|u| Box::pin(async move { u.questions().by_id("ARC-F0001", true).await }))
        .await
        .expect("mit Sperre lesen");
    assert_eq!(gesperrt, Some(erwartet));
    assert_eq!(by_id(&temp, "ARC-F0099").await.expect("lesen"), None);
    temp.dispose().await;
}

#[tokio::test]
async fn beantwortet_eine_frage_und_ersetzt_die_tags() {
    let (temp, project_id, _) = board_mit_tickets().await;
    let mut q = question(project_id);
    q.tags = vec!["db".into(), "api".into()];
    insert(&temp, &q).await.expect("anlegen");

    let mut beantwortet = question(project_id);
    beantwortet.rev = 2;
    beantwortet.state = QuestionState::Answered;
    beantwortet.tags = vec!["db".into()];
    beantwortet.updated_at = LATER.into();
    beantwortet.decision = Some(BoardQuestionDecision {
        outcome: BoardQuestionDecisionOutcome::Answered,
        option: Some("O1".into()),
        text: Some("SQL Server".into()),
        reason: "Lizenz".into(),
        by_id: TEST_USER_ID.into(),
        by_name: "Florian".into(),
        at: LATER.into(),
    });

    update(&temp, &beantwortet).await.expect("schreiben");

    assert_eq!(
        by_id(&temp, "ARC-F0001").await.expect("lesen"),
        Some(beantwortet)
    );
    temp.dispose().await;
}

#[tokio::test]
async fn verwirft_eine_frage_und_offnet_sie_wieder() {
    let (temp, project_id, _) = board_mit_tickets().await;
    insert(&temp, &question(project_id)).await.expect("anlegen");

    let mut verworfen = question(project_id);
    verworfen.rev = 2;
    verworfen.state = QuestionState::Discarded;
    verworfen.updated_at = LATER.into();
    verworfen.decision = Some(BoardQuestionDecision {
        outcome: BoardQuestionDecisionOutcome::Discarded,
        option: None,
        text: None,
        reason: "überholt".into(),
        by_id: TEST_USER_ID.into(),
        by_name: "Florian".into(),
        at: LATER.into(),
    });
    update(&temp, &verworfen).await.expect("verwerfen");

    let gelesen = by_id(&temp, "ARC-F0001")
        .await
        .expect("lesen")
        .expect("Frage");
    let entscheidung = gelesen.decision.expect("Entscheidung");
    assert_eq!(
        entscheidung.outcome,
        BoardQuestionDecisionOutcome::Discarded
    );
    assert_eq!(entscheidung.text, None);

    let mut wieder_offen = question(project_id);
    wieder_offen.rev = 3;
    wieder_offen.updated_at = LATER.into();
    update(&temp, &wieder_offen).await.expect("wieder öffnen");

    let gelesen = by_id(&temp, "ARC-F0001")
        .await
        .expect("lesen")
        .expect("Frage");
    assert_eq!(gelesen.decision, None);
    assert_eq!(gelesen.rev, 3);
    temp.dispose().await;
}

#[tokio::test]
async fn meldet_einen_rev_konflikt_und_andert_nichts() {
    let (temp, project_id, _) = board_mit_tickets().await;
    insert(&temp, &question(project_id)).await.expect("anlegen");
    let mut veraltet = question(project_id);
    veraltet.rev = 3;
    veraltet.title = "Neu".into();

    let fehler = update(&temp, &veraltet).await.expect_err("Konflikt");
    assert_eq!(fehler.code(), Some("conflict"));

    let unverandert = by_id(&temp, "ARC-F0001")
        .await
        .expect("lesen")
        .expect("Frage");
    assert_eq!(unverandert.rev, 1);
    assert_eq!(unverandert.title, "Welcher Speicher?");
    temp.dispose().await;
}

#[tokio::test]
async fn filtert_fragen() {
    let (temp, project_id, other_project_id) = board_mit_tickets().await;
    let mut erste = question(project_id);
    erste.ticket_ids = vec!["ARC-0001".into()];
    insert(&temp, &erste).await.expect("erste");

    let mut zweite = question(project_id);
    zweite.id = "ARC-F0002".into();
    zweite.number = 2;
    zweite.state = QuestionState::Answered;
    zweite.decision = Some(BoardQuestionDecision {
        outcome: BoardQuestionDecisionOutcome::Answered,
        option: None,
        text: Some("ja".into()),
        reason: String::new(),
        by_id: TEST_USER_ID.into(),
        by_name: "Florian".into(),
        at: LATER.into(),
    });
    insert(&temp, &zweite).await.expect("zweite");

    let mut fremde = question(other_project_id);
    fremde.id = "OTH-F0001".into();
    insert(&temp, &fremde).await.expect("fremde");

    let alle = list(&temp, BoardQuestionFilter::default())
        .await
        .expect("alle");
    assert_eq!(ids(&alle), ["ARC-F0001", "ARC-F0002", "OTH-F0001"]);

    let im_projekt = list(
        &temp,
        BoardQuestionFilter {
            project_id: Some(project_id),
            ..Default::default()
        },
    )
    .await
    .expect("Projekt");
    assert_eq!(ids(&im_projekt), ["ARC-F0001", "ARC-F0002"]);

    let offen = list(
        &temp,
        BoardQuestionFilter {
            states: Some(vec![QuestionState::Open]),
            ..Default::default()
        },
    )
    .await
    .expect("Zustand");
    assert_eq!(ids(&offen), ["ARC-F0001", "OTH-F0001"]);

    let zum_ticket = list(
        &temp,
        BoardQuestionFilter {
            ticket_ids: Some(vec!["ARC-0001".into()]),
            ..Default::default()
        },
    )
    .await
    .expect("Ticket");
    assert_eq!(ids(&zum_ticket), ["ARC-F0001"]);

    // Leere Listen ergeben keine Fragen
    for filter in [
        BoardQuestionFilter {
            ids: Some(Vec::new()),
            ..Default::default()
        },
        BoardQuestionFilter {
            states: Some(Vec::new()),
            ..Default::default()
        },
        BoardQuestionFilter {
            ticket_ids: Some(Vec::new()),
            ..Default::default()
        },
    ] {
        assert!(list(&temp, filter).await.expect("leer").is_empty());
    }

    // Kriterien werden mit „und“ verknüpft
    let kombiniert = list(
        &temp,
        BoardQuestionFilter {
            project_id: Some(project_id),
            ids: Some(vec!["ARC-F0002".into(), "OTH-F0001".into()]),
            ..Default::default()
        },
    )
    .await
    .expect("kombiniert");
    assert_eq!(ids(&kombiniert), ["ARC-F0002"]);
    temp.dispose().await;
}

#[tokio::test]
async fn findet_und_lost_bezuge_auf_ein_ticket() {
    let (temp, project_id, _) = board_mit_tickets().await;
    let mut erste = question(project_id);
    erste.ticket_ids = vec!["ARC-0001".into(), "ARC-0002".into()];
    insert(&temp, &erste).await.expect("erste");
    let mut zweite = question(project_id);
    zweite.id = "ARC-F0002".into();
    zweite.number = 2;
    zweite.depends_on = vec!["ARC-0001".into(), "ARC-0002".into()];
    insert(&temp, &zweite).await.expect("zweite");

    let referenzen = temp
        .db()
        .transaction(|u| {
            Box::pin(async move { u.questions().ids_referencing_ticket("ARC-0001").await })
        })
        .await
        .expect("Bezüge");
    assert_eq!(referenzen, ["ARC-F0001", "ARC-F0002"]);

    let gelost = temp
        .db()
        .transaction(|u| Box::pin(async move { u.questions().remove_ticket("ARC-0001").await }))
        .await
        .expect("lösen");
    assert_eq!(gelost, ["ARC-F0001", "ARC-F0002"]);

    let danach = temp
        .db()
        .transaction(|u| {
            Box::pin(async move { u.questions().ids_referencing_ticket("ARC-0001").await })
        })
        .await
        .expect("Bezüge danach");
    assert!(danach.is_empty());

    // Die Fragen selbst bleiben unverändert (Rev erhöht der Dienst)
    let erste = by_id(&temp, "ARC-F0001")
        .await
        .expect("lesen")
        .expect("Frage");
    assert_eq!(erste.ticket_ids, ["ARC-0002"]);
    assert_eq!(erste.rev, 1);
    let zweite = by_id(&temp, "ARC-F0002")
        .await
        .expect("lesen")
        .expect("Frage");
    assert_eq!(zweite.depends_on, ["ARC-0002"]);
    assert_eq!(zweite.rev, 1);
    temp.dispose().await;
}

#[tokio::test]
async fn speichert_guids_kleingeschrieben() {
    let (temp, project_id, _) = board_mit_tickets().await;
    let mut q = question(project_id);
    q.id = "ARC-F0003".into();
    q.number = 3;
    q.created_by_id = TEST_USER_ID.to_uppercase();
    insert(&temp, &q).await.expect("anlegen");

    let gelesen = by_id(&temp, "ARC-F0003")
        .await
        .expect("lesen")
        .expect("Frage");
    assert_eq!(gelesen.created_by_id, TEST_USER_ID);
    temp.dispose().await;
}

// ---------------------------------------------------------------------------------------------
// Kommentare an Fragen
// ---------------------------------------------------------------------------------------------

/// Temp-Datei mit Projekt `ARC` und zwei Fragen (roh angelegt)
async fn zwei_fragen() -> TempSqlite {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let project_id = temp.seed_project("ARC").await.expect("Projekt");
    seed_question(&temp, "ARC-F0001", project_id, 1)
        .await
        .expect("erste Frage");
    seed_question(&temp, "ARC-F0002", project_id, 2)
        .await
        .expect("zweite Frage");
    temp
}

/// Legt einen Kommentar über das Repository an
async fn insert_comment(
    temp: &TempSqlite,
    comment: NewQuestionComment,
) -> Result<BoardQuestionComment> {
    temp.db()
        .transaction(move |u| {
            let comment = comment.clone();
            Box::pin(async move { u.question_comments().insert(&comment).await })
        })
        .await
}

/// Liest die Kommentare einer Frage über das Repository
async fn list_comments(temp: &TempSqlite, question_id: &str) -> Result<Vec<BoardQuestionComment>> {
    let question_id = question_id.to_string();
    temp.db()
        .transaction(move |u| {
            let question_id = question_id.clone();
            Box::pin(async move { u.question_comments().list(&question_id).await })
        })
        .await
}

/// Ein Kommentar an `ARC-F0001` mit festen Angaben
fn comment(body: &str, kind: AuthorKind, created_at: &str) -> NewQuestionComment {
    NewQuestionComment {
        question_id: "ARC-F0001".into(),
        author_id: TEST_USER_ID.into(),
        author_name: "Florian".into(),
        author_kind: kind,
        body: body.into(),
        created_at: created_at.into(),
    }
}

#[tokio::test]
async fn legt_kommentare_an_und_liest_sie() {
    let temp = zwei_fragen().await;

    let c1 = insert_comment(
        &temp,
        comment("Kommentar zur Lizenz äöü", AuthorKind::Human, TEST_NOW),
    )
    .await
    .expect("erster Kommentar");
    let c2 = insert_comment(
        &temp,
        comment(
            "Noch einer",
            AuthorKind::Session,
            "2026-10-13T10:01:00.000Z",
        ),
    )
    .await
    .expect("zweiter Kommentar");

    assert!(c1.id > 0);
    assert!(c2.id > c1.id);
    assert_eq!(
        list_comments(&temp, "ARC-F0001").await.expect("lesen"),
        [c1, c2]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn listet_nur_kommentare_der_eigenen_frage() {
    let temp = zwei_fragen().await;
    let mut fremder = comment("Gehört zur zweiten Frage", AuthorKind::Human, TEST_NOW);
    fremder.question_id = "ARC-F0002".into();
    insert_comment(&temp, fremder).await.expect("Kommentar");

    assert!(list_comments(&temp, "ARC-F0001")
        .await
        .expect("lesen")
        .is_empty());
    assert!(list_comments(&temp, "ARC-F0099")
        .await
        .expect("lesen")
        .is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn normiert_guid_und_datum_eines_kommentars() {
    let temp = zwei_fragen().await;
    let mut gross = comment(
        "Groß geschrieben",
        AuthorKind::Human,
        "2026-10-13T10:00:00Z",
    );
    gross.author_id = TEST_USER_ID.to_uppercase();
    insert_comment(&temp, gross).await.expect("Kommentar");

    let gelesen = list_comments(&temp, "ARC-F0001").await.expect("lesen");
    assert_eq!(gelesen[0].author_id, TEST_USER_ID);
    assert_eq!(gelesen[0].created_at, TEST_NOW);
    temp.dispose().await;
}

// ---------------------------------------------------------------------------------------------
// Verlauf der Fragen
// ---------------------------------------------------------------------------------------------

/// Ein Verlaufseintrag an `ARC-F0001`
fn history_entry() -> BoardQuestionHistoryEntry {
    BoardQuestionHistoryEntry {
        question_id: "ARC-F0001".into(),
        at: TEST_NOW.into(),
        by_id: TEST_USER_ID.into(),
        by_name: "claude".into(),
        by_kind: Some(AuthorKind::Session),
        entry_type: BoardQuestionHistoryType::Created,
        text: "Frage angelegt".into(),
    }
}

/// Schreibt Verlaufseinträge über das Repository
async fn insert_history(temp: &TempSqlite, entries: Vec<BoardQuestionHistoryEntry>) -> Result<()> {
    temp.db()
        .transaction(move |u| {
            let entries = entries.clone();
            Box::pin(async move { u.question_history().insert(&entries).await })
        })
        .await
}

/// Liest den Verlauf einer Frage über das Repository
async fn list_history(
    temp: &TempSqlite,
    question_id: &str,
) -> Result<Vec<BoardQuestionHistoryEntry>> {
    let question_id = question_id.to_string();
    temp.db()
        .transaction(move |u| {
            let question_id = question_id.clone();
            Box::pin(async move { u.question_history().list(&question_id).await })
        })
        .await
}

#[tokio::test]
async fn schreibt_und_liest_den_verlauf_in_reihenfolge() {
    let temp = zwei_fragen().await;
    let h1 = history_entry();
    let h2 = BoardQuestionHistoryEntry {
        at: "2026-10-13T10:05:00.000Z".into(),
        by_name: "Florian".into(),
        by_kind: Some(AuthorKind::Human),
        entry_type: BoardQuestionHistoryType::Decided,
        text: "Beantwortet: SQL Server – Lizenz äöü".into(),
        ..h1.clone()
    };
    insert_history(&temp, vec![h1.clone(), h2.clone()])
        .await
        .expect("zwei Einträge");

    // Ein Eintrag ohne eigentlichen Autor (Ticket gelöscht)
    let h3 = BoardQuestionHistoryEntry {
        at: "2026-10-13T10:10:00.000Z".into(),
        by_kind: None,
        entry_type: BoardQuestionHistoryType::TicketDeleted,
        text: "ARC-0001 gelöscht".into(),
        ..h1.clone()
    };
    insert_history(&temp, vec![h3.clone()])
        .await
        .expect("dritter Eintrag");

    assert_eq!(
        list_history(&temp, "ARC-F0001").await.expect("lesen"),
        [h1, h2, h3]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_nur_verlaufseintrage_der_eigenen_frage() {
    let temp = zwei_fragen().await;
    let h1 = history_entry();
    insert_history(&temp, vec![h1.clone()])
        .await
        .expect("erste");
    insert_history(
        &temp,
        vec![BoardQuestionHistoryEntry {
            question_id: "ARC-F0002".into(),
            text: "Andere Frage".into(),
            ..h1.clone()
        }],
    )
    .await
    .expect("zweite");
    insert_history(&temp, Vec::new())
        .await
        .expect("leere Liste");

    assert_eq!(list_history(&temp, "ARC-F0001").await.expect("lesen"), [h1]);
    temp.dispose().await;
}

#[tokio::test]
async fn normiert_guid_und_datum_eines_verlaufseintrags() {
    let temp = zwei_fragen().await;
    insert_history(
        &temp,
        vec![BoardQuestionHistoryEntry {
            by_id: TEST_USER_ID.to_uppercase(),
            at: "2026-10-13T10:00:00Z".into(),
            ..history_entry()
        }],
    )
    .await
    .expect("Eintrag");

    let gelesen = list_history(&temp, "ARC-F0001").await.expect("lesen");
    assert_eq!(gelesen[0].by_id, TEST_USER_ID);
    assert_eq!(gelesen[0].at, TEST_NOW);
    temp.dispose().await;
}

// ---------------------------------------------------------------------------------------------
// Recherchen
// ---------------------------------------------------------------------------------------------

/// Legt eine Recherche über das Repository an
async fn insert_research(
    temp: &TempSqlite,
    research: NewQuestionResearch,
) -> Result<BoardQuestionResearch> {
    temp.db()
        .transaction(move |u| {
            let research = research.clone();
            Box::pin(async move { u.question_research().insert(&research).await })
        })
        .await
}

/// Liest die Recherchen einer Frage über das Repository
async fn list_research(temp: &TempSqlite, question_id: &str) -> Result<Vec<BoardQuestionResearch>> {
    let question_id = question_id.to_string();
    temp.db()
        .transaction(move |u| {
            let question_id = question_id.clone();
            Box::pin(async move { u.question_research().list(&question_id).await })
        })
        .await
}

/// Eine Recherche an `ARC-F0001`
fn research(
    at: &str,
    kind: AuthorKind,
    context: &str,
    options: Vec<BoardQuestionOption>,
) -> NewQuestionResearch {
    NewQuestionResearch {
        question_id: "ARC-F0001".into(),
        at: at.into(),
        by_id: TEST_USER_ID.into(),
        by_name: "claude".into(),
        kind,
        context: context.into(),
        options,
    }
}

#[tokio::test]
async fn speichert_mit_optionen_und_liest_in_optionsreihenfolge() {
    let temp = zwei_fragen().await;

    let r1 = insert_research(
        &temp,
        research(
            TEST_NOW,
            AuthorKind::Session,
            "Kontext",
            vec![option("O1", true), option("O2", false)],
        ),
    )
    .await
    .expect("erste Recherche");
    let r2 = insert_research(
        &temp,
        research(
            LATER,
            AuthorKind::Subagent,
            "",
            vec![option("O10", false), option("O3", false)],
        ),
    )
    .await
    .expect("zweite Recherche");
    assert!(r2.id > r1.id);

    let gelesen = list_research(&temp, "ARC-F0001").await.expect("lesen");
    // Optionen nach Nummer: O3 vor O10
    let zweite = BoardQuestionResearch {
        options: vec![option("O3", false), option("O10", false)],
        ..r2
    };
    assert_eq!(gelesen, [r1, zweite]);
    assert!(gelesen[0].options[0].recommended);
    assert!(!gelesen[0].options[1].recommended);
    assert_eq!(gelesen[0].options[0].pros, ["schnell"]);
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_eine_leere_liste_ohne_recherchen() {
    let temp = zwei_fragen().await;

    assert!(list_research(&temp, "ARC-F0002")
        .await
        .expect("lesen")
        .is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn fasst_zahl_der_recherchen_und_optionen_der_neuesten_je_frage_zusammen() {
    let temp = zwei_fragen().await;
    insert_research(
        &temp,
        research(
            TEST_NOW,
            AuthorKind::Session,
            "Kontext",
            vec![option("O1", true), option("O2", false)],
        ),
    )
    .await
    .expect("erste");
    insert_research(
        &temp,
        research(
            LATER,
            AuthorKind::Subagent,
            "",
            vec![option("O10", false), option("O3", false)],
        ),
    )
    .await
    .expect("zweite");
    let mut zur_zweiten_frage = research(TEST_NOW, AuthorKind::Session, "", Vec::new());
    zur_zweiten_frage.question_id = "ARC-F0002".into();
    insert_research(&temp, zur_zweiten_frage)
        .await
        .expect("dritte");

    let fragen = vec![
        "ARC-F0001".to_string(),
        "ARC-F0002".to_string(),
        "ARC-F0099".to_string(),
    ];
    let zusammenfassung = temp
        .db()
        .transaction(move |u| {
            let fragen = fragen.clone();
            Box::pin(async move { u.question_research().summaries(&fragen).await })
        })
        .await
        .expect("Zusammenfassung");
    // Fragen ohne Recherche fehlen (kein Eintrag mit 0)
    assert_eq!(
        zusammenfassung.into_iter().collect::<Vec<_>>(),
        [
            (
                "ARC-F0001".to_string(),
                BoardQuestionResearchSummary {
                    count: 2,
                    option_count: 2
                }
            ),
            (
                "ARC-F0002".to_string(),
                BoardQuestionResearchSummary {
                    count: 1,
                    option_count: 0
                }
            ),
        ]
    );

    let ohne_fragen = temp
        .db()
        .transaction(|u| Box::pin(async move { u.question_research().summaries(&[]).await }))
        .await
        .expect("leere Liste");
    assert!(ohne_fragen.is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn normiert_guid_und_datum_einer_recherche() {
    let temp = zwei_fragen().await;
    let mut gross = research("2026-10-13T10:00:00Z", AuthorKind::Session, "", Vec::new());
    gross.by_id = TEST_USER_ID.to_uppercase();
    insert_research(&temp, gross).await.expect("Recherche");

    let gelesen = list_research(&temp, "ARC-F0001").await.expect("lesen");
    assert_eq!(gelesen[0].by_id, TEST_USER_ID);
    assert_eq!(gelesen[0].at, TEST_NOW);
    temp.dispose().await;
}
