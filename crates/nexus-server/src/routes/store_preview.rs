//! Routengruppe `store_preview`: `GET /api/themes/store/preview?repoId=…&id=…` liefert das
//! Vorschaubild eines Store-Eintrags.
//!
//! Die CSP der Oberfläche lässt keine Bilder fremder Hosts zu (`img-src`), die Vorschau eines
//! Eintrags liegt aber beim Store (z. B. als Release-Datei auf GitHub). Der Server holt sie
//! deshalb selbst ab: nur von der Adresse, die im Index des eingetragenen Repos steht, mit
//! Zeit- und Größenlimit, und er reicht sie nur durch, wenn der Inhalt ein Bild ist.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use nexus_core::theme::StoreRepo;
use nexus_core::HarnessError;
use serde::Deserialize;

use crate::error::{ApiError, ApiResult};
use crate::routes::static_or_not_found;
use crate::routes::store_install::STORE_ENTRY_NOT_FOUND;
use crate::state::AppState;

/// Fehlercode: der Eintrag hat kein Vorschaubild (HTTP 404)
pub const STORE_PREVIEW_NOT_FOUND: &str = "store-preview-not-found";
/// Fehlercode: unter der Vorschau-Adresse liegt kein Bild (HTTP 502)
pub const STORE_PREVIEW_INVALID: &str = "store-preview-invalid";

/// Meldet die Route an; andere Methoden fallen in den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/themes/store/preview",
        get(preview).fallback(static_or_not_found),
    )
}

/// `v` (Version des Eintrags) hängt die Oberfläche nur an, damit der Browser nach einem Update
/// neu lädt; der Server braucht es nicht
#[derive(Debug, Deserialize)]
struct PreviewQuery {
    #[serde(rename = "repoId")]
    repo_id: Option<String>,
    id: Option<String>,
}

/// Holt das Vorschaubild beim Store und reicht es durch
async fn preview(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PreviewQuery>,
) -> ApiResult<Response> {
    let repo_id = required(query.repo_id, "repoId")?;
    let id = required(query.id, "id")?;
    let repo = state.store_repos.get(&repo_id)?;
    let url = preview_url(&state, &repo, &id).await?;
    let bytes = state.store_download.download_preview(&url).await?;
    let content_type = image_type(&bytes).ok_or_else(|| {
        HarnessError::new(
            STORE_PREVIEW_INVALID,
            "Unter der Vorschau-Adresse des Themes liegt kein Bild.",
        )
    })?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::CACHE_CONTROL, "private, max-age=86400"),
        ],
        bytes,
    )
        .into_response())
}

/// Vorschau-Adresse des Eintrags `id`: aus dem zuletzt geladenen Index des Repos, sonst wird
/// er einmal geholt. Eintrag unbekannt → `store-entry-not-found`, ohne Vorschau →
/// `store-preview-not-found`
async fn preview_url(state: &AppState, repo: &StoreRepo, id: &str) -> ApiResult<String> {
    let preview = match state.store_index.remembered_preview(&repo.id, id) {
        Some(preview) => preview,
        None => {
            state
                .store_index
                .load_repo(repo)
                .await?
                .entries
                .into_iter()
                .find(|entry| entry.id == id)
                .ok_or_else(|| {
                    HarnessError::new(
                        STORE_ENTRY_NOT_FOUND,
                        format!("Das Theme {id} steht nicht im Index dieses Repos."),
                    )
                })?
                .preview
        }
    };
    preview.ok_or_else(|| {
        HarnessError::new(
            STORE_PREVIEW_NOT_FOUND,
            format!("Das Theme {id} hat kein Vorschaubild."),
        )
        .into()
    })
}

/// Pflichtangabe der Adresse als nicht leerer Text; sonst 400
fn required(value: Option<String>, key: &str) -> ApiResult<String> {
    value
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| ApiError::new("bad-request", format!("{key} fehlt.")))
}

/// Inhaltstyp eines Bilds nach seinen ersten Bytes; alles andere (auch SVG, das Skripte tragen
/// kann) ist kein Vorschaubild
fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]) {
        return Some("image/webp");
    }
    if bytes.get(4..12) == Some(&b"ftypavif"[..]) {
        return Some("image/avif");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erkennt_bilder_an_den_ersten_bytes() {
        assert_eq!(image_type(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(image_type(b"\xff\xd8\xff\xe0...."), Some("image/jpeg"));
        assert_eq!(image_type(b"GIF87a...."), Some("image/gif"));
        assert_eq!(image_type(b"GIF89a...."), Some("image/gif"));
        assert_eq!(
            image_type(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
            Some("image/webp")
        );
        assert_eq!(
            image_type(b"\x00\x00\x00\x1cftypavif\x00\x00"),
            Some("image/avif")
        );
    }

    #[test]
    fn alles_andere_ist_kein_bild() {
        assert_eq!(image_type(b""), None);
        assert_eq!(image_type(b"RIFF\x00\x00\x00\x00WAVEfmt "), None);
        assert_eq!(
            image_type(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            None
        );
        assert_eq!(image_type(b"<!doctype html>"), None);
        assert_eq!(image_type(b"PK\x03\x04"), None);
    }
}
