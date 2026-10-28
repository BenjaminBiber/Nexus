//! Ermittelt die für den Harness wichtigen Git-Pfade (immer mit Schrägstrichen).

use std::path::Path;

use crate::error::{HarnessError, Result};
use crate::git::GitRunner;

/// Git-Pfade eines Projekts oder Worktrees.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitPaths {
    git: GitRunner,
}

impl GitPaths {
    /// Pfade über einen eigenen Git-Aufrufer
    pub fn new() -> Self {
        Self::default()
    }

    /// Gemeinsames Git-Verzeichnis, für Hauptordner und alle Worktrees identisch
    pub fn common_dir(&self, cwd: &Path) -> Result<String> {
        self.git
            .run(cwd, &["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .map(|out| normalize(&out))
            .map_err(|_| {
                HarnessError::new(
                    "not-a-git-project",
                    "Hier ist kein Git-Projekt. `harness init` muss in einem Git-Repository laufen.",
                )
                .into()
            })
    }

    /// Wurzel des Hauptordners (nicht des Worktrees), abgeleitet aus dem gemeinsamen Verzeichnis
    pub fn main_root(&self, cwd: &Path) -> Result<String> {
        Ok(self.root_of(&self.common_dir(cwd)?))
    }

    /// Wurzel des Hauptordners zu einem schon ermittelten gemeinsamen Verzeichnis (ohne Git-Aufruf).
    /// Gerechnet wird auf dem Text mit Schrägstrichen, damit ein Windows-Pfad auch unter Linux
    /// richtig zerlegt wird – genau wie `dirname` in Node.
    pub fn root_of(&self, common_dir: &str) -> String {
        let normalized = normalize(common_dir);
        match normalized.rsplit_once('/') {
            Some(("", _)) => "/".to_string(),
            Some((parent, _)) => parent.to_string(),
            None => ".".to_string(),
        }
    }

    /// Wurzel des aktuellen Arbeitsordners (im Worktree dessen eigene Wurzel)
    pub fn top_level(&self, cwd: &Path) -> Result<String> {
        self.git
            .run(cwd, &["rev-parse", "--show-toplevel"])
            .map(|out| normalize(&out))
            .map_err(|_| {
                HarnessError::new("not-a-git-project", "Hier ist kein Git-Projekt.").into()
            })
    }
}

/// Vereinheitlicht Pfade auf Schrägstriche, damit Windows und Linux gleich vergleichbar sind
fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leitet_die_wurzel_aus_dem_gemeinsamen_verzeichnis_ab() {
        let paths = GitPaths::new();
        assert_eq!(
            paths.root_of("/tmp/Mein Projekt ä/.git"),
            "/tmp/Mein Projekt ä"
        );
        assert_eq!(paths.root_of("C:\\Projekte\\x\\.git"), "C:/Projekte/x");
        assert_eq!(paths.root_of("C:/Projekte/x/.git"), "C:/Projekte/x");
        // Wie `dirname` in Node: ohne Ordner darüber bleibt die Wurzel bzw. der Punkt
        assert_eq!(paths.root_of("/.git"), "/");
        assert_eq!(paths.root_of(".git"), ".");
    }
}
