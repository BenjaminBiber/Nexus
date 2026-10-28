//! Gemeinsame Bausteine der Tests von `nexus-db`: Helfer der SQLite-Tests (portiert aus
//! `packages/db/tests/sqlite/helpers/`) und die Test-Datenbank der SQL-Server-Tests.
//!
//! Liegt als `common/mod.rs` und nicht als `common.rs`, damit Cargo das Verzeichnis nicht als
//! eigenes Testprogramm übersetzt.
//!
//! Jede Testdatei bindet nur ein, was sie braucht – ungenutzte Helfer sind deshalb erlaubt.
#![allow(dead_code)]

pub mod crosscheck;
pub mod schema_inspect;
pub mod temp_sqlite;
pub mod test_db;
