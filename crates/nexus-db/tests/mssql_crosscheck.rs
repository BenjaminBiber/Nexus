//! Gegenprobe TypeScript ↔ Rust auf derselben SQL-Server-Datenbank (`AiHarness_Test`).
//!
//! Beide Seiten legen den festen Datensatz aus `tests/crosscheck/dataset.json` an und lesen ihn
//! über ihre Repositories wieder aus; verglichen wird kanonisches JSON. Der Datensatz, das
//! Anlegen, das Auslesen und der Vergleich stehen in `common/crosscheck.rs` – dieselben
//! Funktionen wie bei der SQLite-Gegenprobe.
//!
//! Aufruf (Node >= 22.13, vorher `npm run build`, weil die TypeScript-Seite über
//! `packages/db/dist` arbeitet):
//! `HARNESS_TEST_DB='<json>' cargo test -p nexus-db --test mssql_crosscheck -- --ignored --test-threads=1`
//!
//! Seriell und mit `reset` vor jedem Fall, weil die Verträge und beide Seiten dieselbe
//! Datenbank benutzen. `DELETE` setzt die `IDENTITY`-Zähler nicht zurück – das macht
//! `test_db::reset_with_identities`, damit Projekte, Boards, Kommentare, Notizen und
//! Recherchen auf beiden Seiten dieselben Ids bekommen.

use nexus_core::board::config::BoardConfig;
use nexus_core::board::database::BoardDatabase;
use nexus_db::factory::AnyBoardDatabase;
use nexus_db::{open_checked, ConnectOptions};
use serde_json::Value;

mod common;

use common::crosscheck::{assert_gleich, dump_rust, node, seed_rust};
use common::test_db;

/// Die Konfig der Test-Datenbank; `None`, solange es keine gibt (dann wird der Fall gemeldet
/// und übersprungen – auch mit `--ignored` ist das die einzige sinnvolle Antwort)
fn config_or_skip(test: &str) -> Option<BoardConfig> {
    test_db::config_or_skip(test)
}

/// Leert die Datenbank samt Id-Zählern und öffnet sie über die Fabrik
async fn frisch(config: &BoardConfig) -> AnyBoardDatabase {
    test_db::reset_with_identities(config).await;
    open_checked(config, ConnectOptions::default())
        .await
        .expect("Verbindung zur Test-Datenbank")
        .db
}

/// Öffnet die Datenbank ohne zu leeren – die andere Seite hat gerade geschrieben
async fn verbinden(config: &BoardConfig) -> AnyBoardDatabase {
    open_checked(config, ConnectOptions::default())
        .await
        .expect("Verbindung zur Test-Datenbank")
        .db
}

/// Die Kennwerte beider Abzüge; sie zeigen Abweichungen am deutlichsten
fn marker(abzug: &Value) -> Value {
    Value::Array(vec![
        abzug["changeMarker"].clone(),
        abzug["questionMarker"].clone(),
    ])
}

#[tokio::test]
#[ignore = "braucht HARNESS_TEST_DB, node und npm run build"]
async fn typescript_schreibt_rust_liest_dasselbe() {
    let Some(config) = config_or_skip("typescript_schreibt_rust_liest_dasselbe") else {
        return;
    };
    test_db::reset_with_identities(&config).await;

    let ts = node(&["seed", "-", "--provider", "mssql"]);
    assert_eq!(ts["result"], Value::String("ok".into()));

    let db = verbinden(&config).await;
    let rust = dump_rust(&db).await.expect("dump_rust");
    db.close().await.expect("schließen");

    let typescript = node(&["dump", "-", "--provider", "mssql"]);
    assert_gleich(&marker(&rust), &marker(&typescript), "ts-seed Kennwerte");
    assert_gleich(&rust, &typescript, "ts-seed");
}

#[tokio::test]
#[ignore = "braucht HARNESS_TEST_DB, node und npm run build"]
async fn rust_schreibt_typescript_liest_dasselbe() {
    let Some(config) = config_or_skip("rust_schreibt_typescript_liest_dasselbe") else {
        return;
    };
    let db = frisch(&config).await;
    seed_rust(&db).await.expect("seed_rust");
    let rust = dump_rust(&db).await.expect("dump_rust");
    db.close().await.expect("schließen");

    let typescript = node(&["dump", "-", "--provider", "mssql"]);
    assert_gleich(&marker(&rust), &marker(&typescript), "rust-seed Kennwerte");
    assert_gleich(&rust, &typescript, "rust-seed");
}

#[tokio::test]
#[ignore = "braucht HARNESS_TEST_DB, node und npm run build"]
async fn beide_seiten_legen_denselben_datensatz_an() {
    let Some(config) = config_or_skip("beide_seiten_legen_denselben_datensatz_an") else {
        return;
    };
    // Erst Rust anlegen und abziehen …
    let db = frisch(&config).await;
    seed_rust(&db).await.expect("seed_rust");
    let rust_abzug = dump_rust(&db).await.expect("dump_rust");
    db.close().await.expect("schließen");

    // … dann aus derselben Ausgangslage TypeScript. Die Ids kommen aus `IDENTITY` und werden
    // erst durch das Zurücksetzen der Zähler wieder vergleichbar.
    test_db::reset_with_identities(&config).await;
    node(&["seed", "-", "--provider", "mssql"]);
    let ts_abzug = node(&["dump", "-", "--provider", "mssql"]);

    assert_gleich(&rust_abzug, &ts_abzug, "rust-seed gegen ts-seed");
}
