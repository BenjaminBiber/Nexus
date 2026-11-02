//! Routengruppe `sessions`, Teil Abo-Limits: `GET /api/claude-usage` und
//! `GET /api/claude-usage/accounts`. Portiert aus
//! `packages/server/src/routes/UsageRoutes.ts`.
//!
//! Nur wenn die Einstellung „claudeUsage“ eingeschaltet ist; sonst bleibt es bei `disabled`,
//! ohne die Anmeldedatei zu lesen oder Anthropic zu fragen.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use nexus_core::dispatch::DispatchConfigStore;
use nexus_core::settings::UserSettingsStore;
use serde::Serialize;

use crate::accounts::AccountUsageEntryDto;
use crate::error::{json, ApiResult};
use crate::routes::static_or_not_found;
use crate::state::AppState;
use crate::usage::ClaudeUsageDto;

/// Vorgabe, ab der ein Account als voll gilt, wenn die Einstellung fehlt
const DEFAULT_LIMIT: i64 = 90;

/// Antwort von `GET /api/claude-usage/accounts`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageOverviewDto {
    /// false: Einstellung „claudeUsage“ aus, dann ist `accounts` leer
    pub enabled: bool,
    /// Vorgabe des Dispatchers, ab der ein Account als voll gilt (Prozent)
    pub default_limit: i64,
    pub accounts: Vec<AccountUsageEntryDto>,
}

/// Meldet die Routen an; andere Methoden auf denselben Pfaden fallen in den Rückfall, damit
/// axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/claude-usage",
            get(usage).fallback(static_or_not_found),
        )
        .route(
            "/api/claude-usage/accounts",
            get(overview).fallback(static_or_not_found),
        )
}

/// Die Auslastung der Abo-Limits, solange die Einstellung sie erlaubt
async fn usage(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    if !enabled(&state) {
        return Ok(json(&ClaudeUsageDto::unavailable("disabled", None)));
    }
    Ok(json(&state.accounts.usage.current().await))
}

/// Die Auslastung aller aktiven Accounts samt Obergrenzen (Ringe in der Seitenleiste)
async fn overview(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let default_limit = DispatchConfigStore::new(state.home.clone())
        .read()
        .map(|read| read.config.account_limit_percent)
        .unwrap_or(DEFAULT_LIMIT);
    if !enabled(&state) {
        return Ok(json(&UsageOverviewDto {
            enabled: false,
            default_limit,
            accounts: Vec::new(),
        }));
    }
    Ok(json(&UsageOverviewDto {
        enabled: true,
        default_limit,
        accounts: state.accounts.usage.overview().await,
    }))
}

/// Erlaubt die Einstellung „claudeUsage“ die Abfrage?
fn enabled(state: &Arc<AppState>) -> bool {
    UserSettingsStore::new(state.home.clone())
        .read()
        .claude_usage
}
