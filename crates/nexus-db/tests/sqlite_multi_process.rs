//! Mehrere Verbindungen auf derselben SQLite-Datei; portiert aus
//! `packages/db/tests/sqlite/SqliteMultiProcess.test.ts`.
//!
//! In TypeScript startet der Test zwei Kindprozesse. In Rust übernehmen das zwei Aufgaben mit
//! je **einer eigenen** [`SqliteBoardDatabase`] auf derselben Datei: dieselben Transaktionen,
//! dieselben Sperren, nur ohne den Umweg über einen zweiten Prozess. Dass auch zwei echte

mod common;

use common::temp_sqlite::{SeededBoard, TempSqlite, TEST_NOW, TEST_USER_ID, TEST_USER_NAME};

use nexus_core::board::basics::{Priority, TicketStatus};
use nexus_core::board::database::{BoardDatabase, TicketFilter};
use nexus_core::board::types::Ticket;
use nexus_core::tx;
use nexus_db::sqlite::SqliteBoardDatabase;

/// Ein Ticket mit Mindestangaben, wie der Dienst es anlegt
fn ticket_fur(board: &SeededBoard, nummer: i32, titel: &str) -> Ticket {
    Ticket {
        id: format!("ARC-{nummer:04}"),
        project_id: board.project_id,
        board_id: board.board_id,
        number: nummer,
        title: titel.to_string(),
        description: String::new(),
        status: TicketStatus::Backlog,
        priority: Priority::Normal,
        labels: Vec::new(),
        depends_on: Vec::new(),
        sort_order: nummer,
        assignee: None,
        needs_planning: true,
        acceptance: String::new(),
        plan: String::new(),
        handbook: String::new(),
        diagram: String::new(),
        spec: None,
        spec_task: None,
        branch: None,
        worktree: None,
        target_branch: None,
        touches: Vec::new(),
        consumes: Vec::new(),
        produces: Vec::new(),
        blocked_reason: None,
        planned_at: None,
        planned_by: None,
        planning_claim_by: None,
        planning_claim_at: None,
        created_at: TEST_NOW.into(),
        created_by_id: TEST_USER_ID.into(),
        created_by_name: TEST_USER_NAME.into(),
        updated_at: TEST_NOW.into(),
        rev: 1,
    }
}

/// Code eines Fehlers: Harness-Code oder „unknown: <Meldung>“
fn code_of(error: nexus_core::Error) -> String {
    error
        .code()
        .map(str::to_string)
        .unwrap_or_else(|| format!("unknown: {error}"))
}

/// Legt über eine eigene Verbindung `count` Tickets an, jedes in einer eigenen Transaktion
async fn create(
    file: String,
    board: SeededBoard,
    count: i32,
    prefix: &str,
) -> (Vec<String>, Vec<String>) {
    let db = SqliteBoardDatabase::open(&file).expect("eigene Verbindung");
    let mut ids = Vec::new();
    let mut errors = Vec::new();
    for i in 1..=count {
        let titel = format!("{prefix} {i}");
        let ergebnis = tx!(&db, [titel, board], |u| {
            let nummer = u.projects().take_ticket_number(board.project_id).await?;
            let ticket = ticket_fur(&board, nummer, &titel);
            u.tickets().insert(&ticket).await?;
            Ok(ticket.id)
        })
        .await;
        match ergebnis {
            Ok(id) => ids.push(id),
            Err(error) => errors.push(code_of(error)),
        }
    }
    db.close().await.expect("schließen");
    (ids, errors)
}

/// Ändert über eine eigene Verbindung den Titel; `rev` ist die erwartete aktuelle Rev.
/// Ergebnis ist `ok` oder der Fehlercode
async fn update(file: String, id: String, rev: i32, titel: String) -> String {
    let db = SqliteBoardDatabase::open(&file).expect("eigene Verbindung");
    let ergebnis = tx!(&db, [id, titel], |u| {
        let ticket =
            u.tickets().by_id(&id, true).await?.ok_or_else(|| {
                nexus_core::Error::unexpected_message(format!("Ticket {id} fehlt"))
            })?;
        u.tickets()
            .update(&Ticket {
                title: titel.clone(),
                rev: rev + 1,
                ..ticket
            })
            .await
    })
    .await;
    db.close().await.expect("schließen");
    match ergebnis {
        Ok(()) => "ok".to_string(),
        Err(error) => code_of(error),
    }
}

/// Benutzer, Projekt ARC und ein Board – die Grundlage beider Fälle
async fn grundlage(temp: &TempSqlite) -> SeededBoard {
    temp.seed_board(None, "Sprint").await.expect("Board")
}

#[tokio::test]
async fn legt_aus_zwei_verbindungen_je_funfzig_tickets_ohne_lucke_an() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = grundlage(&temp).await;
    let file = temp.file().to_string();

    let (a, b) = tokio::join!(
        create(file.clone(), board.clone(), 50, "A"),
        create(file.clone(), board.clone(), 50, "B"),
    );

    assert_eq!(a.1, Vec::<String>::new());
    assert_eq!(b.1, Vec::<String>::new());
    assert_eq!(a.0.len(), 50);
    assert_eq!(b.0.len(), 50);

    let tickets = tx!(temp.db(), |u| {
        u.tickets().list(&TicketFilter::default()).await
    })
    .await
    .expect("Tickets");
    assert_eq!(tickets.len(), 100);
    let mut nummern: Vec<i32> = tickets.iter().map(|t| t.number).collect();
    nummern.sort_unstable();
    assert_eq!(nummern, (1..=100).collect::<Vec<i32>>());
    for id in a.0.iter().chain(&b.0) {
        assert!(
            tickets.iter().any(|t| &t.id == id),
            "Ticket {id} fehlt in der Datei"
        );
    }

    let projects = tx!(temp.db(), |u| { u.projects().list().await })
        .await
        .expect("Projekte");
    assert_eq!(projects[0].next_ticket_number, 101);
    temp.dispose().await;
}

#[tokio::test]
async fn andert_dieselbe_rev_aus_zwei_verbindungen_genau_einmal() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = grundlage(&temp).await;
    let file = temp.file().to_string();
    let (ids, errors) = create(file.clone(), board.clone(), 1, "Start").await;
    assert_eq!(errors, Vec::<String>::new());
    let id = ids[0].clone();

    let (a, b) = tokio::join!(
        update(file.clone(), id.clone(), 1, "Von A".into()),
        update(file.clone(), id.clone(), 1, "Von B".into()),
    );

    let mut ergebnisse = vec![a.clone(), b];
    ergebnisse.sort();
    assert_eq!(ergebnisse, vec!["conflict".to_string(), "ok".to_string()]);

    let gewinner = if a == "ok" { "Von A" } else { "Von B" };
    let ticket = tx!(temp.db(), [id], |u| { u.tickets().by_id(&id, false).await })
        .await
        .expect("lesen")
        .expect("Ticket");
    assert_eq!(ticket.rev, 2);
    assert_eq!(ticket.title, gewinner);
    temp.dispose().await;
}
