//! Portiert `packages/core/src/git/BranchCommitReader.ts`.

use std::path::Path;

use chrono::{DateTime, SecondsFormat};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::git::{GitCommands, GitRunner};

/// Höchstzahl der Commits, wenn keine andere genannt wird
const DEFAULT_LIMIT: usize = 50;

/// Trennzeichen zwischen den Feldern einer Zeile von `git log`
const SEP: char = '\u{1f}';

/// Ein Commit eines Ticket-Branches
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "git.ts")]
pub struct BranchCommit {
    pub hash: String,
    pub author: String,
    /// Autorzeit als ISO (UTC)
    pub at: String,
    pub subject: String,
}

/// Zustand der Abfrage: `unavailable` = kein Repo, Branch und Merge-Commit nicht (mehr)
/// da; `error` = Git scheiterte unerwartet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "git.ts")]
pub enum BranchCommitsStatus {
    Ok,
    Unavailable,
    Error,
}

/// Ergebnis von [`BranchCommitReader::read`]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "git.ts")]
pub struct BranchCommits {
    pub status: BranchCommitsStatus,
    pub commits: Vec<BranchCommit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Woher die Commits kommen: Branch, Merge-Commit und Zielbranch des Tickets
#[derive(Debug, Clone, Default)]
pub struct BranchCommitSource<'a> {
    pub branch: Option<&'a str>,
    pub merge_commit: Option<&'a str>,
    pub target_branch: Option<&'a str>,
}

impl<'a> BranchCommitSource<'a> {
    /// Quelle nur mit Branch
    pub fn branch(branch: &'a str) -> Self {
        Self {
            branch: Some(branch),
            ..Self::default()
        }
    }

    /// Quelle nur mit Merge-Commit
    pub fn merge_commit(hash: &'a str) -> Self {
        Self {
            merge_commit: Some(hash),
            ..Self::default()
        }
    }

    /// Setzt den Zielbranch des Tickets
    pub fn with_target(mut self, target: Option<&'a str>) -> Self {
        self.target_branch = target;
        self
    }
}

/// Liest die Commits eines Ticket-Branches: vor dem Merge Zielbranch..Branch (ohne
/// Zielbranch HEAD..Branch), nach dem Merge über den Merge-Commit (Merge-Commit: zweiter
/// Elternteil ohne ersten; Squash: nur der Commit selbst). Liefert nie einen Fehler.
#[derive(Debug, Clone, Copy)]
pub struct BranchCommitReader {
    limit: usize,
    git: GitRunner,
}

impl Default for BranchCommitReader {
    fn default() -> Self {
        Self::new()
    }
}

impl BranchCommitReader {
    /// Leser mit der Standardgrenze
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_LIMIT)
    }

    /// Leser mit eigener Höchstzahl
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            git: GitRunner::new(),
        }
    }

    /// Commits der Quelle, neueste zuerst
    pub fn read(&self, repo_path: Option<&Path>, source: &BranchCommitSource<'_>) -> BranchCommits {
        let Some(repo_path) = repo_path.filter(|path| path.exists()) else {
            return unavailable("Kein Repo-Pfad auf diesem Rechner.");
        };
        if !self.git.try_run(repo_path, &["rev-parse", "--git-dir"]).ok {
            return unavailable("Der Repo-Pfad ist kein Git-Repo.");
        }
        let Some(range) = self.range(repo_path, source) else {
            return unavailable(
                "Branch und Merge-Commit sind in diesem Repo nicht (mehr) vorhanden.",
            );
        };
        let limit = format!("-n{}", self.limit);
        let format = format!("--format=%H{SEP}%an{SEP}%aI{SEP}%s");
        let mut args = vec!["log", "--no-merges", &limit, &format];
        args.extend(range.iter().map(String::as_str));
        args.push("--");
        let log = self.git.try_run(repo_path, &args);
        if !log.ok {
            return BranchCommits {
                status: BranchCommitsStatus::Error,
                commits: Vec::new(),
                message: Some(log.err),
            };
        }
        let commits = if log.out.is_empty() {
            Vec::new()
        } else {
            log.out.split('\n').map(commit).collect()
        };
        BranchCommits {
            status: BranchCommitsStatus::Ok,
            commits,
            message: None,
        }
    }

    /// Bereich für `git log`; `None`, wenn weder Merge-Commit noch Branch auflösbar ist
    fn range(&self, repo_path: &Path, source: &BranchCommitSource<'_>) -> Option<Vec<String>> {
        let merge = source
            .merge_commit
            .filter(|hash| is_hash(hash))
            .and_then(|hash| self.resolve(repo_path, &format!("{hash}^{{commit}}")));
        if let Some(merge) = merge {
            let listed = self
                .git
                .try_run(repo_path, &["rev-list", "--parents", "-n1", &merge]);
            let parents = listed.out.split(' ').count() - 1;
            return Some(if parents >= 2 {
                vec![format!("{merge}^1..{merge}^2")]
            } else {
                vec!["-n1".to_string(), merge]
            });
        }
        let name = source.branch.filter(|name| !name.starts_with('-'))?;
        let branch = self.resolve(repo_path, &format!("refs/heads/{name}"))?;
        Some(vec![format!(
            "{}..{branch}",
            self.base(repo_path, source.target_branch)
        )])
    }

    /// Beginn des Bereichs: lokaler Zielbranch des Tickets, wenn auflösbar, sonst HEAD
    fn base(&self, repo_path: &Path, target: Option<&str>) -> String {
        match target.filter(|name| !name.is_empty() && !name.starts_with('-')) {
            Some(name) => self
                .resolve(repo_path, &format!("refs/heads/{name}"))
                .unwrap_or_else(|| "HEAD".to_string()),
            None => "HEAD".to_string(),
        }
    }

    /// Volle Commit-ID oder `None`
    fn resolve(&self, repo_path: &Path, reference: &str) -> Option<String> {
        let found = self
            .git
            .try_run(repo_path, &["rev-parse", "--verify", "--quiet", reference]);
        (found.ok && !found.out.is_empty()).then_some(found.out)
    }
}

/// Eine Zeile von `git log` als Commit
fn commit(line: &str) -> BranchCommit {
    let mut fields = line.split(SEP);
    let hash = fields.next().unwrap_or_default().to_string();
    let author = fields.next().unwrap_or_default().to_string();
    let at = fields.next().unwrap_or_default().to_string();
    // Der Betreff darf das Trennzeichen enthalten: alles Übrige gehört dazu
    let subject = fields.collect::<Vec<_>>().join(&SEP.to_string());
    let at = match DateTime::parse_from_rfc3339(&at) {
        Ok(date) => date.to_utc().to_rfc3339_opts(SecondsFormat::Millis, true),
        Err(_) => at,
    };
    BranchCommit {
        hash,
        author,
        at,
        subject,
    }
}

/// Sieht der Wert wie eine (gekürzte) Commit-ID aus?
fn is_hash(value: &str) -> bool {
    (4..=40).contains(&value.len()) && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Ergebnis „nicht vorhanden“ mit Begründung
fn unavailable(message: &str) -> BranchCommits {
    BranchCommits {
        status: BranchCommitsStatus::Unavailable,
        commits: Vec::new(),
        message: Some(message.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::TempRepo;

    /// Projekt mit main und einem Ticket-Branch mit zwei Commits; ausgecheckt bleibt main
    fn repo() -> TempRepo {
        let repo = TempRepo::new();
        repo.git(&["branch", "-M", "main"]);
        repo.git(&["checkout", "-q", "-b", "ticket/ARC-0001-x"]);
        for name in ["a", "b"] {
            commit(&repo, name);
        }
        repo.git(&["checkout", "-q", "main"]);
        repo
    }

    /// Commit mit dem Betreff „Commit <name>“ und der Datei <name>.txt
    fn commit(repo: &TempRepo, name: &str) {
        repo.write(&format!("{name}.txt"), name);
        repo.git(&["add", "."]);
        repo.git(&["commit", "-q", "-m", &format!("Commit {name}")]);
    }

    /// Betreffzeilen der gelesenen Commits
    fn subjects(result: &BranchCommits) -> Vec<&str> {
        result
            .commits
            .iter()
            .map(|commit| commit.subject.as_str())
            .collect()
    }

    /// Zweites Projekt: release/1 von main mit Commit r, ticket/ARC-0002-y von release/1
    /// mit Commit t; ausgecheckt bleibt main
    fn repo_mit_zielbranch() -> TempRepo {
        let repo = repo();
        repo.git(&["checkout", "-q", "-b", "release/1"]);
        commit(&repo, "r");
        repo.git(&["checkout", "-q", "-b", "ticket/ARC-0002-y"]);
        commit(&repo, "t");
        repo.git(&["checkout", "-q", "main"]);
        repo
    }

    #[test]
    fn liest_die_commits_des_branches_vor_dem_merge_neueste_zuerst() {
        let repo = repo();

        let result = BranchCommitReader::new().read(
            Some(repo.path()),
            &BranchCommitSource::branch("ticket/ARC-0001-x"),
        );

        assert_eq!(result.status, BranchCommitsStatus::Ok);
        assert_eq!(subjects(&result), ["Commit b", "Commit a"]);
        assert_eq!(result.commits[0].author, "t");
        assert_eq!(result.commits[0].hash.len(), 40);
        assert!(
            result.commits[0].at.ends_with('Z'),
            "{:?}",
            result.commits[0]
        );
    }

    #[test]
    fn liest_die_commits_nach_einem_merge_commit_uber_dessen_eltern() {
        let repo = repo();
        repo.git(&["merge", "--no-ff", "-q", "-m", "Merge", "ticket/ARC-0001-x"]);
        let hash = repo.git(&["rev-parse", "--short", "HEAD"]);
        repo.git(&["branch", "-D", "ticket/ARC-0001-x"]);

        let result = BranchCommitReader::new().read(
            Some(repo.path()),
            &BranchCommitSource {
                branch: Some("ticket/ARC-0001-x"),
                merge_commit: Some(&hash),
                target_branch: None,
            },
        );

        assert_eq!(result.status, BranchCommitsStatus::Ok);
        assert_eq!(subjects(&result), ["Commit b", "Commit a"]);
    }

    #[test]
    fn liefert_bei_squash_nur_den_squash_commit() {
        let repo = repo();
        repo.git(&["merge", "--squash", "-q", "ticket/ARC-0001-x"]);
        repo.git(&["commit", "-q", "-m", "Squash ARC-0001"]);
        let hash = repo.git(&["rev-parse", "HEAD"]);

        let result = BranchCommitReader::new()
            .read(Some(repo.path()), &BranchCommitSource::merge_commit(&hash));

        assert_eq!(subjects(&result), ["Squash ARC-0001"]);
    }

    #[test]
    fn meldet_fehlenden_branch_fehlendes_repo_und_ungultige_angaben_als_unavailable() {
        let repo = repo();
        let reader = BranchCommitReader::new();
        let fehlt = repo.base().join("fehlt");

        for (path, source) in [
            (
                Some(repo.path()),
                BranchCommitSource::branch("gibt-es-nicht"),
            ),
            (Some(repo.path()), BranchCommitSource::default()),
            (
                Some(fehlt.as_path()),
                BranchCommitSource::branch("ticket/ARC-0001-x"),
            ),
            (None, BranchCommitSource::branch("ticket/ARC-0001-x")),
            (
                Some(repo.path()),
                BranchCommitSource {
                    branch: Some("--output=x"),
                    merge_commit: Some("--all"),
                    target_branch: None,
                },
            ),
        ] {
            let result = reader.read(path, &source);
            assert_eq!(
                result.status,
                BranchCommitsStatus::Unavailable,
                "{source:?} in {path:?}"
            );
        }
    }

    #[test]
    fn begrenzt_auf_die_angegebene_zahl() {
        let repo = repo();

        let result = BranchCommitReader::with_limit(1).read(
            Some(repo.path()),
            &BranchCommitSource::branch("ticket/ARC-0001-x"),
        );

        assert_eq!(result.commits.len(), 1);
    }

    #[test]
    fn liest_vor_dem_merge_nur_die_commits_seit_dem_zielbranch() {
        let repo = repo_mit_zielbranch();

        let result = BranchCommitReader::new().read(
            Some(repo.path()),
            &BranchCommitSource::branch("ticket/ARC-0002-y").with_target(Some("release/1")),
        );

        assert_eq!(result.status, BranchCommitsStatus::Ok);
        assert_eq!(subjects(&result), ["Commit t"]);
    }

    #[test]
    fn zahlt_ohne_zielbranch_ab_dem_ausgecheckten_branch() {
        let repo = repo_mit_zielbranch();

        let result = BranchCommitReader::new().read(
            Some(repo.path()),
            &BranchCommitSource::branch("ticket/ARC-0002-y"),
        );

        assert_eq!(subjects(&result), ["Commit t", "Commit r"]);
    }

    #[test]
    fn fallt_bei_nicht_vorhandenem_oder_ungultigem_zielbranch_auf_head_zuruck() {
        let repo = repo_mit_zielbranch();

        for target in ["release/2", "--output=x"] {
            let result = BranchCommitReader::new().read(
                Some(repo.path()),
                &BranchCommitSource::branch("ticket/ARC-0002-y").with_target(Some(target)),
            );

            assert_eq!(result.status, BranchCommitsStatus::Ok, "{target}");
            assert_eq!(subjects(&result), ["Commit t", "Commit r"], "{target}");
        }
    }

    #[test]
    fn liest_nach_dem_merge_weiter_uber_den_merge_commit_auch_mit_zielbranch() {
        let repo = repo_mit_zielbranch();
        repo.git(&["checkout", "-q", "release/1"]);
        repo.git(&["merge", "--no-ff", "-q", "-m", "Merge", "ticket/ARC-0002-y"]);
        let hash = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "-q", "main"]);
        repo.git(&["branch", "-D", "ticket/ARC-0002-y"]);

        let result = BranchCommitReader::new().read(
            Some(repo.path()),
            &BranchCommitSource {
                branch: Some("ticket/ARC-0002-y"),
                merge_commit: Some(&hash),
                target_branch: Some("release/1"),
            },
        );

        assert_eq!(subjects(&result), ["Commit t"]);
    }
}
