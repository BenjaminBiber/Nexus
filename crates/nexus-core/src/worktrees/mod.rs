//! Branch und Worktree je Ticket: anlegen, beitreten, abschließen, aufräumen und das
//! Verzeichnis der Worktrees im Git-Verzeichnis des Projekts.
//! Spiegelt `packages/core/src/worktrees`.

pub mod cleaner;
pub mod links;
pub mod manager;
pub mod registry;
pub mod touches;

pub use cleaner::WorktreeCleaner;
pub use links::{LinkRemoval, WorktreeLinks};
pub use manager::{
    FinishResult, RemoveMergedResult, RepoBoard, ResumeResult, StartOptions, StartResult,
    WorktreeManager, WorktreeOptions,
};
pub use registry::{SetupState, WorktreeEntry, WorktreeMap, WorktreeRegistry};
pub use touches::TouchesChecker;
