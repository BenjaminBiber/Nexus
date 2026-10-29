//! Optimierungs-Vorschläge zur Dispatcher-Einstellung: Kennzahlen aus den Läufen aller
//! Projekte, fester Regelkatalog, Übernehmen einer Änderung und der Arbeitsordner der
//! Claude-Analyse. Portiert aus `packages/core/src/dispatch/optimization/`.

pub mod advisor;
pub mod config_patcher;
pub mod facts;
pub mod state_store;
pub mod types;
pub mod workspace;

pub use advisor::{suggestion_id, OptimizationAdvisor};
pub use config_patcher::ConfigPatcher;
pub use facts::{sorted_json, FactsInput, OptimizationFactsCollector};
pub use state_store::{StoredAnalysis, SuggestionState, SuggestionStateStore};
pub use types::{
    AdvisorResult, ConfigPatchOp, EscalationPath, InsufficientData, OptimizationBasis,
    OptimizationFacts, ProfileFacts, RuleFacts, StartSource, Suggestion, SuggestionEvidence,
    SuggestionKind, SuggestionSource, SuggestionTarget, MIN_RUNS,
};
pub use workspace::AnalysisWorkspace;
