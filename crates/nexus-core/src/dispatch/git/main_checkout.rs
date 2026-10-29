//! Portiert `packages/core/src/dispatch/git/MainCheckout.ts`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Result;
use crate::git::{GitCommands, GitRunner};

/// Orte der Rules, die der Harness im Projekt anlegt (wie `PROJECT_RULE_PATHS` in
/// `packages/core/src/install/ProjectRules.ts`); sie zählen nie als Änderung
const PROJECT_RULE_PATHS: [&str; 2] = [
    ".claude/rules/ai-harness-docs.md",
    ".claude/rules/ai-harness-board.md",
];

/// Lokale Dateien des Harness (Plugin-Einstellung, Rules für Doku- und Board-Regeln);
/// zählen nie als Änderung
const HARNESS_FILES: [&str; 3] = [
    ".claude/settings.local.json",
    PROJECT_RULE_PATHS[0],
    PROJECT_RULE_PATHS[1],
];

/// Zeiger auf die aktuelle Spec aus der Spec-Kit-Zeit; bleibt in der Ausnahmeliste, damit
/// alte Arbeitsordner keinen Merge blockieren – ungestaged kein Hindernis
const SPEC_POINTER: &str = ".specify/feature.json";

/// Zustände, in denen nicht gemergt werden darf (Datei im Git-Verzeichnis → laufende
/// Aktion)
const BUSY: [(&str, &str); 5] = [
    ("MERGE_HEAD", "ein Merge"),
    ("rebase-merge", "ein Rebase"),
    ("rebase-apply", "ein Rebase"),
    ("CHERRY_PICK_HEAD", "ein Cherry-Pick"),
    ("REVERT_HEAD", "ein Revert"),
];

/// Zustand des Hauptordners vor einem Merge
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct CheckoutState {
    /// Ausgecheckter Branch; `None` bei Detached HEAD
    pub branch: Option<String>,
    /// Grund, warum gerade nicht gemergt werden darf, oder `None`
    pub busy: Option<String>,
    /// Geänderte oder neue Dateien, die keine lokale Datei des Harness sind (sortiert)
    pub blocking_files: Vec<String>,
    /// Davon die im Index vorgemerkten (gestagten) Dateien – mit ihnen weist Git jeden
    /// Merge ab (sortiert)
    pub staged_files: Vec<String>,
}

/// Prüft den Hauptordner eines Repos, bevor der Dispatcher hineinmergt.
#[derive(Debug, Clone, Copy, Default)]
pub struct MainCheckout<G: GitCommands = GitRunner> {
    git: G,
}

impl MainCheckout<GitRunner> {
    /// Prüfer mit dem gewöhnlichen Git-Aufrufer
    pub fn new() -> Self {
        Self {
            git: GitRunner::new(),
        }
    }
}

impl<G: GitCommands> MainCheckout<G> {
    /// Prüfer mit eigenem Git-Aufrufer
    pub fn with_git(git: G) -> Self {
        Self { git }
    }

    /// Branch, laufende Git-Aktion und störende lokale Änderungen
    pub fn inspect(&self, root: &Path) -> Result<CheckoutState> {
        let symbolic = self
            .git
            .try_run(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
        let running = self.running(root)?;
        let blocking = self.blocking(root)?;
        Ok(CheckoutState {
            branch: symbolic.ok.then_some(symbolic.out),
            busy: running.map(|what| format!("Im Hauptordner läuft gerade {what}.")),
            blocking_files: blocking.iter().map(|(file, _)| file.clone()).collect(),
            staged_files: blocking
                .iter()
                .filter(|(_, staged)| *staged)
                .map(|(file, _)| file.clone())
                .collect(),
        })
    }

    /// Läuft gerade ein Merge, Rebase, Cherry-Pick oder Revert? Liefert z. B. „ein Merge“
    /// oder `None`
    pub fn running(&self, root: &Path) -> Result<Option<&'static str>> {
        for (name, what) in BUSY {
            let path = self.git.run(root, &["rev-parse", "--git-path", name])?;
            if resolved(root, &path).exists() {
                return Ok(Some(what));
            }
        }
        Ok(None)
    }

    /// Alle lokal geänderten oder neuen Dateien des Ordners (ohne Ausnahmen); bei
    /// Umbenennung/Kopie beide Pfade
    pub fn changed(&self, root: &Path) -> Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in self.status(root)? {
            for path in entry.paths {
                if !files.contains(&path) {
                    files.push(path);
                }
            }
        }
        Ok(files)
    }

    /// Lokal geänderte oder neue Dateien aus `git status` außer denen des Harness; der
    /// Spec-Zeiger zählt nur gestagt. Bei Umbenennung/Kopie zählen beide Pfade.
    fn blocking(&self, root: &Path) -> Result<Vec<(String, bool)>> {
        let mut files: Vec<(String, bool)> = Vec::new();
        for entry in self.status(root)? {
            let first = entry.xy.chars().next().unwrap_or(' ');
            let unstaged_only = first == ' ' || entry.xy == "??";
            let allowed = |file: &String| {
                HARNESS_FILES.contains(&file.as_str()) || (unstaged_only && file == SPEC_POINTER)
            };
            if !entry.paths.iter().all(allowed) {
                files.push((entry.paths[0].clone(), !unstaged_only));
            }
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(files)
    }

    /// Einträge aus `git status --porcelain=v1 -z`: Statusspalten und Pfade (bei
    /// Umbenennung/Kopie neuer, dann alter Pfad)
    fn status(&self, root: &Path) -> Result<Vec<StatusEntry>> {
        let out = self.git.raw(
            root,
            &[
                "-c",
                "core.quotePath=false",
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
            ],
        )?;
        let entries: Vec<&str> = out.split('\0').collect();
        let mut result = Vec::new();
        let mut index = 0;
        while index < entries.len() {
            let entry = entries[index];
            index += 1;
            if entry.len() < 4 {
                continue;
            }
            let xy: String = entry.chars().take(2).collect();
            let mut paths = vec![entry[3..].to_string()];
            // Umbenennung/Kopie: der nächste Eintrag ist der alte Pfad
            if xy.chars().any(|c| c == 'R' || c == 'C') {
                if let Some(old) = entries.get(index).filter(|old| !old.is_empty()) {
                    paths.push((*old).to_string());
                }
                index += 1;
            }
            result.push(StatusEntry { xy, paths });
        }
        Ok(result)
    }
}

/// Eine Zeile aus `git status --porcelain=v1`
struct StatusEntry {
    xy: String,
    paths: Vec<String>,
}

/// Macht einen von Git gelieferten Pfad absolut (Git antwortet je nach Aufruf relativ)
fn resolved(root: &Path, path: &str) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::TempRepo;

    /// Projekt mit einer verfolgten Datei
    fn repo() -> TempRepo {
        let repo = TempRepo::without_commit();
        repo.commit("tracked.txt", "a");
        repo
    }

    /// Zustand des Hauptordners
    fn inspect(repo: &TempRepo) -> CheckoutState {
        MainCheckout::new().inspect(repo.path()).unwrap()
    }

    #[test]
    fn meldet_branch_und_einen_sauberen_ordner() {
        let repo = repo();
        let branch = repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]);
        assert_eq!(
            inspect(&repo),
            CheckoutState {
                branch: Some(branch),
                busy: None,
                blocking_files: vec![],
                staged_files: vec![],
            }
        );
    }

    #[test]
    fn listet_geanderte_und_neue_dateien_ausser_der_plugin_einstellung() {
        let repo = repo();
        repo.write("tracked.txt", "b");
        repo.write("config/app.yaml", "x");
        repo.write("neu ä.txt", "x");
        repo.write(".claude/settings.local.json", "{}");
        assert_eq!(
            inspect(&repo).blocking_files,
            ["config/app.yaml", "neu ä.txt", "tracked.txt"]
        );
    }

    #[test]
    fn lasst_den_spec_zeiger_durch_solange_er_nicht_gestaged_ist() {
        let repo = repo();
        repo.write(".specify/feature.json", "{}");
        assert!(inspect(&repo).blocking_files.is_empty());
        repo.git(&["add", ".specify/feature.json"]);
        assert_eq!(inspect(&repo).blocking_files, [".specify/feature.json"]);
    }

    #[test]
    fn erkennt_detached_head_und_laufenden_merge() {
        let repo = repo();
        repo.git(&["checkout", "-q", "--detach"]);
        assert_eq!(inspect(&repo).branch, None);
        repo.git(&["checkout", "-q", "-"]);
        let head = repo.git(&["rev-parse", "HEAD"]);
        std::fs::write(repo.path().join(".git").join("MERGE_HEAD"), head).unwrap();
        assert_eq!(
            inspect(&repo).busy.as_deref(),
            Some("Im Hauptordner läuft gerade ein Merge.")
        );
    }

    #[test]
    fn nennt_die_gestagten_dateien_gesondert() {
        let repo = repo();
        repo.write("tracked.txt", "b");
        repo.write("gestagt.txt", "x");
        repo.git(&["add", "gestagt.txt"]);
        let state = inspect(&repo);
        assert_eq!(state.blocking_files, ["gestagt.txt", "tracked.txt"]);
        assert_eq!(state.staged_files, ["gestagt.txt"]);
    }

    #[test]
    fn nennt_bei_umbenennung_den_neuen_pfad() {
        let repo = repo();
        repo.commit("alt.yaml", "inhalt");
        repo.git(&["mv", "alt.yaml", "neu.yaml"]);
        assert_eq!(inspect(&repo).blocking_files, ["neu.yaml"]);
    }

    #[test]
    fn lasst_die_plugin_einstellung_immer_durch_auch_gestagt() {
        let repo = repo();
        repo.write(".claude/settings.local.json", "{}");
        repo.git(&["add", "-f", ".claude/settings.local.json"]);
        assert!(inspect(&repo).blocking_files.is_empty());
    }

    #[test]
    fn lasst_die_rules_fur_doku_und_board_regeln_durch() {
        let repo = repo();
        repo.write(".claude/rules/ai-harness-docs.md", "x");
        repo.write(".claude/rules/ai-harness-board.md", "x");
        assert!(inspect(&repo).blocking_files.is_empty());
    }

    #[test]
    fn nennt_alle_geanderten_dateien_ohne_ausnahmen() {
        let repo = repo();
        repo.write("tracked.txt", "b");
        repo.write(".claude/settings.local.json", "{}");
        let mut changed = MainCheckout::new().changed(repo.path()).unwrap();
        changed.sort();
        assert_eq!(changed, [".claude/settings.local.json", "tracked.txt"]);
    }
}
