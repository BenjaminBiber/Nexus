//! Speichert die manuelle Reihenfolge von „Meine Aufgaben“ je Benutzer lokal in
//! `~/.harness/inbox-order.json`. Eine fehlende oder kaputte Datei ist nie ein Fehler: dann
//! gilt die Standardsortierung. Portiert aus
//! `packages/core/src/board/inbox/InboxOrderFile.ts`.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::board::inbox::types::{INBOX_KEY_PATTERN, MAX_INBOX_KEYS, MAX_INBOX_KEY_LENGTH};
use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;
use crate::text::utf16_len;

/// Das Format gespeicherter Schlüssel als Muster
static KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(INBOX_KEY_PATTERN).expect("festes Muster"));

/// Liest und schreibt `inbox-order.json`.
#[derive(Debug, Clone)]
pub struct InboxOrderFile {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl InboxOrderFile {
    /// Die Reihenfolge im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Die Datei, in der die Reihenfolge steht
    pub fn file(&self) -> PathBuf {
        self.home.inbox_order_file()
    }

    /// Gespeicherte Schlüssel in ihrer Reihenfolge; ungültige Einträge werden übergangen
    pub fn read(&self) -> Vec<String> {
        let Some(raw) = fs::read_to_string(self.file())
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        else {
            return Vec::new();
        };
        let Some(keys) = raw.get("keys").and_then(Value::as_array) else {
            return Vec::new();
        };
        clean(
            keys.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string),
        )
    }

    /// Schreibt die Schlüssel atomar (doppelte und ungültige fallen weg); der Ordner wird bei
    /// Bedarf angelegt
    pub fn write(&self, keys: &[String]) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let json = serde_json::json!({ "version": 1, "keys": clean(keys.iter().cloned()) });
        // Zwei Leerzeichen Einrückung und ein Zeilenumbruch am Ende, wie
        // `JSON.stringify(x, null, 2)` samt angehängtem „\n“
        let text = format!(
            "{}\n",
            serde_json::to_string_pretty(&json).map_err(Error::unexpected)?
        );
        self.writer.write(&self.file(), &text)
    }

    /// Setzt die Reihenfolge zurück
    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(self.file()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::unexpected(error)),
        }
    }
}

/// Nur gültige Schlüssel, jeden einmal, höchstens [`MAX_INBOX_KEYS`]
fn clean(keys: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    keys.into_iter()
        .filter(|key| utf16_len(key) <= MAX_INBOX_KEY_LENGTH && KEY.is_match(key))
        .filter(|key| seen.insert(key.clone()))
        .take(MAX_INBOX_KEYS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein leerer Benutzerordner samt Reihenfolge darin
    fn file() -> (tempfile::TempDir, InboxOrderFile) {
        let dir = tempfile::Builder::new()
            .prefix("harness-inbox-")
            .tempdir()
            .expect("ein temporärer Ordner geht immer");
        let file = InboxOrderFile::new(HarnessHome::at(dir.path()));
        (dir, file)
    }

    /// Die Schlüssel als Liste
    fn keys(values: &[&str]) -> Vec<String> {
        values.iter().map(|key| key.to_string()).collect()
    }

    #[test]
    fn liest_ohne_datei_eine_leere_reihenfolge() {
        let (_dir, file) = file();
        assert!(file.read().is_empty());
    }

    #[test]
    fn schreibt_inbox_order_json_und_liest_sie_wieder() {
        let (dir, file) = file();
        let gespeichert = keys(&["question:AIH-F0001", "ticket:AIH-0003", "note:7"]);
        file.write(&gespeichert).unwrap();
        assert_eq!(file.file(), dir.path().join("inbox-order.json"));
        assert_eq!(
            fs::read_to_string(file.file()).unwrap(),
            "{\n  \"version\": 1,\n  \"keys\": [\n    \"question:AIH-F0001\",\n    \"ticket:AIH-0003\",\n    \"note:7\"\n  ]\n}\n"
        );
        assert_eq!(file.read(), gespeichert);
    }

    #[test]
    fn liefert_bei_kaputter_datei_eine_leere_reihenfolge_statt_eines_fehlers() {
        let (_dir, file) = file();
        fs::write(file.file(), "{ kaputt").unwrap();
        assert!(file.read().is_empty());
        fs::write(file.file(), r#"{"version":1,"keys":"x"}"#).unwrap();
        assert!(file.read().is_empty());
    }

    #[test]
    fn ubergeht_ungultige_und_doppelte_schlussel() {
        let (_dir, file) = file();
        fs::write(
            file.file(),
            r#"{"version":1,"keys":["ticket:A-1",42,"quatsch","ticket:A-1","note:2"]}"#,
        )
        .unwrap();
        assert_eq!(file.read(), keys(&["ticket:A-1", "note:2"]));
    }

    #[test]
    fn setzt_mit_clear_zuruck() {
        let (_dir, file) = file();
        file.write(&keys(&["ticket:A-1"])).unwrap();
        file.clear().unwrap();
        assert!(file.read().is_empty());
        // Ein zweites Zurücksetzen ist kein Fehler
        file.clear().unwrap();
    }
}
