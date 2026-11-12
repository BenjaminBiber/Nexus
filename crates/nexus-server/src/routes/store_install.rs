//! Routengruppe `store_install`: ein Theme aus dem Store herunterladen, prüfen und
//! installieren.
//!
//! `POST /api/themes/store/install` mit `{ "repoId", "id" }`: Eintrag im Index des Repos
//! suchen, Paket laden, sha256 und optionale Signatur prüfen, Manifest abgleichen, dann über
//! `install_theme` installieren. Antwort `{ manifest, verified, repoId, sha256 }`.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use nexus_core::theme::store_install::{check_manifest_matches, verify_package, Verification};
use nexus_core::theme::{StoreIndexEntry, StoreRepo};
use nexus_core::themes::install_theme;
use nexus_core::HarnessError;
use serde_json::{json as json_value, Value};

use crate::error::{json, ApiError, ApiResult};
use crate::extract::JsonBody;
use crate::routes::static_or_not_found;
use crate::routes::theme_packages::blocking;
use crate::state::AppState;

/// Fehlercode: id steht nicht im Index des Repos (HTTP 404)
pub const STORE_ENTRY_NOT_FOUND: &str = "store-entry-not-found";
/// Fehlercode: Eintrag passt nicht zur SDK-Version (HTTP 409)
pub const STORE_ENTRY_INCOMPATIBLE: &str = "store-entry-incompatible";

/// Meldet die Route an; andere Methoden fallen in den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/themes/store/install",
        post(install).fallback(static_or_not_found),
    )
}

/// Lädt, prüft und installiert einen Store-Eintrag
async fn install(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let repo_id = text_field(&body, "repoId")?;
    let id = text_field(&body, "id")?;
    let (repo, entry) = compatible_entry(&state, &repo_id, &id).await?;
    let (bytes, verification) = download_verified(&state, &repo, &entry).await?;
    let home = state.home.clone();
    let manifest = blocking(move || install_theme(&home, &bytes)).await?;
    Ok(json(&json_value!({
        "manifest": manifest,
        "verified": verification.verified,
        "repoId": repo.id,
        "sha256": verification.sha256,
    })))
}

/// Repo und kompatiblen Eintrag `id` aus seinem Index; sonst 404/409/502
pub(crate) async fn compatible_entry(
    state: &AppState,
    repo_id: &str,
    id: &str,
) -> ApiResult<(StoreRepo, StoreIndexEntry)> {
    let repo = state.store_repos.get(repo_id)?;
    let index = state.store_index.load_repo(&repo).await?;
    let entry = index
        .entries
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| {
            HarnessError::new(
                STORE_ENTRY_NOT_FOUND,
                format!("Das Theme {id} steht nicht im Index dieses Repos."),
            )
        })?;
    if !entry.compatible {
        return Err(HarnessError::new(
            STORE_ENTRY_INCOMPATIBLE,
            format!(
                "Das Theme {id} braucht SDK {} und passt nicht zu dieser Nexus-Version.",
                entry.sdk_version
            ),
        )
        .into());
    }
    Ok((repo, entry))
}

/// Lädt das Paket und prüft sha256, Signatur und Manifest; nichts landet auf der Platte
pub(crate) async fn download_verified(
    state: &AppState,
    repo: &StoreRepo,
    entry: &StoreIndexEntry,
) -> ApiResult<(Vec<u8>, Verification)> {
    let bytes = state.store_download.download(&entry.download_url).await?;
    let verification = verify_package(&bytes, entry, repo.public_key.as_deref())?;
    check_manifest_matches(&bytes, entry)?;
    Ok((bytes, verification))
}

/// Pflichtfeld als nicht leerer Text; sonst 400
pub(crate) fn text_field(body: &Value, key: &str) -> ApiResult<String> {
    body.get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::new("bad-request", format!("{key} muss ein Text sein.")))
}
