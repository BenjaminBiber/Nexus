//! Routengruppe `store_index`: die zusammengeführten Index-Dateien aller Store-Repos.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::error::{json, ApiResult};
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Meldet die Route an; andere Methoden fallen in den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/themes/store/index",
        get(index).fallback(static_or_not_found),
    )
}

/// Lädt alle Repos neu; Fehler einzelner Repos stehen im Körper, die Route liefert immer 200
async fn index(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let repos = state.store_repos.list();
    Ok(json(&state.store_index.load_all(repos).await))
}
