//! Benutzerweite Liste der angemeldeten Projekte (`projects.json`); geschrieben wird nur
//! unter Sperre, damit zwei gleichzeitige Anmeldungen nichts verlieren.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, HarnessError, Result};
use crate::locking::{AtomicWriter, FileLock, FileLockOptions};
use crate::project::HarnessProject;
use crate::registry::HarnessHome;

/// Ein angemeldetes Projekt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredProject {
    pub id: String,
    pub name: String,
    pub path: String,
}

/// Ein angemeldetes Projekt mit der Angabe, ob es noch erreichbar ist
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectState {
    #[serde(flatten)]
    pub project: RegisteredProject,
    pub reachable: bool,
}

/// Ergebnis einer Anmeldung: das Projekt und ob es neu dazugekommen ist
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedProject {
    pub project: RegisteredProject,
    pub created: bool,
}

/// Liest und schreibt die Liste der angemeldeten Projekte.
#[derive(Debug)]
pub struct ProjectRegistry {
    home: HarnessHome,
    lock: FileLock,
    writer: AtomicWriter,
}

impl ProjectRegistry {
    /// Liste im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        let lock = FileLock::new(home.projects_lock(), FileLockOptions::default());
        Self {
            home,
            lock,
            writer: AtomicWriter::new(),
        }
    }

    /// Alle angemeldeten Projekte
    pub fn list(&self) -> Result<Vec<RegisteredProject>> {
        let file = self.home.projects_file();
        if !file.exists() {
            return Ok(Vec::new());
        }
        let corrupt = || {
            HarnessError::new(
                "projects-corrupt",
                format!("Die Projektliste ist beschädigt: {}", file.display()),
            )
        };
        let text = fs::read_to_string(&file).map_err(|_| corrupt())?;
        serde_json::from_str(&text).map_err(|_| corrupt().into())
    }

    /// Projekte mit Angabe, ob Ordner und Harness noch erreichbar sind (Einträge bleiben erhalten)
    pub fn list_with_state(&self) -> Result<Vec<ProjectState>> {
        Ok(self
            .list()?
            .into_iter()
            .map(|project| ProjectState {
                reachable: Self::is_reachable(&project.path),
                project,
            })
            .collect())
    }

    /// Sucht ein Projekt über seine ID
    pub fn find(&self, id: &str) -> Result<Option<RegisteredProject>> {
        Ok(self.list()?.into_iter().find(|project| project.id == id))
    }

    /// Meldet ein Projekt an (wiederholbar); auch ein Worktree-Pfad führt zum Hauptordner
    pub fn add(&self, path: &Path) -> Result<AddedProject> {
        let opened = HarnessProject::open(path)?;
        let project = RegisteredProject {
            id: self.id_of(&opened.root),
            name: Path::new(&opened.root)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: opened.root.clone(),
        };
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let created = self.lock.with_lock(|| {
            let all = self.list()?;
            if all.iter().any(|known| known.id == project.id) {
                return Ok(false);
            }
            let mut next = all;
            next.push(project.clone());
            let json = serde_json::to_string_pretty(&next).map_err(Error::unexpected)?;
            self.writer.write(&self.home.projects_file(), &json)?;
            Ok(true)
        })?;
        Ok(AddedProject { project, created })
    }

    /// Stabile Kurz-ID aus dem Pfad (unter Windows ohne Beachtung der Schreibweise)
    pub fn id_of(&self, path: &str) -> String {
        let normalized = path.replace('\\', "/");
        let normalized = normalized.trim_end_matches('/');
        let key = if cfg!(windows) {
            normalized.to_lowercase()
        } else {
            normalized.to_string()
        };
        sha1_smol::Sha1::from(key).digest().to_string()[..8].to_string()
    }

    /// Ordner vorhanden und Harness eingerichtet?
    fn is_reachable(path: &str) -> bool {
        HarnessProject::open(Path::new(path)).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Liste in einem leeren Benutzerordner
    fn registry(dir: &Path) -> ProjectRegistry {
        ProjectRegistry::new(HarnessHome::at(dir))
    }

    #[test]
    fn ist_anfangs_leer() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(registry(dir.path()).list().unwrap(), Vec::new());
    }

    #[test]
    fn meldet_eine_beschadigte_projektliste_verstandlich() {
        let dir = tempfile::tempdir().unwrap();
        let registry = registry(dir.path());
        fs::write(registry.home.projects_file(), "{kaputt").unwrap();

        let error = registry.list().unwrap_err();

        assert_eq!(error.code(), Some("projects-corrupt"));
        assert!(error.to_string().contains("projects.json"), "{error}");
    }

    #[test]
    fn bildet_eine_stabile_kurz_id_ohne_abschliessende_schragstriche() {
        let registry = registry(Path::new("h"));
        let id = registry.id_of("/tmp/Mein Projekt");

        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "{id}");
        assert_eq!(registry.id_of("/tmp/Mein Projekt///"), id);
        assert_eq!(registry.id_of("\\tmp\\Mein Projekt"), id);
        assert_ne!(registry.id_of("/tmp/Anderes Projekt"), id);
    }
}
