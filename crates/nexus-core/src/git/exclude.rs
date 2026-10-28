//! Nimmt Dateien lokal aus Git heraus: Einträge in `<git-common-dir>/info/exclude` gelten für
//! Hauptordner und alle Worktrees, ohne dass sich eine versionierte `.gitignore` ändert.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::locking::AtomicWriter;

/// Schreibt Zeilen in `info/exclude`.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitExclude {
    writer: AtomicWriter,
}

impl GitExclude {
    /// Neuer Schreiber (hält keinen Zustand)
    pub fn new() -> Self {
        Self::default()
    }

    /// Hängt die Zeile an, falls sie fehlt; liefert false, wenn sie schon eingetragen ist
    pub fn add(&self, common_dir: &Path, line: &str) -> Result<bool> {
        let info = common_dir.join("info");
        let file = info.join("exclude");
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(Error::unexpected(error)),
        };
        // `trim` entfernt auch das \r von Windows-Zeilenenden
        if text.split('\n').any(|existing| existing.trim() == line) {
            return Ok(false);
        }
        let separator = if text.is_empty() || text.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        fs::create_dir_all(&info).map_err(Error::unexpected)?;
        self.writer
            .write(&file, &format!("{text}{separator}{line}\n"))?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inhalt von `info/exclude`
    fn read(common_dir: &Path) -> String {
        fs::read_to_string(common_dir.join("info").join("exclude")).unwrap()
    }

    #[test]
    fn legt_info_exclude_mit_der_zeile_an_wenn_es_die_datei_noch_nicht_gibt() {
        let dir = tempfile::Builder::new()
            .prefix("harness-exclude-")
            .tempdir()
            .unwrap();

        assert!(GitExclude::new()
            .add(dir.path(), "/.claude/rules/x.md")
            .unwrap());
        assert_eq!(read(dir.path()), "/.claude/rules/x.md\n");
    }

    #[test]
    fn ist_wiederholbar_und_meldet_false_wenn_die_zeile_schon_eingetragen_ist() {
        let dir = tempfile::Builder::new()
            .prefix("harness-exclude-")
            .tempdir()
            .unwrap();
        let exclude = GitExclude::new();

        exclude.add(dir.path(), "/.claude/rules/x.md").unwrap();

        assert!(!exclude.add(dir.path(), "/.claude/rules/x.md").unwrap());
        assert_eq!(read(dir.path()), "/.claude/rules/x.md\n");
    }

    #[test]
    fn hangt_an_vorhandenen_inhalt_an_auch_wenn_der_letzte_zeilenumbruch_fehlt() {
        let dir = tempfile::Builder::new()
            .prefix("harness-exclude-")
            .tempdir()
            .unwrap();
        fs::create_dir(dir.path().join("info")).unwrap();
        fs::write(
            dir.path().join("info").join("exclude"),
            "# Kommentar\n*.log",
        )
        .unwrap();

        GitExclude::new()
            .add(dir.path(), "/.claude/rules/x.md")
            .unwrap();

        assert_eq!(
            read(dir.path()),
            "# Kommentar\n*.log\n/.claude/rules/x.md\n"
        );
    }

    #[test]
    fn erkennt_eine_vorhandene_zeile_auch_mit_windows_zeilenenden() {
        let dir = tempfile::Builder::new()
            .prefix("harness-exclude-")
            .tempdir()
            .unwrap();
        fs::create_dir(dir.path().join("info")).unwrap();
        fs::write(
            dir.path().join("info").join("exclude"),
            "# Kommentar\r\n/.claude/rules/x.md\r\n",
        )
        .unwrap();

        assert!(!GitExclude::new()
            .add(dir.path(), "/.claude/rules/x.md")
            .unwrap());
    }
}
