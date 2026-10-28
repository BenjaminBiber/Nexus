//! Kommentare, Notizen, Verlauf und Löschprotokoll auf SQLite; portiert aus
//! `packages/db/tests/sqlite/SqliteCommentRepository.test.ts`, `SqliteNoteRepository.test.ts`,
//! `SqliteHistoryRepository.test.ts` und `SqliteDeletionRepository.test.ts`.

mod common;

use common::temp_sqlite::{SeedTicket, TempSqlite, TEST_NOW, TEST_USER_ID};

use nexus_core::board::basics::{NoteState, TicketStatus};
use nexus_core::board::database::{BoardDatabase, NewComment, NewNote, NoteFilter};
use nexus_core::board::types::{Comment, Deletion, HistoryEntry, Note};
use nexus_db::sqlite_params;

/// Ein neuer Kommentar an `ticket_id` mit dem Text `body`
fn neuer_kommentar(ticket_id: &str, body: &str, read_by_agent_at: Option<&str>) -> NewComment {
    NewComment {
        ticket_id: ticket_id.to_string(),
        author_id: TEST_USER_ID.to_string(),
        author_name: "Test".to_string(),
        body: body.to_string(),
        created_at: TEST_NOW.to_string(),
        read_by_agent_at: read_by_agent_at.map(str::to_string),
    }
}

/// Eine neue Notiz des Boards; die Felder darüber hinaus setzen die Tests selbst
fn neue_notiz(board_id: i32) -> NewNote {
    NewNote {
        board_id,
        author_id: TEST_USER_ID.to_string(),
        author_name: "Tester".to_string(),
        title: "Cache leert sich".to_string(),
        body: "Befund: Cache leert sich".to_string(),
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

/// Ein Verlaufseintrag an ARC-1 (Statuswechsel), den die Tests abwandeln
fn eintrag() -> HistoryEntry {
    HistoryEntry {
        ticket_id: "ARC-1".to_string(),
        at: TEST_NOW.to_string(),
        by_id: TEST_USER_ID.to_string(),
        by_name: "Tester".to_string(),
        field: "status".to_string(),
        from: Some("backlog".to_string()),
        to: Some("planned".to_string()),
        note: None,
    }
}

/// Ein Eintrag des Löschprotokolls
fn loschung(at: &str, ticket_id: &str, title: &str, from: Option<TicketStatus>) -> Deletion {
    Deletion {
        at: at.to_string(),
        by_id: TEST_USER_ID.to_string(),
        by_name: "Tester".to_string(),
        ticket_id: ticket_id.to_string(),
        ticket_title: title.to_string(),
        from_status: from,
    }
}

/// Legt einen Kommentar in eigener Transaktion an
async fn insert_comment(temp: &TempSqlite, comment: NewComment) -> Comment {
    temp.db()
        .transaction(move |u| {
            let comment = comment.clone();
            Box::pin(async move { u.comments().insert(&comment).await })
        })
        .await
        .expect("Kommentar anlegen")
}

/// Legt eine Notiz in eigener Transaktion an
async fn insert_note(temp: &TempSqlite, note: NewNote) -> Note {
    temp.db()
        .transaction(move |u| {
            let note = note.clone();
            Box::pin(async move { u.notes().insert(&note).await })
        })
        .await
        .expect("Notiz anlegen")
}

/// Die Notiz mit dieser Id
async fn note_by_id(temp: &TempSqlite, id: i32) -> Option<Note> {
    temp.db()
        .transaction(move |u| Box::pin(async move { u.notes().by_id(id, false).await }))
        .await
        .expect("Notiz lesen")
}

/// Die Notizen eines Filters
async fn list_notes(temp: &TempSqlite, filter: NoteFilter) -> Vec<Note> {
    temp.db()
        .transaction(move |u| {
            let filter = filter.clone();
            Box::pin(async move { u.notes().list(&filter).await })
        })
        .await
        .expect("Notizen lesen")
}

/// Die Kommentare eines Tickets
async fn list_comments(temp: &TempSqlite, ticket_id: &str) -> Vec<Comment> {
    let ticket_id = ticket_id.to_string();
    temp.db()
        .transaction(move |u| {
            let ticket_id = ticket_id.clone();
            Box::pin(async move { u.comments().list(&ticket_id).await })
        })
        .await
        .expect("Kommentare lesen")
}

// ---------------------------------------------------------------------------------------------
// Kommentare
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn legt_einen_kommentar_an_und_liefert_seine_id() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket");

    let c1 = insert_comment(&temp, neuer_kommentar("ARC-1", "Erster", None)).await;
    assert!(c1.id > 0);
    assert_eq!(c1.read_by_agent_at, None);

    let c2 = insert_comment(&temp, neuer_kommentar("ARC-1", "Zweiter", None)).await;
    assert!(c2.id > c1.id);
    temp.dispose().await;
}

#[tokio::test]
async fn listet_kommentare_eines_tickets_in_reihenfolge_des_anlegens() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket 1");
    temp.seed_ticket(&SeedTicket::new("ARC-2", &board, 2))
        .await
        .expect("Ticket 2");

    let c1 = insert_comment(&temp, neuer_kommentar("ARC-1", "Erster", None)).await;
    let c2 = insert_comment(&temp, neuer_kommentar("ARC-1", "Zweiter", None)).await;

    assert_eq!(list_comments(&temp, "ARC-1").await, vec![c1, c2]);
    assert_eq!(list_comments(&temp, "ARC-2").await, Vec::new());
    temp.dispose().await;
}

#[tokio::test]
async fn zahlt_kommentare_je_ticket_eines_boards() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    let andere = temp
        .seed_board(Some(board.project_id), "Andere")
        .await
        .expect("Board 2");
    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket 1");
    temp.seed_ticket(&SeedTicket::new("ARC-2", &board, 2))
        .await
        .expect("Ticket 2");
    temp.seed_ticket(&SeedTicket::new("ARC-4", &andere, 4))
        .await
        .expect("Ticket 4");

    for (ticket, body) in [
        ("ARC-1", "a"),
        ("ARC-1", "b"),
        ("ARC-2", "x"),
        ("ARC-4", "y"),
    ] {
        insert_comment(&temp, neuer_kommentar(ticket, body, None)).await;
    }

    let anzahl = temp
        .db()
        .transaction({
            let board_id = board.board_id;
            move |u| Box::pin(async move { u.comments().count_by_board(board_id).await })
        })
        .await
        .expect("zählen");
    assert_eq!(
        anzahl.into_iter().collect::<Vec<_>>(),
        vec![("ARC-1".to_string(), 2), ("ARC-2".to_string(), 1)]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn setzt_und_liest_die_lesemarke_des_agenten() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket 1");
    temp.seed_ticket(&SeedTicket::new("ARC-2", &board, 2))
        .await
        .expect("Ticket 2");

    let c1 = insert_comment(&temp, neuer_kommentar("ARC-1", "Erster", None)).await;
    let c2 = insert_comment(&temp, neuer_kommentar("ARC-1", "Zweiter", None)).await;
    let c3 = insert_comment(
        &temp,
        neuer_kommentar("ARC-2", "gelesen", Some("2026-10-13T11:00:00.000Z")),
    )
    .await;
    assert_eq!(
        c3.read_by_agent_at.as_deref(),
        Some("2026-10-13T11:00:00.000Z")
    );

    let ungelesen = unread(&temp, vec!["ARC-1".to_string(), "ARC-2".to_string()]).await;
    assert_eq!(ungelesen, vec![c1.clone(), c2.clone()]);

    mark_read(&temp, vec![c1.id], "2026-10-13T12:00:00.000Z").await;
    let nach_erstem = list_comments(&temp, "ARC-1").await;
    assert_eq!(
        nach_erstem[0].read_by_agent_at.as_deref(),
        Some("2026-10-13T12:00:00.000Z")
    );

    // Eine gesetzte Lesemarke bleibt stehen
    mark_read(&temp, vec![c1.id, c2.id], "2026-10-13T13:00:00.000Z").await;
    let nach_zweitem = list_comments(&temp, "ARC-1").await;
    assert_eq!(
        nach_zweitem[0].read_by_agent_at.as_deref(),
        Some("2026-10-13T12:00:00.000Z")
    );
    assert_eq!(
        nach_zweitem[1].read_by_agent_at.as_deref(),
        Some("2026-10-13T13:00:00.000Z")
    );

    assert_eq!(unread(&temp, vec!["ARC-1".to_string()]).await, Vec::new());
    assert_eq!(unread(&temp, Vec::new()).await, Vec::new());
    mark_read(&temp, Vec::new(), TEST_NOW).await;
    temp.dispose().await;
}

/// Die ungelesenen Kommentare dieser Tickets
async fn unread(temp: &TempSqlite, ticket_ids: Vec<String>) -> Vec<Comment> {
    temp.db()
        .transaction(move |u| {
            let ticket_ids = ticket_ids.clone();
            Box::pin(async move { u.comments().unread_by_agent(&ticket_ids).await })
        })
        .await
        .expect("ungelesene lesen")
}

/// Setzt die Lesemarke dieser Kommentare
async fn mark_read(temp: &TempSqlite, ids: Vec<i64>, at: &str) {
    let at = at.to_string();
    temp.db()
        .transaction(move |u| {
            let ids = ids.clone();
            let at = at.clone();
            Box::pin(async move { u.comments().mark_read_by_agent(&ids, &at).await })
        })
        .await
        .expect("Lesemarke setzen");
}

#[tokio::test]
async fn loscht_kommentare_per_kaskade_mit_ihrem_ticket() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket");
    insert_comment(&temp, neuer_kommentar("ARC-1", "Erster", None)).await;

    temp.execute("DELETE FROM Tickets WHERE Id = 'ARC-1'", Vec::new())
        .await
        .expect("Ticket löschen");

    assert_eq!(list_comments(&temp, "ARC-1").await, Vec::new());
    assert_eq!(temp.count("Comments").await.expect("Zahl"), 0);
    temp.dispose().await;
}

// ---------------------------------------------------------------------------------------------
// Notizen
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn legt_eine_notiz_an_und_liest_sie() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");

    let n = insert_note(
        &temp,
        NewNote {
            labels: vec!["bug".to_string(), "ui".to_string()],
            source: Some("cli".to_string()),
            body: "Ärger im Formular".to_string(),
            ..neue_notiz(board.board_id)
        },
    )
    .await;

    assert!(n.id > 0);
    assert_eq!(note_by_id(&temp, n.id).await, Some(n.clone()));
    assert_eq!(note_by_id(&temp, 9999).await, None);
    temp.dispose().await;
}

#[tokio::test]
async fn ubernimmt_eine_notiz() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    let n = insert_note(&temp, neue_notiz(board.board_id)).await;

    update_note(
        &temp,
        Note {
            state: NoteState::Taken,
            ticket_id: Some("ARC-0001".to_string()),
            ..n.clone()
        },
    )
    .await;

    let gelesen = note_by_id(&temp, n.id).await.expect("Notiz");
    assert_eq!(gelesen.state, NoteState::Taken);
    assert_eq!(gelesen.ticket_id.as_deref(), Some("ARC-0001"));
    temp.dispose().await;
}

#[tokio::test]
async fn verwirft_eine_notiz_mit_grund() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    let n = insert_note(&temp, neue_notiz(board.board_id)).await;

    update_note(
        &temp,
        Note {
            state: NoteState::Dropped,
            reason: Some("doppelt".to_string()),
            ..n.clone()
        },
    )
    .await;

    let gelesen = note_by_id(&temp, n.id).await.expect("Notiz");
    assert_eq!(gelesen.state, NoteState::Dropped);
    assert_eq!(gelesen.reason.as_deref(), Some("doppelt"));
    temp.dispose().await;
}

/// Schreibt die Notiz zurück
async fn update_note(temp: &TempSqlite, note: Note) {
    temp.db()
        .transaction(move |u| {
            let note = note.clone();
            Box::pin(async move { u.notes().update(&note).await })
        })
        .await
        .expect("Notiz schreiben");
}

#[tokio::test]
async fn filtert_notizen_nach_board_status_und_quelle() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    let b2 = temp
        .seed_board(Some(board.project_id), "Anderes")
        .await
        .expect("Board 2");

    let offen = insert_note(
        &temp,
        NewNote {
            source: Some("cli".to_string()),
            ..neue_notiz(board.board_id)
        },
    )
    .await;
    let ubernommen = insert_note(
        &temp,
        NewNote {
            state: NoteState::Taken,
            ticket_id: Some("ARC-0001".to_string()),
            ..neue_notiz(board.board_id)
        },
    )
    .await;
    insert_note(&temp, neue_notiz(b2.board_id)).await;

    let alle = list_notes(&temp, NoteFilter::of_board(board.board_id)).await;
    assert_eq!(alle, vec![offen.clone(), ubernommen]);

    let nur_offen = list_notes(
        &temp,
        NoteFilter {
            states: Some(vec![NoteState::Open]),
            ..NoteFilter::of_board(board.board_id)
        },
    )
    .await;
    assert_eq!(nur_offen, vec![offen.clone()]);

    let ohne_stand = list_notes(
        &temp,
        NoteFilter {
            states: Some(Vec::new()),
            ..NoteFilter::of_board(board.board_id)
        },
    )
    .await;
    assert_eq!(ohne_stand, Vec::new());

    let aus_cli = list_notes(
        &temp,
        NoteFilter {
            source: Some("cli".to_string()),
            ..NoteFilter::of_board(board.board_id)
        },
    )
    .await;
    assert_eq!(aus_cli, vec![offen.clone()]);

    let anderes_board = list_notes(&temp, NoteFilter::of_board(b2.board_id)).await;
    assert!(anderes_board.iter().all(|note| note.id != offen.id));
    temp.dispose().await;
}

#[tokio::test]
async fn altere_zeilen_ohne_titel_und_zahler_bekommen_leeren_titel_und_eins() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    let n = insert_note(&temp, neue_notiz(board.board_id)).await;

    update_note(
        &temp,
        Note {
            title: "Anderer Titel".to_string(),
            seen_count: 4,
            seen_in: vec!["ARC-0002".to_string(), "ARC-0003".to_string()],
            ..n.clone()
        },
    )
    .await;
    let geandert = note_by_id(&temp, n.id).await.expect("Notiz");
    assert_eq!(geandert.title, "Anderer Titel");
    assert_eq!(geandert.seen_count, 4);
    assert_eq!(geandert.seen_in, ["ARC-0002", "ARC-0003"]);

    let alt = temp
        .number(
            "INSERT INTO Notes (BoardId, AuthorId, AuthorName, Body, State, CreatedAt) VALUES (@b, @u, 'Alt', 'Alter Befund', 'open', @now) RETURNING Id",
            sqlite_params! { "b": board.board_id, "u": TEST_USER_ID, "now": TEST_NOW },
        )
        .await
        .expect("alte Notiz")
        .expect("Id");
    let alte = note_by_id(&temp, i32::try_from(alt).expect("Id"))
        .await
        .expect("Notiz");
    assert_eq!(alte.title, "");
    assert_eq!(alte.body, "Alter Befund");
    assert_eq!(alte.seen_count, 1);
    assert_eq!(alte.seen_in, Vec::<String>::new());
    temp.dispose().await;
}

#[tokio::test]
async fn leere_labels_bleiben_leer_und_die_guid_wird_kleingeschrieben() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");

    let n = insert_note(
        &temp,
        NewNote {
            author_id: TEST_USER_ID.to_uppercase(),
            ..neue_notiz(board.board_id)
        },
    )
    .await;

    assert_eq!(n.labels, Vec::<String>::new());
    let gelesen = note_by_id(&temp, n.id).await.expect("Notiz");
    assert_eq!(gelesen.labels, Vec::<String>::new());
    assert_eq!(gelesen.author_id, TEST_USER_ID);
    temp.dispose().await;
}

// ---------------------------------------------------------------------------------------------
// Verlauf
// ---------------------------------------------------------------------------------------------

/// Schreibt Verlaufseinträge
async fn insert_history(temp: &TempSqlite, entries: Vec<HistoryEntry>) {
    temp.db()
        .transaction(move |u| {
            let entries = entries.clone();
            Box::pin(async move { u.history().insert(&entries).await })
        })
        .await
        .expect("Verlauf schreiben");
}

/// Der Verlauf eines Tickets
async fn list_history(temp: &TempSqlite, ticket_id: &str) -> Vec<HistoryEntry> {
    let ticket_id = ticket_id.to_string();
    temp.db()
        .transaction(move |u| {
            let ticket_id = ticket_id.clone();
            Box::pin(async move { u.history().list(&ticket_id).await })
        })
        .await
        .expect("Verlauf lesen")
}

#[tokio::test]
async fn schreibt_und_liest_den_verlauf_in_reihenfolge_des_schreibens() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket");

    let e1 = eintrag();
    let e2 = HistoryEntry {
        at: "2026-10-13T10:05:00.000Z".to_string(),
        field: "title".to_string(),
        from: Some("Alt".to_string()),
        to: Some("Neu äöü".to_string()),
        note: Some("Grund".to_string()),
        ..eintrag()
    };
    insert_history(&temp, vec![e1.clone(), e2.clone()]).await;
    let e3 = HistoryEntry {
        at: "2026-10-13T10:10:00.000Z".to_string(),
        from: None,
        to: None,
        ..eintrag()
    };
    insert_history(&temp, vec![e3.clone()]).await;

    assert_eq!(list_history(&temp, "ARC-1").await, vec![e1, e2, e3]);
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_nur_verlaufseintrage_des_tickets() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket 1");
    temp.seed_ticket(&SeedTicket::new("ARC-2", &board, 2))
        .await
        .expect("Ticket 2");

    let e1 = eintrag();
    let e2 = HistoryEntry {
        ticket_id: "ARC-2".to_string(),
        ..eintrag()
    };
    insert_history(&temp, vec![e1.clone(), e2.clone()]).await;

    assert_eq!(list_history(&temp, "ARC-1").await, vec![e1]);
    assert_eq!(list_history(&temp, "ARC-2").await, vec![e2]);
    temp.dispose().await;
}

#[tokio::test]
async fn normiert_guid_und_datum_im_verlauf() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket");

    insert_history(
        &temp,
        vec![HistoryEntry {
            by_id: TEST_USER_ID.to_uppercase(),
            at: "2026-10-13T10:00:00Z".to_string(),
            ..eintrag()
        }],
    )
    .await;

    let gelesen = list_history(&temp, "ARC-1").await;
    assert_eq!(gelesen[0].by_id, TEST_USER_ID);
    assert_eq!(gelesen[0].at, "2026-10-13T10:00:00.000Z");
    temp.dispose().await;
}

#[tokio::test]
async fn eine_leere_verlaufsliste_schreibt_nichts() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");
    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket");

    insert_history(&temp, Vec::new()).await;
    assert_eq!(list_history(&temp, "ARC-1").await, Vec::new());
    temp.dispose().await;
}

// ---------------------------------------------------------------------------------------------
// Löschprotokoll
// ---------------------------------------------------------------------------------------------

/// Schreibt einen Eintrag des Löschprotokolls
async fn insert_deletion(temp: &TempSqlite, deletion: Deletion) {
    temp.db()
        .transaction(move |u| {
            let deletion = deletion.clone();
            Box::pin(async move { u.deletions().insert(&deletion).await })
        })
        .await
        .expect("Löschung schreiben");
}

/// Das ganze Löschprotokoll
async fn list_deletions(temp: &TempSqlite) -> Vec<Deletion> {
    temp.db()
        .transaction(|u| Box::pin(async move { u.deletions().list().await }))
        .await
        .expect("Löschprotokoll lesen")
}

#[tokio::test]
async fn protokolliert_loschungen_und_liest_sie_in_reihenfolge() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");

    let d1 = loschung(
        "2026-10-13T10:00:00.000Z",
        "ARC-0007",
        "Ärger mit Umlauten",
        Some(TicketStatus::InProgress),
    );
    let d2 = loschung("2026-10-13T11:00:00.000Z", "ARC-0008", "Zweites", None);
    insert_deletion(&temp, d1.clone()).await;
    insert_deletion(&temp, d2.clone()).await;

    assert_eq!(list_deletions(&temp).await, vec![d1, d2]);
    temp.dispose().await;
}

#[tokio::test]
async fn das_loschprotokoll_ist_ohne_eintrage_leer() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    assert_eq!(list_deletions(&temp).await, Vec::new());
    temp.dispose().await;
}

#[tokio::test]
async fn normiert_guid_und_datum_im_loschprotokoll() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");

    insert_deletion(
        &temp,
        Deletion {
            by_id: TEST_USER_ID.to_uppercase(),
            at: "2026-10-13T10:00:00Z".to_string(),
            ..loschung(
                "2026-10-13T10:00:00.000Z",
                "ARC-0007",
                "Ärger mit Umlauten",
                Some(TicketStatus::InProgress),
            )
        },
    )
    .await;

    let gelesen = list_deletions(&temp).await;
    assert_eq!(gelesen[0].by_id, TEST_USER_ID);
    assert_eq!(gelesen[0].at, "2026-10-13T10:00:00.000Z");
    temp.dispose().await;
}

#[tokio::test]
async fn ein_unbekannter_status_im_loschprotokoll_scheitert_laut() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");
    temp.execute(
        "INSERT INTO Deletions (At, ById, ByName, TicketId, TicketTitle, FromStatus) VALUES (@at, @by, 'Tester', 'ARC-0009', 'Kaputt', 'erfunden')",
        sqlite_params! { "at": TEST_NOW, "by": TEST_USER_ID },
    )
    .await
    .expect("Zeile");

    let fehler = temp
        .db()
        .transaction(|u| Box::pin(async move { u.deletions().list().await }))
        .await
        .expect_err("Fehler");
    assert!(fehler.to_string().contains("erfunden"), "{fehler}");
    temp.dispose().await;
}
