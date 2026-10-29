//! Symlinks und Junctions in einem Worktree finden und nur die Links entfernen.
//! Portiert aus `packages/core/src/worktrees/WorktreeLinks.ts`.

use std::fs;
use std::path::{Path, PathBuf};

/// Ergebnis des Entfernens: entfernte und stehen gebliebene Links
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkRemoval {
    /// Entfernte Links (nur der Link selbst, nie sein Ziel)
    pub removed: Vec<PathBuf>,
    /// Links, die sich nicht entfernen ließen – dann darf der Ordner nicht gelöscht werden
    pub failed: Vec<PathBuf>,
}

/// Findet Symlinks und Junctions (Windows-Reparse-Points) in einem Worktree, ohne ihnen zu
/// folgen, und entfernt nur die Links selbst. Muss vor jedem `git worktree remove` laufen:
/// Git löscht sonst womöglich durch eine Junction hindurch den Inhalt des Ziels
/// (z. B. node_modules des Hauptordners).
#[derive(Debug, Clone, Copy, Default)]
pub struct WorktreeLinks;

impl WorktreeLinks {
    /// Neue Hilfe (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Alle Links unterhalb von `root` (rekursiv, aber nie in einen Link hinein); `root`
    /// selbst als Link zählt mit. Nicht lesbare Ordner und Einträge kommen nach
    /// `unreadable`: darin könnte ein Link stecken – der Aufrufer darf dann nicht löschen
    pub fn find(&self, root: &Path, unreadable: &mut Vec<PathBuf>) -> Vec<PathBuf> {
        if is_link(root) {
            return vec![root.to_path_buf()];
        }
        let mut found = Vec::new();
        visit(root, &mut found, unreadable);
        found
    }

    /// Entfernt alle Links unterhalb von `root` (nie rekursiv, das Ziel bleibt); `root`
    /// selbst als Link wird nie angefasst
    pub fn remove_all(&self, root: &Path) -> LinkRemoval {
        if is_link(root) {
            return LinkRemoval {
                removed: Vec::new(),
                failed: vec![root.to_path_buf()],
            };
        }
        let mut result = LinkRemoval::default();
        let mut unreadable = Vec::new();
        for link in self.find(root, &mut unreadable) {
            if remove_link(&link) {
                result.removed.push(link);
            } else {
                result.failed.push(link);
            }
        }
        // Nicht durchsuchbare Ordner zählen als gescheitert, damit der Worktree stehen bleibt
        result.failed.extend(unreadable);
        result
    }
}

/// Durchsucht einen Ordner und steigt nur in echte Ordner ab
fn visit(dir: &Path, found: &mut Vec<PathBuf>, unreadable: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        unreadable.push(dir.to_path_buf());
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            unreadable.push(dir.to_path_buf());
            continue;
        };
        let path = entry.path();
        let Ok(stat) = fs::symlink_metadata(&path) else {
            unreadable.push(path);
            continue;
        };
        if stat.file_type().is_symlink() {
            found.push(path);
        } else if stat.is_dir() {
            visit(&path, found, unreadable);
        }
    }
}

/// Entfernt genau den Link: Datei-Symlink per `remove_file`, Junction/Verzeichnis-Link per
/// `remove_dir` (nie rekursiv)
fn remove_link(path: &Path) -> bool {
    if fs::remove_file(path).is_err() && fs::remove_dir(path).is_err() {
        return false;
    }
    fs::symlink_metadata(path).is_err()
}

/// Ist der Pfad selbst ein Link (Symlink oder Junction)?
fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|stat| stat.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::make_temp_dir;

    /// Legt einen Link auf einen Ordner an (unter Windows als Junction wie der Agent selbst)
    fn link_dir(target: &Path, link: &Path) {
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(target, link).unwrap();
        #[cfg(not(windows))]
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    /// Ziel außerhalb des Worktrees mit Inhalt (wie node_modules des Hauptordners)
    fn target(dir: &Path) -> PathBuf {
        let t = dir.join("haupt").join("node_modules");
        fs::create_dir_all(t.join(".bin")).unwrap();
        fs::write(t.join(".package-lock.json"), "{}").unwrap();
        fs::write(t.join(".bin").join("tool"), "x").unwrap();
        t
    }

    #[test]
    fn findet_junctions_ohne_zu_folgen_und_entfernt_nur_den_link() {
        let dir = make_temp_dir("harness-links-");
        let t = target(dir.path());
        let wt = dir.path().join("wt");
        fs::create_dir_all(wt.join("app")).unwrap();
        fs::write(wt.join("app").join("a.txt"), "a").unwrap();
        let link = wt.join("app").join("node_modules");
        link_dir(&t, &link);
        let links = WorktreeLinks::new();

        assert_eq!(links.find(&wt, &mut Vec::new()), std::slice::from_ref(&link));
        assert_eq!(
            links.remove_all(&wt),
            LinkRemoval {
                removed: vec![link.clone()],
                failed: Vec::new(),
            }
        );
        assert!(!link.exists());
        assert_eq!(
            fs::read_to_string(t.join(".package-lock.json")).unwrap(),
            "{}"
        );
        assert_eq!(fs::read_to_string(t.join(".bin").join("tool")).unwrap(), "x");
        assert_eq!(
            fs::read_to_string(wt.join("app").join("a.txt")).unwrap(),
            "a"
        );
    }

    #[test]
    fn fasst_einen_ordner_der_selbst_ein_link_ist_nicht_an() {
        let dir = make_temp_dir("harness-links-");
        let t = target(dir.path());
        let wt = dir.path().join("wt-link");
        link_dir(&t, &wt);

        assert_eq!(
            WorktreeLinks::new().remove_all(&wt),
            LinkRemoval {
                removed: Vec::new(),
                failed: vec![wt.clone()],
            }
        );
        assert!(t.join(".package-lock.json").exists());
    }
}
