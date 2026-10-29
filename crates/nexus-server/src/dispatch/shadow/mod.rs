//! Der Schattenbetrieb (`NEXUS_DISPATCH=shadow`): Node dispatcht und schreibt, Rust rechnet
//! nach und vergleicht.
//!
//! [`service::ShadowService`] hält die Teile zusammen, [`watcher::ShadowWatcher`] meldet neue
//! Lauf-Dateien, [`decision::ShadowDecisionMaker`] rechnet die Entscheidung eines Starts nach,
//! [`evaluation::ShadowEvaluator`] die Auswertung eines Ergebnisses und
//! [`readiness::ShadowReadinessCheck`] gleicht die Bereitschaft mit dem Sidecar ab. Die
//! Zeilen, ihre Ablage und der Bericht liegen im Fachkern (`nexus_core::dispatch::shadow`).

pub mod decision;
pub mod evaluation;
pub mod readiness;
pub mod service;
pub mod watcher;

pub use decision::ShadowDecisionMaker;
pub use evaluation::{EvaluationStatus, ShadowEvaluator, WAIT_MINUTES};
pub use readiness::{HttpSidecarStatus, ShadowReadinessCheck, SidecarStatus, TIMING_MINUTES};
pub use service::{ShadowDeps, ShadowService};
pub use watcher::ShadowWatcher;
