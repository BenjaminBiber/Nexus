//! Portiert `packages/core/src/dispatch/git/TargetBranches.ts`.

use std::path::{Path, PathBuf};

use crate::error::{HarnessError, Result};
use crate::git::{GitCommands, GitRunner, PathKey, LONG_PATHS_ARGS};

/// Ergebnis von [`TargetBranches::ensure`]: `from` nennt die Herkunft eines neu
/// angelegten Branches (sonst `None`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsuredBranch {
    pub created: bool,
    pub from: Option<String>,
}

/// Git-Helfer für Zielbranches: lokal anlegen, Ort des Checkouts finden und der
/// Merge-Worktree, in dem ein Ticket in seinen Zielbranch gemergt wird, ohne den
/// Hauptordner des Nutzers anzufassen. Kein Push, kein Fetch.
#[derive(Debug, Clone, Copy, Default)]
pub struct TargetBranches {
    git: GitRunner,
    keys: PathKey,
}

impl TargetBranches {
    /// Neuer Helfer (hält keinen Zustand)
    pub fn new() -> Self {
        Self {
            git: GitRunner::new(),
            keys: PathKey::new(),
        }
    }

    /// Legt den Branch lokal an, falls er fehlt: von origin/<name>, sonst vom HEAD des
    /// Hauptordners (immer ohne Upstream)
    pub fn ensure(&self, root: &Path, name: &str) -> Result<EnsuredBranch> {
        if self.exists(root, &format!("refs/heads/{name}")) {
            return Ok(EnsuredBranch {
                created: false,
                from: None,
            });
        }
        let remote = format!("refs/remotes/origin/{name}");
        if self.exists(root, &remote) {
            self.git
                .run(root, &["branch", "--no-track", name, &remote])?;
            return Ok(EnsuredBranch {
                created: true,
                from: Some(format!("origin/{name}")),
            });
        }
        let current = self.git.run(root, &["branch", "--show-current"])?;
        let from = if current.is_empty() {
            self.git.run(root, &["rev-parse", "--short", "HEAD"])?
        } else {
            current
        };
        self.git
            .run(root, &["branch", "--no-track", name, "HEAD"])?;
        Ok(EnsuredBranch {
            created: true,
            from: Some(from),
        })
    }

    /// Ordner, in dem der Branch ausgecheckt ist (Hauptordner oder Worktree), so wie Git
    /// ihn nennt – Vergleiche über [`PathKey`] –, sonst `None`
    pub fn checked_out_at(&self, root: &Path, name: &str) -> Result<Option<String>> {
        let listed = self.git.run(root, &["worktree", "list", "--porcelain"])?;
        let wanted = format!("branch refs/heads/{name}");
        let mut current: Option<&str> = None;
        for line in listed.split('\n') {
            let line = line.trim_end_matches('\r');
            if let Some(path) = line.strip_prefix("worktree ") {
                current = Some(path);
            } else if line == wanted {
                if let Some(path) = current {
                    return Ok(Some(path.to_string()));
                }
            } else if line.is_empty() {
                current = None;
            }
        }
        Ok(None)
    }

    /// Ordner des Merge-Worktrees eines Zielbranches:
    /// <repo>-worktrees/_merge/<Branch, „/“ als „-“>
    pub fn merge_worktree_path(&self, root: &Path, name: &str) -> String {
        let parent = root.parent().unwrap_or_else(|| Path::new(""));
        let base = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let path: PathBuf = parent
            .join(format!("{base}-worktrees"))
            .join("_merge")
            .join(name.replace('/', "-"));
        path.to_string_lossy().replace('\\', "/")
    }

    /// Öffnet den Merge-Worktree des Zielbranches: ein vorhandener Worktree dieses
    /// Branches wird wiederverwendet, ein anderer Ordner am selben Platz abgelehnt, sonst
    /// (nach `worktree prune`) neu angelegt
    pub fn open_merge_worktree(&self, root: &Path, name: &str) -> Result<String> {
        let path = self.merge_worktree_path(root, name);
        if Path::new(&path).exists() {
            if self.is_worktree_of(root, &path, name)? {
                return Ok(path);
            }
            return Err(HarnessError::new(
                "merge-worktree-foreign",
                format!(
                    "Der Ordner {path} existiert schon und ist kein Worktree von Branch {name}. \
Bitte den Ordner prüfen und von Hand aufräumen."
                ),
            )
            .into());
        }
        self.git.run(root, &["worktree", "prune"])?;
        let mut args: Vec<&str> = LONG_PATHS_ARGS.to_vec();
        args.extend(["worktree", "add", &path, name]);
        self.git.run(root, &args)?;
        Ok(path)
    }

    /// Entfernt den Merge-Worktree ohne Gewalt (ungesicherte Änderungen bleiben stehen);
    /// liefert die Git-Meldung bei Fehlschlag, sonst `None`
    pub fn close_merge_worktree(&self, root: &Path, path: &str) -> Option<String> {
        let attempt = self.git.try_run(root, &["worktree", "remove", path]);
        if attempt.ok {
            None
        } else {
            Some(attempt.err)
        }
    }

    /// Gibt es die Referenz?
    fn exists(&self, root: &Path, reference: &str) -> bool {
        self.git
            .try_run(root, &["rev-parse", "--verify", "--quiet", reference])
            .ok
    }

    /// Ist der Ordner in `git worktree list` als Worktree des Branches eingetragen?
    fn is_worktree_of(&self, root: &Path, path: &str, name: &str) -> Result<bool> {
        Ok(match self.checked_out_at(root, name)? {
            Some(place) => self.keys.same(place, path),
            None => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::TempRepo;

    /// Ort eines Branches in vergleichbarer Schreibweise (Git liefert Schrägstriche,
    /// die Tests je nach Plattform Rückstriche)
    fn place_of(repo: &TempRepo, name: &str) -> Option<String> {
        TargetBranches::new()
            .checked_out_at(repo.path(), name)
            .unwrap()
            .map(|place| PathKey::new().of(place))
    }

    /// Schlüssel eines Pfades
    fn key(path: impl AsRef<Path>) -> String {
        PathKey::new().of(path)
    }

    #[test]
    fn legt_einen_fehlenden_zielbranch_vom_head_des_hauptordners_an() {
        let repo = TempRepo::new();
        let target = TargetBranches::new();
        let head = repo.git(&["rev-parse", "HEAD"]);

        let ensured = target.ensure(repo.path(), "release/1").unwrap();

        assert_eq!(
            ensured,
            EnsuredBranch {
                created: true,
                from: Some(repo.git(&["branch", "--show-current"])),
            }
        );
        assert_eq!(repo.git(&["rev-parse", "refs/heads/release/1"]), head);
        assert_eq!(
            target.ensure(repo.path(), "release/1").unwrap(),
            EnsuredBranch {
                created: false,
                from: None,
            }
        );
    }

    #[test]
    fn nimmt_bei_losgelostem_head_den_kurz_hash_als_herkunft() {
        let repo = TempRepo::new();
        repo.git(&["checkout", "-q", "--detach"]);

        let ensured = TargetBranches::new()
            .ensure(repo.path(), "release/1")
            .unwrap();

        assert_eq!(
            ensured,
            EnsuredBranch {
                created: true,
                from: Some(repo.git(&["rev-parse", "--short", "HEAD"])),
            }
        );
    }

    #[test]
    fn nimmt_origin_name_wenn_es_ihn_gibt() {
        let repo = TempRepo::new();
        repo.git(&["commit", "--allow-empty", "-q", "-m", "remote"]);
        let remote = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["update-ref", "refs/remotes/origin/release/2", &remote]);
        repo.git(&["reset", "-q", "--hard", "HEAD~1"]);

        let ensured = TargetBranches::new()
            .ensure(repo.path(), "release/2")
            .unwrap();

        assert_eq!(
            ensured,
            EnsuredBranch {
                created: true,
                from: Some("origin/release/2".to_string()),
            }
        );
        assert_eq!(repo.git(&["rev-parse", "refs/heads/release/2"]), remote);
    }

    #[test]
    fn setzt_beim_anlegen_aus_origin_name_keinen_upstream() {
        let repo = TempRepo::new();
        repo.git(&[
            "remote",
            "add",
            "origin",
            "https://example.invalid/repo.git",
        ]);
        let head = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["update-ref", "refs/remotes/origin/release/2", &head]);

        TargetBranches::new()
            .ensure(repo.path(), "release/2")
            .unwrap();

        let configured = GitRunner::new().try_run(
            repo.path(),
            &["config", "--get-regexp", r"^branch\.release/2\."],
        );
        assert_eq!(configured.out, "");
    }

    #[test]
    fn findet_den_ort_eines_ausgecheckten_branches() {
        let repo = TempRepo::new();
        let target = TargetBranches::new();

        let current = repo.git(&["branch", "--show-current"]);
        assert_eq!(place_of(&repo, &current), Some(key(repo.path())));

        let worktree = repo.add_worktree("w1");
        assert_eq!(place_of(&repo, "branch-w1"), Some(key(&worktree)));

        target.ensure(repo.path(), "release/3").unwrap();
        assert_eq!(
            target.checked_out_at(repo.path(), "release/3").unwrap(),
            None
        );
    }

    #[test]
    fn offnet_verwendet_wieder_und_schliesst_den_merge_worktree() {
        let repo = TempRepo::new();
        let target = TargetBranches::new();
        target.ensure(repo.path(), "release/4").unwrap();

        let path = target
            .open_merge_worktree(repo.path(), "release/4")
            .unwrap();

        assert_eq!(path, target.merge_worktree_path(repo.path(), "release/4"));
        assert!(path.ends_with("-worktrees/_merge/release-4"), "{path}");
        assert_eq!(
            target
                .open_merge_worktree(repo.path(), "release/4")
                .unwrap(),
            path
        );
        assert_eq!(place_of(&repo, "release/4"), Some(key(&path)));
        assert_eq!(target.close_merge_worktree(repo.path(), &path), None);
        assert!(!Path::new(&path).exists());
    }

    #[test]
    fn lehnt_einen_fremden_ordner_am_ort_des_merge_worktrees_ab() {
        let repo = TempRepo::new();
        let target = TargetBranches::new();
        target.ensure(repo.path(), "release/5").unwrap();
        std::fs::create_dir_all(target.merge_worktree_path(repo.path(), "release/5")).unwrap();

        let error = target
            .open_merge_worktree(repo.path(), "release/5")
            .unwrap_err();

        assert_eq!(error.code(), Some("merge-worktree-foreign"));
    }

    #[test]
    fn lehnt_einen_worktree_eines_anderen_branches_am_ort_des_merge_worktrees_ab() {
        let repo = TempRepo::new();
        let target = TargetBranches::new();
        target.ensure(repo.path(), "release/6").unwrap();
        target.ensure(repo.path(), "release/7").unwrap();
        let path = target.merge_worktree_path(repo.path(), "release/6");
        repo.git(&["worktree", "add", "-q", &path, "release/7"]);

        let error = target
            .open_merge_worktree(repo.path(), "release/6")
            .unwrap_err();

        assert_eq!(error.code(), Some("merge-worktree-foreign"));
    }

    #[test]
    fn legt_den_merge_worktree_neu_an_wenn_sein_ordner_von_hand_geloscht_wurde() {
        let repo = TempRepo::new();
        let target = TargetBranches::new();
        target.ensure(repo.path(), "release/8").unwrap();
        let path = target
            .open_merge_worktree(repo.path(), "release/8")
            .unwrap();
        std::fs::remove_dir_all(&path).unwrap();

        assert_eq!(
            target
                .open_merge_worktree(repo.path(), "release/8")
                .unwrap(),
            path
        );
        assert_eq!(place_of(&repo, "release/8"), Some(key(&path)));
    }

    #[test]
    fn meldet_wenn_der_merge_worktree_nicht_entfernt_werden_kann() {
        let repo = TempRepo::new();
        let target = TargetBranches::new();
        target.ensure(repo.path(), "release/9").unwrap();
        let path = target
            .open_merge_worktree(repo.path(), "release/9")
            .unwrap();
        // Eine unversionierte Datei hindert `worktree remove` ohne --force
        std::fs::write(Path::new(&path).join("neu.txt"), "x").unwrap();

        let message = target.close_merge_worktree(repo.path(), &path).unwrap();

        assert!(message.contains("--force"), "{message}");
        assert!(Path::new(&path).exists());
    }
}
