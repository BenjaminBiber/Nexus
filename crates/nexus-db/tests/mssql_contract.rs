//! Die drei gemeinsamen Verträge gegen SQL Server; Vorbild `packages/db/tests/
//! MssqlBoardDatabase.test.ts` und `MssqlQuestionRepository.test.ts`, die dieselben
//! Verträge gegen die Test-Datenbank laufen lassen.
//!
//! Geöffnet wird über die Fabrik (`open_checked`), damit die Verträge genau den Weg nehmen,
//! den CLI und Server später gehen: Anbieterwahl, Schema-Prüfung, fehlende Erweiterungen und
//! der eigene Benutzer.
//!
//! Alle Fälle sind `#[ignore]`, weil sie eine echte Datenbank brauchen. Aufruf:
//! `HARNESS_TEST_DB='<json>' cargo test -p nexus-db --test mssql_contract -- --ignored --test-threads=1`
//!
//! Seriell, weil alle Fälle dieselbe Datenbank leeren: `--test-threads=1` und zusätzlich die
//! Sperre, die das Vertrags-Makro vor dem Leeren nimmt und bis zum Ende des Falls hält.

use nexus_core::board::config::BoardConfig;
use nexus_db::factory::AnyBoardDatabase;
use nexus_db::{open_checked, ConnectOptions};
use tokio::sync::{Mutex, MutexGuard};

mod common;

use common::test_db;

/// Hält die Test-Datenbank für genau einen Fall; Vorbild ist das serielle `describe` der
/// TypeScript-Tests
static LOCK: Mutex<()> = Mutex::const_new(());

/// Die Konfig der Test-Datenbank. Diese Fälle laufen nur mit `--ignored`, also absichtlich –
/// fehlt die Variable, ist der Abbruch mit klarer Meldung richtig.
fn config() -> BoardConfig {
    test_db::config().unwrap_or_else(|| {
        panic!(
            "{} fehlt – diese Verträge brauchen eine SQL-Server-Test-Datenbank.",
            test_db::ENV_VAR
        )
    })
}

/// Öffnet die Board-Datenbank über die Fabrik (Anbieter `mssql` aus der Konfig)
async fn connect_test_db() -> AnyBoardDatabase {
    open_checked(&config(), ConnectOptions::default())
        .await
        .expect("Verbindung zur Test-Datenbank")
        .db
}

/// Leert alle Tabellen des Schemas `board`
async fn reset_test_db() {
    test_db::reset(&config()).await;
}

/// Die Sperre für einen Fall; der Vertrag hält sie bis zu seinem Ende
async fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().await
}

nexus_core::board_database_contract!(
    mssql,
    || async { crate::connect_test_db().await },
    || async { crate::reset_test_db().await },
    || async { crate::lock().await },
    #[ignore = "braucht HARNESS_TEST_DB"]
);

nexus_core::question_repository_contract!(
    mssql_questions,
    || async { crate::connect_test_db().await },
    || async { crate::reset_test_db().await },
    || async { crate::lock().await },
    #[ignore = "braucht HARNESS_TEST_DB"]
);

nexus_core::dispatch_run_contract!(
    mssql_runs,
    || async { crate::connect_test_db().await },
    || async { crate::reset_test_db().await },
    || async { crate::lock().await },
    #[ignore = "braucht HARNESS_TEST_DB"]
);
