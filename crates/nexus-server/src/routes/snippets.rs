//! Routengruppe `snippets`: CSS-Schnipsel des Nutzers auflisten, anlegen/ändern,
//! an-/ausschalten, löschen und die aktiven als ein Stylesheet ausliefern.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::Router;
use nexus_core::HarnessError;
use serde_json::Value;

use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Meldet die Routen an; andere Methoden fallen in den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/snippets", get(list).fallback(static_or_not_found))
        .route(
            "/api/snippets/{id}",
            put(save)
                .patch(toggle)
                .delete(remove)
                .fallback(static_or_not_found),
        )
        .route(
            "/snippets/active.css",
            get(active).fallback(static_or_not_found),
        )
}

/// Alle Schnipsel mit Inhalt
async fn list(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&state.snippets.list()))
}

/// Legt an (201) oder ändert (200); Körper `{name, css, enabled?}`
async fn save(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let name = text(&body, "name")?;
    let css = text(&body, "css")?;
    let enabled = match body.get("enabled") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(enabled)) => Some(*enabled),
        Some(_) => return Err(invalid("enabled muss true oder false sein.").into()),
    };
    let (created, snippet) = state.snippets.put(&id, name, css, enabled)?;
    let status = match created {
        true => StatusCode::CREATED,
        false => StatusCode::OK,
    };
    Ok((status, json(&snippet)).into_response())
}

/// Schaltet an oder aus; Körper `{enabled}`
async fn toggle(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let Some(enabled) = body.get("enabled").and_then(Value::as_bool) else {
        return Err(invalid("enabled muss true oder false sein.").into());
    };
    Ok(json(&state.snippets.set_enabled(&id, enabled)?))
}

/// Löscht den Schnipsel
async fn remove(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    state.snippets.delete(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Die aktiven Schnipsel als Stylesheet, nie zwischengespeichert
async fn active(State(state): State<Arc<AppState>>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        state.snippets.active_css(),
    )
        .into_response()
}

/// Pflichtfeld als Text
fn text<'a>(body: &'a Value, key: &str) -> ApiResult<&'a str> {
    body.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{key} muss ein Text sein.")).into())
}

fn invalid(message: impl Into<String>) -> HarnessError {
    HarnessError::new(nexus_core::theme::snippet_store::SNIPPET_INVALID, message)
}
