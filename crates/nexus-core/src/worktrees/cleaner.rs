//! Räumt Worktrees auf, deren Tickets alle erledigt sind.
//! Portiert aus `packages/core/src/worktrees/WorktreeCleaner.ts`.

use std::collections::BTreeSet;
use std::path::Path;

use crate::error::Result;
use crate::git::{GitCommands, PathKey};
use crate::install::PROJECT_RULE_PATHS;
use crate::worktrees::links::WorktreeLinks;
use crate::worktrees::registry::{WorktreeEntry, WorktreeRegistry};

/// Lokale Dateien, die der Harness selbst im Worktree anlegt (Plugin-Einstellung, Rules für
/// Doku- und Board-Regeln); sie zählen nicht als Änderung, auch wenn ein älteres Projekt sie
/// noch nicht in `info/exclude` führt.
pub(crate) fn harness_files() -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    files.insert(".claude/settings.local.json".to_string());
    for path in PROJECT_RULE_PATHS {
        files.insert((*path).to_string());
    }
    files
}

/// Räumt Worktrees auf, deren Tickets alle erledigt sind. Der Branch bleibt, es wird nie
/// gemergt (ADR 0002).
#[derive(Debug)]
pub struct WorktreeCleaner<'a, G: GitCommands> {
    registry: &'a WorktreeRegistry,
    git: &'a G,
    links: WorktreeLinks,
    keys: PathKey,
}

impl<'a, G: GitCommands> WorktreeCleaner<'a, G> {
    /// Aufräumer für dieses Verzeichnis und diesen Git-Aufrufer
    pub fn new(registry: &'a WorktreeRegistry, git: &'a G) -> Self {
        Self {
            registry,
            git,
            links: WorktreeLinks::new(),
            keys: PathKey::new(),
        }
    }

    /// Entfernt alle bereiten, sauberen Worktrees, in denen `cwd` nicht liegt; liefert die
    /// entfernten Pfade. Vorher werden Symlinks/Junctions im Worktree entfernt (nur die
    /// Links, nie ihr Ziel); bleibt ein Link stehen oder scheitert Git, bleibt der Worktree
    /// stehen (kein Abbruch von start/done)
    pub fn cleanup(&self, cwd: &Path) -> Result<Vec<String>> {
        let mut removed = Vec::new();
        for (branch, entry) in self
            .registry
            .read()?
            .iter()
            .map(|(branch, entry)| (branch.clone(), entry.clone()))
            .collect::<Vec<_>>()
        {
            if !self.removable(&entry, cwd) {
                continue;
            }
            if !self
                .links
                .remove_all(Path::new(&entry.path))
                .failed
                .is_empty()
            {
                continue;
            }
            // --force nur, weil die lokalen Dateien des Harness nicht im Git liegen;
            // Links gibt es jetzt keine mehr
            if !self
                .git
                .try_run(cwd, &["worktree", "remove", "--force", &entry.path])
                .ok
            {
                continue;
            }
            self.registry.mark_removed(&branch)?;
            removed.push(entry.path);
        }
        Ok(removed)
    }

    /// Bereit zum Merge, noch vorhanden, nicht der aktuelle Ordner und ohne eigene Änderungen
    fn removable(&self, entry: &WorktreeEntry, cwd: &Path) -> bool {
        if !entry.ready_to_merge || entry.is_removed() || !Path::new(&entry.path).exists() {
            return false;
        }
        if self.contains(Path::new(&entry.path), cwd) {
            return false;
        }
        self.is_clean(Path::new(&entry.path))
    }

    /// Ist `path` der Ordner `outer` selbst oder liegt darin (Groß-/Kleinschreibung unter
    /// Windows egal, wie in TypeScript)
    fn contains(&self, outer: &Path, path: &Path) -> bool {
        let a = self.keys.of(outer);
        let b = self.keys.of(path);
        b == a || b.starts_with(&format!("{a}/"))
    }

    /// Sauber: keine geänderten oder neuen Dateien außer den lokalen Dateien des Harness
    fn is_clean(&self, path: &Path) -> bool {
        let Ok(output) = self
            .git
            .run(path, &["status", "--porcelain", "--untracked-files=all"])
        else {
            return false;
        };
        let harness = harness_files();
        // Die Ausgabe ist getrimmt: Statuskürzel ("M", "??") abschneiden statt feste Spalten
        // anzunehmen
        output
            .split('\n')
            .filter(|line| !line.trim().is_empty())
            .map(|line| strip_status(line.trim()))
            .all(|file| harness.contains(&file.replace('\\', "/")))
    }
}

/// Schneidet das Statuskürzel und die folgenden Leerzeichen ab
fn strip_status(line: &str) -> String {
    match line.split_once(char::is_whitespace) {
        Some((_, rest)) => rest.trim_start().to_string(),
        None => line.to_string(),
    }
}
