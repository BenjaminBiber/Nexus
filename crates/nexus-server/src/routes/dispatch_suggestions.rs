//! Routengruppe `/api/dispatch/suggestions`: Optimierungs-Vorschläge zur
//! Dispatcher-Einstellung – Liste, Übernehmen, Aus- und Einblenden, Claude-Analyse.
//! Portiert aus `packages/server/src/routes/DispatchSuggestionRoutes.ts`.
//!
//! Die Routen gelten wie die übrigen Dispatch-Routen nur mit `NEXUS_DISPATCH=rust`; bei
//! `node` beantwortet sie der Sidecar.

use std::sync::Arc;

use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::routes::dispatch::config_view;
use crate::routes::static_or_not_found;
use crate::state::AppState;
use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;

/// Meldet die Routen der Gruppe an. Der Rückfall je Pfad gehört dazu: hat eine Rust-Route den
/// Pfad, aber nicht die Methode, darf axum kein 405 liefern.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/dispatch/suggestions",
            get(list).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/suggestions/{id}/apply",
            post(apply).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/suggestions/{id}/dismiss",
            post(dismiss)
                .delete(undismiss)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/suggestions/analyze",
            post(analyze).fallback(static_or_not_found),
        )
}

/// `GET /api/dispatch/suggestions`
async fn list(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&state.suggestions.list().await?))
}

/// `POST /api/dispatch/suggestions/{id}/apply`: Einstellung samt neuer Liste
async fn apply(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let applied = state.suggestions.apply(&id, body.get("configHash")).await?;
    Ok(json(&serde_json::json!({
        "view": config_view(&state)?,
        "list": applied.list,
    })))
}

/// `POST /api/dispatch/suggestions/{id}/dismiss`: ausblenden
async fn dismiss(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    Ok(json(&state.suggestions.dismiss(&id, true).await?))
}

/// `DELETE /api/dispatch/suggestions/{id}/dismiss`: wieder einblenden
async fn undismiss(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    Ok(json(&state.suggestions.dismiss(&id, false).await?))
}

/// `POST /api/dispatch/suggestions/analyze`: die Claude-Analyse
async fn analyze(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&state.suggestion_analyzer.analyze().await?))
}
