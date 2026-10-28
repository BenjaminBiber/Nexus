//! Öffnen, PRAGMAs, Warteschlange, Transaktionen, Kennwerte und Schließen; portiert aus
//! `packages/db/tests/sqlite/SqliteBoardDatabase.test.ts`. Dazu die Gleichheit mit einer von
//! TypeScript angelegten Datei und die SQLite-Fälle aus `packages/db/tests/SqlErrorMapper.test.ts`.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::temp_sqlite::{SeedTicket, TempSqlite, TEST_NOW, TEST_USER_ID, TEST_USER_NAME};

use nexus_core::board::database::BoardDatabase;
use nexus_core::board::BoardProvider;
use nexus_db::sqlite::{error_map, SqliteBoardDatabase};
use nexus_db::sqlite_params;
use nexus_db::{ManagedBoardDatabase, SchemaVersionGuard};
use rusqlite::{Connection, OpenFlags};

/// Die vom TypeScript-Stand angelegte Board-Datei (siehe `tests/fixtures/README.md`)
const TS_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ts-board.db");

/// Ein zweiter Benutzer für Tests mit zwei Schreibern
const USER_B: &str = "22222222-2222-2222-2222-222222222222";

/// `type`, `name` und `sql` aller Einträge in `sqlite_master`, nach Typ und Namen sortiert
fn schema_eintrage(conn: &Connection) -> Vec<(String, String, Option<String>)> {
    let mut statement = conn
        .prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name")
        .expect("prepare");
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("query")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("Zeilen")
}

#[tokio::test]
async fn legt_ordner_und_datei_an_und_setzt_die_pragmas() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let verschachtelt = temp
        .dir()
        .join("a")
        .join("b")
        .join("board.db")
        .to_string_lossy()
        .to_string();
    let db = SqliteBoardDatabase::open(&verschachtelt).expect("öffnen");
    assert!(std::path::Path::new(&verschachtelt).exists());
    db.close().await.expect("schließen");

    assert_eq!(
        temp.text("PRAGMA journal_mode", Vec::new())
            .await
            .expect("journal_mode"),
        Some("wal".to_string())
    );
    assert_eq!(
        temp.number("PRAGMA foreign_keys", Vec::new())
            .await
            .expect("foreign_keys"),
        Some(1)
    );
    assert_eq!(
        temp.number("PRAGMA busy_timeout", Vec::new())
            .await
            .expect("busy_timeout"),
        Some(10000)
    );
    temp.dispose().await;
}

#[tokio::test]
async fn meldet_schema_version_3_und_alle_erweiterungen() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let version = temp.db().schema_version().await.expect("Version");
    let features = temp.db().schema_features().await.expect("Erweiterungen");
    assert_eq!(version, Some(3));
    assert!(
        features.contains(&"dispatch-runs".to_string()),
        "{features:?}"
    );

    let fehlend = SchemaVersionGuard
        .check(version, temp.file(), &features, BoardProvider::Sqlite)
        .expect("kein Fehler");
    assert!(fehlend.is_empty(), "{fehlend:?}");

    // Bei SQLite gibt es nach dem Öffnen nichts nachzutragen und nichts zu merken
    assert!(temp
        .db()
        .ensure_schema_features(&["dispatch-runs".to_string()])
        .await
        .expect("nachtragen")
        .is_empty());
    temp.db().set_missing_features(Vec::new());
    temp.dispose().await;
}

#[tokio::test]
async fn offnet_eine_vorhandene_datei_erneut_ohne_datenverlust() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");
    temp.db().close().await.expect("schließen");

    let wieder = SqliteBoardDatabase::open(temp.file()).expect("erneut öffnen");
    let anzahl = wieder
        .run_in_transaction(|runner| {
            Box::pin(async move {
                let zeilen =
                    runner.query_rows("SELECT COUNT(*) AS N FROM Users", &Vec::new(), |row| {
                        row.get::<_, i64>(0)
                    })?;
                Ok(zeilen.into_iter().next().unwrap_or_default())
            })
        })
        .await
        .expect("lesen");
    assert_eq!(anzahl, 1);
    assert_eq!(wieder.schema_version().await.expect("Version"), Some(3));
    wieder.close().await.expect("schließen");
    temp.dispose().await;
}

#[tokio::test]
async fn fuhrt_gleichzeitige_transaktionen_nacheinander_aus() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let folge = Arc::new(Mutex::new(Vec::<&'static str>::new()));

    let folge_a = Arc::clone(&folge);
    let a = temp.db().run_in_transaction(move |_runner| {
        let folge = Arc::clone(&folge_a);
        Box::pin(async move {
            folge.lock().unwrap().push("a1");
            tokio::time::sleep(Duration::from_millis(30)).await;
            folge.lock().unwrap().push("a2");
            Ok(())
        })
    });
    let folge_b = Arc::clone(&folge);
    let b = temp.db().run_in_transaction(move |_runner| {
        let folge = Arc::clone(&folge_b);
        Box::pin(async move {
            folge.lock().unwrap().push("b1");
            folge.lock().unwrap().push("b2");
            Ok(())
        })
    });
    let (a, b) = tokio::join!(a, b);
    a.expect("a");
    b.expect("b");

    assert_eq!(*folge.lock().unwrap(), ["a1", "a2", "b1", "b2"]);
    temp.dispose().await;
}

#[tokio::test]
async fn serialisiert_zwei_instanzen_auf_derselben_datei_ohne_zu_blockieren() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let zweite = SqliteBoardDatabase::open(temp.file()).expect("zweite Instanz");

    // Eine nebenläufige Aufgabe zählt mit: wird die Laufzeit blockiert, kommt sie nicht dran
    let ticks = Arc::new(AtomicUsize::new(0));
    let zahler = Arc::clone(&ticks);
    let uhr = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(10)).await;
            zahler.fetch_add(1, Ordering::SeqCst);
        }
    });

    let a = temp.db().run_in_transaction(|runner| {
        Box::pin(async move {
            runner.execute(
                "INSERT INTO Users (Id, Name) VALUES (@id, @name)",
                &sqlite_params! { "id": TEST_USER_ID, "name": "A" },
            )?;
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(())
        })
    });
    let b = zweite.run_in_transaction(|runner| {
        Box::pin(async move {
            runner.execute(
                "INSERT INTO Users (Id, Name) VALUES (@id, @name)",
                &sqlite_params! { "id": USER_B, "name": "B" },
            )?;
            Ok(())
        })
    });
    let (a, b) = tokio::join!(a, b);
    a.expect("a");
    b.expect("b");
    uhr.abort();

    assert!(ticks.load(Ordering::SeqCst) > 5, "{ticks:?}");
    assert_eq!(temp.count("Users").await.expect("Benutzer"), 2);
    zweite.close().await.expect("schließen");
    temp.dispose().await;
}

#[tokio::test]
async fn liest_ohne_auf_die_schreibsperre_eines_anderen_prozesses_zu_warten() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let anderer = temp.raw();
    anderer
        .execute_batch("BEGIN IMMEDIATE")
        .expect("Schreibsperre");

    let beginn = std::time::Instant::now();
    assert_eq!(temp.count("Users").await.expect("lesen"), 0);
    assert!(beginn.elapsed() < Duration::from_secs(2), "{beginn:?}");

    anderer.execute_batch("ROLLBACK").expect("freigeben");
    drop(anderer);
    temp.dispose().await;
}

#[tokio::test]
async fn wiederholt_die_arbeit_mit_schreibsperre_nach_einem_fremden_schreibvorgang() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let anderer = Arc::new(Mutex::new(Some(temp.raw())));
    let laufe = Arc::new(AtomicUsize::new(0));

    let laufe_in_arbeit = Arc::clone(&laufe);
    let anderer_in_arbeit = Arc::clone(&anderer);
    temp.db()
        .run_in_transaction(move |runner| {
            let laufe = Arc::clone(&laufe_in_arbeit);
            let anderer = Arc::clone(&anderer_in_arbeit);
            Box::pin(async move {
                let lauf = laufe.fetch_add(1, Ordering::SeqCst) + 1;
                runner.query_rows("SELECT COUNT(*) AS N FROM Users", &Vec::new(), |row| {
                    row.get::<_, i64>(0)
                })?;
                // Nur im ersten Durchlauf schreibt der andere Prozess dazwischen
                if lauf == 1 {
                    let guard = anderer.lock().unwrap();
                    guard
                        .as_ref()
                        .expect("Roh-Verbindung")
                        .execute_batch(
                            "INSERT INTO Users (Id, Name) VALUES ('22222222-2222-2222-2222-222222222222', 'B')",
                        )
                        .expect("fremder Schreibvorgang");
                }
                runner.execute(
                    "INSERT INTO Users (Id, Name) VALUES (@id, @name)",
                    &sqlite_params! { "id": TEST_USER_ID, "name": "A" },
                )?;
                Ok(())
            })
        })
        .await
        .expect("Wiederholung gelingt");

    assert_eq!(laufe.load(Ordering::SeqCst), 2);
    anderer.lock().unwrap().take();
    assert_eq!(temp.count("Users").await.expect("Benutzer"), 2);
    temp.dispose().await;
}

#[tokio::test]
async fn rollt_bei_einem_fehler_zuruck() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let fehler = temp
        .db()
        .run_in_transaction(|runner| {
            Box::pin(async move {
                runner.execute(
                    "INSERT INTO Users (Id, Name) VALUES (@id, @name)",
                    &sqlite_params! { "id": TEST_USER_ID, "name": "X" },
                )?;
                Err::<(), _>(nexus_core::Error::unexpected_message("boom"))
            })
        })
        .await
        .expect_err("boom");
    assert_eq!(fehler.to_string(), "boom");

    assert_eq!(temp.count("Users").await.expect("Benutzer"), 0);
    // Die Verbindung ist danach wieder nutzbar
    assert_eq!(
        temp.seed_user().await.expect("Benutzer"),
        TEST_USER_ID.to_string()
    );
    temp.dispose().await;
}

#[tokio::test]
async fn gibt_nach_einem_sql_fehler_die_verbindung_frei() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let board = temp.seed_board(None, "Sprint").await.expect("Board");

    let fehler = temp
        .seed_ticket(&SeedTicket::new("ARC-1", &board, 1).with_status("falsch"))
        .await
        .expect_err("ungültiger Status");
    assert!(fehler.to_string().contains("CK_Tickets_Status"), "{fehler}");

    temp.seed_ticket(&SeedTicket::new("ARC-1", &board, 1))
        .await
        .expect("Ticket danach");
    temp.dispose().await;
}

#[tokio::test]
async fn liefert_eine_unit_of_work_mit_allen_17_repositories() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    // Kompiliertest der 17 Zugriffe: jeder meldet (noch) not-implemented
    let codes = temp
        .db()
        .transaction(|u| {
            Box::pin(async move {
                let mut codes: Vec<String> = Vec::new();
                let mut merke = |fehler: nexus_core::Error| {
                    codes.push(fehler.code().unwrap_or_default().to_string());
                };
                merke(u.users().list().await.unwrap_err());
                merke(u.projects().list().await.unwrap_err());
                merke(
                    u.boards()
                        .list(&nexus_core::board::database::BoardFilter::default())
                        .await
                        .unwrap_err(),
                );
                merke(
                    u.tickets()
                        .list(&nexus_core::board::database::TicketFilter::default())
                        .await
                        .unwrap_err(),
                );
                merke(u.comments().list("ARC-1").await.unwrap_err());
                merke(u.steps().list("ARC-1").await.unwrap_err());
                merke(u.ticket_sessions().tickets_of("sess-1").await.unwrap_err());
                merke(u.history().list("ARC-1").await.unwrap_err());
                merke(
                    u.notes()
                        .list(&nexus_core::board::database::NoteFilter::of_board(1))
                        .await
                        .unwrap_err(),
                );
                merke(u.deletions().list().await.unwrap_err());
                merke(
                    u.questions()
                        .list(&nexus_core::board::database::BoardQuestionFilter::default())
                        .await
                        .unwrap_err(),
                );
                merke(u.question_research().list("ARC-F1").await.unwrap_err());
                merke(u.question_comments().list("ARC-F1").await.unwrap_err());
                merke(u.question_history().list("ARC-F1").await.unwrap_err());
                merke(
                    u.search()
                        .find(&nexus_core::board::search_types::SearchRequest {
                            terms: vec!["x".to_string()],
                            kinds: nexus_core::board::search_types::SEARCH_KINDS.to_vec(),
                            per_source: 5,
                        })
                        .await
                        .unwrap_err(),
                );
                merke(u.dispatch_runs().existing(&[]).await.unwrap_err());
                merke(u.dispatch_stats().dispatch_comments(1).await.unwrap_err());
                Ok(codes)
            })
        })
        .await
        .expect("Transaktion");

    assert_eq!(codes.len(), 17);
    assert!(
        codes.iter().all(|code| code == "not-implemented"),
        "{codes:?}"
    );
    temp.dispose().await;
}

#[tokio::test]
async fn andert_den_change_marker_bei_insert_update_und_delete() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let a = temp.seed_board(None, "Sprint").await.expect("Board A");
    let b = temp
        .seed_board(Some(a.project_id), "Andere")
        .await
        .expect("Board B");

    let m0 = temp.db().change_marker(a.board_id).await.expect("Marker");
    let mb = temp.db().change_marker(b.board_id).await.expect("Marker");
    let teile: Vec<&str> = m0.split(':').collect();
    assert_eq!(teile.len(), 8, "{m0}");
    assert_eq!(
        [teile[0], teile[1], teile[2], teile[3], teile[5], teile[6], teile[7]],
        ["0", "-", "-", "-", "0", "-", "0"],
        "{m0}"
    );
    assert!(teile[4].parse::<i64>().is_ok(), "{m0}");

    temp.seed_ticket(&SeedTicket::new("ARC-1", &a, 1))
        .await
        .expect("Ticket");
    let m1 = temp.db().change_marker(a.board_id).await.expect("Marker");
    assert_ne!(m1, m0);

    temp.execute(
        "UPDATE Tickets SET Title = 'Neu' WHERE Id = 'ARC-1'",
        Vec::new(),
    )
    .await
    .expect("Update");
    let m2 = temp.db().change_marker(a.board_id).await.expect("Marker");
    assert_ne!(m2, m1);

    temp.execute(
        "INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt) VALUES ('ARC-1', @u, 'Test', 'Hallo', @now)",
        sqlite_params! { "u": a.user_id.clone(), "now": TEST_NOW },
    )
    .await
    .expect("Kommentar");
    let m3 = temp.db().change_marker(a.board_id).await.expect("Marker");
    assert_ne!(m3, m2);

    temp.execute("DELETE FROM Tickets WHERE Id = 'ARC-1'", Vec::new())
        .await
        .expect("Delete");
    let m4 = temp.db().change_marker(a.board_id).await.expect("Marker");
    assert_ne!(m4, m3);

    // Das andere Board bleibt unberührt
    assert_eq!(
        temp.db().change_marker(b.board_id).await.expect("Marker"),
        mb
    );
    temp.dispose().await;
}

#[tokio::test]
async fn lasst_den_marker_bei_rollback_unverandert() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let a = temp.seed_board(None, "Sprint").await.expect("Board");
    let m0 = temp.db().change_marker(a.board_id).await.expect("Marker");

    let project_id = a.project_id;
    let board_id = a.board_id;
    let fehler = temp
        .db()
        .run_in_transaction(move |runner| {
            Box::pin(async move {
                runner.execute(
                    "INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Status, Priority, SortOrder, NeedsPlanning,
                       CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev)
                     VALUES ('ARC-9', @projectId, @boardId, 9, 'Titel', 'backlog', 'normal', 0, 0, @now, @userId, @userName, @now, 1)",
                    &sqlite_params! {
                        "projectId": project_id,
                        "boardId": board_id,
                        "now": TEST_NOW,
                        "userId": TEST_USER_ID,
                        "userName": TEST_USER_NAME,
                    },
                )?;
                Err::<(), _>(nexus_core::Error::unexpected_message("boom"))
            })
        })
        .await
        .expect_err("boom");
    assert_eq!(fehler.to_string(), "boom");
    assert_eq!(
        temp.db().change_marker(a.board_id).await.expect("Marker"),
        m0
    );
    temp.dispose().await;
}

#[tokio::test]
async fn andert_den_question_marker_mit_den_fragen_eines_projekts() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let project_id = temp.seed_project("ARC").await.expect("Projekt");
    assert_eq!(
        temp.db().question_marker(project_id).await.expect("Marker"),
        "0:-"
    );

    temp.execute(
        "INSERT INTO Questions (Id, ProjectId, Number, Title, State, OriginKind, OriginName, CreatedAt, CreatedById, UpdatedAt, Rev)
         VALUES ('ARC-Q1', @p, 1, 'Frage', 'open', 'human', 'Test', @now, @u, @now, 1)",
        sqlite_params! { "p": project_id, "now": TEST_NOW, "u": TEST_USER_ID },
    )
    .await
    .expect("Frage");
    let m1 = temp.db().question_marker(project_id).await.expect("Marker");
    let teile: Vec<&str> = m1.split(':').collect();
    assert_eq!(teile[0], "1", "{m1}");
    assert!(teile[1].parse::<i64>().is_ok(), "{m1}");

    temp.execute(
        "UPDATE Questions SET Title = 'Neu' WHERE Id = 'ARC-Q1'",
        Vec::new(),
    )
    .await
    .expect("Update");
    assert_ne!(
        temp.db().question_marker(project_id).await.expect("Marker"),
        m1
    );
    temp.dispose().await;
}

#[tokio::test]
async fn lasst_sich_mehrfach_schliessen_und_meldet_danach_board_db_closed() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let file = temp.dir().join("x.db").to_string_lossy().to_string();
    let db = SqliteBoardDatabase::open(&file).expect("öffnen");
    db.close().await.expect("erstes Schließen");
    db.close().await.expect("zweites Schließen");

    let fehler = db
        .run_in_transaction(|_runner| Box::pin(async move { Ok(1) }))
        .await
        .expect_err("geschlossen");
    assert_eq!(fehler.code(), Some("board-db-closed"));
    let fehler = db.change_marker(1).await.expect_err("geschlossen");
    assert_eq!(fehler.code(), Some("board-db-closed"));
    temp.dispose().await;
}

#[tokio::test]
async fn lasst_eine_roh_verbindung_bestatigte_daten_sehen() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");
    let raw = temp.raw();
    let anzahl: i64 = raw
        .query_row("SELECT COUNT(*) AS N FROM Users", [], |row| row.get(0))
        .expect("lesen");
    assert_eq!(anzahl, 1);
    let keys: i64 = raw
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("foreign_keys");
    assert_eq!(keys, 1);
    drop(raw);
    temp.dispose().await;
}

#[tokio::test]
async fn meldet_eine_nicht_zu_offnende_datei_verstandlich() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    // Ein Ordner ist keine Datei
    let fehler = SqliteBoardDatabase::open(&temp.dir().to_string_lossy())
        .err()
        .expect("ein Ordner ist keine Datei");
    assert_eq!(fehler.code(), Some("board-db-unreachable"));
    assert!(
        fehler.to_string().contains("lässt sich nicht öffnen"),
        "{fehler}"
    );
    temp.dispose().await;
}

// ---------------------------------------------------------------------------------------------
// Gleichheit mit der von TypeScript angelegten Datei
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn offnet_eine_von_typescript_angelegte_datei_ohne_eine_stufe_nachzuholen() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    let kopie = temp.dir().join("ts-kopie.db");
    std::fs::copy(TS_FIXTURE, &kopie).expect("Fixture kopieren");

    let db = SqliteBoardDatabase::open(&kopie.to_string_lossy()).expect("öffnen");
    let stufen = db
        .run_in_transaction(|runner| {
            Box::pin(async move {
                runner.query_rows(
                    "SELECT Id FROM SchemaSteps ORDER BY Id",
                    &Vec::new(),
                    |row| row.get::<_, String>(0),
                )
            })
        })
        .await
        .expect("Stufen lesen");

    // Keine neue Zeile: alle acht Stufen stehen schon drin
    assert_eq!(
        stufen,
        [
            "001_initial",
            "002_questions",
            "003_harness",
            "004_dispatch_runs",
            "005_rowver",
            "006_note_structure",
            "007_dispatch_run_details",
            "008_target_branch",
        ]
    );
    assert_eq!(db.schema_version().await.expect("Version"), Some(3));
    let mut features = db.schema_features().await.expect("Erweiterungen");
    features.sort();
    assert_eq!(
        features,
        [
            "dispatch-run-details",
            "dispatch-runs",
            "note-structure",
            "target-branch",
        ]
    );
    db.close().await.expect("schließen");
    temp.dispose().await;
}

#[tokio::test]
async fn legt_dasselbe_schema_an_wie_typescript() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    // Die Fixture nur lesen, und zwar als unveränderlich: sonst legt SQLite neben der
    // eingecheckten Datei die WAL-Dateien -wal und -shm an
    let ts = Connection::open_with_flags(
        format!("file:{TS_FIXTURE}?immutable=1"),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("Fixture öffnen");
    let von_ts = schema_eintrage(&ts);
    drop(ts);

    let rust = temp.raw();
    let von_rust = schema_eintrage(&rust);
    drop(rust);

    assert_eq!(von_rust.len(), von_ts.len(), "Zahl der Einträge");
    for (rust, ts) in von_rust.iter().zip(von_ts.iter()) {
        assert_eq!(rust, ts, "{}/{}", ts.0, ts.1);
    }
    temp.dispose().await;
}

// ---------------------------------------------------------------------------------------------
// Fehlerübersetzung an echten Fehlern der Datei (SQLite-Teil von SqlErrorMapper.test.ts)
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn erkennt_echte_fehler_der_datei() {
    let temp = TempSqlite::create().expect("Temp-Datei");
    temp.seed_user().await.expect("Benutzer");

    // Doppelter Primärschlüssel
    let fehler = temp
        .execute(
            "INSERT INTO Users (Id, Name) VALUES (@id, @name)",
            sqlite_params! { "id": TEST_USER_ID, "name": "noch mal" },
        )
        .await
        .expect_err("doppelt");
    assert!(error_map::is_unique_violation(&fehler), "{fehler}");
    // Ein doppelter Schlüssel bleibt roh: die Repositories deuten ihn fachlich
    assert!(error_map::map(fehler).harness().is_none());

    // Gesperrte Datei: die zweite Verbindung wartet nicht und meldet BUSY
    let halter = temp.raw();
    let wartet_nicht = Connection::open(temp.file()).expect("zweite Verbindung");
    halter.execute_batch("BEGIN IMMEDIATE").expect("Sperre");
    let roh = wartet_nicht
        .execute_batch("BEGIN IMMEDIATE")
        .expect_err("gesperrt");
    let fehler = error_map::wrap(roh);
    assert!(error_map::is_sqlite_busy(&fehler), "{fehler}");
    let ubersetzt = error_map::map(fehler);
    assert_eq!(ubersetzt.code(), Some("conflict"));
    assert_eq!(
        ubersetzt.to_string(),
        "Gleichzeitige Änderung – bitte erneut versuchen."
    );

    halter.execute_batch("ROLLBACK").expect("freigeben");
    drop(wartet_nicht);
    drop(halter);
    temp.dispose().await;
}
