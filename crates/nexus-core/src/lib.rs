//! Fachkern von Nexus: Typen und Regeln des Boards, unabhängig von Datenbank,
//! HTTP-Schicht und Oberfläche. Die Grundtypen sind 1:1 aus `packages/core` portiert,
//! samt Fehlercodes und deutschen Meldungen.

pub mod actors;
pub mod board;
pub mod clock;
pub mod counter;
pub mod customization;
pub mod dispatch;
pub mod error;
pub mod git;
pub mod install;
pub mod locking;
pub mod process;
pub mod project;
pub mod questions;
pub mod registry;
pub mod sessions;
pub mod settings;
// Auch in den eigenen Tests gebraucht: die Dienste des Boards prüfen gegen `BoardFixture`
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod text;
pub mod workflow;
pub mod worktrees;

mod random;

#[cfg(test)]
mod test_support;

pub use actors::{Actor, AuthorKind, QuestionActor};
pub use clock::Clock;
pub use error::{Error, HarnessError, Result};
pub use project::HarnessProject;
pub use registry::HarnessHome;

/// Name des Crates, wie er in Protokollen und Diagnoseausgaben erscheint.
pub const CRATE_NAME: &str = "nexus-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_ist_gesetzt() {
        assert_eq!(CRATE_NAME, "nexus-core");
    }
}
