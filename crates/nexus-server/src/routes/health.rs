//! `GET /api/health`; portiert aus `packages/server/src/routes/HealthRoutes.ts`.
//!
//! Der Name bleibt `ai-harness`: CLI, `ServerStopper` und die App prüfen daran, dass der
//! Server unter dem Port wirklich der eigene ist.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::state::AppState;

/// Name, den der Server von sich meldet
pub const SERVER_NAME: &str = "ai-harness";

/// Meldet die Route an. Der Rückfall der Methoden gehört dazu: hat eine Rust-Route den Pfad,
/// aber nicht die Methode, darf axum kein 405 liefern – Fastify antwortet dort mit 404 bzw.
/// der Node-Server selbst.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/health",
        get(health).fallback(crate::proxy::proxy_or_static),
    )
}

/// Name, Version und Prozessnummer des laufenden Servers
async fn health(State(state): State<Arc<AppState>>) -> Response {
    crate::error::json(&serde_json::json!({
        "name": SERVER_NAME,
        "version": state.config.version,
        "pid": state.pid,
    }))
}
