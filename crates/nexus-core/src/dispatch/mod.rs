//! Der Dispatcher: Vorsilben seiner Kommentare, die Git-Bausteine für den Merge, später
//! Profile und Auswahlregeln.

pub mod git;
pub mod git_bash;
pub mod markers;
pub mod merge_strategy;

pub use git_bash::{EnvLookup, GitBashLocator, ProcessEnv};
pub use merge_strategy::MergeStrategy;
