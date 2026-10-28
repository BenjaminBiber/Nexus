//! SQL-Server-Umsetzung der Board-Datenbank (P2-11 ff.).
//!
//! Die Schicht baut `packages/db/src/Mssql*.ts` mit `tiberius` und `bb8-tiberius` nach:
//!
//! | TypeScript | hier |
//! |---|---|
//! | `MssqlBoardDatabase.ts` | [`database`] |
//! | `SqlRunner.ts` | [`runner`] |
//! | `MssqlSchemaRunner.ts` | [`schema_runner`] |
//! | `MssqlFeatureGate.ts` | [`feature_gate`] |
//! | `SqlErrorMapper.ts` (SQL-Server-Teil) | [`error_map`] |
//! | `MssqlUnitOfWork.ts` | [`unit_of_work`] |
//! | `Mssql<X>Repository.ts` | [`repos`] |
//!
//! Das Schema der Datenbank bleibt unverändert: alte und neue Fassung lesen und schreiben
//! dieselbe Datenbank.

pub mod database;
pub mod error_map;
pub mod feature_gate;
pub mod repos;
pub mod runner;
pub mod schema_runner;
pub mod testing;
pub mod unit_of_work;

pub use database::MssqlBoardDatabase;
pub use error_map::{MssqlErrorMap, SQL_DEADLOCK_VICTIM, SQL_OUT_OF_MEMORY, SQL_UNIQUE_VIOLATIONS};
pub use feature_gate::MssqlFeatureGate;
pub use runner::{
    rewrite_parameters, to_iso, MsCell, MsRow, MsType, MsValue, MssqlClient, MssqlRunner,
    TiberiusRunner, QUERY_TIMEOUT,
};
pub use schema_runner::{
    script_text, MssqlSchemaRunner, SchemaBatchError, SchemaBatchRunner, Warn,
};
pub use unit_of_work::MssqlUnitOfWork;
