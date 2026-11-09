//! Routengruppe `themes`: installierte Theme-Pakete auflisten, hochladen und entfernen.
//!
//! * `GET /api/themes` – Manifeste der installierten Themes, nach id sortiert
//! * `POST /api/themes/install` – Körper `{ "data": "<Base64 des .nexus-Pakets>" }`, Antwort:
//!   das Manifest. Der Schutz lässt schreibende Anfragen nur als JSON durch, deshalb Base64.
//! * `DELETE /api/themes/{id}` – entfernt ein Theme, Antwort 204
//!
//! Die Prüfung und das Entpacken stecken in `nexus_core::themes`; hier werden nur deren Fehler
//! in Antworten übersetzt.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::Router;
use base64::Engine;
use nexus_core::themes::{
    install_theme, list_themes, remove_theme, ThemeManifest, ThemePackageError,
};
use nexus_core::{Error, HarnessError};

use crate::error::{json, ApiError, ApiResult};
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Größte Anfrage beim Hochladen: das größte Paket als Base64 plus Luft für das JSON
const MAX_INSTALL_BODY: usize =
    nexus_core::themes::package::MAX_PACKAGE_BYTES as usize * 4 / 3 + 64 * 1024;

/// Meldet die drei Routen an; andere Methoden fallen in den Rückfall (kein 405)
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/themes", get(list).fallback(static_or_not_found))
        .route(
            "/api/themes/install",
            post(install)
                .layer(DefaultBodyLimit::max(MAX_INSTALL_BODY))
                .fallback(static_or_not_found),
        )
        .route(
            "/api/themes/{id}",
            delete(remove).fallback(static_or_not_found),
        )
}

/// Die Manifeste der installierten Themes
async fn list(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let home = state.home.clone();
    let themes = blocking(move || list_themes(&home)).await?;
    let manifests: Vec<ThemeManifest> = themes.into_iter().map(|t| t.manifest).collect();
    Ok(json(&manifests))
}

/// Installiert das hochgeladene Paket; dieselbe id ersetzt die alte Installation
async fn install(State(state): State<Arc<AppState>>, body: Bytes) -> ApiResult<Response> {
    let package = decode_package(&body)?;
    let home = state.home.clone();
    let manifest = blocking(move || install_theme(&home, &package)).await?;
    Ok(json(&manifest))
}

/// Entfernt ein installiertes Theme
async fn remove(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    let home = state.home.clone();
    blocking(move || remove_theme(&home, &id)).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Liest `data` aus dem JSON-Körper und dekodiert das Base64. Der Körper wird selbst gelesen,
/// weil `JsonBody` bei 1 MiB endet und Pakete größer sein dürfen.
fn decode_package(body: &[u8]) -> ApiResult<Vec<u8>> {
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| ApiError::bad_request())?;
    let data = value
        .get("data")
        .and_then(|data| data.as_str())
        .ok_or_else(|| {
            ApiError::new(
                "bad-request",
                "Das Feld data mit dem Paket als Base64 fehlt.",
            )
        })?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| ApiError::new("bad-request", "Das Feld data ist kein gültiges Base64."))
}

/// Führt Dateiarbeit der Paketverwaltung außerhalb des Laufzeit-Threads aus
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ThemePackageError> + Send + 'static,
) -> ApiResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ApiError(Error::unexpected(error)))?
        .map_err(api_error)
}

/// Übersetzt Fehler der Paketverwaltung: unbekannte id → 404, ungültige id → 400, Manifest
/// mit Feldfehlern → 422, jeder andere Prüffehler → 422 `theme-package-invalid`, Dateifehler
/// → 500 ohne Interna
fn api_error(error: ThemePackageError) -> ApiError {
    let message = error.to_string();
    match error {
        ThemePackageError::NotInstalled { .. } => ApiError::new("theme-not-found", message),
        ThemePackageError::InvalidId { .. } => ApiError::new("theme-id-invalid", message),
        ThemePackageError::Manifest(errors) => HarnessError::new("theme-manifest-invalid", message)
            .with_details(serde_json::json!({ "errors": errors.errors() }))
            .into(),
        ThemePackageError::Io(error) => ApiError(Error::unexpected(error)),
        ThemePackageError::TooLarge { .. }
        | ThemePackageError::FileTooLarge { .. }
        | ThemePackageError::TooManyEntries { .. }
        | ThemePackageError::UnsafePath { .. }
        | ThemePackageError::ForbiddenFileType { .. }
        | ThemePackageError::MissingFile { .. }
        | ThemePackageError::ForbiddenCss { .. }
        | ThemePackageError::NotZip(_) => ApiError::new("theme-package-invalid", message),
    }
}
