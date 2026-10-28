//! Übersetzung roher SQLite-Fehler in erwartbare Harness-Fehler; portiert aus dem SQLite-Teil
//! von `packages/db/src/SqlErrorMapper.ts`. Die Meldungen sind wortgleich zu TypeScript.
//!
//! In TypeScript tragen die Fehler von `node:sqlite` den erweiterten Ergebniscode im Feld
//! `errcode`. In Rust steckt er in [`rusqlite::Error::SqliteFailure`]. Damit die Prüfung auch
//! noch möglich ist, nachdem ein Repository den Fehler als [`Error`] weitergegeben hat, verpackt
//! [`wrap`] ihn als unerwarteten Fehler mit dem rohen Treiberfehler als Quelle; [`sqlite_code`]
//! holt den Code dort wieder heraus.

use nexus_core::{Error, HarnessError};

/// Primärcode von SQLite: Datei von einem anderen Prozess gesperrt
pub const SQLITE_BUSY: i32 = 5;
/// Primärcode von SQLite: Tabelle in derselben Verbindung gesperrt
pub const SQLITE_LOCKED: i32 = 6;
/// Primärcode von SQLite: Lese- oder Schreibfehler
pub const SQLITE_IOERR: i32 = 10;
/// Primärcode von SQLite: Datenträger oder Datei voll
pub const SQLITE_FULL: i32 = 13;
/// Erweiterter Code von SQLite: Verstoß gegen einen Primärschlüssel
pub const SQLITE_CONSTRAINT_PRIMARYKEY: i32 = 1555;
/// Erweiterter Code von SQLite: Verstoß gegen einen eindeutigen Index
pub const SQLITE_CONSTRAINT_UNIQUE: i32 = 2067;

/// Verpackt einen rohen Treiberfehler, ohne ihn zu übersetzen: so bleibt der erweiterte
/// Ergebniscode für [`is_sqlite_busy`], [`is_unique_violation`] und [`map`] erhalten
pub fn wrap(error: rusqlite::Error) -> Error {
    Error::unexpected(error)
}

/// Erweiterter Ergebniscode eines rohen SQLite-Fehlers; `None` bei jedem anderen Fehler
pub fn sqlite_code(error: &Error) -> Option<i32> {
    let Error::Unexpected { source, .. } = error else {
        return None;
    };
    let raw = source.as_ref()?.downcast_ref::<rusqlite::Error>()?;
    rusqlite_code(raw)
}

/// Erweiterter Ergebniscode direkt am Treiberfehler
fn rusqlite_code(error: &rusqlite::Error) -> Option<i32> {
    match error {
        rusqlite::Error::SqliteFailure(e, _) => Some(e.extended_code),
        _ => None,
    }
}

/// Kurztext des Treiberfehlers (bei SQLite die Meldung von `sqlite3_errmsg`)
fn sqlite_message(error: &Error) -> Option<String> {
    let Error::Unexpected { source, .. } = error else {
        return None;
    };
    match source.as_ref()?.downcast_ref::<rusqlite::Error>()? {
        rusqlite::Error::SqliteFailure(_, message) => message.clone(),
        _ => None,
    }
}

/// Gesperrte Datei wird zu `conflict` (erneut versuchen), volle Platte bzw. E/A-Fehler zu
/// `board-db-storage`; alles andere bleibt unverändert.
///
/// Doppelte Schlüssel bleiben absichtlich roh: die Repositories entscheiden mit
/// [`is_unique_violation`] selbst, was ein doppelter Schlüssel fachlich bedeutet.
pub fn map(error: Error) -> Error {
    let Some(code) = sqlite_code(&error) else {
        return error;
    };
    match code & 0xff {
        // Datei von einem anderen Prozess gesperrt: wie ein Deadlock erneut versuchen
        SQLITE_BUSY | SQLITE_LOCKED => HarnessError::new(
            "conflict",
            "Gleichzeitige Änderung – bitte erneut versuchen.",
        )
        .into(),
        SQLITE_FULL => HarnessError::new(
            "board-db-storage",
            "Die Board-Datei kann nicht wachsen: Der Datenträger ist voll – bitte Speicherplatz freigeben und erneut versuchen.",
        )
        .into(),
        SQLITE_IOERR => {
            let detail = sqlite_message(&error)
                .map(|m| format!(" ({m})"))
                .unwrap_or_default();
            HarnessError::new(
                "board-db-storage",
                format!("Lese- oder Schreibfehler an der Board-Datei{detail} – bitte Datenträger und Zugriffsrechte prüfen."),
            )
            .into()
        }
        _ => error,
    }
}

/// Roher Fehler „Datei gesperrt“ (SQLITE_BUSY samt erweiterter Codes wie BUSY_SNAPSHOT), noch
/// nicht übersetzt
pub fn is_sqlite_busy(error: &Error) -> bool {
    sqlite_code(error).is_some_and(|code| code & 0xff == SQLITE_BUSY)
}

/// Primärcodes, nach denen die Board-Datei als nicht erreichbar gilt: `SQLITE_PERM` (3),
/// `SQLITE_READONLY` (8), `SQLITE_CANTOPEN` (14) und `SQLITE_NOTADB` (26). BUSY/LOCKED
/// (Konflikt) und IOERR/FULL (`board-db-storage`) übersetzt [`map`].
pub const SQLITE_FILE_CODES: [i32; 4] = [3, 8, 14, 26];

/// Datei- oder Zugriffsfehler an der Board-Datei (die Datei ist damit nicht erreichbar);
/// der Server verwirft daraufhin seine Board-Laufzeit
pub fn is_file_error(error: &Error) -> bool {
    sqlite_code(error).is_some_and(|code| SQLITE_FILE_CODES.contains(&(code & 0xff)))
}

/// Doppelter Schlüssel: UNIQUE (2067) oder PRIMARY KEY (1555)
pub fn is_unique_violation(error: &Error) -> bool {
    sqlite_code(error).is_some_and(|code| {
        code == SQLITE_CONSTRAINT_UNIQUE || code == SQLITE_CONSTRAINT_PRIMARYKEY
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::ffi::{Error as FfiError, ErrorCode};

    /// Fehler, wie rusqlite ihn wirft: erweiterter Ergebniscode plus Kurztext
    fn sqlite_error(extended: i32, message: &str) -> Error {
        let ffi = FfiError {
            code: ErrorCode::Unknown,
            extended_code: extended,
        };
        wrap(rusqlite::Error::SqliteFailure(
            ffi,
            Some(message.to_string()),
        ))
    }

    #[test]
    fn eine_gesperrte_datei_wird_zu_conflict() {
        for code in [5, 6, 517, 262] {
            let mapped = map(sqlite_error(code, "database is locked"));
            let harness = mapped.harness().unwrap_or_else(|| panic!("{code}"));
            assert_eq!(harness.code, "conflict", "{code}");
            assert_eq!(
                harness.message, "Gleichzeitige Änderung – bitte erneut versuchen.",
                "{code}"
            );
        }
    }

    #[test]
    fn ein_voller_datentrager_wird_zu_board_db_storage() {
        let mapped = map(sqlite_error(13, "database or disk is full"));
        let harness = mapped.harness().unwrap();
        assert_eq!(harness.code, "board-db-storage");
        assert!(harness.message.contains("Datenträger ist voll"));
    }

    #[test]
    fn ein_ea_fehler_wird_zu_board_db_storage_mit_kurztext() {
        for code in [10, 3338] {
            let mapped = map(sqlite_error(code, "disk I/O error"));
            let harness = mapped.harness().unwrap_or_else(|| panic!("{code}"));
            assert_eq!(harness.code, "board-db-storage", "{code}");
            assert!(harness.message.contains("disk I/O error"), "{code}");
        }
    }

    #[test]
    fn doppelte_schlussel_bleiben_unverandert() {
        for code in [2067, 1555] {
            let error = sqlite_error(code, "constraint failed");
            assert!(is_unique_violation(&error), "{code}");
            assert!(map(error).harness().is_none(), "{code}");
        }
    }

    #[test]
    fn andere_codes_sind_kein_doppelter_schlussel_und_kein_busy() {
        for code in [1299, 787] {
            let error = sqlite_error(code, "constraint failed");
            assert!(!is_unique_violation(&error), "{code}");
            assert!(!is_sqlite_busy(&error), "{code}");
        }
        assert!(is_sqlite_busy(&sqlite_error(517, "locked")));
        assert!(!is_sqlite_busy(&sqlite_error(6, "locked")));
    }

    #[test]
    fn datei_und_zugriffsfehler_gelten_als_fehler_der_datei() {
        // PERM, READONLY, CANTOPEN, NOTADB und ein erweiterter Code (CANTOPEN_ISDIR)
        for code in [3, 8, 14, 26, 1038] {
            assert!(
                is_file_error(&sqlite_error(code, "unable to open database file")),
                "{code}"
            );
        }
        // BUSY, LOCKED, IOERR und FULL sind keine Fehler der Datei selbst
        for code in [5, 517, 6, 10, 13, 2067] {
            assert!(!is_file_error(&sqlite_error(code, "x")), "{code}");
        }
        assert!(!is_file_error(
            &HarnessError::new("board-db-storage", "voll").into()
        ));
    }

    #[test]
    fn fremde_fehler_bleiben_unberuhrt() {
        let error = Error::unexpected_message("Treiber antwortet nicht");
        assert_eq!(sqlite_code(&error), None);
        assert!(!is_sqlite_busy(&error));
        assert!(!is_unique_violation(&error));
        assert_eq!(map(error).to_string(), "Treiber antwortet nicht");

        // Ein erwartbarer Fehler wird nie erneut übersetzt
        let harness: Error = HarnessError::new("conflict", "x").into();
        assert_eq!(map(harness).code(), Some("conflict"));
    }

    #[test]
    fn ein_echter_fehler_der_datei_wird_erkannt() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE T (Id INTEGER PRIMARY KEY, K TEXT UNIQUE)")
            .unwrap();
        conn.execute_batch("INSERT INTO T VALUES (1, 'a')").unwrap();

        let doppelt = wrap(
            conn.execute_batch("INSERT INTO T VALUES (2, 'a')")
                .unwrap_err(),
        );
        assert!(is_unique_violation(&doppelt));
        let primar = wrap(
            conn.execute_batch("INSERT INTO T VALUES (1, 'b')")
                .unwrap_err(),
        );
        assert!(is_unique_violation(&primar));
    }
}
