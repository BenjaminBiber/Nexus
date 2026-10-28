//! Datenzugriff von Nexus: liest und schreibt die unveränderten Schemata von
//! SQLite (`board.db`) und SQL Server.
//!
//! Die Verträge selbst (Trait `BoardDatabase`, die 17 Repository-Traits, `BoardUnitOfWork`)
//! liegen in `nexus-core`; dieses Crate hängt davon ab und gibt sie weiter, damit Aufrufer
//! alles über `nexus_db::…` beziehen können. Umgekehrt darf `nexus-core` nie von `nexus-db`
//! abhängen – sonst entstünde ein Zyklus, sobald die Dienste des Fachkerns den Trait brauchen.

pub mod managed;
pub mod mssql;
pub mod row_groups;
pub mod schema_version_guard;
pub mod sql_chunks;
pub mod sqlite;

pub use managed::{ConnectOptions, ManagedBoardDatabase, DEFAULT_CONNECTION_TIMEOUT_MS};
pub use row_groups::RowGroups;
pub use schema_version_guard::{
    SchemaVersionGuard, EXPECTED_SCHEMA_VERSION, REQUIRED_SCHEMA_FEATURES,
};
pub use sql_chunks::{SqlChunks, SQL_CHUNK_SIZE};

// Die Verträge aus dem Fachkern samt der Speicher-Datenbank für Tests.
pub use nexus_core::board::database::*;
pub use nexus_core::board::in_memory::InMemoryBoardDatabase;

/// Name des Crates, wie er in Protokollen und Diagnoseausgaben erscheint.
pub const CRATE_NAME: &str = "nexus-db";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_ist_gesetzt() {
        assert_eq!(CRATE_NAME, "nexus-db");
    }

    #[test]
    fn fachkern_ist_eingebunden() {
        assert_eq!(nexus_core::CRATE_NAME, "nexus-core");
    }

    #[test]
    fn die_vertrage_des_fachkerns_sind_uber_nexus_db_erreichbar() {
        // Die Weitergabe mit `pub use` ist Vertrag: P2-5 und P2-11 beziehen alles über nexus_db
        let filter = crate::TicketFilter::default();
        assert!(!filter.without_texts);
        fn nimmt_eine_datenbank<D: crate::BoardDatabase>(_db: &D) {}
        let _ = nimmt_eine_datenbank::<DummyNiemals>;
    }

    /// Nur als Typ gebraucht, nie gebaut – zeigt, dass der Trait über nexus_db ansprechbar ist
    enum DummyNiemals {}

    impl crate::BoardDatabase for DummyNiemals {
        async fn transaction<'a, T, W>(&'a self, _work: W) -> nexus_core::Result<T>
        where
            T: Send + 'static,
            W: crate::TransactionWork<T> + 'a,
        {
            match *self {}
        }

        async fn change_marker(&self, _board_id: i32) -> nexus_core::Result<String> {
            match *self {}
        }

        async fn question_marker(&self, _project_id: i32) -> nexus_core::Result<String> {
            match *self {}
        }

        async fn close(&self) -> nexus_core::Result<()> {
            match *self {}
        }
    }
}
