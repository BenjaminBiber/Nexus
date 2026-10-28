//! Alle Pfade im Harness-Ordner (`<git-common-dir>/harness`).

use std::path::{Path, PathBuf};

/// Pfade im Harness-Ordner eines Projekts
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessLayout {
    dir: PathBuf,
}

impl HarnessLayout {
    /// Harness-Ordner im gemeinsamen Git-Verzeichnis
    pub fn new(common_dir: impl AsRef<Path>) -> Self {
        Self {
            dir: common_dir.as_ref().join("harness"),
        }
    }

    /// Der Harness-Ordner selbst
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Zugriffs-Token für Hook-Aufrufe
    pub fn token_file(&self) -> PathBuf {
        self.dir.join("token")
    }

    /// Zählerstände des Projekts
    pub fn counters_file(&self) -> PathBuf {
        self.dir.join("counters.json")
    }

    /// Sperre der Zählerstände
    pub fn counters_lock(&self) -> PathBuf {
        self.dir.join("counters.lock")
    }

    /// Nummern der Sessions
    pub fn session_numbers_file(&self) -> PathBuf {
        self.dir.join("session-numbers.json")
    }

    /// Sperre der Session-Nummern
    pub fn session_numbers_lock(&self) -> PathBuf {
        self.dir.join("session-numbers.lock")
    }

    /// Bekannte Worktrees
    pub fn worktrees_file(&self) -> PathBuf {
        self.dir.join("worktrees.json")
    }

    /// Sperre der Worktrees
    pub fn worktrees_lock(&self) -> PathBuf {
        self.dir.join("worktrees.lock")
    }

    /// Kontext zu offenen Fragen
    pub fn question_context_file(&self) -> PathBuf {
        self.dir.join("question-context.json")
    }

    /// Sperre des Fragen-Kontexts
    pub fn question_context_lock(&self) -> PathBuf {
        self.dir.join("question-context.lock")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liegt_im_gemeinsamen_git_verzeichnis() {
        let layout = HarnessLayout::new("/tmp/x/.git");

        assert_eq!(layout.dir(), Path::new("/tmp/x/.git/harness"));
        assert_eq!(layout.token_file(), Path::new("/tmp/x/.git/harness/token"));
        assert_eq!(
            layout.counters_file(),
            Path::new("/tmp/x/.git/harness/counters.json")
        );
        assert_eq!(
            layout.counters_lock(),
            Path::new("/tmp/x/.git/harness/counters.lock")
        );
        assert_eq!(
            layout.session_numbers_file(),
            Path::new("/tmp/x/.git/harness/session-numbers.json")
        );
        assert_eq!(
            layout.session_numbers_lock(),
            Path::new("/tmp/x/.git/harness/session-numbers.lock")
        );
        assert_eq!(
            layout.worktrees_file(),
            Path::new("/tmp/x/.git/harness/worktrees.json")
        );
        assert_eq!(
            layout.worktrees_lock(),
            Path::new("/tmp/x/.git/harness/worktrees.lock")
        );
        assert_eq!(
            layout.question_context_file(),
            Path::new("/tmp/x/.git/harness/question-context.json")
        );
        assert_eq!(
            layout.question_context_lock(),
            Path::new("/tmp/x/.git/harness/question-context.lock")
        );
    }
}
