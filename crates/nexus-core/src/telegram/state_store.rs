//! Laufzeit-Zustand des Bots in `<home>/telegram/state.json`: welche Updates schon
//! abgeholt sind.
//!
//! Die Datei steht neben `telegram.yaml`, aber nicht darin: Sie enthält keine Geheimnisse
//! und wird nach jedem Update geschrieben, die Einstellung nur selten.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Was der Poller über Neustarts hinweg braucht
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TelegramState {
    /// Das nächste erwartete Update (`update_id` des letzten verarbeiteten plus eins);
    /// 0 = noch keines abgeholt
    pub offset: i64,
}

/// Liest und schreibt den Laufzeit-Zustand des Bots.
#[derive(Debug, Clone)]
pub struct TelegramStateStore {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl TelegramStateStore {
    /// Store im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Pfad der Datei (für Meldungen)
    pub fn file(&self) -> PathBuf {
        self.home.telegram_state_file()
    }

    /// Der gespeicherte Zustand; ohne Datei und bei kaputtem JSON der leere Zustand –
    /// ein verlorener Versatz kostet höchstens doppelt verarbeitete Updates
    pub fn read(&self) -> Result<TelegramState> {
        let text = match fs::read_to_string(self.file()) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(TelegramState::default())
            }
            Err(error) => return Err(Error::unexpected(error)),
        };
        Ok(serde_json::from_str(&text).unwrap_or_default())
    }

    /// Merkt den nächsten erwarteten Versatz; schreibt atomar
    pub fn write_offset(&self, offset: i64) -> Result<()> {
        self.write(&TelegramState { offset })
    }

    /// Schreibt den Zustand atomar
    pub fn write(&self, state: &TelegramState) -> Result<()> {
        fs::create_dir_all(self.home.telegram_dir()).map_err(Error::unexpected)?;
        let text = serde_json::to_string_pretty(state).map_err(Error::unexpected)?;
        self.writer.write(&self.file(), &text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Benutzerordner und Store darin
    fn store() -> (tempfile::TempDir, TelegramStateStore) {
        let dir = tempfile::Builder::new()
            .prefix("harness-telegram-state-")
            .tempdir()
            .unwrap();
        let store = TelegramStateStore::new(HarnessHome::at(dir.path()));
        (dir, store)
    }

    #[test]
    fn liefert_ohne_datei_den_leeren_zustand() {
        let (_dir, store) = store();
        assert_eq!(store.read().unwrap().offset, 0);
    }

    #[test]
    fn merkt_den_versatz_uber_einen_neustart_hinweg() {
        let (dir, store) = store();

        store.write_offset(4711).unwrap();

        // Ein zweiter Store auf demselben Ordner ist der Poller nach dem Neustart
        let nach_neustart = TelegramStateStore::new(HarnessHome::at(dir.path()));
        assert_eq!(nach_neustart.read().unwrap().offset, 4711);
    }

    #[test]
    fn schreibt_camelcase_und_legt_den_ordner_an() {
        let (dir, store) = store();

        store.write_offset(7).unwrap();

        let text = fs::read_to_string(dir.path().join("telegram").join("state.json")).unwrap();
        assert!(text.contains("\"offset\": 7"), "{text}");
    }

    #[test]
    fn beginnt_bei_kaputtem_json_wieder_bei_null() {
        let (dir, store) = store();
        fs::create_dir_all(dir.path().join("telegram")).unwrap();
        fs::write(dir.path().join("telegram").join("state.json"), "{kaputt").unwrap();

        assert_eq!(store.read().unwrap().offset, 0);
    }
}
