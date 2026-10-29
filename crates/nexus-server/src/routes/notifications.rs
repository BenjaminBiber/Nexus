//! Routengruppe `notifications`: die beiden Testmeldungen der Benutzereinstellungen;
//! portiert aus `packages/server/src/routes/UserSettingsRoutes.ts`.
//!
//! `/api/user-settings` selbst gehört zur Gruppe `settings`; hier stehen nur die beiden
//! Unterpfade, damit der Notschalter `NEXUS_PROXY_GROUPS=notifications` genau sie
//! zurück zum Node-Server schiebt.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::post;
use axum::Router;

use crate::error::{json, ApiResult};
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Meldet die beiden Routen der Gruppe an; andere Methoden auf denselben Pfaden fallen in
/// den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/user-settings/test-notification",
            post(test_notification).fallback(static_or_not_found),
        )
        .route(
            "/api/user-settings/test-ntfy",
            post(test_ntfy).fallback(static_or_not_found),
        )
}

/// Zeigt eine Beispielmeldung: im offenen Tab, sonst als Systemmeldung
async fn test_notification(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&result(state.notifications.send_test().await)))
}

/// Schickt die Beispielmeldung als Push über ntfy
async fn test_ntfy(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&result(state.notifications.send_ntfy_test().await)))
}

/// Antwort beider Routen: `{ ok, error }` mit `ok = error == null`
fn result(error: Option<String>) -> serde_json::Value {
    serde_json::json!({ "ok": error.is_none(), "error": error })
}
