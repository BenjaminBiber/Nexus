//! Routengruppe `store_updates`: Theme-Updates aus dem Store erkennen und installieren.
//!
//! `GET /api/themes/store/updates` vergleicht die installierten Themes per semver mit den
//! Indizes aller Repos; nicht erreichbare Repos stehen in `errors`, die Antwort bleibt 200.
//! `POST /api/themes/store/update` mit `{ "repoId", "id", "confirm"? }` lädt und prüft das
//! neue Paket wie die Installation; bringt es Code (`main.js`) oder neue Domains mit, braucht
//! es `confirm: true` (D2), sonst 409 ohne etwas zu schreiben. Ein installiertes Theme merkt
//! sich sein Repo nicht: Updates werden über die `id` in allen Repos gesucht.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use nexus_core::theme::store_updates::{assess_update, find_updates, is_newer};
use nexus_core::themes::{install_theme, list_themes};
use nexus_core::HarnessError;
use serde_json::{json as json_value, Value};

use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::routes::static_or_not_found;
use crate::routes::store_install::{compatible_entry, download_verified, text_field};
use crate::routes::theme_packages::blocking;
use crate::state::AppState;

/// Fehlercode: Theme ist nicht installiert (HTTP 404)
pub const STORE_UPDATE_NOT_INSTALLED: &str = "store-update-not-installed";
/// Fehlercode: Index-Version ist nicht neuer als die installierte (HTTP 409)
pub const STORE_UPDATE_NOT_NEWER: &str = "store-update-not-newer";
/// Fehlercode: Update bringt Code oder neue Domains und ist nicht bestätigt (HTTP 409)
pub const STORE_UPDATE_CONFIRMATION_REQUIRED: &str = "store-update-confirmation-required";

/// Meldet die Routen an; andere Methoden fallen in den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/themes/store/updates",
            get(updates).fallback(static_or_not_found),
        )
        .route(
            "/api/themes/store/update",
            post(update).fallback(static_or_not_found),
        )
}

/// Verfügbare Updates aller Repos; Repo-Fehler landen in `errors`
async fn updates(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let home = state.home.clone();
    let installed: Vec<_> = blocking(move || list_themes(&home))
        .await?
        .into_iter()
        .map(|theme| theme.manifest)
        .collect();
    let loaded = state.store_index.load_all(state.store_repos.list()).await;
    let entries: Vec<_> = loaded
        .entries
        .into_iter()
        .map(|entry| (entry.repo_id, entry.entry))
        .collect();
    let errors: Vec<Value> = loaded
        .repos
        .into_iter()
        .filter_map(|repo| {
            repo.error.map(|error| {
                json_value!({ "repoId": repo.id, "code": error.code, "message": error.message })
            })
        })
        .collect();
    Ok(json(&json_value!({
        "updates": find_updates(&installed, &entries),
        "errors": errors,
    })))
}

/// Installiert das Update eines installierten Themes aus dem angegebenen Repo
async fn update(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let repo_id = text_field(&body, "repoId")?;
    let id = text_field(&body, "id")?;
    let confirm = body.get("confirm").and_then(Value::as_bool) == Some(true);
    let home = state.home.clone();
    let installed = blocking(move || list_themes(&home))
        .await?
        .into_iter()
        .map(|theme| theme.manifest)
        .find(|manifest| manifest.id == id)
        .ok_or_else(|| {
            HarnessError::new(
                STORE_UPDATE_NOT_INSTALLED,
                format!("Das Theme {id} ist nicht installiert."),
            )
        })?;
    let (repo, entry) = compatible_entry(&state, &repo_id, &id).await?;
    if !is_newer(&installed.version, &entry.version) {
        return Err(HarnessError::new(
            STORE_UPDATE_NOT_NEWER,
            format!(
                "Im Store liegt keine neuere Version von {id}: installiert {}, angeboten {}.",
                installed.version, entry.version
            ),
        )
        .into());
    }
    let (bytes, verification) = download_verified(&state, &repo, &entry).await?;
    let risk = assess_update(&installed, &bytes)?;
    if risk.needs_confirmation() && !confirm {
        let mut error = HarnessError::new(
            STORE_UPDATE_CONFIRMATION_REQUIRED,
            "Das Update bringt Code oder neue Domains mit und muss bestätigt werden.",
        );
        error.details = Some(json_value!({
            "hasCode": risk.has_code,
            "newDomains": risk.new_domains,
        }));
        return Err(error.into());
    }
    let home = state.home.clone();
    let manifest = blocking(move || install_theme(&home, &bytes)).await?;
    Ok(json(&json_value!({
        "manifest": manifest,
        "verified": verification.verified,
        "repoId": repo.id,
        "sha256": verification.sha256,
        "previousVersion": installed.version,
    })))
}
