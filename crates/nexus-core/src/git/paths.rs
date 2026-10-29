//! Ermittelt die für den Harness wichtigen Git-Pfade (immer mit Schrägstrichen).

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{HarnessError, Result};
use crate::git::{GitCommands, GitRunner};

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

    /// Gemeinsames Git-Verzeichnis, für Hauptordner und alle Worktrees identisch.
    /// Zuerst ohne Git-Prozess ([`GitPaths::common_dir_fast`]), sonst über `git rev-parse`.
    pub fn common_dir(&self, cwd: &Path) -> Result<String> {
        if let Some(fast) = Self::common_dir_fast(cwd) {
            return Ok(fast);
        }
        self.git
            .run(
                cwd,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            )
            .map(|out| normalize(&out))
            .map_err(|_| {
                HarnessError::new(
                    "not-a-git-project",
                    "Hier ist kein Git-Projekt. `nexus init` muss in einem Git-Repository laufen.",
                )
                .into()
            })
    }

    /// Gemeinsames Git-Verzeichnis ohne Git-Prozess: vom Ordner aufwärts `.git` suchen, bei
    /// einem Worktree die Datei `gitdir:` und darin `commondir` auflösen. Ein Git-Start kostet
    /// einige 10 ms, das zählt in jedem Hook-Aufruf.
    ///
    /// Bei allem Unklaren (`GIT_DIR` gesetzt, fehlende Dateien, kein Git-Verzeichnis am Ziel)
    /// liefert sie nichts; dann entscheidet `git rev-parse`.
    pub fn common_dir_fast(cwd: &Path) -> Option<String> {
        if std::env::var_os("GIT_DIR").is_some() || std::env::var_os("GIT_COMMON_DIR").is_some() {
            return None;
        }
        let start = real_path(cwd)?;
        let mut folder = start.as_path();
        loop {
            match marker(folder) {
                Marker::Common(found) => {
                    return is_git_dir(&found).then(|| normalize(&found.to_string_lossy()))
                }
                Marker::Unclear => return None,
                Marker::None => folder = folder.parent()?,
            }
        }
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

/// Was ein Ordner über das gemeinsame Git-Verzeichnis verrät
enum Marker {
    /// Hier liegt `.git`; das gemeinsame Verzeichnis ist dieser Pfad
    Common(PathBuf),
    /// Hier liegt `.git`, lässt sich aber nicht deuten – dann entscheidet `git rev-parse`
    Unclear,
    /// Hier liegt kein `.git`; weiter aufwärts
    None,
}

/// Deutet das `.git` eines Ordners: ein Ordner ist selbst das Verzeichnis, eine Datei
/// (`gitdir: …`) zeigt auf das Verzeichnis eines Worktrees
fn marker(folder: &Path) -> Marker {
    let git = folder.join(".git");
    let Ok(meta) = fs::metadata(&git) else {
        return Marker::None;
    };
    if meta.is_dir() {
        return common_of(&git);
    }
    let Ok(text) = fs::read_to_string(&git) else {
        return Marker::Unclear;
    };
    match text.trim().strip_prefix("gitdir:") {
        Some(target) => common_of(&folder.join(target.trim())),
        None => Marker::Unclear,
    }
}

/// Gemeinsames Verzeichnis zu einem Git-Verzeichnis: die Datei `commondir` zeigt vom
/// Verzeichnis eines Worktrees auf das gemeinsame, sonst ist es das Verzeichnis selbst
fn common_of(git_dir: &Path) -> Marker {
    let common = match fs::read_to_string(git_dir.join("commondir")) {
        Ok(target) => git_dir.join(target.trim()),
        Err(_) => git_dir.to_path_buf(),
    };
    match real_path(&common) {
        Some(path) => Marker::Common(path),
        None => Marker::Unclear,
    }
}

/// Ist das ein Git-Verzeichnis (und nicht irgendein Ordner namens `.git`)?
fn is_git_dir(path: &Path) -> bool {
    path.join("HEAD").is_file() && path.join("objects").is_dir()
}

/// Aufgelöster Pfad ohne die Windows-Schreibweise `\\?\`; Git liefert Pfade genauso
/// aufgelöst, weil es mit dem Arbeitsordner des Prozesses rechnet
fn real_path(path: &Path) -> Option<PathBuf> {
    let resolved = fs::canonicalize(path).ok()?;
    #[cfg(windows)]
    {
        let text = resolved.to_string_lossy().into_owned();
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return Some(PathBuf::from(rest));
        }
    }
    Some(resolved)
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

    /// Dasselbe Ergebnis wie `git rev-parse --path-format=absolute --git-common-dir`
    #[test]
    fn findet_das_gemeinsame_verzeichnis_ohne_git_prozess() {
        let repo = crate::test_support::TempRepo::new();
        let worktree = repo.add_worktree("wt-1");
        let unterordner = repo.path().join("src/tief");
        std::fs::create_dir_all(&unterordner).unwrap();

        for ordner in [repo.path(), worktree.as_path(), unterordner.as_path()] {
            let erwartet = GitPaths::new()
                .git
                .run(
                    ordner,
                    &["rev-parse", "--path-format=absolute", "--git-common-dir"],
                )
                .map(|out| normalize(&out))
                .unwrap();
            assert_eq!(
                GitPaths::common_dir_fast(ordner),
                Some(erwartet),
                "{}",
                ordner.display()
            );
        }
    }

    #[test]
    fn ausserhalb_eines_repos_entscheidet_git() {
        let dir = crate::test_support::make_temp_dir("harness-ohne-git-");
        assert_eq!(GitPaths::common_dir_fast(dir.path()), None);
        assert_eq!(
            GitPaths::new().common_dir(dir.path()).unwrap_err().code(),
            Some("not-a-git-project")
        );
    }
}
