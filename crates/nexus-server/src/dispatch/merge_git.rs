//! Die Git-Seite des Merges als Schnittstelle: Zustand eines Ordners, der Merge selbst und
//! die Zielbranches. Im Betrieb stehen dahinter [`MainCheckout`], [`BranchMerger`] und
//! [`TargetBranches`] aus `nexus-core`.
//!
//! In TypeScript ersetzen die Tests diese Aufrufe mit `vi.spyOn(BranchMerger.prototype, …)`;
//! in Rust tritt an dieselbe Stelle eine Attrappe für [`MergeGit`].

use std::path::Path;

use nexus_core::dispatch::git::{
    BranchMerger, CheckoutState, EnsuredBranch, MainCheckout, MergeOutcome, TargetBranches,
};
use nexus_core::dispatch::MergeStrategy;
use nexus_core::Result;

/// Alles, was der Merge von Git braucht.
pub trait MergeGit: std::fmt::Debug + Send + Sync {
    /// Branch, laufende Git-Aktion und störende lokale Änderungen eines Ordners
    fn inspect(&self, root: &Path) -> Result<CheckoutState>;

    /// Läuft gerade ein Merge, Rebase, Cherry-Pick oder Revert? (z. B. „ein Merge“)
    fn running(&self, root: &Path) -> Result<Option<String>>;

    /// Mergt den Branch im Ordner; bei Problemen bleibt der Ordner wie vorher
    fn merge(
        &self,
        root: &Path,
        branch: &str,
        message: Option<&str>,
        strategy: MergeStrategy,
    ) -> Result<MergeOutcome>;

    /// Ist `ancestor` in `branch` enthalten?
    fn contains(&self, root: &Path, ancestor: &str, branch: &str) -> Result<bool>;

    /// Den Branch gibt es nicht mehr, aber `target` enthält schon seinen Merge; liefert dessen
    /// Kurzkennung
    fn merged_earlier(&self, root: &Path, branch: &str, target: &str) -> Option<String>;

    /// Lokal geänderte Dateien, die der Branch ebenfalls ändert
    fn overlapping(&self, root: &Path, branch: &str) -> Result<Vec<String>>;

    /// Legt den Zielbranch an, falls er fehlt
    fn ensure_target(&self, root: &Path, name: &str) -> Result<EnsuredBranch>;

    /// Ordner, in dem der Zielbranch ausgecheckt ist, oder `None`
    fn checked_out_at(&self, root: &Path, name: &str) -> Result<Option<String>>;

    /// Pfad des Merge-Worktrees des Zielbranches (auch wenn es ihn noch nicht gibt)
    fn merge_worktree_path(&self, root: &Path, name: &str) -> String;

    /// Öffnet den Merge-Worktree des Zielbranches
    fn open_merge_worktree(&self, root: &Path, name: &str) -> Result<String>;

    /// Schließt den Merge-Worktree; liefert die Git-Meldung, wenn er stehen bleibt
    fn close_merge_worktree(&self, root: &Path, path: &str) -> Option<String>;
}

/// Die Git-Seite des Merges, wie der Dispatcher sie im Betrieb benutzt.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealMergeGit {
    checkout: MainCheckout,
    merger: BranchMerger,
    targets: TargetBranches,
}

impl RealMergeGit {
    /// Git-Seite mit den gewöhnlichen Git-Aufrufern
    pub fn new() -> Self {
        Self {
            checkout: MainCheckout::new(),
            merger: BranchMerger::new(),
            targets: TargetBranches::new(),
        }
    }
}

impl MergeGit for RealMergeGit {
    fn inspect(&self, root: &Path) -> Result<CheckoutState> {
        self.checkout.inspect(root)
    }

    fn running(&self, root: &Path) -> Result<Option<String>> {
        Ok(self.checkout.running(root)?.map(str::to_string))
    }

    fn merge(
        &self,
        root: &Path,
        branch: &str,
        message: Option<&str>,
        strategy: MergeStrategy,
    ) -> Result<MergeOutcome> {
        self.merger.merge(root, branch, message, strategy)
    }

    fn contains(&self, root: &Path, ancestor: &str, branch: &str) -> Result<bool> {
        Ok(self.merger.contains(root, ancestor, branch))
    }

    fn merged_earlier(&self, root: &Path, branch: &str, target: &str) -> Option<String> {
        self.merger.merged_earlier(root, branch, target)
    }

    fn overlapping(&self, root: &Path, branch: &str) -> Result<Vec<String>> {
        self.merger.overlapping(root, branch)
    }

    fn ensure_target(&self, root: &Path, name: &str) -> Result<EnsuredBranch> {
        self.targets.ensure(root, name)
    }

    fn checked_out_at(&self, root: &Path, name: &str) -> Result<Option<String>> {
        self.targets.checked_out_at(root, name)
    }

    fn merge_worktree_path(&self, root: &Path, name: &str) -> String {
        self.targets.merge_worktree_path(root, name)
    }

    fn open_merge_worktree(&self, root: &Path, name: &str) -> Result<String> {
        self.targets.open_merge_worktree(root, name)
    }

    fn close_merge_worktree(&self, root: &Path, path: &str) -> Option<String> {
        self.targets.close_merge_worktree(root, path)
    }
}
