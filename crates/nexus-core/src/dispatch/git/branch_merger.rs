//! Portiert `packages/core/src/dispatch/git/BranchMerger.ts`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::dispatch::git::main_checkout::MainCheckout;
use crate::dispatch::merge_strategy::MergeStrategy;
use crate::error::{HarnessError, Result};
use crate::git::{GitAttempt, GitCommands, GitRunner};

/// Trailer eines Squash-Commits: `Harness-Squashed: <branch> <volle Spitze des Branches>`
pub const SQUASH_TRAILER: &str = "Harness-Squashed";

/// Höchstens so viele Commits listet ein Squash-Commit auf
const MAX_SQUASH_LIST: usize = 50;

/// Ergebnis eines Merge-Versuchs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Gemergt; die Kurzkennung des neuen HEAD
    Merged { commit: String },
    /// Konflikt; die betroffenen Dateien (der eigene Versuch ist zurückgesetzt)
    Conflict { files: Vec<String> },
    /// Nicht gemergt, ohne etwas zu verändern
    Refused { message: String },
}

/// Mergt einen Ticket-Branch in den ausgecheckten Branch des Hauptordners; bei Problemen
/// bleibt der Ordner wie vorher. Fremde Git-Zustände (laufender Merge, Rebase,
/// Cherry-Pick, Revert) werden nie angefasst.
#[derive(Debug, Clone, Copy, Default)]
pub struct BranchMerger<G: GitCommands = GitRunner> {
    git: G,
    checkout: MainCheckout<G>,
}

impl BranchMerger<GitRunner> {
    /// Merger mit dem gewöhnlichen Git-Aufrufer
    pub fn new() -> Self {
        Self::with_git(GitRunner::new())
    }
}

impl<G: GitCommands + Clone> BranchMerger<G> {
    /// Merger mit eigenem Git-Aufrufer
    pub fn with_git(git: G) -> Self {
        Self {
            checkout: MainCheckout::with_git(git.clone()),
            git,
        }
    }

    /// Strategie „merge“: `git merge --no-ff --no-edit [-m <message>] refs/heads/<branch>`
    /// (ohne Nachricht die Git-Standardnachricht). Strategie „squash“:
    /// `git merge --squash refs/heads/<branch>` und ein Commit mit der Nachricht, der
    /// Liste der zusammengefassten Commits und dem Trailer
    /// `Harness-Squashed: <branch> <Spitze>` (auch leer, damit `integrated` den Branch
    /// erkennt). Ist der Branch schon enthalten oder schon zusammengefasst → kein neuer
    /// Commit. Läuft schon eine Git-Aktion → Abweisung ohne Eingriff, Konflikt → Dateien
    /// und abort (nur für den eigenen Merge), sonstige Abweisung → Git-Meldung
    pub fn merge(
        &self,
        root: &Path,
        branch: &str,
        message: Option<&str>,
        strategy: MergeStrategy,
    ) -> Result<MergeOutcome> {
        if let Some(running) = self.checkout.running(root)? {
            return Ok(MergeOutcome::Refused {
                message: format!("Im Hauptordner läuft bereits {running}; es wird nicht gemergt."),
            });
        }
        if self.squashed_into(root, branch, "HEAD") {
            return self.merged(root);
        }
        if strategy == MergeStrategy::Squash {
            return self.squash(root, branch, message);
        }
        let reference = format!("refs/heads/{branch}");
        let mut args = vec![
            "-c",
            "core.quotePath=false",
            "merge",
            "--no-ff",
            "--no-edit",
        ];
        if let Some(message) = message {
            args.extend(["-m", message]);
        }
        args.push(&reference);
        let attempt = self.git.try_run(root, &args);
        if attempt.ok {
            return self.merged(root);
        }
        // Vorher lief keine Aktion: ein MERGE_HEAD jetzt stammt von diesem Aufruf
        let started = self
            .git
            .try_run(root, &["rev-parse", "--verify", "--quiet", "MERGE_HEAD"])
            .ok;
        self.after_failure(root, &attempt, started, MergeStrategy::Merge)
    }

    /// Ist der Branch im Ziel angekommen – als Vorfahre (Merge) oder per Squash-Commit mit
    /// Trailer für seine aktuelle Spitze? Ein Branch, den es nicht gibt, gilt als nicht
    /// angekommen.
    pub fn integrated(&self, root: &Path, branch: &str, target: &str) -> bool {
        if !self.branch_exists(root, branch) {
            return false;
        }
        self.contains(root, &format!("refs/heads/{branch}"), target)
            || self.squashed_into(root, branch, target)
    }

    /// Den Branch gibt es nicht mehr, aber `target` enthält schon seinen Merge – z. B.
    /// nach einem Abbruch zwischen Merge und Aufräumen. Als Merge zählt nur ein
    /// Merge-Commit (zwei Eltern) mit Betreff „… (<branch>)“ oder ein Commit mit dem
    /// Squash-Trailer für den Branch; ein Revert oder ein Commit, der den Branch nur
    /// erwähnt, zählt nicht. Liefert die Kurzkennung dieses Commits, sonst `None`
    pub fn merged_earlier(&self, root: &Path, branch: &str, target: &str) -> Option<String> {
        if self.branch_exists(root, branch) {
            return None;
        }
        let patterns = [
            format!("--grep=({branch})"),
            format!("--grep={SQUASH_TRAILER}: {branch} "),
        ];
        // Je Kandidat: Kurzkennung, Eltern, Betreff und die Werte des Squash-Trailers
        // (Felder mit \x1f, Commits mit \x1e getrennt)
        let format =
            format!("--format=%h%x1f%p%x1f%s%x1f%(trailers:key={SQUASH_TRAILER},valueonly)%x1e");
        let mut args = vec!["log", &format, "--fixed-strings"];
        args.extend(patterns.iter().map(String::as_str));
        args.push(target);
        let found = self.git.try_run(root, &args);
        if !found.ok {
            return None;
        }
        for record in found.out.split('\u{1e}') {
            let record = record.trim_start();
            let fields: Vec<&str> = record.split('\u{1f}').collect();
            let hash = fields.first().copied().unwrap_or_default();
            if hash.is_empty() {
                continue;
            }
            let parents = fields.get(1).copied().unwrap_or_default();
            let subject = fields.get(2).copied().unwrap_or_default();
            let trailers = fields.get(3).copied().unwrap_or_default();
            let is_merge = parents.split_whitespace().count() >= 2
                && subject.ends_with(&format!("({branch})"));
            let is_squash = trailers
                .lines()
                .any(|value| value.trim_start().starts_with(&format!("{branch} ")));
            if is_merge || is_squash {
                return Some(hash.to_string());
            }
        }
        None
    }

    /// Trägt ein Commit in `target` den Trailer
    /// `Harness-Squashed: <branch> <aktuelle Spitze des Branches>`?
    pub fn squashed_into(&self, root: &Path, branch: &str, target: &str) -> bool {
        let tip = self.git.try_run(
            root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{branch}^{{commit}}"),
            ],
        );
        if !tip.ok || tip.out.is_empty() {
            return false;
        }
        let grep = format!("--grep={SQUASH_TRAILER}: {branch} {}", tip.out);
        let found = self.git.try_run(
            root,
            &["log", "-1", "--format=%H", "--fixed-strings", &grep, target],
        );
        found.ok && !found.out.is_empty()
    }

    /// Dateien, die der Branch seit dem gemeinsamen Vorgänger mit HEAD ändert und die im
    /// Hauptordner lokal geändert oder neu sind (sprachunabhängig, ohne Ausnahmen,
    /// sortiert) – die Ursache, wenn Git den Merge abweist
    pub fn overlapping(&self, root: &Path, branch: &str) -> Result<Vec<String>> {
        let local: BTreeSet<String> = self.checkout.changed(root)?.into_iter().collect();
        let diff = self.git.raw(
            root,
            &[
                "diff",
                "--name-only",
                "-z",
                "--no-renames",
                &format!("HEAD...refs/heads/{branch}"),
            ],
        )?;
        let files: BTreeSet<String> = diff
            .split('\0')
            .filter(|file| !file.is_empty() && local.contains(*file))
            .map(str::to_string)
            .collect();
        Ok(files.into_iter().collect())
    }

    /// Commit-Kennung einer Referenz
    pub fn head(&self, root: &Path, reference: &str) -> Result<String> {
        self.git.run(root, &["rev-parse", reference])
    }

    /// Ist `ancestor` in `branch` enthalten?
    pub fn contains(&self, root: &Path, ancestor: &str, branch: &str) -> bool {
        self.git
            .try_run(root, &["merge-base", "--is-ancestor", ancestor, branch])
            .ok
    }

    /// Löscht einen gemergten Branch mit `-d`. Mit `squashed_tip` (per Squash
    /// zusammengefasst, Git hält ihn für ungemergt): nur, wenn er noch genau auf dieser
    /// Spitze steht und in keinem Worktree ausgecheckt ist (`update-ref -d` mit altem
    /// Wert, damit ein Commit nach der Prüfung nicht verloren geht). Liefert die
    /// Git-Meldung, wenn es nicht geht
    pub fn delete_branch(
        &self,
        root: &Path,
        branch: &str,
        squashed_tip: Option<&str>,
    ) -> Option<String> {
        let Some(tip) = squashed_tip else {
            let attempt = self.git.try_run(root, &["branch", "-d", branch]);
            return if attempt.ok {
                None
            } else {
                Some(format!("Branch {branch} nicht gelöscht: {}", attempt.err))
            };
        };
        let reference = format!("refs/heads/{branch}");
        let checked_out = self.git.try_run(
            root,
            &["for-each-ref", "--format=%(worktreepath)", &reference],
        );
        if checked_out.ok && !checked_out.out.trim().is_empty() {
            return Some(format!(
                "Branch {branch} nicht gelöscht: er ist in {} ausgecheckt.",
                checked_out.out.trim()
            ));
        }
        let attempt = self
            .git
            .try_run(root, &["update-ref", "-d", &reference, tip]);
        if !attempt.ok {
            let reason = if attempt.err.is_empty() {
                "er hat inzwischen neue Commits.".to_string()
            } else {
                attempt.err
            };
            return Some(format!("Branch {branch} nicht gelöscht: {reason}"));
        }
        self.git.try_run(
            root,
            &["config", "--remove-section", &format!("branch.{branch}")],
        );
        None
    }

    /// Squash: ist der Branch schon Vorfahre, gibt es nichts zu tun; sonst
    /// `merge --squash` und ein eigener Commit. Bei --squash gibt es kein MERGE_HEAD:
    /// begonnen hat der Aufruf, wenn danach SQUASH_MSG oder Konfliktdateien da sind.
    fn squash(&self, root: &Path, branch: &str, message: Option<&str>) -> Result<MergeOutcome> {
        let reference = format!("refs/heads/{branch}");
        if self.contains(root, &reference, "HEAD") {
            return self.merged(root);
        }
        // Vorgemerkte Änderungen würde ein Squash (beim Vorspulen prüft Git den Index
        // nicht) mit in den Commit nehmen
        if !self.git.try_run(root, &["diff", "--cached", "--quiet"]).ok {
            return Ok(MergeOutcome::Refused {
                message: "Im Hauptordner sind Änderungen vorgemerkt (git add); ein Squash würde \
sie mit übernehmen. Bitte committen oder aus dem Index nehmen."
                    .to_string(),
            });
        }
        // --no-verify wie beim Merge-Commit von `git merge`, der keinen pre-commit-Hook
        // ausführt
        let paragraphs = self.squash_message(root, branch, message)?;
        let mut commit = vec!["commit", "-q", "--allow-empty", "--no-verify"];
        for paragraph in &paragraphs {
            commit.extend(["-m", paragraph.as_str()]);
        }
        let pending_before = self.squash_pending(root);
        let attempt = self.git.try_run(
            root,
            &[
                "-c",
                "core.quotePath=false",
                "merge",
                "--squash",
                &reference,
            ],
        );
        if attempt.ok {
            let done = self.git.try_run(root, &commit);
            if done.ok {
                return self.merged(root);
            }
            self.abort(root, MergeStrategy::Squash)?;
            let detail = [done.err, done.out]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(MergeOutcome::Refused {
                message: format!("Der Squash-Commit scheiterte: {detail}"),
            });
        }
        let unmerged = self.git.try_run(root, &["ls-files", "-u"]);
        let started = (!pending_before && self.squash_pending(root))
            || (unmerged.ok && !unmerged.out.is_empty());
        self.after_failure(root, &attempt, started, MergeStrategy::Squash)
    }

    /// Gescheiterter Merge oder Squash: Konfliktdateien ermitteln, den eigenen Versuch
    /// zurücksetzen und Konflikt bzw. Abweisung melden
    fn after_failure(
        &self,
        root: &Path,
        attempt: &GitAttempt,
        started: bool,
        strategy: MergeStrategy,
    ) -> Result<MergeOutcome> {
        let mut conflicted: Vec<String> = Vec::new();
        let listed = self.git.try_run(
            root,
            &[
                "-c",
                "core.quotePath=false",
                "diff",
                "--name-only",
                "--diff-filter=U",
            ],
        );
        if listed.ok {
            conflicted = listed
                .out
                .split('\n')
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect();
        }
        // Wie `finally` in TypeScript: der eigene Versuch wird auch zurückgesetzt, wenn
        // die Konfliktliste nicht zu lesen war
        if started {
            self.abort(root, strategy)?;
        }
        if started || !conflicted.is_empty() {
            return Ok(MergeOutcome::Conflict { files: conflicted });
        }
        let message = [attempt.err.clone(), attempt.out.clone()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(MergeOutcome::Refused {
            message: if message.is_empty() {
                "Git hat den Merge abgelehnt.".to_string()
            } else {
                message
            },
        })
    }

    /// Nachricht des Squash-Commits als `-m`-Absätze: Betreff, Liste der
    /// zusammengefassten Commits (älteste zuerst, höchstens MAX_SQUASH_LIST), Trailer mit
    /// der vollen Spitze des Branches
    fn squash_message(
        &self,
        root: &Path,
        branch: &str,
        message: Option<&str>,
    ) -> Result<Vec<String>> {
        let reference = format!("refs/heads/{branch}");
        let tip = self
            .git
            .run(root, &["rev-parse", &format!("{reference}^{{commit}}")])?;
        let listed = self.git.run(
            root,
            &[
                "log",
                "--reverse",
                "--format=- %s",
                &format!("HEAD..{reference}"),
            ],
        )?;
        let subjects: Vec<&str> = listed.split('\n').filter(|line| !line.is_empty()).collect();
        let mut shown: Vec<String> = subjects
            .iter()
            .take(MAX_SQUASH_LIST)
            .map(|line| (*line).to_string())
            .collect();
        if subjects.len() > shown.len() {
            shown.push(format!(
                "(… und {} weitere Commits)",
                subjects.len() - shown.len()
            ));
        }
        let mut paragraphs = vec![match message {
            Some(message) if !message.is_empty() => message.to_string(),
            _ => format!("Squash {branch}"),
        }];
        if !shown.is_empty() {
            paragraphs.push(shown.join("\n"));
        }
        paragraphs.push(format!("{SQUASH_TRAILER}: {branch} {tip}"));
        Ok(paragraphs)
    }

    /// Liegt eine SQUASH_MSG im Git-Verzeichnis (ein begonnener, nicht abgeschlossener
    /// Squash)?
    fn squash_pending(&self, root: &Path) -> bool {
        let path = self
            .git
            .try_run(root, &["rev-parse", "--git-path", "SQUASH_MSG"]);
        if !path.ok {
            return false;
        }
        let candidate = PathBuf::from(&path.out);
        let full = if candidate.is_absolute() {
            candidate
        } else {
            root.join(candidate)
        };
        full.exists()
    }

    /// Ergebnis „gemergt“ mit der Kurzkennung von HEAD
    fn merged(&self, root: &Path) -> Result<MergeOutcome> {
        Ok(MergeOutcome::Merged {
            commit: self.git.run(root, &["rev-parse", "--short", "HEAD"])?,
        })
    }

    /// Gibt es den Branch (als Commit auflösbar)?
    fn branch_exists(&self, root: &Path, branch: &str) -> bool {
        self.git
            .try_run(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{branch}^{{commit}}"),
                ],
            )
            .ok
    }

    /// Bricht den eigenen Merge ab; scheitert das, steht der Ordner noch im
    /// Merge-Zustand → Fehler statt Konfliktmeldung. Ein Squash hat kein MERGE_HEAD:
    /// `git reset --merge` setzt Index und vom Squash geänderte Dateien zurück, lässt aber
    /// nicht gestagte lokale Änderungen stehen (wie `merge --abort`) und entfernt
    /// SQUASH_MSG.
    fn abort(&self, root: &Path, strategy: MergeStrategy) -> Result<()> {
        let command: Vec<&str> = match strategy {
            MergeStrategy::Squash => vec!["reset", "-q", "--merge"],
            MergeStrategy::Merge => vec!["merge", "--abort"],
        };
        let aborted = self.git.try_run(root, &command);
        if aborted.ok {
            return Ok(());
        }
        let git = if aborted.err.is_empty() {
            aborted.out
        } else {
            aborted.err
        };
        let manual = format!(
            "git {}",
            command
                .iter()
                .filter(|argument| **argument != "-q")
                .cloned()
                .collect::<Vec<_>>()
                .join(" ")
        );
        // details: Ordner, Git-Meldung und Befehl, damit ein Aufrufer außerhalb des
        // Hauptordners den richtigen Ordner nennt
        Err(HarnessError::new(
            "merge-abort-failed",
            format!(
                "Der Hauptordner steht noch im Merge-Zustand: {git}. \
Bitte von Hand aufräumen ({manual})."
            ),
        )
        .with_details(json!({
            "root": root.to_string_lossy(),
            "git": git,
            "command": manual,
        }))
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};

    use crate::test_support::TempRepo;

    /// Projekt mit einem Commit auf dem Hauptbranch; `main` ist dessen Name
    fn repo() -> (TempRepo, String) {
        let repo = TempRepo::new();
        repo.commit("a.txt", "basis");
        let main = repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]);
        (repo, main)
    }

    /// Merge mit Merge-Commit
    fn merge(repo: &TempRepo, branch: &str, message: Option<&str>) -> MergeOutcome {
        BranchMerger::new()
            .merge(repo.path(), branch, message, MergeStrategy::Merge)
            .unwrap()
    }

    /// Merge mit Squash
    fn squash(repo: &TempRepo, branch: &str, message: Option<&str>) -> MergeOutcome {
        BranchMerger::new()
            .merge(repo.path(), branch, message, MergeStrategy::Squash)
            .unwrap()
    }

    /// Liegt ein MERGE_HEAD im Git-Verzeichnis?
    fn merge_head(repo: &TempRepo) -> bool {
        repo.path().join(".git").join("MERGE_HEAD").exists()
    }

    /// Reste eines Squash oder Merge im Git-Verzeichnis?
    fn leftovers(repo: &TempRepo) -> bool {
        repo.path().join(".git").join("SQUASH_MSG").exists() || merge_head(repo)
    }

    /// Ticket-Branch und Hauptbranch ändern a.txt unterschiedlich
    fn prepare_conflict(repo: &TempRepo, main: &str) {
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("a.txt", "ticket");
        repo.git(&["checkout", "-q", main]);
        repo.commit("a.txt", "haupt");
    }

    /// Ticket-Branch und Hauptbranch haken verschiedene Aufgaben derselben tasks.md ab –
    /// seit dem Wegfall des Spec-Kit-Formats ist das ein gewöhnlicher Konflikt
    fn prepare_tasks_conflict(repo: &TempRepo, main: &str) {
        repo.commit(
            "specs/001-x/tasks.md",
            "# Aufgaben\n- [ ] T001 Erste\n- [ ] T002 Zweite\n",
        );
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit(
            "specs/001-x/tasks.md",
            "# Aufgaben\n- [X] T001 Erste\n- [ ] T002 Zweite\n",
        );
        repo.git(&["checkout", "-q", main]);
        repo.commit(
            "specs/001-x/tasks.md",
            "# Aufgaben\n- [ ] T001 Erste\n- [X] T002 Zweite\n",
        );
    }

    /// Entscheidet je Git-Aufruf, ob er scheitert
    type FailingRule = Arc<dyn Fn(&[&str]) -> Option<GitAttempt> + Send + Sync>;

    /// Git-Aufrufer, der ausgewählte Aufrufe scheitern lässt und alle Aufrufe mitschreibt
    #[derive(Clone)]
    struct FakeGit {
        inner: GitRunner,
        calls: Arc<Mutex<Vec<Vec<String>>>>,
        failing: FailingRule,
    }

    impl FakeGit {
        fn new(failing: impl Fn(&[&str]) -> Option<GitAttempt> + Send + Sync + 'static) -> Self {
            Self {
                inner: GitRunner::new(),
                calls: Arc::new(Mutex::new(Vec::new())),
                failing: Arc::new(failing),
            }
        }

        fn record(&self, args: &[&str]) {
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|arg| (*arg).to_string()).collect());
        }

        fn called(&self, args: &[&str]) -> bool {
            let wanted: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
            self.calls.lock().unwrap().contains(&wanted)
        }
    }

    impl GitCommands for FakeGit {
        fn raw(&self, cwd: &Path, args: &[&str]) -> Result<String> {
            self.record(args);
            match (self.failing)(args) {
                Some(failed) => Err(HarnessError::new("git-failed", failed.err).into()),
                None => self.inner.raw(cwd, args),
            }
        }

        fn try_run(&self, cwd: &Path, args: &[&str]) -> GitAttempt {
            self.record(args);
            (self.failing)(args).unwrap_or_else(|| self.inner.try_run(cwd, args))
        }
    }

    /// Fehlgeschlagener Versuch mit dieser Fehlerausgabe
    fn failed(err: &str) -> GitAttempt {
        GitAttempt {
            ok: false,
            out: String::new(),
            err: err.to_string(),
        }
    }

    #[test]
    fn mergt_ohne_konflikt_mit_eigenem_merge_commit_und_loscht_den_branch() {
        let (repo, main) = repo();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);
        let merger = BranchMerger::new();

        let result = merge(&repo, "ticket/x", None);

        assert!(matches!(result, MergeOutcome::Merged { .. }), "{result:?}");
        assert_eq!(
            repo.git(&["log", "-1", "--format=%P"]).split(' ').count(),
            2
        );
        assert!(merger.contains(repo.path(), "ticket/x", &main));
        assert_eq!(merger.delete_branch(repo.path(), "ticket/x", None), None);
        assert_eq!(repo.git(&["branch", "--list", "ticket/x"]), "");
    }

    #[test]
    fn nutzt_eine_ubergebene_merge_nachricht_statt_der_git_standardnachricht() {
        let (repo, main) = repo();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);

        let result = merge(&repo, "ticket/x", Some("Merge ARC-0012 „Login“ (ticket/x)"));

        assert!(matches!(result, MergeOutcome::Merged { .. }), "{result:?}");
        assert_eq!(
            repo.git(&["log", "-1", "--format=%s"]),
            "Merge ARC-0012 „Login“ (ticket/x)"
        );
    }

    #[test]
    fn merged_earlier_findet_merge_und_squash_aber_keinen_revert_oder_blosse_erwahnung() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);
        repo.commit("c.txt", "erwähnt (ticket/x)");

        assert!(matches!(
            merge(&repo, "ticket/x", Some("Merge ARC-0012 „Login“ (ticket/x)")),
            MergeOutcome::Merged { .. }
        ));
        let merge_commit = repo.git(&["rev-parse", "--short", "HEAD"]);
        assert_eq!(merger.merged_earlier(repo.path(), "ticket/x", "HEAD"), None);
        repo.git(&["branch", "-D", "ticket/x"]);
        assert_eq!(
            merger.merged_earlier(repo.path(), "ticket/x", "HEAD"),
            Some(merge_commit.clone())
        );
        repo.git(&["revert", "--no-edit", "-m", "1", "HEAD"]);
        assert!(repo
            .git(&["log", "-1", "--format=%s"])
            .contains("(ticket/x)"));
        assert_eq!(
            merger.merged_earlier(repo.path(), "ticket/x", "HEAD"),
            Some(merge_commit)
        );

        // Nur Erwähnung und Revert: kein früherer Merge
        repo.git(&["checkout", "-q", "-b", "ticket/y"]);
        repo.commit("d.txt", "neu");
        repo.git(&["checkout", "-q", &main]);
        repo.commit("e.txt", "Revert \"Merge ARC-0013 (ticket/y)\"");
        repo.git(&["branch", "-D", "ticket/y"]);
        assert_eq!(merger.merged_earlier(repo.path(), "ticket/y", "HEAD"), None);

        // Squash: der Trailer zählt
        repo.git(&["checkout", "-q", "-b", "ticket/z"]);
        repo.commit("f.txt", "neu");
        repo.git(&["checkout", "-q", &main]);
        assert!(matches!(
            squash(&repo, "ticket/z", Some("ARC-0014 (squash von ticket/z)")),
            MergeOutcome::Merged { .. }
        ));
        let squash_commit = repo.git(&["rev-parse", "--short", "HEAD"]);
        repo.git(&["branch", "-D", "ticket/z"]);
        assert_eq!(
            merger.merged_earlier(repo.path(), "ticket/z", "HEAD"),
            Some(squash_commit)
        );
    }

    #[test]
    fn meldet_konflikte_mit_dateien_und_bricht_den_merge_ab() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        prepare_conflict(&repo, &main);
        let head = merger.head(repo.path(), "HEAD").unwrap();

        let result = merge(&repo, "ticket/x", None);

        assert_eq!(
            result,
            MergeOutcome::Conflict {
                files: vec!["a.txt".to_string()]
            }
        );
        assert_eq!(merger.head(repo.path(), "HEAD").unwrap(), head);
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert!(!merger.contains(repo.path(), &main, "ticket/x"));
    }

    #[test]
    fn meldet_einen_abgewiesenen_merge_wenn_eine_lokal_geanderte_datei_betroffen_ist() {
        let (repo, main) = repo();
        repo.commit("cfg.yaml", "a");
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("cfg.yaml", "b");
        repo.git(&["checkout", "-q", &main]);
        repo.write("cfg.yaml", "lokal");

        let result = merge(&repo, "ticket/x", None);

        match result {
            MergeOutcome::Refused { message } => assert!(message.contains("cfg.yaml"), "{message}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(repo.git(&["status", "--porcelain"]), "M cfg.yaml");
    }

    #[test]
    fn overlapping_schnittmenge_aus_branch_anderungen_und_lokalen_dateien() {
        let (repo, main) = repo();
        repo.commit("cfg.yaml", "a");
        repo.commit("frei.txt", "a");
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("cfg.yaml", "b");
        repo.commit("neu.txt", "ticket");
        repo.commit("nur-ticket.txt", "ticket");
        repo.git(&["checkout", "-q", &main]);
        repo.commit("haupt.txt", "haupt");
        for file in ["cfg.yaml", "neu.txt", "frei.txt", "haupt.txt"] {
            repo.write(file, "lokal");
        }

        let overlapping = BranchMerger::new()
            .overlapping(repo.path(), "ticket/x")
            .unwrap();

        assert_eq!(overlapping, ["cfg.yaml", "neu.txt"]);
    }

    #[test]
    fn overlapping_ohne_lokale_anderungen_leer() {
        let (repo, main) = repo();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);

        assert!(BranchMerger::new()
            .overlapping(repo.path(), "ticket/x")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn meldet_einen_fehler_beim_loschen_eines_nicht_gemergten_branches() {
        let (repo, main) = repo();
        repo.git(&["checkout", "-q", "-b", "ticket/y"]);
        repo.commit("c.txt", "x");
        repo.git(&["checkout", "-q", &main]);

        let message = BranchMerger::new()
            .delete_branch(repo.path(), "ticket/y", None)
            .unwrap();

        assert!(message.contains("ticket/y"), "{message}");
    }

    #[test]
    fn weist_ab_wenn_im_hauptordner_schon_ein_merge_lauft_und_lasst_ihn_unangetastet() {
        let (repo, main) = repo();
        prepare_conflict(&repo, &main);
        repo.git(&["checkout", "-q", "-b", "ticket/y"]);
        repo.commit("c.txt", "y");
        repo.git(&["checkout", "-q", &main]);
        let (ok, _, _) = repo.try_git(&["merge", "--no-edit", "ticket/x"]);
        assert!(!ok);
        assert!(merge_head(&repo));

        let result = merge(&repo, "ticket/y", None);

        match result {
            MergeOutcome::Refused { message } => assert!(
                message.contains("Im Hauptordner läuft bereits ein Merge"),
                "{message}"
            ),
            other => panic!("{other:?}"),
        }
        assert!(merge_head(&repo));
        assert!(!repo.path().join("c.txt").exists());
    }

    #[test]
    fn weist_bei_laufendem_cherry_pick_oder_rebase_ab_ohne_zu_mergen() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);
        let head = merger.head(repo.path(), "HEAD").unwrap();
        std::fs::write(repo.path().join(".git").join("CHERRY_PICK_HEAD"), &head).unwrap();

        match merge(&repo, "ticket/x", None) {
            MergeOutcome::Refused { message } => {
                assert!(message.contains("Cherry-Pick"), "{message}")
            }
            other => panic!("{other:?}"),
        }

        std::fs::remove_file(repo.path().join(".git").join("CHERRY_PICK_HEAD")).unwrap();
        std::fs::create_dir(repo.path().join(".git").join("rebase-merge")).unwrap();

        match merge(&repo, "ticket/x", None) {
            MergeOutcome::Refused { message } => assert!(message.contains("Rebase"), "{message}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(merger.head(repo.path(), "HEAD").unwrap(), head);
    }

    #[test]
    fn bricht_den_merge_auch_ab_wenn_die_konfliktliste_nicht_gelesen_werden_kann() {
        let (repo, main) = repo();
        prepare_conflict(&repo, &main);
        let git = FakeGit::new(|args| args.contains(&"diff").then(|| failed("kaputt")));

        let result = BranchMerger::with_git(git.clone())
            .merge(repo.path(), "ticket/x", None, MergeStrategy::Merge)
            .unwrap();

        assert_eq!(result, MergeOutcome::Conflict { files: Vec::new() });
        assert!(git.called(&["merge", "--abort"]));
        assert!(!merge_head(&repo));
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
    }

    #[test]
    fn wirft_merge_abort_failed_wenn_der_abbruch_scheitert() {
        let (repo, main) = repo();
        prepare_conflict(&repo, &main);
        let git =
            FakeGit::new(|args| (args.join(" ") == "merge --abort").then(|| failed("gesperrt")));

        let error = BranchMerger::with_git(git)
            .merge(repo.path(), "ticket/x", None, MergeStrategy::Merge)
            .unwrap_err();

        assert_eq!(error.code(), Some("merge-abort-failed"));
        assert!(error.to_string().contains("gesperrt"), "{error}");
        assert!(error.to_string().contains("git merge --abort"), "{error}");
        // Der Aufrufer kennt den Ort des Merges und nennt damit den richtigen Ordner
        assert_eq!(
            error.harness().unwrap().details,
            Some(json!({
                "root": repo.path().to_string_lossy(),
                "git": "gesperrt",
                "command": "git merge --abort",
            }))
        );
        assert!(merge_head(&repo));
        repo.git(&["merge", "--abort"]);
    }

    #[test]
    fn meldet_einen_konflikt_in_einer_tasks_md_wie_bei_jeder_anderen_datei() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        prepare_tasks_conflict(&repo, &main);
        let head = merger.head(repo.path(), "HEAD").unwrap();

        let result = merge(&repo, "ticket/x", None);

        assert_eq!(
            result,
            MergeOutcome::Conflict {
                files: vec!["specs/001-x/tasks.md".to_string()]
            }
        );
        assert_eq!(merger.head(repo.path(), "HEAD").unwrap(), head);
        assert!(!merge_head(&repo));
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
    }

    #[test]
    fn meldet_einen_konflikt_in_zwei_dateien_mit_beiden_pfaden() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        prepare_conflict(&repo, &main);
        repo.git(&["checkout", "-q", "ticket/x"]);
        repo.commit("b.txt", "ticket");
        repo.git(&["checkout", "-q", &main]);
        repo.commit("b.txt", "haupt");
        let head = merger.head(repo.path(), "HEAD").unwrap();

        let result = merge(&repo, "ticket/x", None);

        assert_eq!(
            result,
            MergeOutcome::Conflict {
                files: vec!["a.txt".to_string(), "b.txt".to_string()]
            }
        );
        assert_eq!(merger.head(repo.path(), "HEAD").unwrap(), head);
        assert!(!merge_head(&repo));
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
    }

    #[test]
    fn mergt_den_branch_auch_wenn_ein_gleichnamiger_tag_existiert() {
        let (repo, main) = repo();
        repo.git(&["tag", "ticket/x"]);
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);

        assert!(matches!(
            merge(&repo, "ticket/x", None),
            MergeOutcome::Merged { .. }
        ));
        assert!(repo.path().join("b.txt").exists());
    }

    #[test]
    fn squash_fasst_alle_commits_in_einem_commit_mit_liste_und_trailer_zusammen() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "eins");
        repo.commit("c.txt", "zwei");
        let tip = merger.head(repo.path(), "ticket/x").unwrap();
        repo.git(&["checkout", "-q", &main]);
        let before = merger.head(repo.path(), "HEAD").unwrap();

        let result = squash(
            &repo,
            "ticket/x",
            Some("ARC-0012 „Login“ (squash von ticket/x)"),
        );

        assert!(matches!(result, MergeOutcome::Merged { .. }), "{result:?}");
        assert_eq!(repo.git(&["log", "-1", "--format=%P"]), before);
        assert_eq!(
            repo.git(&["log", "-1", "--format=%s"]),
            "ARC-0012 „Login“ (squash von ticket/x)"
        );
        let body = repo.git(&["log", "-1", "--format=%b"]);
        assert!(body.contains("- b.txt=eins"), "{body}");
        assert!(body.contains("- c.txt=zwei"), "{body}");
        assert_eq!(
            repo.git(&[
                "log",
                "-1",
                "--format=%(trailers:key=Harness-Squashed,valueonly)"
            ]),
            format!("ticket/x {tip}")
        );
        assert_eq!(repo.git(&["show", "HEAD:c.txt"]), "zwei");
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert!(!leftovers(&repo));
        assert!(!merger.contains(repo.path(), "ticket/x", &main));
        assert!(merger.integrated(repo.path(), "ticket/x", "HEAD"));
    }

    #[test]
    fn squash_kurzt_die_liste_der_zusammengefassten_commits() {
        let (repo, main) = repo();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        for index in 0..60 {
            repo.commit("b.txt", &format!("v{index}"));
        }
        repo.git(&["checkout", "-q", &main]);

        assert!(matches!(
            squash(&repo, "ticket/x", Some("ARC-0012 (squash von ticket/x)")),
            MergeOutcome::Merged { .. }
        ));
        let body = repo.git(&["log", "-1", "--format=%b"]);
        assert_eq!(
            body.lines().filter(|line| line.starts_with("- ")).count(),
            50
        );
        assert!(body.contains("und 10 weitere"), "{body}");
    }

    #[test]
    fn squash_meldet_einen_konflikt_in_einer_tasks_md_wie_bei_jeder_anderen_datei() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        prepare_tasks_conflict(&repo, &main);
        let before = merger.head(repo.path(), "HEAD").unwrap();

        let result = squash(&repo, "ticket/x", Some("ARC-0012 (squash von ticket/x)"));

        assert_eq!(
            result,
            MergeOutcome::Conflict {
                files: vec!["specs/001-x/tasks.md".to_string()]
            }
        );
        assert_eq!(merger.head(repo.path(), "HEAD").unwrap(), before);
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert!(!leftovers(&repo));
    }

    #[test]
    fn squash_meldet_einen_konflikt_und_lasst_lokale_anderungen_exakt_stehen() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.commit("cfg.yaml", "a");
        prepare_conflict(&repo, &main);
        repo.git(&["checkout", "-q", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);
        repo.write("cfg.yaml", "lokal");
        repo.write("frei.txt", "neu");
        let head = merger.head(repo.path(), "HEAD").unwrap();
        let status = repo.git(&["status", "--porcelain"]);

        let result = squash(&repo, "ticket/x", Some("m"));

        assert_eq!(
            result,
            MergeOutcome::Conflict {
                files: vec!["a.txt".to_string()]
            }
        );
        assert_eq!(merger.head(repo.path(), "HEAD").unwrap(), head);
        assert_eq!(repo.git(&["status", "--porcelain"]), status);
        assert_eq!(repo.git(&["show", ":a.txt"]), "haupt");
        assert!(!repo.path().join("b.txt").exists());
        assert_eq!(repo.git(&["show", ":cfg.yaml"]), "a");
        assert!(!leftovers(&repo));
        assert!(!merger.integrated(repo.path(), "ticket/x", "HEAD"));
    }

    #[test]
    fn squash_weist_ab_wenn_eine_lokal_geanderte_datei_betroffen_ist() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.commit("cfg.yaml", "a");
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("cfg.yaml", "b");
        repo.git(&["checkout", "-q", &main]);
        repo.write("cfg.yaml", "lokal");
        let head = merger.head(repo.path(), "HEAD").unwrap();

        match squash(&repo, "ticket/x", Some("m")) {
            MergeOutcome::Refused { message } => assert!(message.contains("cfg.yaml"), "{message}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(repo.git(&["status", "--porcelain"]), "M cfg.yaml");
        assert_eq!(merger.head(repo.path(), "HEAD").unwrap(), head);
    }

    #[test]
    fn squash_weist_bei_laufendem_merge_ab_ohne_ihn_anzufassen() {
        let (repo, main) = repo();
        prepare_conflict(&repo, &main);
        repo.git(&["checkout", "-q", "-b", "ticket/y"]);
        repo.commit("c.txt", "y");
        repo.git(&["checkout", "-q", &main]);
        let (ok, _, _) = repo.try_git(&["merge", "--no-edit", "ticket/x"]);
        assert!(!ok);

        assert!(matches!(
            squash(&repo, "ticket/y", Some("m")),
            MergeOutcome::Refused { .. }
        ));
        assert!(merge_head(&repo));
        repo.git(&["merge", "--abort"]);
    }

    #[test]
    fn squash_legt_fur_einen_branch_ohne_neue_anderungen_einen_leeren_commit_mit_trailer_an() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        let tip = merger.head(repo.path(), "ticket/x").unwrap();
        repo.git(&["checkout", "-q", &main]);
        // Eigener Commit vorher: sonst ergäbe der Cherry-Pick in derselben Sekunde
        // denselben Commit wie im Branch
        repo.commit("haupt.txt", "haupt");
        repo.git(&["cherry-pick", "ticket/x"]);
        let before = merger.head(repo.path(), "HEAD").unwrap();

        let result = squash(&repo, "ticket/x", Some("ARC-0012 (squash von ticket/x)"));

        assert!(matches!(result, MergeOutcome::Merged { .. }), "{result:?}");
        assert_eq!(repo.git(&["log", "-1", "--format=%P"]), before);
        assert_eq!(
            repo.git(&[
                "log",
                "-1",
                "--format=%(trailers:key=Harness-Squashed,valueonly)"
            ]),
            format!("ticket/x {tip}")
        );
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert!(!leftovers(&repo));
    }

    #[test]
    fn legt_keinen_commit_an_wenn_der_branch_schon_enthalten_oder_zusammengefasst_ist() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.git(&["checkout", "-q", &main]);
        let head = merger.head(repo.path(), "HEAD").unwrap();

        assert_eq!(
            squash(&repo, "ticket/x", Some("m")),
            MergeOutcome::Merged {
                commit: repo.git(&["rev-parse", "--short", "HEAD"])
            }
        );
        assert_eq!(merger.head(repo.path(), "HEAD").unwrap(), head);

        repo.git(&["checkout", "-q", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);
        assert!(matches!(
            squash(&repo, "ticket/x", Some("m")),
            MergeOutcome::Merged { .. }
        ));
        let squash_commit = merger.head(repo.path(), "HEAD").unwrap();
        assert!(matches!(
            squash(&repo, "ticket/x", Some("m")),
            MergeOutcome::Merged { .. }
        ));
        assert!(matches!(
            merge(&repo, "ticket/x", Some("m")),
            MergeOutcome::Merged { .. }
        ));
        assert_eq!(merger.head(repo.path(), "HEAD").unwrap(), squash_commit);
    }

    #[test]
    fn integrated_erkennt_vorfahren_und_zusammengefasste_branches_aber_keine_neueren_spitzen() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);

        assert!(!merger.integrated(repo.path(), "ticket/x", "HEAD"));
        squash(&repo, "ticket/x", Some("m"));
        assert!(merger.integrated(repo.path(), "ticket/x", "HEAD"));
        assert!(!merger.integrated(repo.path(), "ticket/x", "HEAD~1"));

        repo.git(&["checkout", "-q", "ticket/x"]);
        repo.commit("c.txt", "danach");
        repo.git(&["checkout", "-q", &main]);
        assert!(!merger.integrated(repo.path(), "ticket/x", "HEAD"));
        repo.git(&["branch", "ticket/y", "HEAD~1"]);
        assert!(merger.integrated(repo.path(), "ticket/y", "HEAD"));
        assert!(!merger.integrated(repo.path(), "gibt/es-nicht", "HEAD"));
    }

    #[test]
    fn ein_branch_dessen_name_prafix_eines_anderen_ist_gilt_nicht_als_zusammengefasst() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.git(&["checkout", "-q", "-b", "ticket/x-2"]);
        repo.commit("b.txt", "neu");
        repo.git(&["branch", "ticket/x"]);
        repo.git(&["checkout", "-q", &main]);

        squash(&repo, "ticket/x-2", Some("m"));

        assert!(merger.integrated(repo.path(), "ticket/x-2", "HEAD"));
        assert!(!merger.integrated(repo.path(), "ticket/x", "HEAD"));
    }

    #[test]
    fn delete_branch_loscht_einen_zusammengefassten_branch_nur_auf_der_gepruften_spitze() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        let tip = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "-q", &main]);
        squash(&repo, "ticket/x", Some("m"));

        assert!(merger
            .delete_branch(repo.path(), "ticket/x", None)
            .unwrap()
            .contains("ticket/x"));

        // Nach der Prüfung kam noch ein Commit dazu: nicht löschen
        repo.git(&["checkout", "-q", "ticket/x"]);
        repo.commit("c.txt", "später");
        repo.git(&["checkout", "-q", &main]);
        assert!(merger
            .delete_branch(repo.path(), "ticket/x", Some(&tip))
            .unwrap()
            .contains("ticket/x"));
        assert_ne!(repo.git(&["branch", "--list", "ticket/x"]), "");

        let current = repo.git(&["rev-parse", "ticket/x"]);
        assert_eq!(
            merger.delete_branch(repo.path(), "ticket/x", Some(&current)),
            None
        );
        assert_eq!(repo.git(&["branch", "--list", "ticket/x"]), "");
    }

    #[test]
    fn verweigert_den_squash_bei_vorgemerkten_anderungen_und_lasst_sie_unverandert() {
        let (repo, main) = repo();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);
        let before = repo.git(&["rev-parse", "HEAD"]);
        repo.write("s.txt", "vorgemerkt");
        repo.git(&["add", "s.txt"]);

        assert!(matches!(
            squash(&repo, "ticket/x", Some("m")),
            MergeOutcome::Refused { .. }
        ));
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), before);
        assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), "s.txt");
        assert_eq!(
            std::fs::read_to_string(repo.path().join("s.txt")).unwrap(),
            "vorgemerkt"
        );
    }

    #[test]
    fn ein_scheiternder_pre_commit_hook_verhindert_den_squash_nicht() {
        let (repo, main) = repo();
        let merger = BranchMerger::new();
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        repo.commit("b.txt", "neu");
        repo.git(&["checkout", "-q", &main]);
        let hooks = repo.path().join(".git").join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        let hook = hooks.join("pre-commit");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        assert!(matches!(
            squash(&repo, "ticket/x", Some("m")),
            MergeOutcome::Merged { .. }
        ));
        assert!(merger.integrated(repo.path(), "ticket/x", "HEAD"));
    }
}
