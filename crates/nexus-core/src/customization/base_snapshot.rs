//! Portiert `packages/core/src/customization/BaseSnapshot.ts`.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::process::hide_console;

use super::overview::{BaseState, Problem};

/// Zustand der Basis mit Warnung und, bei veralteter Basis, den Unterschieden
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseCheck {
    pub state: BaseState,
    pub warnings: Vec<Problem>,
    pub diff: Option<String>,
}

/// Basis einer Anpassung: Kopie des Standards zum Zeitpunkt des Kopierens; vergleicht sie
/// mit dem aktuellen Standard.
#[derive(Debug, Clone, Copy, Default)]
pub struct BaseSnapshot;

impl BaseSnapshot {
    /// Neuer Vergleicher (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Einheitliches JSON für Workflow und Basis (zwei Leerzeichen, Zeilenende am Schluss)
    pub fn format_json(value: &impl Serialize) -> Result<String> {
        let text = serde_json::to_string_pretty(value).map_err(Error::unexpected)?;
        Ok(format!("{text}\n"))
    }

    /// Legt die Basis als Kopie des Standards (Datei oder Ordner) an; eine vorhandene wird ersetzt
    pub fn save(&self, standard: &Path, base: &Path) -> Result<()> {
        remove(base)?;
        if let Some(parent) = base.parent() {
            std::fs::create_dir_all(parent).map_err(Error::unexpected)?;
        }
        copy(standard, base)
    }

    /// Legt die Basis aus einem Text an (Workflow als JSON)
    pub fn save_text(&self, text: &str, base: &Path) -> Result<()> {
        if let Some(parent) = base.parent() {
            std::fs::create_dir_all(parent).map_err(Error::unexpected)?;
        }
        std::fs::write(base, text).map_err(Error::unexpected)
    }

    /// `Unknown` ohne Basis, `Current` bei gleichem Inhalt, sonst `Outdated`
    pub fn state(&self, base: &Path, standard: &Path) -> Result<BaseState> {
        if !base.exists() {
            return Ok(BaseState::Unknown);
        }
        Ok(if self.tree(base)? == self.tree(standard)? {
            BaseState::Current
        } else {
            BaseState::Outdated
        })
    }

    /// Unterschiede Basis → Standard über `git diff --no-index` (Rückgabecode 1 bedeutet
    /// „unterschiedlich“, kein Fehler)
    pub fn diff(&self, base: &Path, standard: &Path) -> String {
        let mut command = Command::new("git");
        command.arg("diff").arg("--no-index").arg("--no-color");
        command.arg("--").arg(base).arg(standard);
        hide_console(&mut command);
        match command.output() {
            Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
            // Ohne Git gibt es keine Unterschiede zu zeigen; die Warnung bleibt bestehen
            Err(_) => String::new(),
        }
    }

    /// Zustand mit Warnung: veraltet mit Diff, unbekannt mit Hinweis, aktuell ohne
    pub fn check(&self, base: &Path, standard: &Path) -> Result<BaseCheck> {
        let state = self.state(base, standard)?;
        Ok(match state {
            BaseState::Outdated => BaseCheck {
                state,
                warnings: vec![Problem::new(
                    "base-outdated",
                    "Der Standard hat sich seit deiner Kopie geändert.",
                )],
                diff: Some(self.diff(base, standard)),
            },
            BaseState::Unknown => BaseCheck {
                state,
                warnings: vec![Problem::new(
                    "base-unknown",
                    "Keine Basis vermerkt; ein geänderter Standard wird nicht erkannt.",
                )],
                diff: None,
            },
            BaseState::Current => BaseCheck {
                state,
                warnings: Vec::new(),
                diff: None,
            },
        })
    }

    /// Alle Dateien unter dem Pfad mit Inhalt; Schlüssel relativ zum Pfad mit `/`
    fn tree(&self, path: &Path) -> Result<BTreeMap<String, String>> {
        let mut files = BTreeMap::new();
        if !path.exists() {
            return Ok(files);
        }
        if path.is_file() {
            files.insert(String::new(), read_text(path)?);
            return Ok(files);
        }
        collect(path, path, &mut files)?;
        Ok(files)
    }
}

/// Sammelt alle Dateien unterhalb von `dir` mit ihrem Pfad relativ zu `root`
fn collect(root: &Path, dir: &Path, files: &mut BTreeMap<String, String>) -> Result<()> {
    for entry in std::fs::read_dir(dir).map_err(Error::unexpected)? {
        let entry = entry.map_err(Error::unexpected)?;
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, files)?;
            continue;
        }
        let relativ = path
            .strip_prefix(root)
            .map_err(Error::unexpected)?
            .to_string_lossy()
            .replace('\\', "/");
        files.insert(relativ, read_text(&path)?);
    }
    Ok(())
}

/// Liest eine Datei mit einheitlichen Zeilenenden
fn read_text(file: &Path) -> Result<String> {
    let bytes = std::fs::read(file).map_err(Error::unexpected)?;
    Ok(String::from_utf8_lossy(&bytes).replace("\r\n", "\n"))
}

/// Löscht Datei oder Ordner, falls vorhanden
fn remove(path: &Path) -> Result<()> {
    let result = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    };
    result.map_err(Error::unexpected)
}

/// Kopiert Datei oder Ordner (mit allem darin)
fn copy(from: &Path, to: &Path) -> Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to).map_err(Error::unexpected)?;
        for entry in std::fs::read_dir(from).map_err(Error::unexpected)? {
            let entry = entry.map_err(Error::unexpected)?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(Error::unexpected)?;
    }
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(Error::unexpected)
}

/// Kopiert Datei oder Ordner; auch von außerhalb dieses Moduls gebraucht (Skills, Styles)
pub(crate) fn copy_path(from: &Path, to: &Path) -> Result<()> {
    copy(from, to)
}

/// Löscht Datei oder Ordner, falls vorhanden; auch von außerhalb dieses Moduls gebraucht
pub(crate) fn remove_path(path: &Path) -> Result<()> {
    remove(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ordner {
        dir: tempfile::TempDir,
    }

    impl Ordner {
        fn neu() -> Self {
            Self {
                dir: tempfile::tempdir().expect("Temp-Ordner"),
            }
        }

        fn pfad(&self, name: &str) -> std::path::PathBuf {
            self.dir.path().join(name)
        }

        fn datei(&self, name: &str, text: &str) -> std::path::PathBuf {
            let path = self.pfad(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            path
        }
    }

    #[test]
    fn erkennt_fehlende_gleiche_und_veraltete_basis_einer_datei() {
        let ordner = Ordner::neu();
        let snapshot = BaseSnapshot::new();
        let standard = ordner.datei("std.md", "neu\n");
        assert_eq!(
            snapshot.state(&ordner.pfad("fehlt.md"), &standard).unwrap(),
            BaseState::Unknown
        );
        assert_eq!(
            snapshot
                .state(&ordner.datei("gleich.md", "neu\r\n"), &standard)
                .unwrap(),
            BaseState::Current
        );
        assert_eq!(
            snapshot
                .state(&ordner.datei("alt.md", "alt\n"), &standard)
                .unwrap(),
            BaseState::Outdated
        );
    }

    #[test]
    fn vergleicht_ordner_mit_allen_dateien() {
        let ordner = Ordner::neu();
        let snapshot = BaseSnapshot::new();
        ordner.datei("std/SKILL.md", "a");
        ordner.datei("std/extra.md", "b");
        ordner.datei("base/SKILL.md", "a");
        let (base, standard) = (ordner.pfad("base"), ordner.pfad("std"));
        assert_eq!(
            snapshot.state(&base, &standard).unwrap(),
            BaseState::Outdated
        );
        snapshot.save(&standard, &base).unwrap();
        assert_eq!(
            snapshot.state(&base, &standard).unwrap(),
            BaseState::Current
        );
    }

    #[test]
    fn liefert_die_unterschiede_basis_zu_standard() {
        let ordner = Ordner::neu();
        let diff = BaseSnapshot::new().diff(
            &ordner.datei("alt.md", "alt\n"),
            &ordner.datei("std.md", "neu\n"),
        );
        assert!(diff.contains("-alt"), "{diff}");
        assert!(diff.contains("+neu"), "{diff}");
    }

    #[test]
    fn fasst_zustand_warnung_und_diff_zusammen() {
        let ordner = Ordner::neu();
        let snapshot = BaseSnapshot::new();
        let standard = ordner.datei("std.md", "neu\n");
        assert_eq!(
            snapshot
                .check(&ordner.datei("gleich.md", "neu\n"), &standard)
                .unwrap(),
            BaseCheck {
                state: BaseState::Current,
                warnings: Vec::new(),
                diff: None,
            }
        );
        let outdated = snapshot
            .check(&ordner.datei("alt.md", "alt\n"), &standard)
            .unwrap();
        assert_eq!(outdated.state, BaseState::Outdated);
        assert_eq!(
            outdated
                .warnings
                .iter()
                .map(|w| w.code.as_str())
                .collect::<Vec<_>>(),
            vec!["base-outdated"]
        );
        assert!(outdated.diff.unwrap().contains("+neu"));
        let unknown = snapshot.check(&ordner.pfad("fehlt.md"), &standard).unwrap();
        assert_eq!(
            unknown
                .warnings
                .iter()
                .map(|w| w.code.as_str())
                .collect::<Vec<_>>(),
            vec!["base-unknown"]
        );
    }

    #[test]
    fn legt_die_basis_aus_text_an_und_formatiert_json_einheitlich() {
        let ordner = Ordner::neu();
        let snapshot = BaseSnapshot::new();
        let base = ordner.pfad(".base/workflow.json");
        let text = BaseSnapshot::format_json(&serde_json::json!([1])).unwrap();
        snapshot.save_text(&text, &base).unwrap();
        assert_eq!(
            snapshot
                .state(&base, &ordner.datei("std.json", "[\n  1\n]\n"))
                .unwrap(),
            BaseState::Current
        );
        assert_eq!(text, "[\n  1\n]\n");
    }
}
