//! Übersetzung der SQL-Server-Fehler in erwartbare Harness-Fehler; portiert aus dem
//! SQL-Server-Teil von `packages/db/src/SqlErrorMapper.ts`. Der SQLite-Teil gehört zu P2-5.
//!
//! # Wo übersetzt wird
//!
//! In TypeScript fängt `MssqlBoardDatabase.transaction` den Fehler der Arbeit ab und schickt
//! ihn durch den Mapper. In Rust liefert die Arbeit schon einen [`Error`] des Fachkerns –
//! übersetzt wird deshalb dort, wo ein Treiberfehler überhaupt entsteht: im Runner
//! ([`crate::mssql::runner::TiberiusRunner`]) und beim Verbindungsaufbau. Die Transaktion
//! muss dann nichts mehr nachübersetzen.
//!
//! # Warum die Nummer erhalten bleibt
//!
//! Unbekannte Nummern bleiben wie in TypeScript unverändert: der Treiberfehler wird als Quelle
//! in [`Error::Unexpected`] verpackt. [`MssqlErrorMap::is_unique_violation`] findet die Nummer
//! dort wieder, damit Repositories doppelte Schlüssel erkennen können (in TypeScript
//! `SqlErrorMapper.isUniqueViolation`).

use nexus_core::{Error, HarnessError};

/// Fehlernummer, wenn die Transaktion als Deadlock-Opfer abgebrochen wurde
pub const SQL_DEADLOCK_VICTIM: u32 = 1205;

/// Fehlernummer, wenn dem Server der Arbeitsspeicher ausgeht (z. B. Container mit zu wenig RAM)
pub const SQL_OUT_OF_MEMORY: u32 = 701;

/// Fehlernummern für doppelte Schlüssel (Primärschlüssel bzw. eindeutiger Index)
pub const SQL_UNIQUE_VIOLATIONS: [u32; 2] = [2627, 2601];

/// Übersetzt bekannte SQL-Server-Fehler in erwartbare Harness-Fehler; alles andere bleibt
/// unverändert.
#[derive(Debug, Clone, Copy, Default)]
pub struct MssqlErrorMap;

impl MssqlErrorMap {
    /// Der Fehler des Treibers als Fehler des Fachkerns: Deadlock-Opfer wird zu `conflict`,
    /// Speichermangel zu `board-db-overloaded`; sonst bleibt der Treiberfehler als Quelle
    /// erhalten.
    pub fn map(error: tiberius::error::Error) -> Error {
        match Self::harness(error.code()) {
            Some(harness) => harness.into(),
            None => Error::unexpected(error),
        }
    }

    /// Dasselbe für den Fehler des Pools, der auch reine E/A-Fehler kennt
    pub fn map_pool(error: bb8_tiberius::Error) -> Error {
        match error {
            bb8_tiberius::Error::Tiberius(error) => Self::map(error),
            bb8_tiberius::Error::Io(error) => Error::unexpected(error),
        }
    }

    /// Der erwartbare Fehler zu einer Fehlernummer des SQL Servers; `None` bei jeder anderen
    /// Nummer und bei Fehlern ohne Nummer (Netz, TLS, Protokoll)
    pub fn harness(code: Option<u32>) -> Option<HarnessError> {
        match code {
            Some(SQL_DEADLOCK_VICTIM) => Some(HarnessError::new(
                "conflict",
                "Gleichzeitige Änderung – bitte erneut versuchen.",
            )),
            Some(SQL_OUT_OF_MEMORY) => Some(HarnessError::new(
                "board-db-overloaded",
                "Der Board-Datenbank geht der Arbeitsspeicher aus – bitte kurz warten und erneut versuchen (ggf. Docker/WSL-Speicher freigeben oder das Speicherlimit des Containers prüfen).",
            )),
            _ => None,
        }
    }

    /// Ist die Nummer die eines doppelten Schlüssels (2627 Primärschlüssel, 2601 Index)?
    pub fn is_unique_violation_number(code: Option<u32>) -> bool {
        code.is_some_and(|code| SQL_UNIQUE_VIOLATIONS.contains(&code))
    }

    /// Doppelter Schlüssel? Erkennt den Fehler auch dann noch, wenn [`MssqlErrorMap::map`] ihn
    /// schon als Quelle verpackt hat.
    pub fn is_unique_violation(error: &Error) -> bool {
        Self::is_unique_violation_number(Self::number_of(error))
    }

    /// Fehlernummer des SQL Servers in einem Fehler des Fachkerns; `None`, wenn der Fehler
    /// nicht vom Treiber kommt
    pub fn number_of(error: &Error) -> Option<u32> {
        match error {
            Error::Unexpected {
                source: Some(source),
                ..
            } => source
                .downcast_ref::<tiberius::error::Error>()
                .and_then(tiberius::error::Error::code),
            _ => None,
        }
    }
}
