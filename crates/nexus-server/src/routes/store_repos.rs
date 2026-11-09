//! Routengruppe `store_repos`: die vom Nutzer eingetragenen Store-Repos auflisten, eintragen
//! und entfernen.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::Router;
use nexus_core::theme::store_repos::{STORE_REPO_INVALID, STORE_REPO_KEY_INVALID};
use nexus_core::HarnessError;
use serde_json::Value;

use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Meldet die Routen an; andere Methoden fallen in den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/themes/store/repos",
            get(list).post(add).fallback(static_or_not_found),
        )
        .route(
            "/api/themes/store/repos/{id}",
            delete(remove).fallback(static_or_not_found),
        )
}

/// Alle eingetragenen Repos
async fn list(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&state.store_repos.list()))
}

/// Trägt ein Repo ein (201); Körper `{url, publicKey?}`
async fn add(State(state): State<Arc<AppState>>, JsonBody(body): JsonBody) -> ApiResult<Response> {
    let Some(url) = body.get("url").and_then(Value::as_str) else {
        return Err(HarnessError::new(STORE_REPO_INVALID, "url muss ein Text sein.").into());
    };
    let public_key = match body.get("publicKey") {
        None | Some(Value::Null) => None,
        Some(Value::String(key)) => Some(key.as_str()),
        Some(_) => {
            return Err(
                HarnessError::new(STORE_REPO_KEY_INVALID, "publicKey muss ein Text sein.").into(),
            )
        }
    };
    let repo = state.store_repos.add(url, public_key)?;
    Ok((StatusCode::CREATED, json(&repo)).into_response())
}

/// Entfernt das Repo
async fn remove(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    state.store_repos.remove(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
