//! Testhilfen für die Modultests: temporäre Ordner und temporäre Git-Projekte.
//! Entspricht `packages/core/tests/unit/helpers/{TempDir,TempRepo}.ts` und der Hilfe
//! `tests/common/mod.rs` der Integrationstests.

// Jeder Modultest nutzt nur einen Teil der Hilfen
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use tempfile::TempDir;

/// Legt einen temporären Ordner an.
pub fn make_temp_dir(prefix: &str) -> TempDir {
    tempfile::Builder::new().prefix(prefix).tempdir().unwrap()
}

/// Kanonischer langer Pfad eines Ordners, ohne die Windows-Schreibweise `\\?\`.
/// Bewusst aufgelöst: unter Windows steht TEMP oft als 8.3-Kurzname, unter macOS zeigt
/// `/var` auf `/private/var`. Git liefert Pfade immer aufgelöst zurück.
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

/// Hält die leere globale Git-Konfiguration der Tests am Leben
static GIT_CONFIG: OnceLock<TempDir> = OnceLock::new();

/// Trennt alle Git-Aufrufe der Tests von der Konfiguration des Rechners: sonst
/// entscheidet z. B. ein global gesetztes `core.longpaths` über das Ergebnis
/// (entspricht `vi.stubEnv('GIT_CONFIG_GLOBAL', …)` in den TypeScript-Tests).
fn isolate_git_config() {
    let home = GIT_CONFIG.get_or_init(|| make_temp_dir("harness-gitconfig-"));
    std::env::set_var("GIT_CONFIG_GLOBAL", home.path().join("gitconfig"));
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
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

    /// Führt Git im Projekt aus und erwartet Erfolg
    pub fn git(&self, args: &[&str]) -> String {
        let output = self.try_git(args);
        assert!(output.0, "git {}: {}", args.join(" "), output.2);
        output.1
    }

    /// Führt Git im Projekt aus und liefert Erfolg, Ausgabe und Fehlerausgabe
    pub fn try_git(&self, args: &[&str]) -> (bool, String, String) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        )
    }

    /// Schreibt eine Datei im Projekt
    pub fn write(&self, file: &str, content: &str) {
        let path = self.path.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// Schreibt eine Datei und committet sie auf dem aktuellen Branch
    pub fn commit(&self, file: &str, content: &str) {
        self.write(file, content);
        self.git(&["add", file]);
        self.git(&["commit", "-q", "-m", &format!("{file}={content}")]);
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
        isolate_git_config();
        let base = make_temp_dir("harness-core-");
        let path = canonical(base.path()).join("Mein Projekt ä");
        std::fs::create_dir(&path).unwrap();
        let repo = Self { base, path };
        repo.git(&["init", "-q"]);
        repo.git(&["config", "user.name", "t"]);
        repo.git(&["config", "user.email", "t@t"]);
        if with_commit {
            repo.git(&["commit", "--allow-empty", "-q", "-m", "init"]);
        }
        repo
    }
}

impl Default for TempRepo {
    fn default() -> Self {
        Self::new()
    }
}
