//! Protokolldatei des Servers (`<home>/server.log`); portiert aus `writeLog` in
//! `packages/server/src/main.ts`.
//!
//! Format und Verhalten bleiben gleich: `<ISO-Zeit> <Zeile>\n`, über 1 MB wird die Datei vor
//! dem Schreiben geleert, und Fehler beim Schreiben werden verschluckt – das Protokoll ist
//! eine Hilfe und kein Grund zum Abbruch.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use nexus_core::HarnessHome;

/// Größe, ab der die Datei vor dem Schreiben geleert wird
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// Schreibt Zeilen in die Protokolldatei des Benutzerordners.
#[derive(Debug, Clone)]
pub struct ServerLog {
    dir: PathBuf,
    file: PathBuf,
}

impl ServerLog {
    /// Protokoll im angegebenen Benutzerordner
    pub fn new(home: &HarnessHome) -> Self {
        Self {
            dir: home.dir().to_path_buf(),
            file: home.log_file(),
        }
    }

    /// Hängt eine Zeile an; über [`MAX_LOG_BYTES`] wird neu begonnen
    pub fn write(&self, line: &str) {
        let _ = self.try_write(line);
    }

    /// Der Pfad der Datei
    pub fn file(&self) -> &std::path::Path {
        &self.file
    }

    /// Das eigentliche Schreiben; jeder Fehler bleibt beim Aufrufer unsichtbar
    fn try_write(&self, line: &str) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let too_big = fs::metadata(&self.file).is_ok_and(|data| data.len() > MAX_LOG_BYTES);
        if too_big {
            fs::write(&self.file, "")?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file)?;
        write!(file, "{} {line}\n", crate::now_iso())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schreibt_zeilen_mit_zeitstempel_und_beginnt_uber_einem_megabyte_neu() {
        let dir = tempfile::tempdir().unwrap();
        // Der Ordner entsteht beim ersten Schreiben mit
        let home = HarnessHome::at(dir.path().join("home"));
        let log = ServerLog::new(&home);

        log.write("Server läuft auf http://127.0.0.1:4710");
        let text = fs::read_to_string(log.file()).unwrap();
        assert!(
            text.ends_with(" Server läuft auf http://127.0.0.1:4710\n"),
            "{text}"
        );
        // Zeitstempel in ISO-Form mit Millisekunden und Z am Ende
        let stamp = text.split(' ').next().unwrap();
        assert_eq!(stamp.len(), 24, "{stamp}");
        assert!(stamp.ends_with('Z'), "{stamp}");

        log.write("zweite Zeile");
        assert_eq!(fs::read_to_string(log.file()).unwrap().lines().count(), 2);

        fs::write(log.file(), "x".repeat(MAX_LOG_BYTES as usize + 1)).unwrap();
        log.write("nach dem Leeren");
        let neu = fs::read_to_string(log.file()).unwrap();
        assert_eq!(neu.lines().count(), 1);
        assert!(neu.ends_with(" nach dem Leeren\n"), "{neu}");
    }

    #[test]
    fn ein_unschreibbares_protokoll_bricht_nichts_ab() {
        let datei = tempfile::NamedTempFile::new().unwrap();
        // Der „Ordner“ ist eine Datei: create_dir_all scheitert, write bleibt ruhig
        let log = ServerLog::new(&HarnessHome::at(datei.path().join("home")));
        log.write("geht ins Leere");
    }
}
