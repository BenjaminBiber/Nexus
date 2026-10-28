//! Git-Aufrufe, Git-Pfade und die Git-Hilfen des Harness: lange Pfade unter Windows,
//! vergleichbare Pfadschlüssel, Branch-Namen und die Commits eines Branches.
//! Die Worktrees folgen in einem späteren Ticket.

pub mod branch_commit_reader;
pub mod branch_names;
pub mod exclude;
pub mod long_paths;
pub mod path_key;
pub mod paths;
pub mod runner;
pub mod target_branch_rule;

pub use branch_commit_reader::{
    BranchCommit, BranchCommitReader, BranchCommitSource, BranchCommits, BranchCommitsStatus,
};
pub use branch_names::BranchNames;
pub use exclude::GitExclude;
pub use long_paths::{LongPaths, LONG_PATHS_ARGS, LONG_PATHS_NOTICE};
pub use path_key::PathKey;
pub use paths::GitPaths;
pub use runner::{GitAttempt, GitCommands, GitRunner};
pub use target_branch_rule::TargetBranchRule;
