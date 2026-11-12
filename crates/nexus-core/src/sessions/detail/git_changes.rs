//! Änderungen einer Session aus Git, wenn sie in einem eigenen Worktree oder auf einem
//! eigenen Branch arbeitet: Diff des Arbeitsverzeichnisses gegen die Merge-Basis mit dem
//! Basis-Branch (`main`, sonst Standard-Branch des Remotes), samt neuer, nicht
//! versionierter Dateien.

use std::path::{Path, PathBuf};

use crate::dispatch::git::review_diff::{name_status, numstat};
use crate::error::{HarnessError, Result};
use crate::git::{GitCommands, GitRunner};

use super::types::{ChangedFile, FileDiff, FileStatus};
use super::unified_diff;

/// Wo und wogegen verglichen wird
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitScope {
    /// Wurzel des Worktrees
    pub root: PathBuf,
    /// Name des Basis-Branches (bzw. `HEAD`, wenn ein Worktree keine Basis kennt)
    pub base: String,
    /// Merge-Basis von `HEAD` und Basis
    pub merge_base: String,
}

/// Erkennung und Diff über Git
#[derive(Debug, Clone, Default)]
pub struct GitChanges<G: GitCommands = GitRunner> {
    git: G,
}

impl GitChanges<GitRunner> {
    /// Mit dem Git des Rechners
    pub fn new() -> Self {
        Self { git: GitRunner }
    }
}

impl<G: GitCommands> GitChanges<G> {
    /// Mit eigenem Git-Aufrufer
    pub fn with_git(git: G) -> Self {
        Self { git }
    }

    /// Vergleichsrahmen, wenn `cwd` in einem verknüpften Worktree oder auf einem anderen
    /// Branch als dem Basis-Branch liegt; sonst (oder ohne Git) nichts
    pub fn scope(&self, cwd: &Path) -> Option<GitScope> {
        if !cwd.is_dir() {
            return None;
        }
        let paths = self.git.try_run(
            cwd,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
                "--git-dir",
                "--git-common-dir",
            ],
        );
        if !paths.ok {
            return None;
        }
        let lines: Vec<&str> = paths.out.lines().map(str::trim).collect();
        let [root, git_dir, common_dir] = lines.as_slice() else {
            return None;
        };
        let linked = normalize(git_dir) != normalize(common_dir);
        let root = PathBuf::from(root);
        let branch = self
            .git
            .try_run(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
        let branch = branch.ok.then_some(branch.out);
        let base = self.base_branch(&root);
        let own_branch = match (&branch, &base) {
            (Some(branch), Some(base)) => {
                branch != base && Some(branch.as_str()) != base.strip_prefix("origin/")
            }
            _ => false,
        };
        if !linked && !own_branch {
            return None;
        }
        let base = base.unwrap_or_else(|| "HEAD".to_string());
        let merge_base = self.git.try_run(&root, &["merge-base", "HEAD", &base]);
        if !merge_base.ok || merge_base.out.is_empty() {
            return None;
        }
        Some(GitScope {
            root,
            base,
            merge_base: merge_base.out,
        })
    }

    /// Geänderte Dateien (committet, uncommittet und neu), nach Pfad sortiert
    pub fn files(&self, scope: &GitScope) -> Result<Vec<ChangedFile>> {
        let root = &scope.root;
        let stats = numstat(
            &self
                .git
                .raw(root, &["diff", "-M", "--numstat", "-z", &scope.merge_base])?,
        );
        let mut files: Vec<ChangedFile> = name_status(&self.git.raw(
            root,
            &["diff", "-M", "--name-status", "-z", &scope.merge_base],
        )?)
        .into_iter()
        .map(|entry| {
            let stat = stats.get(&entry.path);
            ChangedFile {
                status: status_of(&entry.status),
                added: stat.map_or(0, |stat| stat.additions as i64),
                removed: stat.map_or(0, |stat| stat.deletions as i64),
                path: entry.path,
            }
        })
        .collect();
        for path in self.untracked(root)? {
            let added = std::fs::read(root.join(&path))
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .map_or(0, |text| text.lines().count() as i64);
            files.push(ChangedFile {
                path,
                status: FileStatus::Added,
                added,
                removed: 0,
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// Diff einer Datei der Liste. Absolute Pfade, `..` und Optionen → `input-invalid`,
    /// Dateien außerhalb der Liste → `change-not-found`
    pub fn file(&self, scope: &GitScope, path: &str) -> Result<FileDiff> {
        check_path(path)?;
        let root = &scope.root;
        if self.untracked(root)?.iter().any(|file| file == path) {
            let content = std::fs::read(root.join(path))
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .unwrap_or_default();
            return Ok(unified_diff::all_added(path, &content, None));
        }
        let entry = name_status(&self.git.raw(
            root,
            &["diff", "-M", "--name-status", "-z", &scope.merge_base],
        )?)
        .into_iter()
        .find(|entry| entry.path == path)
        .ok_or_else(|| not_found(path))?;
        let mut args = vec!["diff", "-M", scope.merge_base.as_str(), "--"];
        if let Some(old) = &entry.old_path {
            args.push(old);
        }
        args.push(&entry.path);
        let patch = self.git.raw(root, &args)?;
        Ok(unified_diff::parse(path, &patch))
    }

    /// Basis-Branch: `main`, sonst der Standard-Branch des Remotes
    fn base_branch(&self, root: &Path) -> Option<String> {
        if self
            .git
            .try_run(
                root,
                &["rev-parse", "--verify", "--quiet", "refs/heads/main"],
            )
            .ok
        {
            return Some("main".to_string());
        }
        let remote = self.git.try_run(
            root,
            &[
                "symbolic-ref",
                "--quiet",
                "--short",
                "refs/remotes/origin/HEAD",
            ],
        );
        (remote.ok && !remote.out.is_empty()).then_some(remote.out)
    }

    /// Neue, nicht versionierte und nicht ignorierte Dateien
    fn untracked(&self, root: &Path) -> Result<Vec<String>> {
        Ok(self
            .git
            .raw(root, &["ls-files", "--others", "--exclude-standard", "-z"])?
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect())
    }
}

/// Art der Änderung zum Buchstaben von `--name-status`
fn status_of(letter: &str) -> FileStatus {
    match letter {
        "A" | "C" => FileStatus::Added,
        "D" => FileStatus::Deleted,
        "R" => FileStatus::Renamed,
        _ => FileStatus::Modified,
    }
}

/// Vergleichbare Schreibweise eines Pfads
fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = path.trim_end_matches('/');
    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path.to_string()
    }
}

/// Nur Pfade relativ zum Repo, ohne `..` und ohne Option
pub fn check_path(path: &str) -> Result<()> {
    let normalized = path.replace('\\', "/");
    if normalized.trim().is_empty() {
        return Err(HarnessError::new("input-invalid", "Der Pfad der Datei fehlt (path).").into());
    }
    let invalid = normalized.starts_with('/')
        || normalized.starts_with('-')
        || normalized.contains(':')
        || normalized.split('/').any(|part| part == "..");
    if invalid {
        return Err(HarnessError::new(
            "input-invalid",
            format!("Der Pfad „{path}“ liegt nicht im Repository."),
        )
        .into());
    }
    Ok(())
}

/// Fehler für eine Datei, die nicht zu den Änderungen gehört
pub fn not_found(path: &str) -> crate::error::Error {
    HarnessError::new(
        "change-not-found",
        format!("Die Datei „{path}“ gehört nicht zu den Änderungen der Session."),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::detail::types::DiffLineKind;
    use crate::test_support::TempRepo;

    /// Projekt auf `main` mit zwei Dateien
    fn repo() -> TempRepo {
        let repo = TempRepo::without_commit();
        repo.write("alt.txt", "a\nb\nc\n");
        repo.write("weg.txt", "weg\n");
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "-q", "-m", "basis"]);
        repo.git(&["branch", "-M", "main"]);
        repo
    }

    fn harness_error(error: crate::error::Error) -> HarnessError {
        match error {
            crate::error::Error::Harness(error) => error,
            other => panic!("unerwarteter Fehler: {other}"),
        }
    }

    #[test]
    fn im_hauptordner_auf_main_gibt_es_keinen_git_rahmen() {
        let repo = repo();
        repo.write("alt.txt", "geändert\n");
        assert_eq!(GitChanges::new().scope(repo.path()), None);
    }

    #[test]
    fn ohne_git_gibt_es_keinen_rahmen() {
        let dir = crate::test_support::make_temp_dir("kein-git-");
        assert_eq!(GitChanges::new().scope(dir.path()), None);
        assert_eq!(GitChanges::new().scope(&dir.path().join("fehlt")), None);
    }

    #[test]
    fn ein_worktree_vergleicht_mit_der_merge_basis_samt_uncommitteten_und_neuen_dateien() {
        let repo = repo();
        let worktree = repo.add_worktree("wt-1");
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(&worktree)
                .output()
                .unwrap();
            assert!(status.status.success(), "{:?}", status);
        };
        std::fs::write(worktree.join("alt.txt"), "a\nB\nc\n").unwrap();
        git(&["commit", "-q", "-am", "ticket"]);
        std::fs::remove_file(worktree.join("weg.txt")).unwrap();
        std::fs::write(worktree.join("neu.txt"), "eins\nzwei\n").unwrap();
        // Weiterer Commit auf main zählt nicht
        repo.commit("nur-main.txt", "x");
        let changes = GitChanges::new();
        let scope = changes.scope(&worktree).expect("Worktree wird erkannt");
        assert_eq!(scope.base, "main");
        let files = changes.files(&scope).unwrap();
        assert_eq!(
            files,
            vec![
                ChangedFile {
                    path: "alt.txt".into(),
                    status: FileStatus::Modified,
                    added: 1,
                    removed: 1
                },
                ChangedFile {
                    path: "neu.txt".into(),
                    status: FileStatus::Added,
                    added: 2,
                    removed: 0
                },
                ChangedFile {
                    path: "weg.txt".into(),
                    status: FileStatus::Deleted,
                    added: 0,
                    removed: 1
                },
            ]
        );
        let diff = changes.file(&scope, "alt.txt").unwrap();
        assert_eq!(diff.hunks.len(), 1);
        let kinds: Vec<DiffLineKind> = diff.hunks[0].lines.iter().map(|line| line.kind).collect();
        assert_eq!(
            kinds,
            vec![
                DiffLineKind::Context,
                DiffLineKind::Remove,
                DiffLineKind::Add,
                DiffLineKind::Context
            ]
        );
        let neu = changes.file(&scope, "neu.txt").unwrap();
        assert_eq!(neu.hunks[0].header, "@@ -0,0 +1,2 @@");
    }

    #[test]
    fn ein_eigener_branch_im_hauptordner_zaehlt_auch() {
        let repo = repo();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("t.txt", "t");
        let changes = GitChanges::new();
        let scope = changes.scope(repo.path()).expect("Branch wird erkannt");
        let files = changes.files(&scope).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "t.txt");
        assert_eq!(files[0].status, FileStatus::Added);
    }

    #[test]
    fn lehnt_pfade_ausserhalb_und_unbekannte_dateien_ab() {
        let repo = repo();
        let worktree = repo.add_worktree("wt-2");
        let changes = GitChanges::new();
        let scope = changes.scope(&worktree).unwrap();
        for path in ["../x", "/etc/passwd", "C:/x", "-p", "a/../../b", ""] {
            let error = harness_error(changes.file(&scope, path).unwrap_err());
            assert_eq!(error.code, "input-invalid", "{path}");
        }
        let error = harness_error(changes.file(&scope, "alt.txt").unwrap_err());
        assert_eq!(error.code, "change-not-found");
    }

    #[test]
    fn meldet_einen_fehlenden_pfad_als_fehlend() {
        let error = harness_error(check_path("  ").unwrap_err());
        assert_eq!(error.code, "input-invalid");
        assert_eq!(error.message, "Der Pfad der Datei fehlt (path).");
    }
}
