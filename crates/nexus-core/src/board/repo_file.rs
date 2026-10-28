//! Liest und schreibt `~/.harness/board-repos.yaml`: je Board-ID der eigene Repo-Pfad
//! dieses Benutzers (None = ausdrücklich kein Pfad). Die Datei gehört dem Benutzer und
//! steht bewusst nicht in der gemeinsamen Datenbank.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde_yaml_ng::Value;

use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Zuordnung Board-ID → Repo-Pfad; nach ID sortiert, wie die Datei sie schreibt
pub type BoardRepoEntries = BTreeMap<i64, Option<String>>;

/// Liest und schreibt die eigenen Repo-Pfade der Boards.
#[derive(Debug, Clone)]
pub struct BoardRepoFile {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl BoardRepoFile {
    /// Datei im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Pfad der Datei (für Meldungen)
    pub fn file(&self) -> PathBuf {
        self.home.board_repos_file()
    }

    /// Liest die Zuordnung; ohne Datei leer
    pub fn read(&self) -> Result<BoardRepoEntries> {
        let file = self.file();
        if !file.exists() {
            return Ok(BoardRepoEntries::new());
        }
        let text = fs::read_to_string(&file).map_err(Error::unexpected)?;
        let raw: Value = serde_yaml_ng::from_str(&text)
            .map_err(|_| self.invalid(format!("{} ist kein gültiges YAML.", file.display())))?;
        let mapping = match raw {
            Value::Null => return Ok(BoardRepoEntries::new()),
            Value::Mapping(mapping) => mapping,
            _ => {
                return Err(self.invalid(format!(
                    "{} enthält keine Zuordnung Board → Repo-Pfad.",
                    file.display()
                )))
            }
        };
        let mut entries = BoardRepoEntries::new();
        for (key, value) in mapping {
            let id = board_id(&key).ok_or_else(|| {
                self.invalid(format!(
                    "In {} ist „{}“ keine Board-ID.",
                    file.display(),
                    key_text(&key)
                ))
            })?;
            let path = match &value {
                Value::Null => None,
                Value::String(path) if !path.trim().is_empty() => Some(path.trim().to_string()),
                Value::String(_) => None,
                _ => {
                    return Err(self.invalid(format!(
                        "In {} ist der Pfad von Board {id} kein Text.",
                        file.display()
                    )))
                }
            };
            entries.insert(id, path);
        }
        Ok(entries)
    }

    /// Schreibt die Zuordnung atomar, nach Board-ID sortiert; der Ordner wird bei Bedarf angelegt
    pub fn write(&self, entries: &BoardRepoEntries) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let yaml = if entries.is_empty() {
            // Eine leere Zuordnung schreibt YAML als `{}`; TypeScript liest das als leer
            "{}\n".to_string()
        } else {
            serde_yaml_ng::to_string(entries).map_err(Error::unexpected)?
        };
        self.writer.write(&self.file(), &yaml)
    }

    /// Fehler mit dem Code, den auch TypeScript meldet
    fn invalid(&self, message: String) -> Error {
        HarnessError::new("board-repos-invalid", message).into()
    }
}

/// Board-ID aus dem Schlüssel: YAML liest `7:` als Zahl, `"7":` als Text – beides zählt,
/// wie `Number(key)` in TypeScript. Erlaubt sind nur ganze Zahlen größer null.
fn board_id(key: &Value) -> Option<i64> {
    let id = match key {
        Value::Number(number) => number.as_i64()?,
        Value::String(text) => text.trim().parse::<i64>().ok()?,
        _ => return None,
    };
    (id > 0).then_some(id)
}

/// Schlüssel als Text für die Fehlermeldung
fn key_text(key: &Value) -> String {
    match key {
        Value::String(text) => text.clone(),
        other => serde_yaml_ng::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Benutzerordner und Datei darin
    fn file() -> (tempfile::TempDir, BoardRepoFile) {
        let dir = tempfile::Builder::new()
            .prefix("harness-repos-")
            .tempdir()
            .unwrap();
        let file = BoardRepoFile::new(HarnessHome::at(dir.path()));
        (dir, file)
    }

    /// Schreibt rohen YAML-Text in die Datei
    fn write_raw(dir: &tempfile::TempDir, content: &str) {
        fs::write(dir.path().join("board-repos.yaml"), content).unwrap();
    }

    #[test]
    fn liest_ohne_datei_eine_leere_zuordnung() {
        let (_dir, file) = file();
        assert_eq!(file.read().unwrap(), BoardRepoEntries::new());
    }

    #[test]
    fn schreibt_die_zuordnung_nach_board_repos_yaml_und_liest_sie_wieder() {
        let (dir, file) = file();
        let entries =
            BoardRepoEntries::from([(7, Some("C:\\Projects\\Arches".to_string())), (8, None)]);

        file.write(&entries).unwrap();

        assert_eq!(file.file(), dir.path().join("board-repos.yaml"));
        // Der Pfad steht unverändert in der Datei, auch mit Rückschrägstrichen
        let text = fs::read_to_string(dir.path().join("board-repos.yaml")).unwrap();
        assert!(text.contains("C:\\Projects\\Arches"), "{text}");
        assert_eq!(file.read().unwrap(), entries);
    }

    #[test]
    fn schreibt_die_zuordnung_in_dem_yaml_das_typescript_auch_schreibt() {
        let (dir, file) = file();

        file.write(&BoardRepoEntries::from([
            (7, Some("C:\\Projects\\Arches ä".to_string())),
            (8, None),
        ]))
        .unwrap();

        // TypeScript schreibt dieselbe Zuordnung als `"7": C:\Projects\Arches ä` – die
        // Schlüssel stehen dort in Anführungszeichen, weil JavaScript nur Text als
        // Schlüssel kennt. Beide Schreibweisen bedeuten dasselbe und werden gelesen.
        assert_eq!(
            fs::read_to_string(dir.path().join("board-repos.yaml")).unwrap(),
            "7: C:\\Projects\\Arches ä\n8: null\n"
        );
    }

    #[test]
    fn liest_die_zuordnung_von_typescript_mit_textschlusseln() {
        let (dir, file) = file();
        write_raw(&dir, "\"7\": C:\\Projects\\Arches\n\"8\": null\n");

        assert_eq!(
            file.read().unwrap(),
            BoardRepoEntries::from([(7, Some("C:\\Projects\\Arches".to_string())), (8, None)])
        );
    }

    #[test]
    fn leere_pfade_zahlen_als_kein_pfad() {
        let (dir, file) = file();
        write_raw(&dir, "7: '   '\n");

        assert_eq!(file.read().unwrap(), BoardRepoEntries::from([(7, None)]));
    }

    #[test]
    fn meldet_kaputtes_yaml_und_ungultige_eintrage() {
        let (dir, file) = file();

        write_raw(&dir, ": : [");
        assert_eq!(file.read().unwrap_err().code(), Some("board-repos-invalid"));

        write_raw(&dir, "abc: C:/x\n");
        assert_eq!(file.read().unwrap_err().code(), Some("board-repos-invalid"));

        write_raw(&dir, "7: 12\n");
        assert_eq!(file.read().unwrap_err().code(), Some("board-repos-invalid"));

        write_raw(&dir, "- 7\n");
        assert_eq!(file.read().unwrap_err().code(), Some("board-repos-invalid"));
    }

    #[test]
    fn eine_leere_zuordnung_ubersteht_schreiben_und_lesen() {
        let (_dir, file) = file();

        file.write(&BoardRepoEntries::new()).unwrap();

        assert_eq!(file.read().unwrap(), BoardRepoEntries::new());
    }
}
