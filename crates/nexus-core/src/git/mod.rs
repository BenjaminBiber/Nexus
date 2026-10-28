//! Git-Aufrufe und Git-Pfade. Der Rest von `packages/core/src/git` folgt in einem
//! späteren Ticket; hier steht nur, was der eingerichtete Harness-Ordner braucht.

pub mod exclude;
pub mod paths;
pub mod runner;

pub use exclude::GitExclude;
pub use paths::GitPaths;
pub use runner::{GitAttempt, GitRunner};
