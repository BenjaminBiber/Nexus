//! Portiert `packages/core/src/customization/WorkflowTools.ts`.

use std::path::Path;

use serde_json::Value;

use crate::error::Result;

use super::overview::Problem;

/// Werkzeuge zum Prüfen und Laden von `custom/workflow.ts`; austauschbar, damit Tests ohne
/// tsc und esbuild auskommen.
pub trait WorkflowTools {
    /// Typfehler der Datei gegen `WorkflowPhase[]`; leer, wenn alles passt. Ein Fehler
    /// bedeutet, dass die Prüfung selbst nicht lief (Rückfall mit `check-failed`).
    fn typecheck(&self, file: &Path) -> Result<Vec<Problem>>;

    /// Default-Export der Datei als reine Daten; die Meldung eines Fehlers erscheint als
    /// `load-failed`
    fn load(&self, file: &Path) -> std::result::Result<Value, String>;
}
