//! Git-Bausteine des Dispatchers: Zielbranches mit Merge-Worktree, der Zustand des
//! Hauptordners und der Merge eines Ticket-Branches.

pub mod branch_merger;
pub mod main_checkout;
pub mod target_branches;

pub use branch_merger::{BranchMerger, MergeOutcome, SQUASH_TRAILER};
pub use main_checkout::{CheckoutState, MainCheckout};
pub use target_branches::{EnsuredBranch, TargetBranches};
