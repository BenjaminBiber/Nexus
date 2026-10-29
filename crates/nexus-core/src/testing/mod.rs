//! Werkzeug für Tests anderer Crates: Standardwerte und die gemeinsamen Vertragstests.
//! Nur mit dem Merkmal `testing` vorhanden, damit es nicht in die Auslieferung gerät.
//!
//! Die drei Makros [`crate::board_database_contract`], [`crate::question_repository_contract`]
//! und [`crate::dispatch_run_contract`] prüfen jede Umsetzung von
//! [`crate::board::database::BoardDatabase`] – heute die Speicher-Datenbank, später SQLite und
//! SQL Server. Sie stehen durch `#[macro_export]` unter `nexus_core::…` bereit; die aufrufende
//! Crate braucht `tokio` (Merkmale `macros` und `rt-multi-thread`) als Dev-Abhängigkeit.

pub mod board_contract;
pub mod board_fixture;
pub mod dispatch_run_contract;
pub mod fixtures;
pub mod question_contract;
pub mod recording_unit;
pub mod search_contract;

pub use board_fixture::BoardFixture;
pub use fixtures::*;
pub use recording_unit::RecordingUnit;
