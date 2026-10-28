//! Testhilfen für die Integrationstests: temporäre Ordner und temporäre Git-Projekte.
//! Entspricht `packages/core/tests/unit/helpers/{TempDir,TempRepo}.ts`.

// Jede Testdatei ist ein eigenes Programm und nutzt nur einen Teil der Hilfen
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// Legt einen temporären Ordner an und liefert ihn mit kanonischem langem Pfad.
/// Bewusst aufgelöst: unter Windows steht TEMP oft als 8.3-Kurzname
/// (`C:\Users\BENJAM~1.BIB\…`), unter macOS zeigt `/var` auf `/private/var`.
/// Git liefert Pfade immer aufgelöst zurück, sonst scheitern alle Pfadvergleiche.
pub fn make_temp_dir(prefix: &str) -> TempDir {
    tempfile::Builder::new().prefix(prefix).tempdir().unwrap()
}

/// Kanonischer langer Pfad eines Ordners, ohne die Windows-Schreibweise `\\?\`
pub fn canonical(path: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    {
        let text = resolved.to_string_lossy().into_owned();
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    resolved
}

/// Vereinheitlicht einen Pfad auf Schrägstriche, wie `GitPaths` es tut
pub fn slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Temporäres Git-Projekt für Tests (Ordnername mit Leerzeichen und Umlaut),
/// auf Wunsch mit weiteren Worktrees.
pub struct TempRepo {
    base: TempDir,
    path: PathBuf,
}

impl TempRepo {
    /// Legt das Projekt mit einem ersten Commit an
    pub fn new() -> Self {
        Self::create(true)
    }

    /// Legt das Projekt ohne Commit an
    pub fn without_commit() -> Self {
        Self::create(false)
    }

    /// Ordner des Projekts
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Ordner, in dem Projekt und Worktrees liegen
    pub fn base(&self) -> PathBuf {
        canonical(self.base.path())
    }

    /// Führt Git im Projekt aus
    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Legt einen zusätzlichen Worktree neben dem Projekt an und liefert seinen Pfad
    pub fn add_worktree(&self, name: &str) -> PathBuf {
        let path = self.base().join(name);
        self.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            &format!("branch-{name}"),
            &path.to_string_lossy(),
        ]);
        path
    }

    /// Legt das Projekt an
    fn create(with_commit: bool) -> Self {
        let base = make_temp_dir("harness-core-");
        let path = canonical(base.path()).join("Mein Projekt ä");
        std::fs::create_dir(&path).unwrap();
        let repo = Self { base, path };
        repo.git(&["init", "-q"]);
        if with_commit {
            repo.git(&[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "--allow-empty",
                "-q",
                "-m",
                "init",
            ]);
        }
        repo
    }
}

impl Default for TempRepo {
    fn default() -> Self {
        Self::new()
    }
}
