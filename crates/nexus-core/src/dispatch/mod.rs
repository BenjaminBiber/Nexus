//! Der Dispatcher: Vorsilben seiner Kommentare, die Git-Bausteine für den Merge, später
//! Profile und Auswahlregeln.

pub mod abort_reason;
pub mod git;
pub mod git_bash;
pub mod markers;
pub mod merge_strategy;

pub use abort_reason::{AbortCategory, AbortReasonClassifier, ABORT_CATEGORIES};
pub use git_bash::{EnvLookup, GitBashLocator, ProcessEnv};
pub use markers::{JoinWorktrees, MERGE_REASON_PREFIX, SETUP_REASON_PREFIX};
pub use merge_strategy::MergeStrategy;
