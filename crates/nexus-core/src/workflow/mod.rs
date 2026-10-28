//! Der Ablauf des Harness als reine Daten. Portiert aus `packages/core/src/workflow`.
//! Die Anleitung im Board, der Hinweis an den Agenten und die Prüfung eines eigenen
//! Workflows lesen alle hieraus.

pub mod phases;

pub use phases::{workflow_phases, WorkflowOption, WorkflowPhase, WorkflowStep};
