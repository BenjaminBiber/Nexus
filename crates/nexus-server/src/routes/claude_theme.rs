//! Routengruppe `claude_theme`: Zustand des Claude-Themes in Claude Desktop.
//!
//! `GET /api/claude-theme` liefert [`ClaudeThemeStatus`]: Schalter, Zustand
//! (`active`/`none`/`foreign`/`error`), aktives Theme, Pfad des Slots und Meldung. Vor dem ersten
//! Abgleich des Servers wird abgeglichen.
//!
//! [`ClaudeThemeStatus`]: nexus_core::themes::ClaudeThemeStatus

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::error::json;
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Meldet die Route an; andere Methoden fallen in den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/claude-theme",
        get(status).fallback(static_or_not_found),
    )
}

/// Zustand des Claude-Themes
async fn status(State(state): State<Arc<AppState>>) -> Response {
    json(&state.claude_theme.status().await)
}
