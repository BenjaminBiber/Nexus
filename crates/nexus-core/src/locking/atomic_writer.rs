//! Schreibt Dateien atomar (temporäre Datei, dann Umbenennen), damit nie ein halber
//! Inhalt sichtbar ist. Name und Wiederholungen entsprechen
//! `packages/core/src/locking/AtomicWriter.ts`, damit TypeScript und Rust dieselben
//! Dateien schreiben können.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::random;

/// Fehler, nach denen das Umbenennen kurz wiederholt wird (Virenscanner, Indexdienst).
/// EPERM und EACCES aus TypeScript haben in Rust die Art `PermissionDenied`, für EBUSY
/// gibt es keine eigene Art – deshalb zusätzlich die Nummer des Systemfehlers.
#[cfg(unix)]
const RETRY_OS_ERRORS: &[i32] = &[16]; // EBUSY

/// Entsprechung unter Windows: die Datei ist kurz von einem anderen Prozess gehalten
#[cfg(windows)]
const RETRY_OS_ERRORS: &[i32] = &[32, 33]; // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION

/// Höchstzahl der Wiederholungen beim Umbenennen
const MAX_ATTEMPTS: u32 = 40;

/// Schreibt Dateien atomar (temporäre Datei, dann Umbenennen).
#[derive(Debug, Clone, Copy, Default)]
pub struct AtomicWriter;

impl AtomicWriter {
    /// Neuer Schreiber (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Schreibt `content` nach `path`; unter Windows wird bei kurzer Sperre durch Dritte wiederholt
    pub fn write(&self, path: &Path, content: &str) -> Result<()> {
        self.write_inner(path, content, false)
    }

    /// Wie [`AtomicWriter::write`], aber die Datei gehört nur dem Benutzer (Unix: `0600`).
    /// Für Dateien mit Geheimnissen wie dem Start-Token in `server.json`. Unter Windows
    /// bleibt es bei den Rechten des Benutzerprofils.
    pub fn write_private(&self, path: &Path, content: &str) -> Result<()> {
        self.write_inner(path, content, true)
    }

    /// Gemeinsamer Weg: temporäre Datei schreiben, auf Wunsch die Rechte setzen, umbenennen.
    /// Die Rechte stehen schon vor dem Umbenennen fest, damit die Zieldatei nie kurz offen liegt.
    fn write_inner(&self, path: &Path, content: &str, private: bool) -> Result<()> {
        let temp = temp_path(path);
        let geschrieben =
            fs::write(&temp, content)
                .and_then(|()| if private { nur_benutzer(&temp) } else { Ok(()) });
        if let Err(error) = geschrieben {
            let _ = fs::remove_file(&temp);
            return Err(Error::unexpected(error));
        }
        match rename_with_retry(&temp, path) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&temp);
                Err(Error::unexpected(error))
            }
        }
    }
}

/// Nimmt Gruppe und anderen Benutzern jedes Recht an der Datei (Unix)
#[cfg(unix)]
fn nur_benutzer(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

/// Unter Windows regelt das Benutzerprofil den Zugriff; hier ist nichts zu tun
#[cfg(not(unix))]
fn nur_benutzer(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// Name der temporären Datei: `<pfad>.<pid>.<hex>.tmp` wie in TypeScript
fn temp_path(path: &Path) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(format!(".{}.{}.tmp", std::process::id(), random::hex(4)));
    PathBuf::from(name)
}

/// Benennt um und wiederholt kurz bei EPERM/EBUSY/EACCES (Virenscanner, Indexdienst)
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    for attempt in 0..=MAX_ATTEMPTS {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < MAX_ATTEMPTS && is_retryable(&error) => {
                sleep(Duration::from_millis(u64::from(10 + attempt * 5)));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("die Schleife endet mit Ok oder Err")
}

/// Hält ein anderer Prozess die Datei gerade kurz fest?
fn is_retryable(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied
        || matches!(error.raw_os_error(), Some(code) if RETRY_OS_ERRORS.contains(&code))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Temporärer Ordner mit Leerzeichen und Umlaut im Namen, wie unter Windows üblich
    fn temp_dir() -> tempfile::TempDir {
        let dir = tempfile::Builder::new()
            .prefix("harness-win-")
            .tempdir()
            .unwrap();
        fs::create_dir(dir.path().join("Mein Ordner ä ö ü ß")).unwrap();
        dir
    }

    #[test]
    fn schreibt_und_uberschreibt_atomar() {
        let base = temp_dir();
        let dir = base.path().join("Mein Ordner ä ö ü ß");
        let file = dir.join("datei mit leerzeichen.json");
        let writer = AtomicWriter::new();

        writer.write(&file, "eins").unwrap();
        writer.write(&file, "zwei").unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "zwei");
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["datei mit leerzeichen.json"]);
    }

    #[test]
    fn ubersteht_viele_gleichzeitige_schreibzugriffe_auf_dieselbe_datei() {
        let base = temp_dir();
        let dir = base.path().join("Mein Ordner ä ö ü ß");
        let file = dir.join("x.json");

        std::thread::scope(|scope| {
            for index in 0..30 {
                let file = file.clone();
                scope.spawn(move || {
                    AtomicWriter::new()
                        .write(&file, &format!("Inhalt {index}"))
                        .unwrap();
                });
            }
        });

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.starts_with("Inhalt "), "{content}");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[cfg(unix)]
    #[test]
    fn schreibt_eine_geschuetzte_datei_nur_fuer_den_benutzer() {
        use std::os::unix::fs::PermissionsExt;

        let base = temp_dir();
        let file = base.path().join("server.json");

        AtomicWriter::new().write_private(&file, "{}").unwrap();

        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{mode:o}");
        assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
    }

    #[test]
    fn meldet_einen_fehlenden_ordner_und_lasst_nichts_liegen() {
        let base = temp_dir();
        let missing = base.path().join("gibt-es-nicht").join("x.json");

        let error = AtomicWriter::new().write(&missing, "x").unwrap_err();

        assert_eq!(error.code(), None);
        let leftovers: Vec<_> = fs::read_dir(base.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}
