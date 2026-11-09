//! Dateien installierter Themes unter `GET /themes/{id}/{*path}` (`theme.css`, `main.js`,
//! `assets/**`).
//!
//! Themes genießen volles Vertrauen (D2), ausgeliefert wird aber nur, was unter
//! `<NEXUS_HOME>/themes/<id>/` liegt: ungültige ID, leere Segmente, `.`/`..`, Backslashes,
//! Laufwerksangaben und Symlinks aus dem Theme-Ordner heraus ergeben 404, ebenso fehlende
//! Dateien und Ordner (kein Listing, keine Oberfläche). Jede Datei trägt ihren Inhaltstyp und
//! `X-Content-Type-Options: nosniff`.

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use nexus_core::themes::is_valid_theme_id;

use crate::error::not_found_response;
use crate::routes::static_files::CONTENT_SECURITY_POLICY;
use crate::state::AppState;

/// Themes werden aktualisiert und tragen keine Hash-Namen
const REVALIDATE: &str = "no-cache";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/themes/{id}/{*path}", get(serve_theme_file))
        // Ohne Dateipfad nie die Oberfläche, sondern 404
        .route("/themes/{id}", get(not_found))
        .route("/themes/{id}/", get(not_found))
}

async fn not_found() -> Response {
    not_found_response()
}

/// Liefert eine Datei eines installierten Themes aus
async fn serve_theme_file(
    State(state): State<Arc<AppState>>,
    Path((id, path)): Path<(String, String)>,
) -> Response {
    let Some(file) = resolve(&state.home.themes_dir(), &id, &path).await else {
        return not_found_response();
    };
    let Ok(bytes) = tokio::fs::read(&file).await else {
        return not_found_response();
    };
    (
        [
            (header::CONTENT_TYPE, content_type(&file)),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::CACHE_CONTROL, REVALIDATE),
            (header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY),
        ],
        bytes,
    )
        .into_response()
}

/// Pfad der angefragten Datei, nur wenn sie eine reguläre Datei innerhalb des Theme-Ordners ist
async fn resolve(themes_dir: &FsPath, id: &str, path: &str) -> Option<PathBuf> {
    if !is_valid_theme_id(id) {
        return None;
    }
    let theme_dir = themes_dir.join(id);
    let mut file = theme_dir.clone();
    for segment in path.split(['/', '\\']) {
        if !is_plain_segment(segment) {
            return None;
        }
        file.push(segment);
    }
    // Symlinks und Junctions dürfen nicht aus dem Theme-Ordner herausführen
    let root = tokio::fs::canonicalize(&theme_dir).await.ok()?;
    let target = tokio::fs::canonicalize(&file).await.ok()?;
    if !target.starts_with(&root) {
        return None;
    }
    let meta = tokio::fs::metadata(&target).await.ok()?;
    meta.is_file().then_some(target)
}

/// Ein Pfadsegment ohne Sonderbedeutung: nicht leer, kein `.`/`..`, kein `:` (Laufwerk,
/// alternative Datenströme) und kein Nullzeichen
fn is_plain_segment(segment: &str) -> bool {
    !segment.is_empty() && segment != "." && segment != ".." && !segment.contains([':', '\0'])
}

/// Inhaltstyp nach Endung; JS und CSS fest, damit der ESM-Import im Browser sicher klappt
fn content_type(file: &FsPath) -> &'static str {
    let extension = file
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        _ => "application/octet-stream",
    }
}
