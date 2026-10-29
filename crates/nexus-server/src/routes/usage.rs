//! Routengruppe `sessions`, Teil Abo-Limits: `GET /api/claude-usage`. Portiert aus
//! `packages/server/src/routes/UsageRoutes.ts`.
//!
//! Nur wenn die Einstellung „claudeUsage“ eingeschaltet ist; sonst bleibt es bei `disabled`,
//! ohne die Anmeldedatei zu lesen oder Anthropic zu fragen.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use nexus_core::settings::UserSettingsStore;

use crate::error::{json, ApiResult};
use crate::proxy::proxy_or_static;
use crate::state::AppState;
use crate::usage::ClaudeUsageDto;

/// Meldet die Route an; andere Methoden auf demselben Pfad fallen in den Rückfall, damit
/// axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/claude-usage", get(usage).fallback(proxy_or_static))
}

/// Die Auslastung der Abo-Limits, solange die Einstellung sie erlaubt
async fn usage(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    if !UserSettingsStore::new(state.home.clone())
        .read()
        .claude_usage
    {
        return Ok(json(&ClaudeUsageDto::unavailable("disabled", None)));
    }
    Ok(json(&state.usage.current().await))
}
