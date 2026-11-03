//! Die Dateiablage des Dispatchers: `dispatch/state.json`, die Lauf-Dateien unter
//! `dispatch/runs`, die Review-Einträge unter `dispatch/reviews` und die Protokolle unter
//! `dispatch-logs`. Namen, Ort und JSON-Form sind dieselben wie in
//! `packages/core/src/dispatch/state`, damit der Node- und der Rust-Dispatcher die Dateien des
//! anderen lesen können.

pub mod log;
pub mod log_outcome;
pub mod log_splitter;
pub mod review_store;
pub mod run_store;
pub mod state_store;
pub mod usage_history;

#[cfg(test)]
mod tests;

pub use log::{DispatchLog, RunLogEntry};
pub use log_outcome::{RunLogOutcome, RunOutcome, RunState};
pub use log_splitter::{RunLogParts, RunLogSplitter, RUN_LOG_OUTPUT_MARKER, RUN_LOG_PROMPT_MARKER};
pub use review_store::{ReviewEntry, ReviewStore};
pub use run_store::{RunEnd, RunRecord, RunStore};
pub use state_store::{
    DispatchState, DispatchStateStore, MergeEntry, MergePhase, PlanningAutoState, PlanningEntry,
    ResearchAutoState, ResearchEntry,
};
pub use usage_history::{UsageHistory, UsageSample, UsageSnapshot};
