//! Portiert den Typ `MergeStrategy` aus `packages/core/src/dispatch/DispatchConfig.ts`;
//! der Rest der Dispatch-Konfiguration folgt in einem späteren Ticket.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Wie ein Ticket-Branch in seinen Zielbranch kommt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "dispatch.ts")]
pub enum MergeStrategy {
    /// Eigener Merge-Commit (`git merge --no-ff`)
    #[default]
    Merge,
    /// Alle Commits des Branches in einem Commit (`git merge --squash`)
    Squash,
}

impl MergeStrategy {
    /// Alle Strategien in der Reihenfolge von TypeScript (`MERGE_STRATEGIES`)
    pub const ALL: [MergeStrategy; 2] = [MergeStrategy::Merge, MergeStrategy::Squash];

    /// Schreibweise in JSON und in der Konfiguration
    pub fn as_str(&self) -> &'static str {
        match self {
            MergeStrategy::Merge => "merge",
            MergeStrategy::Squash => "squash",
        }
    }
}
