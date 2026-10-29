//! Git-Bausteine des Dispatchers: Zielbranches mit Merge-Worktree, der Zustand des
//! Hauptordners, der Merge eines Ticket-Branches und der Diff für das Review.

pub mod branch_merger;
pub mod main_checkout;
pub mod review_diff;
pub mod target_branches;

pub use branch_merger::{BranchMerger, MergeOutcome, SQUASH_TRAILER};
pub use main_checkout::{CheckoutState, MainCheckout};
pub use review_diff::{ReviewDiff, ReviewFile, ReviewFileList, ReviewPatch};
pub use target_branches::{EnsuredBranch, TargetBranches};
