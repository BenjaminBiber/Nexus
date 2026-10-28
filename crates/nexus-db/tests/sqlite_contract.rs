//! Die drei gemeinsamen Verträge gegen SQLite; Vorbild
//! `packages/db/tests/sqlite/SqliteBoardDatabaseContract.test.ts`,
//! `SqliteQuestionContract.test.ts` und `SqliteDispatchRunContract.test.ts`.
//!
//! Jeder Fall bekommt eine frische Datei in einem eigenen temporären Ordner. Der Ordner muss
//! bis zum Ende des Falls leben; weil die Makros nur die Datenbank durchreichen, bleibt er
//! bewusst liegen (`std::mem::forget`) und das Betriebssystem räumt das Temp-Verzeichnis auf.

use nexus_db::sqlite::SqliteBoardDatabase;
use tempfile::TempDir;

/// Frische Board-Datei in einem eigenen temporären Ordner
fn open_fresh() -> SqliteBoardDatabase {
    let dir = TempDir::with_prefix("nexus-sqlite-contract-").expect("temporärer Ordner");
    let file = dir.path().join("board.db").to_string_lossy().to_string();
    let db = SqliteBoardDatabase::open(&file).expect("board.db öffnen");
    // Der Ordner darf nicht vor dem Testende verschwinden; der Vertrag hält nur die Datenbank.
    std::mem::forget(dir);
    db
}

nexus_core::board_database_contract!(sqlite, || async { crate::open_fresh() });

nexus_core::question_repository_contract!(sqlite_questions, || async { crate::open_fresh() });

nexus_core::dispatch_run_contract!(sqlite_runs, || async { crate::open_fresh() });
