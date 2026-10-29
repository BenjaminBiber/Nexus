//! `GET /api/workflow` und `GET /api/customizations`; portiert aus
//! `packages/server/src/routes/WorkflowRoutes.ts` und `CustomizationRoutes.ts`.
//!
//! Beide Routen lesen nur, ohne eigene Fachlogik: den angewendeten Workflow (oder den
//! Standard) aus [`WorkflowSource`] und die Übersicht der Anpassungen ohne Diffs aus
//! [`OverviewFile`]. Das Lesen geschieht im Dateisystem und blockiert, deshalb läuft es in
//! `spawn_blocking`.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use nexus_core::customization::OverviewFile;
use nexus_core::workflow::WorkflowSource;
use serde_json::Value;

use crate::state::AppState;

/// Meldet die beiden Routen der Gruppe `workflow` an
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/workflow",
            get(workflow).fallback(crate::routes::static_or_not_found),
        )
        .route(
            "/api/customizations",
            get(customizations).fallback(crate::routes::static_or_not_found),
        )
}

/// Angewendeter Workflow (eigener oder Standard), für die Anleitung im Board
async fn workflow(State(state): State<Arc<AppState>>) -> Response {
    let home = state.home.clone();
    let selection = tokio::task::spawn_blocking(move || WorkflowSource::new(home).read())
        .await
        .expect("Lesen des Workflows");
    crate::error::json(&selection)
}

/// Übersicht der Anpassungen ohne Diffs; ohne Datei der Stand „unbekannt“
async fn customizations(State(state): State<Arc<AppState>>) -> Response {
    let home = state.home.clone();
    let overview = tokio::task::spawn_blocking(move || OverviewFile::new(home).read_value())
        .await
        .expect("Lesen der Übersicht");
    crate::error::json(&without_diffs(overview))
}

/// Entfernt `diff` aus jedem Eintrag, ohne die übrige Feldreihenfolge zu stören; ohne
/// Übersicht `{ state: 'unknown' }`
fn without_diffs(overview: Option<Value>) -> Value {
    let Some(mut overview) = overview else {
        return serde_json::json!({ "state": "unknown" });
    };
    if let Some(entries) = overview.get_mut("entries").and_then(Value::as_array_mut) {
        for entry in entries {
            if let Some(entry) = entry.as_object_mut() {
                entry.shift_remove("diff");
            }
        }
    }
    overview
}
