//! Gegenprobe TypeScript ↔ Rust auf derselben SQLite-Datei.
//!
//! Der Datensatz, das Anlegen und Auslesen über die Rust-Repositories, der Aufruf des
//! Node-Skripts und der Vergleich der Abzüge stehen in `common/crosscheck.rs`; hier steht nur,
//! was die Datei-Datenbank betrifft – samt den beiden Fällen, in denen zwei Prozesse
//! gleichzeitig auf eine Datei schreiben.
//!
//! Die TypeScript-Seite ist `tests/crosscheck/crosscheck.mjs`; sie arbeitet über
//! `packages/db/dist`, deshalb braucht jeder Fall vorher `npm run build`. Alle Fälle sind
//! darum `#[ignore]` und laufen nur mit `-- --ignored`.

use std::collections::BTreeMap;

use serde_json::Value;
use tempfile::TempDir;

use nexus_core::board::database::{BoardDatabase, TicketFilter};
use nexus_core::board::types::Ticket;
use nexus_core::{tx, Error};
use nexus_db::sqlite::SqliteBoardDatabase;

mod common;

use common::crosscheck::{
    assert_gleich, dump_rust, node, node_result, node_spawn, seed_rust, Dataset,
};

/// Der Benutzer, mit dem `init`, `burst` und `bump` des Node-Skripts arbeiten
const BURST_USER: &str = "11111111-1111-1111-1111-111111111111";
/// Sein Name
const BURST_USER_NAME: &str = "Flo Öztürk 🙂";
/// Zeitpunkt der Grunddaten von `init` (gleich auf beiden Seiten)
const BURST_NOW: &str = "2026-10-13T10:00:00.000Z";

// =============================================================================== Helfer

/// Ein temporärer Ordner mit dem Pfad einer noch nicht angelegten `board.db`
fn temp_file() -> (TempDir, String) {
    let dir = TempDir::with_prefix("nexus-crosscheck-").expect("temporärer Ordner");
    let file = dir.path().join("board.db").to_string_lossy().to_string();
    (dir, file)
}

/// Öffnet die Datei, führt die Arbeit aus und schließt wieder – die andere Seite braucht sie frei
async fn with_database<T, F>(file: &str, work: F) -> T
where
    F: AsyncFnOnce(&SqliteBoardDatabase) -> T,
{
    let db = SqliteBoardDatabase::open(file).expect("board.db öffnen");
    let ergebnis = work(&db).await;
    db.close().await.expect("schließen");
    ergebnis
}

/// Die Struktur der Datei, wie `sqlite_master` sie meldet (ohne die Reihenfolge des Anlegens)
fn schema_of(file: &str) -> Vec<(String, String, String, Option<String>)> {
    let conn = rusqlite::Connection::open(file).expect("Datei öffnen");
    let mut stmt = conn
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name")
        .expect("sqlite_master");
    let zeilen = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .expect("Zeilen")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("Zeilen lesen");
    drop(stmt);
    drop(conn);
    zeilen
}

/// Die IDs der eingespielten Schema-Stufen
fn schema_steps(file: &str) -> Vec<String> {
    let conn = rusqlite::Connection::open(file).expect("Datei öffnen");
    let mut stmt = conn
        .prepare("SELECT Id FROM SchemaSteps ORDER BY Id")
        .expect("SchemaSteps");
    let zeilen = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("Zeilen")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("Zeilen lesen");
    drop(stmt);
    drop(conn);
    zeilen
}

/// Legt über die Rust-Repositories `count` Tickets an, jedes in einer eigenen Transaktion
/// (wie `burst` auf der TypeScript-Seite)
async fn burst_rust(
    db: &SqliteBoardDatabase,
    count: i32,
    prefix: &str,
) -> (Vec<String>, Vec<String>) {
    let mut ids = Vec::new();
    let mut errors = Vec::new();
    for i in 1..=count {
        let titel = format!("{prefix} {i}");
        let ergebnis = tx!(db, [titel], |u| {
            let nummer = u.projects().take_ticket_number(1).await?;
            let ticket = burst_ticket(nummer, &titel);
            u.tickets().insert(&ticket).await?;
            Ok(ticket.id)
        })
        .await;
        match ergebnis {
            Ok(id) => ids.push(id),
            Err(error) => errors.push(
                error
                    .code()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("unknown: {error}")),
            ),
        }
    }
    (ids, errors)
}

/// Ein Ticket mit Mindestangaben für `burst_rust` (gleiche Felder wie im Node-Skript)
fn burst_ticket(nummer: i32, titel: &str) -> Ticket {
    Ticket {
        id: format!("ARC-{nummer:04}"),
        project_id: 1,
        board_id: 1,
        number: nummer,
        title: titel.to_string(),
        description: String::new(),
        status: nexus_core::board::basics::TicketStatus::Backlog,
        priority: nexus_core::board::basics::Priority::Normal,
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
        created_at: BURST_NOW.into(),
        created_by_id: BURST_USER.into(),
        created_by_name: BURST_USER_NAME.into(),
        updated_at: BURST_NOW.into(),
        rev: 1,
    }
}

/// Ein Update über die Rust-Repositories, das `rev` als aktuelle Rev erwartet;
/// Ergebnis `ok` oder der Fehlercode (wie `bump` auf der TypeScript-Seite)
async fn bump_rust(db: &SqliteBoardDatabase, id: &str, rev: i32) -> String {
    let id = id.to_string();
    let ergebnis = tx!(db, [id], |u| {
        let ticket = u
            .tickets()
            .by_id(&id, true)
            .await?
            .ok_or_else(|| Error::unexpected_message(format!("Ticket {id} fehlt")))?;
        u.tickets()
            .update(&Ticket {
                title: "Von Rust".into(),
                rev: rev + 1,
                ..ticket
            })
            .await
    })
    .await;
    match ergebnis {
        Ok(()) => "ok".to_string(),
        Err(error) => error
            .code()
            .map(str::to_string)
            .unwrap_or_else(|| format!("unknown: {error}")),
    }
}

// =============================================================================== Fälle

#[tokio::test]
#[ignore = "braucht node und npm run build"]
async fn typescript_schreibt_rust_liest_dasselbe() {
    let (_dir, file) = temp_file();
    let ts = node(&["seed", &file]);
    assert_eq!(ts["result"], Value::String("ok".into()));

    let rust = with_database(&file, async |db| dump_rust(db).await.expect("dump_rust")).await;
    let typescript = node(&["dump", &file]);
    assert_gleich(&rust, &typescript, "ts-seed");
}

#[tokio::test]
#[ignore = "braucht node und npm run build"]
async fn rust_schreibt_typescript_liest_dasselbe() {
    let (_dir, file) = temp_file();
    let rust_abzug = with_database(&file, async |db| {
        seed_rust(db).await.expect("seed_rust");
        dump_rust(db).await.expect("dump_rust")
    })
    .await;
    let typescript = node(&["dump", &file]);
    assert_gleich(&rust_abzug, &typescript, "rust-seed");

    // Und derselbe Datensatz, von TypeScript angelegt, ergibt denselben Abzug
    let (_dir_ts, file_ts) = temp_file();
    node(&["seed", &file_ts]);
    let ts_abzug = node(&["dump", &file_ts]);
    assert_gleich(&rust_abzug, &ts_abzug, "rust-seed gegen ts-seed");
}

#[tokio::test]
#[ignore = "braucht node und npm run build"]
async fn beide_seiten_legen_dasselbe_schema_an() {
    let (_dir_rust, file_rust) = temp_file();
    with_database(&file_rust, async |_db| ()).await;
    let (_dir_ts, file_ts) = temp_file();
    node(&["init", &file_ts]);

    assert_eq!(schema_of(&file_rust), schema_of(&file_ts));
    assert_eq!(schema_steps(&file_rust), schema_steps(&file_ts));
    assert!(!schema_steps(&file_rust).is_empty());

    // TypeScript öffnet die von Rust angelegte Datei, ohne eine Stufe nachzuziehen
    let vorher = schema_steps(&file_rust);
    node(&["dump", &file_rust]);
    assert_eq!(schema_steps(&file_rust), vorher);
}

#[tokio::test]
#[ignore = "braucht node und npm run build"]
async fn zwei_prozesse_legen_je_funfzig_tickets_ohne_lucke_an() {
    let (_dir, file) = temp_file();
    let init = node(&["init", &file]);
    assert_eq!(init["boardId"], Value::from(1));

    // Node läuft los, Rust legt währenddessen über eine eigene Verbindung an
    let child = node_spawn(&["burst", &file, "50", "TS"]);
    let (rust_ids, rust_errors) =
        with_database(&file, async |db| burst_rust(db, 50, "RS").await).await;
    let ts = node_result(child, "burst");

    let ts_errors: Vec<String> = serde_json::from_value(ts["errors"].clone()).expect("errors");
    let ts_ids: Vec<String> = serde_json::from_value(ts["ids"].clone()).expect("ids");
    assert_eq!(rust_errors, Vec::<String>::new());
    assert_eq!(ts_errors, Vec::<String>::new());
    assert_eq!(rust_ids.len(), 50);
    assert_eq!(ts_ids.len(), 50);

    let (tickets, next) = with_database(&file, async |db| {
        tx!(db, |u| {
            let tickets = u.tickets().list(&TicketFilter::default()).await?;
            let next = u.projects().list().await?[0].next_ticket_number;
            Ok((tickets, next))
        })
        .await
        .expect("lesen")
    })
    .await;

    assert_eq!(tickets.len(), 100);
    let mut nummern: Vec<i32> = tickets.iter().map(|t| t.number).collect();
    nummern.sort_unstable();
    assert_eq!(nummern, (1..=100).collect::<Vec<i32>>());
    let ids: BTreeMap<String, ()> = tickets.iter().map(|t| (t.id.clone(), ())).collect();
    for id in rust_ids.iter().chain(&ts_ids) {
        assert!(ids.contains_key(id), "Ticket {id} fehlt in der Datei");
    }
    assert_eq!(next, 101);
}

#[tokio::test]
#[ignore = "braucht node und npm run build"]
async fn gleiche_rev_aus_zwei_prozessen_ergibt_genau_einen_gewinner() {
    let (_dir, file) = temp_file();
    node(&["init", &file]);
    let id = with_database(&file, async |db| {
        let (ids, errors) = burst_rust(db, 1, "Start").await;
        assert_eq!(errors, Vec::<String>::new());
        ids[0].clone()
    })
    .await;

    let child = node_spawn(&["bump", &file, &id, "1"]);
    let rust = with_database(&file, async |db| bump_rust(db, &id, 1).await).await;
    let ts = node_result(child, "bump");

    let mut ergebnisse = vec![
        rust.clone(),
        ts["result"].as_str().expect("result").to_string(),
    ];
    ergebnisse.sort();
    assert_eq!(ergebnisse, vec!["conflict".to_string(), "ok".to_string()]);

    let gewinner = if rust == "ok" {
        "Von Rust"
    } else {
        "Von TypeScript"
    };
    let ticket = with_database(&file, async |db| {
        let id = id.clone();
        tx!(db, [id], |u| { u.tickets().by_id(&id, false).await })
            .await
            .expect("lesen")
            .expect("Ticket")
    })
    .await;
    assert_eq!(ticket.rev, 2);
    assert_eq!(ticket.title, gewinner);
}

/// Der Datensatz bleibt lesbar – die gemeinsame Form wird von beiden Gegenproben benutzt
#[test]
fn der_datensatz_lasst_sich_lesen() {
    let data: std::sync::Arc<Dataset> = common::crosscheck::dataset();
    assert!(!data.ticket_ids().is_empty());
    assert_eq!(data.project_ids(), [1, 2]);
}
