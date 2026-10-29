//! Prüft, ob das Spec-Dokument eines Tickets vollständig committet ist.
//! Portiert aus `packages/core/src/docs/SpecCommitCheck.ts`.

use std::path::Path;

use crate::git::{GitCommands, GitRunner};

/// Prüft, ob das Spec-Dokument eines Tickets vollständig committet ist: weder geändert noch
/// ungetrackt und im HEAD vorhanden. Worktrees und Agenten sehen nur den letzten Commit; die
/// Prüfung liefert dafür eine Warnung (nie einen Abbruch).
#[derive(Debug, Clone, Copy, Default)]
pub struct SpecCommitCheck<G: GitCommands = GitRunner> {
    git: G,
}

impl SpecCommitCheck<GitRunner> {
    /// Prüfung mit dem echten Git
    pub fn new() -> Self {
        Self { git: GitRunner }
    }
}

impl<G: GitCommands> SpecCommitCheck<G> {
    /// Prüfung mit eigenem Git (Tests)
    pub fn with_git(git: G) -> Self {
        Self { git }
    }

    /// Warnung zum Spec-Dokument (Pfad relativ zu `root`) oder `None`: alles committet, kein
    /// Git-Repo, leerer Pfad oder die Datei gibt es nicht (z. B. ein abgeschnittener Pfad am
    /// Ticket)
    pub fn warning_for_doc(&self, root: &Path, doc_path: &str) -> Option<String> {
        if doc_path.is_empty() {
            return None;
        }
        if !self
            .git
            .try_run(root, &["rev-parse", "--is-inside-work-tree"])
            .ok
        {
            return None;
        }
        let file = Path::new(doc_path);
        let absolute = if file.is_absolute() {
            file.to_path_buf()
        } else {
            root.join(file)
        };
        if !absolute.exists() {
            return None;
        }
        if self.committed(root, doc_path) {
            None
        } else {
            Some(message(doc_path))
        }
    }

    /// `true`, wenn das Dokument im HEAD liegt und weder geändert noch ungetrackt ist
    fn committed(&self, root: &Path, doc_path: &str) -> bool {
        let status = self.git.try_run(
            root,
            &[
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--",
                doc_path,
            ],
        );
        if !status.ok || !status.out.is_empty() {
            return false;
        }
        let tree = self
            .git
            .try_run(root, &["ls-tree", "--name-only", "HEAD", "--", doc_path]);
        tree.ok && !tree.out.is_empty()
    }
}

/// Text der Warnung
pub fn message(doc_path: &str) -> String {
    format!("Das Spec-Dokument {doc_path} ist nicht (vollständig) committet – Worktrees und Agenten sehen nur den letzten Commit. Erst committen.")
}
