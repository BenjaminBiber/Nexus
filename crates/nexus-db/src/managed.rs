//! Board-Datenbank eines Anbieters samt Schema-Stand; portiert aus
//! `packages/db/src/ManagedBoardDatabase.ts`.

use std::future::Future;

use nexus_core::board::database::BoardDatabase;
use nexus_core::Result;

/// Zeitlimit für den Verbindungsaufbau, wenn keines angegeben ist
pub const DEFAULT_CONNECTION_TIMEOUT_MS: u64 = 8000;

/// Einstellungen für den Verbindungsaufbau (gelten für jeden Anbieter; SQLite ignoriert das
/// Zeitlimit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConnectOptions {
    /// Zeitlimit für den Verbindungsaufbau in ms (Standard
    /// [`DEFAULT_CONNECTION_TIMEOUT_MS`])
    pub connection_timeout_ms: Option<u64>,
}

impl ConnectOptions {
    /// Das wirksame Zeitlimit: das angegebene oder der Standard
    pub fn connection_timeout_ms(&self) -> u64 {
        self.connection_timeout_ms
            .unwrap_or(DEFAULT_CONNECTION_TIMEOUT_MS)
    }
}

/// Board-Datenbank eines Anbieters (SQL Server, SQLite), wie die Laufzeit sie braucht:
/// Board-Zugriff plus Schema-Stand für den [`crate::schema_version_guard::SchemaVersionGuard`];
/// `close` kommt aus [`BoardDatabase`].
///
/// `set_missing_features` nimmt `&self`: die Laufzeit trägt die fehlenden Erweiterungen nach dem
/// Öffnen nach, während die Datenbank schon geteilt ist. Umsetzungen brauchen dafür innere
/// Veränderlichkeit (etwa `RwLock`).
pub trait ManagedBoardDatabase: BoardDatabase {
    /// Höchste eingespielte Schema-Version; `None`, wenn das Schema fehlt
    fn schema_version(&self) -> impl Future<Output = Result<Option<i32>>> + Send + '_;

    /// Eingespielte additive Erweiterungen (z. B. `dispatch-runs`); leer, wenn es keine gibt
    fn schema_features(&self) -> impl Future<Output = Result<Vec<String>>> + Send + '_;

    /// Spielt fehlende additive Erweiterungen selbst ein, soweit Rechte und Anbieter das
    /// zulassen; liefert die eingespielten Namen
    fn ensure_schema_features<'a>(
        &'a self,
        missing: &'a [String],
    ) -> impl Future<Output = Result<Vec<String>>> + Send + 'a;

    /// Merkt die Erweiterungen, die danach immer noch fehlen; nur die Zugriffe darauf
    /// verweigern sich dann
    fn set_missing_features(&self, names: Vec<String>);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohne_angabe_gilt_das_standard_zeitlimit() {
        assert_eq!(ConnectOptions::default().connection_timeout_ms(), 8000);
        assert_eq!(DEFAULT_CONNECTION_TIMEOUT_MS, 8000);
    }

    #[test]
    fn ein_angegebenes_zeitlimit_gilt_unverandert() {
        let options = ConnectOptions {
            connection_timeout_ms: Some(250),
        };
        assert_eq!(options.connection_timeout_ms(), 250);
        // auch 0 ist eine Angabe und wird nicht durch den Standard ersetzt
        assert_eq!(
            ConnectOptions {
                connection_timeout_ms: Some(0)
            }
            .connection_timeout_ms(),
            0
        );
    }
}
