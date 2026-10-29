//! Der Dispatcher: Vorsilben seiner Kommentare, die Git-Bausteine für den Merge, die
//! Einstellungen, die schon die Worktrees brauchen, später Profile und Auswahlregeln.

pub mod config;
pub mod git;
pub mod git_bash;
pub mod markers;
pub mod merge_strategy;

pub use config::{JoinWorktrees, JOIN_WORKTREES, MERGE_REASON_PREFIX};
pub use git_bash::{EnvLookup, GitBashLocator, ProcessEnv};
pub use merge_strategy::MergeStrategy;
