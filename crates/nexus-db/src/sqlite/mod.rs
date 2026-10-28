//! SQLite-Umsetzung der Board-Datenbank; portiert aus `packages/db/src/sqlite/`.
//!
//! Die Datei `board.db` ist Vertrag: eine von TypeScript angelegte Datei muss Nexus ohne
//! Änderung öffnen und umgekehrt – deshalb stehen die Schema-Stufen zeichengenau in
//! [`schema`] und die PRAGMAs in derselben Reihenfolge wie dort.

pub mod database;
pub mod error_map;
pub mod repos;
pub mod runner;
pub mod schema;
pub mod schema_runner;
pub mod unit_of_work;

pub use database::{SqliteBoardDatabase, SqliteFut, SqliteWork};
pub use runner::{bool_of, date, guid, iso, iso_millis, ParamName, Params, SqlValue, SqliteRunner};
pub use schema::{SqliteSchemaStep, SQLITE_SCHEMA_STEPS};
pub use schema_runner::SqliteSchemaRunner;
pub use unit_of_work::SqliteUnitOfWork;
