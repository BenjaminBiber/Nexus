//! Ein eingerichtetes Git-Repo: Harness-Ordner im gemeinsamen Git-Verzeichnis mit Token,
//! Zählern und Session-Nummern. Tickets und Fragen liegen in der Board-Datenbank, nicht hier.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use crate::counter::Counter;
use crate::error::{Error, HarnessError, Result};
use crate::git::GitPaths;
use crate::project::{HarnessLayout, TokenStore};
use crate::sessions::SessionNumbers;

/// Gemeinsames Git-Verzeichnis je Ordner: ändert sich praktisch nie, ein Git-Start kostet
/// unter Windows einige 100 ms. Gemerkt werden nur Erfolge; ob der Harness eingerichtet ist,
/// prüft `open` trotzdem jedes Mal.
static COMMON_DIRS: LazyLock<Mutex<HashMap<PathBuf, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Ein eingerichtetes Projekt mit seinem Harness-Ordner
#[derive(Debug)]
pub struct HarnessProject {
    /// Pfade im Harness-Ordner
    pub layout: HarnessLayout,
    /// Wurzel des Hauptordners (auch wenn das Projekt aus einem Worktree geöffnet wurde)
    pub root: String,
    counter: Arc<Counter>,
    session_numbers: SessionNumbers,
}

impl HarnessProject {
    /// Richtet das Projekt ein; wiederholbar, vorhandene Daten bleiben unverändert
    pub fn init(cwd: &Path) -> Result<Self> {
        let project = Self::locate(cwd)?;
        fs::create_dir_all(project.layout.dir()).map_err(Error::unexpected)?;
        if !project.layout.counters_file().exists() {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(project.layout.counters_file())
            {
                Ok(mut file) => file
                    .write_all(br#"{"session":0}"#)
                    .map_err(Error::unexpected)?,
                // Ein anderer Prozess war schneller – seine Zählerstände gelten
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(Error::unexpected(error)),
            }
        }
        TokenStore::new(project.layout.clone()).ensure()?;
        Ok(project)
    }

    /// Öffnet ein eingerichtetes Projekt; ohne `harness init` gibt es einen Hinweis
    pub fn open(cwd: &Path) -> Result<Self> {
        let project = Self::locate_cached(cwd)?;
        if !project.layout.counters_file().exists() && !project.layout.token_file().exists() {
            return Err(HarnessError::new(
                "not-initialized",
                "Dieses Projekt ist nicht eingerichtet. Bitte zuerst `harness init` ausführen.",
            )
            .into());
        }
        Ok(project)
    }

    /// Zähler des Projekts
    pub fn counter(&self) -> &Counter {
        &self.counter
    }

    /// Nummern der Sessions dieses Projekts
    pub fn session_numbers(&self) -> &SessionNumbers {
        &self.session_numbers
    }

    /// Ermittelt Harness-Ordner und Projektwurzel aus einem beliebigen Ordner (auch Worktree)
    fn locate(cwd: &Path) -> Result<Self> {
        let git = GitPaths::new();
        let common_dir = git.common_dir(cwd)?;
        remember(cwd, &common_dir);
        Ok(Self::at(&git, &common_dir))
    }

    /// Wie `locate`, aber mit gemerktem Git-Verzeichnis
    fn locate_cached(cwd: &Path) -> Result<Self> {
        let git = GitPaths::new();
        let known = COMMON_DIRS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(cwd)
            .cloned();
        let common_dir = match known {
            Some(common_dir) => common_dir,
            None => {
                let common_dir = git.common_dir(cwd)?;
                remember(cwd, &common_dir);
                common_dir
            }
        };
        Ok(Self::at(&git, &common_dir))
    }

    /// Projekt zu einem bekannten gemeinsamen Git-Verzeichnis
    fn at(git: &GitPaths, common_dir: &str) -> Self {
        let layout = HarnessLayout::new(common_dir);
        let counter = Arc::new(Counter::new(layout.clone()));
        Self {
            session_numbers: SessionNumbers::new(layout.clone(), counter.clone()),
            counter,
            root: git.root_of(common_dir),
            layout,
        }
    }
}

/// Merkt sich das gemeinsame Git-Verzeichnis eines Ordners
fn remember(cwd: &Path, common_dir: &str) {
    COMMON_DIRS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(cwd.to_path_buf(), common_dir.to_string());
}
