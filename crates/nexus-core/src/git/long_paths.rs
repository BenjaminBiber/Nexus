//! Portiert `packages/core/src/git/LongPaths.ts`.

use std::path::Path;

use crate::git::{GitCommands, GitRunner};

/// Hinweis, wenn die Einstellung neu gesetzt wurde
pub const LONG_PATHS_NOTICE: &str =
    "core.longpaths im Repo eingeschaltet, damit lange Pfade im Worktree funktionieren.";

/// Git-Argumente, die beim `worktree add` selbst lange Pfade erlauben (falls das Setzen
/// der Konfiguration scheitert)
pub const LONG_PATHS_ARGS: [&str; 2] = ["-c", "core.longpaths=true"];

/// Schaltet unter Windows `core.longpaths` in der lokalen Repo-Konfiguration ein:
/// Harness-Worktrees liegen unter <repo>-worktrees/<ID> und sind damit oft tiefer als der
/// Hauptordner, sonst scheitert `git worktree add` an langen Pfaden. Die Einstellung gilt
/// für das Repo und alle seine Worktrees.
#[derive(Debug, Clone, Copy, Default)]
pub struct LongPaths {
    git: GitRunner,
}

impl LongPaths {
    /// Neue Hilfe für die laufende Plattform
    pub fn new() -> Self {
        Self::default()
    }

    /// Setzt die Einstellung, falls sie unter Windows fehlt; liefert true, wenn sie neu
    /// gesetzt wurde (sonst false, auch bei Fehlern)
    pub fn ensure(&self, repo_root: &Path) -> bool {
        self.ensure_on(repo_root, cfg!(windows))
    }

    /// Wie [`LongPaths::ensure`], aber mit ausdrücklich genannter Plattform (für Tests)
    pub fn ensure_on(&self, repo_root: &Path, windows: bool) -> bool {
        if !windows {
            return false;
        }
        let current = self
            .git
            .try_run(repo_root, &["config", "--get", "core.longpaths"]);
        if current.ok && current.out.to_lowercase() == "true" {
            return false;
        }
        self.git
            .try_run(repo_root, &["config", "core.longpaths", "true"])
            .ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::TempRepo;

    /// Wert von `core.longpaths` in der lokalen Repo-Konfiguration, leer wenn nicht gesetzt
    fn value(repo: &TempRepo) -> String {
        let (ok, out, _) = repo.try_git(&["config", "--local", "--get", "core.longpaths"]);
        if ok {
            out
        } else {
            String::new()
        }
    }

    #[test]
    fn setzt_core_longpaths_unter_windows_einmalig_und_ist_idempotent() {
        let repo = TempRepo::new();
        let long_paths = LongPaths::new();

        assert!(long_paths.ensure_on(repo.path(), true));
        assert_eq!(value(&repo), "true");
        assert!(!long_paths.ensure_on(repo.path(), true));
        assert_eq!(value(&repo), "true");
    }

    #[test]
    fn tut_unter_anderen_plattformen_nichts() {
        let repo = TempRepo::new();

        assert!(!LongPaths::new().ensure_on(repo.path(), false));
        assert_eq!(value(&repo), "");
    }
}
