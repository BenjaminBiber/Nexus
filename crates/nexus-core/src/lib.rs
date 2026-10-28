//! Fachkern von Nexus: Typen und Regeln des Boards, unabhängig von Datenbank,
//! HTTP-Schicht und Oberfläche. Die Grundtypen sind 1:1 aus `packages/core` portiert,
//! samt Fehlercodes und deutschen Meldungen.

pub mod actors;
pub mod board;
pub mod error;
pub mod questions;
pub mod text;

pub use actors::{Actor, AuthorKind, QuestionActor};
pub use error::{Error, HarnessError, Result};

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
