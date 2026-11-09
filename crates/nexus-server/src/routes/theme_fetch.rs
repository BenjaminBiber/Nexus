//! Routengruppe `theme_fetch`: `GET /api/themes/{id}/fetch?url=…[&sha256=…]` lädt eine
//! fremde Ressource für ein installiertes Theme über [`crate::theme_proxy`] (D7).

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use nexus_core::themes::{is_valid_theme_id, parse_manifest};
use serde::Deserialize;

use crate::error::{not_found_response, ApiError};
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Kopf, der sagt, ob die Antwort aus dem Cache des Themes kommt
pub const CACHE_HEADER: &str = "x-nexus-cache";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/themes/{id}/fetch",
        get(fetch).fallback(static_or_not_found),
    )
}

#[derive(Debug, Deserialize)]
struct FetchQuery {
    url: Option<String>,
    sha256: Option<String>,
}

/// Lädt die Ressource; ohne gültiges, installiertes Theme 404
async fn fetch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<FetchQuery>,
) -> Response {
    if !is_valid_theme_id(&id) {
        return not_found_response();
    }
    let theme_dir = state.home.themes_dir().join(&id);
    let Ok(text) = tokio::fs::read_to_string(theme_dir.join("manifest.json")).await else {
        return not_found_response();
    };
    let Ok(manifest) = parse_manifest(&text) else {
        return not_found_response();
    };
    let Some(url) = query.url.filter(|url| !url.is_empty()) else {
        return ApiError::new("bad-request", "url fehlt.").into_response();
    };
    match state
        .theme_proxy
        .fetch(
            &theme_dir,
            &manifest.permissions.network,
            &url,
            query.sha256.as_deref(),
        )
        .await
    {
        Ok(fetched) => (
            [
                (header::CONTENT_TYPE.as_str(), fetched.content_type.as_str()),
                (header::X_CONTENT_TYPE_OPTIONS.as_str(), "nosniff"),
                (header::CACHE_CONTROL.as_str(), "private, max-age=86400"),
                (
                    CACHE_HEADER,
                    if fetched.from_cache { "hit" } else { "miss" },
                ),
            ],
            fetched.bytes,
        )
            .into_response(),
        Err(error) => ApiError::from(error).into_response(),
    }
}
