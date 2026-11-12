//! Routengruppe `theme_settings`: eigene Einstellungen eines Themes (`settingsSchema`) lesen
//! und speichern, je Theme in `theme-settings.json`.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use nexus_core::themes::THEME_SETTINGS_INVALID;
use nexus_core::HarnessError;
use serde_json::Value;

use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Meldet die Routen an; andere Methoden fallen in den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/themes/{id}/settings",
        get(read).put(save).fallback(static_or_not_found),
    )
}

/// Gespeicherte Werte des Themes; `{}`, wenn nichts gespeichert ist
async fn read(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    Ok(json(&state.theme_settings.get(&id)?))
}

/// Ersetzt die Werte des Themes; Körper `{key: bool|string}`
async fn save(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let Value::Object(values) = body else {
        return Err(HarnessError::new(
            THEME_SETTINGS_INVALID,
            "Die Einstellungen müssen ein Objekt sein.",
        )
        .into());
    };
    Ok(json(&state.theme_settings.put(&id, values)?))
}
