//! Projekte des Servers: Liste, Anmeldung und geöffnete Projekte; portiert aus
//! `packages/server/src/projects/ProjectService.ts`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use nexus_core::registry::{ProjectRegistry, ProjectState, RegisteredProject};
use nexus_core::{HarnessError, HarnessHome, HarnessProject, Result};
use serde_json::Value;

/// Ein angemeldetes Projekt und ob es neu dazugekommen ist
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredNow {
    pub project: ProjectState,
    pub created: bool,
}

/// Ein geöffnetes Projekt samt seinem Eintrag in der Projektliste
#[derive(Debug, Clone)]
pub struct OpenedProject {
    pub project: Arc<HarnessProject>,
    pub entry: RegisteredProject,
}

/// Verwaltet die Projekte des Servers: Liste, Anmeldung und geöffnete Projekte
/// (zwischengespeichert).
pub struct ProjectService {
    registry: ProjectRegistry,
    opened: RwLock<HashMap<String, Arc<HarnessProject>>>,
}

impl ProjectService {
    /// Dienst über der Projektliste des Benutzerordners
    pub fn new(home: HarnessHome) -> Self {
        Self {
            registry: ProjectRegistry::new(home),
            opened: RwLock::new(HashMap::new()),
        }
    }

    /// Die Projektliste selbst (Folgetickets brauchen sie direkt)
    pub fn registry(&self) -> &ProjectRegistry {
        &self.registry
    }

    /// Alle Projekte mit Erreichbarkeit
    pub fn list(&self) -> Result<Vec<ProjectState>> {
        self.registry.list_with_state()
    }

    /// Meldet ein Projekt an; nur absolute Pfade
    pub fn register(&self, path: &Value) -> Result<RegisteredNow> {
        let path = path.as_str().filter(|path| !path.trim().is_empty());
        let Some(path) = path else {
            return Err(HarnessError::new("path-missing", "Der Pfad des Projekts fehlt.").into());
        };
        if !Path::new(path).is_absolute() {
            return Err(HarnessError::new("path-relative", "Der Pfad muss absolut sein.").into());
        }
        let added = self.registry.add(Path::new(path))?;
        Ok(RegisteredNow {
            // Gerade angemeldet, also erreichbar
            project: ProjectState {
                project: added.project,
                reachable: true,
            },
            created: added.created,
        })
    }

    /// Öffnet ein Projekt über einen (auch im Worktree liegenden) Pfad; es muss angemeldet
    /// sein
    pub fn open_by_path(&self, path: &Path) -> Result<OpenedProject> {
        let opened = HarnessProject::open(path)?;
        self.open(&self.registry.id_of(&opened.root))
    }

    /// Das geöffnete Projekt zur ID; unbekannt → 404
    pub fn open(&self, id: &str) -> Result<OpenedProject> {
        let entry = self.registry.find(id)?.ok_or_else(|| {
            HarnessError::new("project-not-found", format!("Projekt {id} gibt es nicht."))
        })?;
        if let Some(project) = self.cached(id) {
            return Ok(OpenedProject { project, entry });
        }
        let project = Arc::new(HarnessProject::open(Path::new(&entry.path))?);
        self.opened
            .write()
            .expect("Sperre der geöffneten Projekte")
            .insert(id.to_string(), project.clone());
        Ok(OpenedProject { project, entry })
    }

    /// Das schon geöffnete Projekt, falls es eines gibt
    fn cached(&self, id: &str) -> Option<Arc<HarnessProject>> {
        self.opened
            .read()
            .expect("Sperre der geöffneten Projekte")
            .get(id)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dienst über einem frischen Benutzerordner
    fn service() -> (ProjectService, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (ProjectService::new(HarnessHome::at(dir.path())), dir)
    }

    #[test]
    fn verlangt_einen_absoluten_pfad() {
        let (service, _dir) = service();
        for leer in [
            Value::Null,
            Value::String(String::new()),
            Value::String("  ".into()),
        ] {
            let error = service.register(&leer).unwrap_err();
            assert_eq!(error.code(), Some("path-missing"));
        }
        let error = service
            .register(&Value::String("relativ/pfad".into()))
            .unwrap_err();
        assert_eq!(error.code(), Some("path-relative"));
    }

    #[test]
    fn meldet_ein_unbekanntes_projekt_als_nicht_gefunden() {
        let (service, _dir) = service();
        assert!(service.list().unwrap().is_empty());
        let error = service.open("abc").unwrap_err();
        assert_eq!(error.code(), Some("project-not-found"));
        assert_eq!(error.to_string(), "Projekt abc gibt es nicht.");
    }
}
