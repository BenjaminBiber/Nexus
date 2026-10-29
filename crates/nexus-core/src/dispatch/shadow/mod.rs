//! Der Schattenbetrieb (`NEXUS_DISPATCH=shadow`): Node dispatcht und schreibt, Rust rechnet
//! jede Entscheidung und jede Auswertung nach und schreibt nur den Vergleich.
//!
//! Hier liegt alles, was ohne Server auskommt: die Zeilen (`types`), ihre Ablage (`store`),
//! der feldweise Vergleich (`compare`), der Unified-Diff der Arbeitsanweisung (`text_diff`)
//! und der Bericht (`report`), den `nexus dispatch-compare` ausgibt. Das Nachrechnen selbst
//! braucht Board und Einstellung und steht deshalb im Server.

pub mod compare;
pub mod report;
pub mod store;
pub mod text_diff;
pub mod types;

pub use compare::{at_path, diffs, text_of};
pub use report::{ShadowReport, ShadowSummary, MAX_VALUE};
pub use store::{ShadowStore, DECISIONS, EVALUATIONS, READINESS};
pub use text_diff::unified;
pub use types::{ShadowDecision, ShadowDiff, ShadowEvaluation, ShadowReadiness};
