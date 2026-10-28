//! Dispatcher-Läufe, Auswertung und Suche auf SQLite; portiert aus
//! `packages/db/tests/sqlite/SqliteDispatchRunRepository.test.ts`,
//! `SqliteDispatchStatsRepository.test.ts` und `SqliteSearchRepository.test.ts`.
//!
//! Wo die TypeScript-Tests ihre Daten über andere Repositories anlegen, legt Rust sie über rohes
//! SQL bzw. die `seed_*`-Helfer an: P2-6 bis P2-8 füllen die übrigen Repositories erst noch.

mod common;

use common::temp_sqlite::TempSqlite;

use nexus_core::board::database::BoardDatabase;
use nexus_core::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunKind, DispatchRunOutcome,
    DispatchRunSource,
};
use nexus_core::board::search_types::{SearchField, SearchKind, SearchRequest, SearchRow};
use nexus_core::text::utf16_len;
use nexus_core::tx;
use nexus_db::sqlite_params;

/// Der Benutzer, der die Testdaten anlegt
const USER: &str = "11111111-1111-1111-1111-111111111111";
/// Fester Zeitpunkt, damit die Tests nicht von der Uhr abhängen
const NOW: &str = "2026-10-13T10:00:00.000Z";
/// Eine Stunde später – für die Sortierung „neueste zuerst“
const LATER: &str = "2026-10-13T11:00:00.000Z";

/// Vollständiger Lauf mit Standardwerten (wie `runFixture` in `DispatchRunContract.ts`)
fn run_fixture(id: &str, subject_id: &str, project_id: i32) -> DispatchRun {
    DispatchRun {
        id: id.to_string(),
        kind: DispatchRunKind::Work,
        subject_id: subject_id.to_string(),
        project_id,
        board_id: None,
        agent: "junior-X".to_string(),
        attempt: Some(1),
        profile: "standard".to_string(),
        escalated_from: None,
        requested_model: Some("opus".to_string()),
        model: Some("claude-opus-5-5".to_string()),
        effort: Some("high".to_string()),
        started_at: NOW.to_string(),
        ended_at: Some("2026-10-13T10:05:00.000Z".to_string()),
        duration_ms: Some(300_000),
        num_turns: Some(12),
        exit_code: Some(0),
        timed_out: false,
        outcome: DispatchRunOutcome::Done,
        abort_category: None,
        abort_reason: None,
        cost_usd: Some(0.123_456),
        input_tokens: Some(10),
        output_tokens: Some(2_000_000_000_000),
        cache_read_tokens: Some(30),
        cache_write_tokens: Some(40),
        model_usage: Some(r#"{"claude-opus-5-5":{"costUsd":0.123456}}"#.to_string()),
        source: DispatchRunSource::Dispatcher,
        max_turns: None,
        timeout_minutes: None,
        profile_reason: None,
        rule_index: None,
        config_hash: None,
        denied_count: None,
        denied_tools: None,
    }
}

/// Läufe des Projekts im Zeitraum, ohne Board-Filter
fn filter(project_id: i32, from: &str, to: &str) -> DispatchRunFilter {
    DispatchRunFilter {
        project_id,
        board_id: None,
        from: from.to_string(),
        to: to.to_string(),
    }
}

// ----------------------------------------------------------------------------------------------
// Dispatcher-Läufe
// ----------------------------------------------------------------------------------------------

#[tokio::test]
async fn speichert_vollstandig_und_idempotent() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let r1 = DispatchRun {
        board_id: Some(1),
        started_at: "2026-10-01T00:00:00.000Z".into(),
        ..run_fixture("ARC-0001-work-1", "ARC-0001", 1)
    };
    let r2 = DispatchRun {
        board_id: Some(2),
        started_at: "2026-10-05T12:00:00.000Z".into(),
        outcome: DispatchRunOutcome::Abort,
        abort_category: Some("timeout".into()),
        abort_reason: Some("Zeitlimit äöü".into()),
        cost_usd: None,
        model: None,
        ended_at: None,
        duration_ms: None,
        num_turns: None,
        exit_code: None,
        timed_out: true,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        model_usage: None,
        escalated_from: Some("klein".into()),
        attempt: Some(2),
        ..run_fixture("ARC-0002-work-1", "ARC-0002", 1)
    };
    let r3 = DispatchRun {
        source: DispatchRunSource::Comments,
        started_at: "2026-09-01T00:00:00.000Z".into(),
        ..run_fixture("c42", "OTH-0001", 2)
    };

    let db = temp.db();
    let eingefugt = tx!(db, [r1, r2, r3], |u| {
        Ok(vec![
            u.dispatch_runs().insert(&r1).await?,
            u.dispatch_runs().insert(&r2).await?,
            u.dispatch_runs().insert(&r3).await?,
        ])
    })
    .await
    .expect("einfügen");
    assert_eq!(eingefugt, [true, true, true]);

    let nochmal = DispatchRun {
        outcome: DispatchRunOutcome::Abort,
        ..r1.clone()
    };
    assert!(!tx!(db, [nochmal], |u| {
        u.dispatch_runs().insert(&nochmal).await
    })
    .await
    .expect("zweites Einfügen"));

    let bereich = filter(1, "2026-10-01T00:00:00.000Z", "2026-10-06T00:00:00.000Z");
    let alle = tx!(db, [bereich], |u| {
        u.dispatch_runs().list(&bereich).await
    })
    .await
    .expect("lesen");
    assert_eq!(alle, [r1, r2]);
    temp.dispose().await;
}

#[tokio::test]
async fn filtert_zeitraum_und_board() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let r1 = DispatchRun {
        board_id: Some(1),
        started_at: "2026-10-01T00:00:00.000Z".into(),
        ..run_fixture("ARC-0001-work-1", "ARC-0001", 1)
    };
    let r2 = DispatchRun {
        board_id: Some(2),
        started_at: "2026-10-05T12:00:00.000Z".into(),
        ..run_fixture("ARC-0002-work-1", "ARC-0002", 1)
    };
    let db = temp.db();
    tx!(db, [r1, r2], |u| {
        u.dispatch_runs().insert(&r1).await?;
        u.dispatch_runs().insert(&r2).await
    })
    .await
    .expect("einfügen");

    let ids = |runs: Vec<DispatchRun>| runs.into_iter().map(|r| r.id).collect::<Vec<_>>();

    // Start einschließlich, Ende ausschließlich
    let eng = filter(1, "2026-10-01T00:00:00.001Z", "2026-10-05T12:00:00.000Z");
    assert!(tx!(db, [eng], |u| { u.dispatch_runs().list(&eng).await })
        .await
        .expect("lesen")
        .is_empty());

    let board = DispatchRunFilter {
        board_id: Some(2),
        ..filter(1, "2026-01-01T00:00:00.000Z", "2027-01-01T00:00:00.000Z")
    };
    assert_eq!(
        ids(
            tx!(db, [board], |u| { u.dispatch_runs().list(&board).await })
                .await
                .expect("lesen")
        ),
        ["ARC-0002-work-1"]
    );

    // Grenzen ohne Millisekunden werden vor dem Textvergleich normalisiert
    let ohne_millis = filter(1, "2026-10-01T00:00:00Z", "2026-10-06T00:00:00Z");
    assert_eq!(
        ids(tx!(db, [ohne_millis], |u| {
            u.dispatch_runs().list(&ohne_millis).await
        })
        .await
        .expect("lesen")),
        ["ARC-0001-work-1", "ARC-0002-work-1"]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_first_started_je_projekt_und_quelle() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let r1 = DispatchRun {
        started_at: "2026-10-01T00:00:00.000Z".into(),
        ..run_fixture("ARC-0001-work-1", "ARC-0001", 1)
    };
    let r2 = DispatchRun {
        started_at: "2026-10-05T12:00:00.000Z".into(),
        ..run_fixture("ARC-0002-work-1", "ARC-0002", 1)
    };
    let db = temp.db();
    tx!(db, [r1, r2], |u| {
        u.dispatch_runs().insert(&r1).await?;
        u.dispatch_runs().insert(&r2).await
    })
    .await
    .expect("einfügen");

    assert_eq!(
        tx!(db, |u| {
            u.dispatch_runs()
                .first_started(1, DispatchRunSource::Dispatcher)
                .await
        })
        .await
        .expect("erster Lauf"),
        Some("2026-10-01T00:00:00.000Z".to_string())
    );
    assert_eq!(
        tx!(db, |u| {
            u.dispatch_runs()
                .first_started(1, DispatchRunSource::Comments)
                .await
        })
        .await
        .expect("erster Lauf"),
        None
    );
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_existing_in_blocken() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let r1 = run_fixture("ARC-0001-work-1", "ARC-0001", 1);
    let r2 = DispatchRun {
        source: DispatchRunSource::Comments,
        ..run_fixture("c42", "OTH-0001", 2)
    };
    let db = temp.db();
    tx!(db, [r1, r2], |u| {
        u.dispatch_runs().insert(&r1).await?;
        u.dispatch_runs().insert(&r2).await
    })
    .await
    .expect("einfügen");

    let gesucht = vec![
        "c42".to_string(),
        "c43".to_string(),
        "ARC-0001-work-1".to_string(),
    ];
    let mut gefunden = tx!(db, [gesucht], |u| {
        u.dispatch_runs().existing(&gesucht).await
    })
    .await
    .expect("vorhanden");
    gefunden.sort();
    assert_eq!(gefunden, ["ARC-0001-work-1", "c42"]);

    let leer: Vec<String> = Vec::new();
    assert!(
        tx!(db, [leer], |u| { u.dispatch_runs().existing(&leer).await })
            .await
            .expect("vorhanden")
            .is_empty()
    );

    // Mehr als ein Block (SQL_CHUNK_SIZE = 500)
    let viele: Vec<String> = (0..1200).map(|i| format!("gen-{i}")).collect();
    tx!(db, [viele], |u| {
        for id in &viele {
            u.dispatch_runs()
                .insert(&run_fixture(id, "ARC-0003", 1))
                .await?;
        }
        Ok(())
    })
    .await
    .expect("einfügen");

    let mut alle = viele.clone();
    alle.push("c42".to_string());
    let gefunden = tx!(db, [alle], |u| { u.dispatch_runs().existing(&alle).await })
        .await
        .expect("vorhanden");
    assert_eq!(gefunden.len(), 1201);
    assert!(gefunden.contains(&"gen-1199".to_string()));
    assert!(gefunden.contains(&"c42".to_string()));
    temp.dispose().await;
}

#[tokio::test]
async fn begrenzt_dauer_und_abbruchgrund_wie_mssql() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    // In Rust passt die Dauer schon vom Typ her in int; eine negative Dauer ist ebenso kaputt
    let run = DispatchRun {
        duration_ms: Some(-1),
        abort_reason: Some("x".repeat(600)),
        ..run_fixture("ARC-0001-work-1", "ARC-0001", 1)
    };
    let db = temp.db();
    tx!(db, [run], |u| { u.dispatch_runs().insert(&run).await })
        .await
        .expect("einfügen");

    let bereich = filter(1, "2026-01-01T00:00:00.000Z", "2027-01-01T00:00:00.000Z");
    let gespeichert = tx!(db, [bereich], |u| {
        u.dispatch_runs().list(&bereich).await
    })
    .await
    .expect("lesen");
    assert_eq!(gespeichert[0].duration_ms, None);
    assert_eq!(
        gespeichert[0].abort_reason.as_deref().map(utf16_len),
        Some(500)
    );
    temp.dispose().await;
}

#[tokio::test]
async fn merkt_sich_jede_fassung_der_einstellung_nur_einmal() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let eins = DispatchConfigVersion {
        hash: "aaa".into(),
        dispatcher: "mac".into(),
        first_seen_at: "2026-10-01T00:00:00Z".into(),
        content: r#"{"a":1}"#.into(),
    };
    let zwei = DispatchConfigVersion {
        hash: "bbb".into(),
        dispatcher: "mac".into(),
        first_seen_at: "2026-10-02T00:00:00.000Z".into(),
        content: r#"{"a":2}"#.into(),
    };
    let db = temp.db();
    let geschrieben = tx!(db, [eins, zwei], |u| {
        Ok(vec![
            u.dispatch_runs().record_config_version(&eins).await?,
            u.dispatch_runs().record_config_version(&zwei).await?,
            u.dispatch_runs().record_config_version(&eins).await?,
        ])
    })
    .await
    .expect("schreiben");
    assert_eq!(geschrieben, [true, true, false]);

    let fassungen = tx!(db, |u| {
        u.dispatch_runs()
            .list_config_versions("2026-10-01T00:00:00.000Z", "2026-10-02T00:00:00.000Z")
            .await
    })
    .await
    .expect("lesen");
    assert_eq!(fassungen.len(), 1);
    assert_eq!(fassungen[0].hash, "aaa");
    // Der Zeitpunkt ohne Millisekunden wird beim Schreiben normalisiert
    assert_eq!(fassungen[0].first_seen_at, "2026-10-01T00:00:00.000Z");
    assert_eq!(fassungen[0].content, r#"{"a":1}"#);
    temp.dispose().await;
}

// ----------------------------------------------------------------------------------------------
// Auswertung der Dispatcher-Läufe
// ----------------------------------------------------------------------------------------------

/// Projekte, Boards und Tickets, die [`seed_stats`] angelegt hat
#[derive(Clone, Copy)]
struct Statistik {
    arc: i32,
    b2: i32,
}

/// Projekte ARC (Boards „Eins“, „Zwei“) und OTH (Board „Drei“), vier Tickets, Verlauf und
/// Kommentare – wie `seed` in `SqliteDispatchStatsRepository.test.ts`
async fn seed_stats(temp: &TempSqlite) -> Statistik {
    let arc = temp.seed_project("ARC").await.expect("Projekt ARC");
    let oth = temp.seed_project("OTH").await.expect("Projekt OTH");
    let b1 = temp.seed_board(Some(arc), "Eins").await.expect("Board 1");
    let b2 = temp.seed_board(Some(arc), "Zwei").await.expect("Board 2");
    let b3 = temp.seed_board(Some(oth), "Drei").await.expect("Board 3");

    for (id, project, board, number) in [
        ("ARC-0001", arc, b1.board_id, 1),
        ("ARC-0002", arc, b2.board_id, 2),
        ("ARC-0003", arc, b1.board_id, 3),
        ("OTH-0001", oth, b3.board_id, 1),
    ] {
        temp.seed_ticket(&common::temp_sqlite::SeedTicket {
            id: id.to_string(),
            project_id: project,
            board_id: board,
            number,
            title: "Titel".into(),
            status: "done".into(),
            sort_order: 0,
        })
        .await
        .expect("Ticket");
    }

    for (ticket, field, at, to) in [
        ("ARC-0001", "status", "2026-10-02T10:00:00.000Z", "done"),
        ("ARC-0001", "title", "2026-10-02T11:00:00.000Z", "x"),
        (
            "ARC-0001",
            "blockedReason",
            "2026-10-02T12:00:00.000Z",
            "warte",
        ),
        ("ARC-0002", "status", "2026-10-03T10:00:00.000Z", "done"),
        ("ARC-0003", "status", "2026-10-03T10:00:00.000Z", "done"),
        (
            "ARC-0003",
            "status",
            "2026-10-04T10:00:00.000Z",
            "in_progress",
        ),
        ("ARC-0003", "status", "2026-10-20T10:00:00.000Z", "done"),
        ("OTH-0001", "status", "2026-10-03T10:00:00.000Z", "done"),
    ] {
        temp.execute(
            "INSERT INTO History (TicketId, At, ById, ByName, Field, FromValue, ToValue, Note)
             VALUES (@ticketId, @at, @user, 'x', @field, NULL, @to, NULL)",
            sqlite_params! { "ticketId": ticket, "at": at, "user": USER, "field": field, "to": to },
        )
        .await
        .expect("Verlauf");
    }

    for body in [
        "[dispatch] Start: an junior-ARC-0001 übergeben",
        "[dispatch] Abbruch Versuch 1/3: x.",
        "[dispatch] Start: wieder",
        "Mensch: [dispatch] Abbruch",
        "[DISPATCH] Start groß",
    ] {
        kommentar(temp, "ARC-0001", body).await;
    }
    kommentar(temp, "OTH-0001", "[dispatch] Start: an junior-OTH-0001").await;

    Statistik {
        arc,
        b2: b2.board_id,
    }
}

/// Ein Kommentar des Test-Benutzers am Ticket
async fn kommentar(temp: &TempSqlite, ticket: &str, body: &str) {
    temp.execute(
        "INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt)
         VALUES (@ticketId, @user, 'dispatcher', @body, @now)",
        sqlite_params! { "ticketId": ticket, "user": USER, "body": body, "now": NOW },
    )
    .await
    .expect("Kommentar");
}

#[tokio::test]
async fn findet_tickets_mit_letztem_wechsel_nach_erledigt_im_zeitraum() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let seed = seed_stats(&temp).await;
    let db = temp.db();

    let bereich = filter(
        seed.arc,
        "2026-10-01T00:00:00.000Z",
        "2026-10-10T00:00:00.000Z",
    );
    assert_eq!(
        tx!(db, [bereich], |u| {
            u.dispatch_stats().done_ticket_ids(&bereich).await
        })
        .await
        .expect("erledigt"),
        ["ARC-0001", "ARC-0002"]
    );

    let mit_board = DispatchRunFilter {
        board_id: Some(seed.b2),
        ..filter(
            seed.arc,
            "2026-10-01T00:00:00.000Z",
            "2026-10-10T00:00:00.000Z",
        )
    };
    assert_eq!(
        tx!(db, [mit_board], |u| {
            u.dispatch_stats().done_ticket_ids(&mit_board).await
        })
        .await
        .expect("erledigt"),
        ["ARC-0002"]
    );

    let ohne_millis = filter(seed.arc, "2026-10-01T00:00:00Z", "2026-10-10T00:00:00.000Z");
    assert_eq!(
        tx!(db, [ohne_millis], |u| {
            u.dispatch_stats().done_ticket_ids(&ohne_millis).await
        })
        .await
        .expect("erledigt"),
        ["ARC-0001", "ARC-0002"]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_statuswechsel_und_blockiert_grunde_in_reihenfolge_des_eintrags() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_stats(&temp).await;
    let db = temp.db();

    let ids = vec!["ARC-0001".to_string(), "ARC-0003".to_string()];
    let verlauf = tx!(db, [ids], |u| {
        u.dispatch_stats().status_history(&ids).await
    })
    .await
    .expect("Verlauf");
    assert_eq!(
        verlauf
            .iter()
            .map(|h| (h.ticket_id.as_str(), h.field.as_str(), h.to.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("ARC-0001", "status", Some("done")),
            ("ARC-0001", "blockedReason", Some("warte")),
            ("ARC-0003", "status", Some("done")),
            ("ARC-0003", "status", Some("in_progress")),
            ("ARC-0003", "status", Some("done")),
        ]
    );
    assert_eq!(verlauf[0].by_id, USER);
    assert_eq!(verlauf[0].at, "2026-10-02T10:00:00.000Z");

    let leer: Vec<String> = Vec::new();
    assert!(tx!(db, [leer], |u| {
        u.dispatch_stats().status_history(&leer).await
    })
    .await
    .expect("Verlauf")
    .is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn zahlt_dispatch_start_und_abbruch_kommentare_je_ticket() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_stats(&temp).await;
    let db = temp.db();

    let ids = vec!["ARC-0001".to_string(), "ARC-0002".to_string()];
    let zahlen = tx!(db, [ids], |u| {
        u.dispatch_stats().dispatch_comment_counts(&ids).await
    })
    .await
    .expect("Zahlen");
    assert_eq!(zahlen.len(), 1);
    assert_eq!(zahlen["ARC-0001"].starts, 2);
    assert_eq!(zahlen["ARC-0001"].aborts, 1);

    let leer: Vec<String> = Vec::new();
    assert!(tx!(db, [leer], |u| {
        u.dispatch_stats().dispatch_comment_counts(&leer).await
    })
    .await
    .expect("Zahlen")
    .is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_die_dispatch_kommentare_des_projekts_in_anlagereihenfolge() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let seed = seed_stats(&temp).await;
    let db = temp.db();

    let kommentare = tx!(db, [seed], |u| {
        u.dispatch_stats().dispatch_comments(seed.arc).await
    })
    .await
    .expect("Kommentare");
    assert_eq!(
        kommentare
            .iter()
            .map(|c| c.body.as_str())
            .collect::<Vec<_>>(),
        [
            "[dispatch] Start: an junior-ARC-0001 übergeben",
            "[dispatch] Abbruch Versuch 1/3: x.",
            "[dispatch] Start: wieder",
        ]
    );
    assert_eq!(kommentare[0].author_id, USER);
    assert_eq!(kommentare[0].created_at, NOW);
    assert_eq!(kommentare[0].read_by_agent_at, None);
    temp.dispose().await;
}

// ----------------------------------------------------------------------------------------------
// Suche
// ----------------------------------------------------------------------------------------------

/// Langes Feld für den Ausschnitt: Fundstelle bei Index 9001
fn langes_feld() -> String {
    format!("{} Fundstelle am Ende", "x".repeat(9000))
}

/// Mindestangaben eines Tickets der Suche
struct Suchticket {
    id: String,
    number: i32,
    title: String,
    description: String,
    acceptance: String,
    plan: String,
    handbook: String,
    project_id: i32,
    board_id: i32,
    status: String,
    created_at: String,
}

impl Suchticket {
    /// Ticket in Projekt 1, Board 1, Status `backlog`, angelegt zu [`NOW`]
    fn new(id: &str, number: i32, title: &str) -> Self {
        Self {
            id: id.to_string(),
            number,
            title: title.to_string(),
            description: String::new(),
            acceptance: String::new(),
            plan: String::new(),
            handbook: String::new(),
            project_id: 1,
            board_id: 1,
            status: "backlog".to_string(),
            created_at: NOW.to_string(),
        }
    }
}

/// Legt ein Ticket der Suche an
async fn suchticket(temp: &TempSqlite, t: &Suchticket) {
    temp.execute(
        "INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Description, Status, Priority, SortOrder, NeedsPlanning,
           Acceptance, \"Plan\", Handbook, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
         VALUES (@id, @projectId, @boardId, @number, @title, @description, @status, 'normal', @number, 0,
           @acceptance, @plan, @handbook, @createdAt, @userId, 'Tester', @createdAt, 1)",
        sqlite_params! {
            "id": t.id.clone(),
            "projectId": t.project_id,
            "boardId": t.board_id,
            "number": t.number,
            "title": t.title.clone(),
            "description": t.description.clone(),
            "status": t.status.clone(),
            "acceptance": t.acceptance.clone(),
            "plan": t.plan.clone(),
            "handbook": t.handbook.clone(),
            "createdAt": t.created_at.clone(),
            "userId": USER,
        },
    )
    .await
    .expect("Ticket");
}

/// Zwei Projekte mit je einem Board und die Tickets, Kommentare, Fragen und Notizen der Fälle
async fn seed_suche(temp: &TempSqlite) {
    temp.execute(
        "INSERT INTO Users (Id, Name, CreatedAt) VALUES (@id, 'Tester', @now)",
        sqlite_params! { "id": USER, "now": NOW },
    )
    .await
    .expect("Benutzer");
    for (key, name) in [("ARC", "Arches"), ("SV1", "Service")] {
        temp.execute(
            "INSERT INTO Projects (\"Key\", Name, CreatedAt, CreatedBy) VALUES (@key, @name, @now, @user)",
            sqlite_params! { "key": key, "name": name, "now": NOW, "user": USER },
        )
        .await
        .expect("Projekt");
    }
    for (project_id, name) in [(1, "Sprint"), (2, "Incidents")] {
        temp.execute(
            "INSERT INTO Boards (ProjectId, OwnerId, Name, CreatedAt) VALUES (@projectId, @user, @name, @now)",
            sqlite_params! { "projectId": project_id, "user": USER, "name": name, "now": NOW },
        )
        .await
        .expect("Board");
    }

    suchticket(
        temp,
        &Suchticket {
            description: "Formular mit Passwort".into(),
            acceptance: "Test ist grün".into(),
            plan: "Plan: OAuth einbauen".into(),
            handbook: "Vorher npm test laufen lassen".into(),
            ..Suchticket::new("ARC-0001", 1, "Großer Ärger im Login")
        },
    )
    .await;
    suchticket(
        temp,
        &Suchticket::new("ARC-0002", 2, "Ein arger Tippfehler"),
    )
    .await;
    suchticket(
        temp,
        &Suchticket {
            description: langes_feld(),
            ..Suchticket::new("ARC-0003", 3, "Rabatt 100% auf _x_ [a]")
        },
    )
    .await;
    suchticket(
        temp,
        &Suchticket {
            project_id: 2,
            board_id: 2,
            status: "in_progress".into(),
            created_at: LATER.into(),
            ..Suchticket::new("SV1-0001", 1, "Incident Speicher voll")
        },
    )
    .await;

    temp.execute(
        "INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt)
         VALUES ('ARC-0002', @user, 'Tester', 'Zwischenstand zur Speicherwahl', @now)",
        sqlite_params! { "user": USER, "now": NOW },
    )
    .await
    .expect("Kommentar");
    temp.execute(
        "INSERT INTO Questions (Id, ProjectId, Number, Title, Description, State, OriginKind, OriginName, DecisionOutcome,
           DecisionText, DecisionReason, DecidedById, DecidedByName, DecidedAt, CreatedAt, CreatedById, UpdatedAt, Rev)
         VALUES ('ARC-F0001', 1, 1, 'Welcher Speicher?', 'Varianten', 'answered', 'session', 'claude', 'answered',
           'SQL Server', 'Lizenz vorhanden', @user, 'Florian', @now, @now, @user, @now, 2)",
        sqlite_params! { "user": USER, "now": NOW },
    )
    .await
    .expect("Frage");
    temp.execute(
        "INSERT INTO QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt)
         VALUES ('ARC-F0001', @user, 'Florian', 'human', 'Kommentar zur Lizenz', @now)",
        sqlite_params! { "user": USER, "now": NOW },
    )
    .await
    .expect("Fragen-Kommentar");
    temp.execute(
        "INSERT INTO Notes (BoardId, AuthorId, AuthorName, Title, Body, Labels, State, CreatedAt)
         VALUES (1, @user, 'Tester', 'Befund: Cache leert sich', 'zweite Zeile', '', 'open', @now)",
        sqlite_params! { "user": USER, "now": NOW },
    )
    .await
    .expect("Notiz");
}

/// Anfrage mit allen Arten und 30 Treffern je Quelle
fn anfrage(terms: &[&str]) -> SearchRequest {
    SearchRequest {
        terms: terms.iter().map(ToString::to_string).collect(),
        kinds: SearchKind::ALL.to_vec(),
        per_source: 30,
    }
}

/// Sucht und liefert die Zeilen
async fn finde(temp: &TempSqlite, request: SearchRequest) -> Vec<SearchRow> {
    let db = temp.db();
    tx!(db, [request], |u| { u.search().find(&request).await })
        .await
        .expect("Suche")
        .rows
}

/// Art, Id und Feld je Zeile
fn form(rows: &[SearchRow]) -> Vec<(SearchKind, &str, SearchField)> {
    rows.iter()
        .map(|r| (r.kind, r.id.as_str(), r.field))
        .collect()
}

#[tokio::test]
async fn findet_arger_uber_arger_aber_nicht_uber_arger_ohne_umlaut() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;

    for term in ["ärger", "ÄRGER"] {
        assert_eq!(
            form(&finde(&temp, anfrage(&[term])).await),
            [(SearchKind::Ticket, "ARC-0001", SearchField::Title)]
        );
    }
    assert_eq!(
        form(&finde(&temp, anfrage(&["arger"])).await),
        [(SearchKind::Ticket, "ARC-0002", SearchField::Title)]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn verlangt_alle_begriffe_im_selben_feld() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;

    assert_eq!(
        form(&finde(&temp, anfrage(&["npm", "laufen"])).await),
        [(SearchKind::Ticket, "ARC-0001", SearchField::Handbook)]
    );
    assert!(finde(&temp, anfrage(&["login", "passwort"]))
        .await
        .is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn durchsucht_alle_quellen_in_der_reihenfolge_des_mssql_repositorys() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;

    let speicher = finde(&temp, anfrage(&["speicher"])).await;
    assert_eq!(
        form(&speicher),
        [
            (SearchKind::Ticket, "SV1-0001", SearchField::Title),
            (SearchKind::Ticket, "ARC-0002", SearchField::Comment),
            (SearchKind::Question, "ARC-F0001", SearchField::Title),
        ]
    );
    assert_eq!(speicher[1].title, "Ein arger Tippfehler");
    assert_eq!(speicher[1].at.as_deref(), Some(NOW));
    assert_eq!(speicher[1].board_id, Some(1));
    assert_eq!(speicher[1].project_id, 1);
    assert_eq!(speicher[2].ticket_id, None);
    assert_eq!(speicher[2].board_id, None);
    assert_eq!(speicher[2].status, "answered");
    assert_eq!(speicher[2].at, None);

    let lizenz = finde(&temp, anfrage(&["lizenz"])).await;
    assert_eq!(
        form(&lizenz),
        [
            (SearchKind::Question, "ARC-F0001", SearchField::Decision),
            (
                SearchKind::Question,
                "ARC-F0001",
                SearchField::QuestionComment
            ),
        ]
    );
    assert_eq!(lizenz[0].text, "SQL Server\nLizenz vorhanden");
    assert_eq!(lizenz[0].at.as_deref(), Some(NOW));

    let cache = finde(&temp, anfrage(&["cache"])).await;
    assert_eq!(
        cache,
        [SearchRow {
            kind: SearchKind::Note,
            id: "1".into(),
            ticket_id: None,
            field: SearchField::Note,
            title: "Befund: Cache leert sich".into(),
            project_id: 1,
            board_id: Some(1),
            status: "open".into(),
            text: "Befund: Cache leert sich\nzweite Zeile".into(),
            text_start: 0,
            text_length: 37,
            at: Some(NOW.into()),
        }]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn beachtet_den_art_filter() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;

    let nur_fragen = SearchRequest {
        kinds: vec![SearchKind::Question],
        ..anfrage(&["speicher"])
    };
    assert_eq!(
        form(&finde(&temp, nur_fragen).await),
        [(SearchKind::Question, "ARC-F0001", SearchField::Title)]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn nimmt_sonderzeichen_wortlich() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;

    for term in ["100%", "_x_", "[a]"] {
        assert_eq!(
            form(&finde(&temp, anfrage(&[term])).await),
            [(SearchKind::Ticket, "ARC-0003", SearchField::Title)],
            "Begriff {term}"
        );
    }
    assert!(finde(&temp, anfrage(&["ra%tt"])).await.is_empty());
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_denselben_ausschnitt_wie_mssql() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;

    let lang = finde(&temp, anfrage(&["fundstelle"])).await;
    assert_eq!(lang[0].id, "ARC-0003");
    assert_eq!(lang[0].field, SearchField::Description);
    assert_eq!(lang[0].text_start, 8851);
    assert_eq!(lang[0].text_length, 9019);
    assert_eq!(utf16_len(&lang[0].text), 168);
    assert!(lang[0].text.ends_with("Fundstelle am Ende"));

    let plan = finde(&temp, anfrage(&["oauth"])).await;
    assert_eq!(plan[0].field, SearchField::Plan);
    assert_eq!(plan[0].text, "Plan: OAuth einbauen");
    assert_eq!(plan[0].text_start, 0);
    assert_eq!(plan[0].text_length, 20);
    temp.dispose().await;
}

#[tokio::test]
async fn zahlt_ein_emoji_vor_der_fundstelle_als_zwei_einheiten() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;
    // „🙂“ belegt zwei UTF-16-Einheiten, aber vier Bytes; die Oberfläche erwartet die
    // JavaScript-Rechnung
    suchticket(
        &temp,
        &Suchticket {
            description: format!("🙂{}treffer", "y".repeat(200)),
            ..Suchticket::new("ARC-0004", 4, "Emoji")
        },
    )
    .await;

    let zeilen = finde(&temp, anfrage(&["treffer"])).await;
    assert_eq!(
        form(&zeilen),
        [(SearchKind::Ticket, "ARC-0004", SearchField::Description)]
    );
    assert_eq!(zeilen[0].text_start, 52);
    assert_eq!(zeilen[0].text_length, 209);
    assert_eq!(utf16_len(&zeilen[0].text), 157);
    assert!(zeilen[0].text.ends_with("treffer"));
    temp.dispose().await;
}

#[tokio::test]
async fn begrenzt_je_quelle_und_sortiert_neueste_zuerst() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;
    for n in 10..=14 {
        suchticket(
            &temp,
            &Suchticket::new(&format!("ARC-00{n}"), n, "Massen i"),
        )
        .await;
    }

    let db = temp.db();
    let drei = SearchRequest {
        kinds: vec![SearchKind::Ticket],
        per_source: 3,
        ..anfrage(&["massen"])
    };
    let begrenzt = tx!(db, [drei], |u| { u.search().find(&drei).await })
        .await
        .expect("Suche");
    assert_eq!(
        begrenzt
            .rows
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        ["ARC-0014", "ARC-0013", "ARC-0012"]
    );
    assert!(begrenzt.truncated);

    let funf = SearchRequest {
        kinds: vec![SearchKind::Ticket],
        per_source: 5,
        ..anfrage(&["massen"])
    };
    let voll = tx!(db, [funf], |u| { u.search().find(&funf).await })
        .await
        .expect("Suche");
    assert_eq!(voll.rows.len(), 5);
    assert!(!voll.truncated);
    temp.dispose().await;
}

#[tokio::test]
async fn findet_einen_alten_treffer_hinter_vielen_neueren_zeilen_ohne_treffer() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;
    suchticket(
        &temp,
        &Suchticket {
            created_at: "2026-01-01T00:00:00.000Z".into(),
            ..Suchticket::new("ARC-0999", 999, "Nadel im Heu")
        },
    )
    .await;
    for n in 0..450 {
        suchticket(
            &temp,
            &Suchticket::new(&format!("ARC-{}", 1000 + n), 1000 + n, "Füller"),
        )
        .await;
    }

    let nur_tickets = SearchRequest {
        kinds: vec![SearchKind::Ticket],
        ..anfrage(&["nadel"])
    };
    assert_eq!(
        form(&finde(&temp, nur_tickets).await),
        [(SearchKind::Ticket, "ARC-0999", SearchField::Title)]
    );
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_ohne_begriffe_oder_ohne_arten_nichts() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    seed_suche(&temp).await;
    let db = temp.db();

    let ohne_begriffe = anfrage(&[]);
    let leer = tx!(db, [ohne_begriffe], |u| {
        u.search().find(&ohne_begriffe).await
    })
    .await
    .expect("Suche");
    assert!(leer.rows.is_empty());
    assert!(!leer.truncated);

    let ohne_arten = SearchRequest {
        kinds: Vec::new(),
        ..anfrage(&["x"])
    };
    let leer = tx!(db, [ohne_arten], |u| { u.search().find(&ohne_arten).await })
        .await
        .expect("Suche");
    assert!(leer.rows.is_empty());
    assert!(!leer.truncated);
    temp.dispose().await;
}
