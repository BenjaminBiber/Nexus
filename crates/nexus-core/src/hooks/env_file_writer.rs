//! Schreibt Umgebungsvariablen in die Datei, die Claude Code (CLAUDE_ENV_FILE) für die Session
//! einliest. Portiert aus `packages/core/src/hooks/EnvFileWriter.ts`.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::error::{Error, Result};

/// Hängt `export NAME='wert'` an die Env-Datei an
pub struct EnvFileWriter;

impl EnvFileWriter {
    /// Bereits gesetzte Variablen bleiben unberührt. Liefert die Zahl der Zeilen.
    /// `vars` ist eine Liste, weil die Reihenfolge in der Datei die der Angabe ist
    /// (wie bei `Object.entries` in TypeScript).
    pub fn append(
        file: Option<&Path>,
        vars: &[(String, String)],
        existing: &HashMap<String, String>,
    ) -> Result<usize> {
        let Some(file) = file else { return Ok(0) };
        let lines: Vec<String> = vars
            .iter()
            .filter(|(name, _)| !existing.get(name).is_some_and(|wert| !wert.is_empty()))
            .map(|(name, value)| format!("export {name}={}\n", quote(value)))
            .collect();
        if lines.is_empty() {
            return Ok(0);
        }
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(Error::unexpected)?;
        }
        let mut handle = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .map_err(Error::unexpected)?;
        handle
            .write_all(lines.concat().as_bytes())
            .map_err(Error::unexpected)?;
        Ok(lines.len())
    }
}

/// Einfache Anführungszeichen, eingebettete werden maskiert
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Variablen als Liste in der angegebenen Reihenfolge
    fn vars(paare: &[(&str, &str)]) -> Vec<(String, String)> {
        paare
            .iter()
            .map(|(name, wert)| (name.to_string(), wert.to_string()))
            .collect()
    }

    /// Eine Datei in einem Ordner, der noch nicht da ist
    fn env_file(dir: &Path) -> std::path::PathBuf {
        dir.join("ordner mit leerzeichen").join("env.sh")
    }

    #[test]
    fn legt_die_datei_an_und_schreibt_export_zeilen() {
        let dir = tempfile::tempdir().unwrap();
        let file = env_file(dir.path());
        let lines = EnvFileWriter::append(
            Some(&file),
            &vars(&[("HARNESS_SESSION", "abc"), ("HARNESS_TICKET", "T-0042")]),
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(lines, 2);
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "export HARNESS_SESSION='abc'\nexport HARNESS_TICKET='T-0042'\n"
        );
    }

    #[test]
    fn hangt_an_vorhandenen_inhalt_an() {
        let dir = tempfile::tempdir().unwrap();
        let file = env_file(dir.path());
        EnvFileWriter::append(Some(&file), &vars(&[("A", "1")]), &HashMap::new()).unwrap();
        let bisher = fs::read_to_string(&file).unwrap();
        fs::write(&file, format!("{bisher}export B=2\n")).unwrap();
        EnvFileWriter::append(Some(&file), &vars(&[("C", "3")]), &HashMap::new()).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "export A='1'\nexport B=2\nexport C='3'\n"
        );
    }

    #[test]
    fn uberschreibt_keine_bereits_gesetzte_umgebungsvariable() {
        let dir = tempfile::tempdir().unwrap();
        let file = env_file(dir.path());
        let lines = EnvFileWriter::append(
            Some(&file),
            &vars(&[("HARNESS_TICKET", "T-0001"), ("HARNESS_SESSION", "s")]),
            &HashMap::from([("HARNESS_TICKET".to_string(), "T-0099".to_string())]),
        )
        .unwrap();
        assert_eq!(lines, 1);
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "export HARNESS_SESSION='s'\n"
        );
    }

    #[test]
    fn maskiert_anfuhrungszeichen() {
        let dir = tempfile::tempdir().unwrap();
        let file = env_file(dir.path());
        EnvFileWriter::append(Some(&file), &vars(&[("X", "a'b")]), &HashMap::new()).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "export X='a'\\''b'\n");
    }

    #[test]
    fn tut_nichts_ohne_dateinamen() {
        let dir = tempfile::tempdir().unwrap();
        let file = env_file(dir.path());
        assert_eq!(
            EnvFileWriter::append(None, &vars(&[("A", "1")]), &HashMap::new()).unwrap(),
            0
        );
        assert!(!file.exists());
    }
}
