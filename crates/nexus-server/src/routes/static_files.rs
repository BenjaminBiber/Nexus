//! Auslieferung der gebauten Oberfläche samt Seitenrouting; portiert aus
//! `packages/server/src/routes/StaticRoutes.ts`.
//!
//! Liegen `.br`/`.gz` neben einer Datei (vom Build erzeugt), gehen sie an Browser, die sie
//! annehmen. Dateien unter `assets/` tragen einen Inhalts-Hash im Namen und ändern sich nie;
//! alles andere wird jedes Mal geprüft.

use std::path::Path;
use std::sync::Arc;

use axum::extract::Request;
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use tower::ServiceExt as _;
use tower_http::services::ServeDir;

use crate::state::AppState;

/// Hinweis, solange die Oberfläche nicht gebaut ist
const HINT: &str = concat!(
    "<!doctype html><meta charset=\"utf-8\"><title>AI Harness</title>",
    "<p>Die Oberfläche ist noch nicht gebaut. Bitte <code>npm run build</code> ausführen.</p>"
);

/// Dateien unter `assets/` ändern sich nie
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Alles andere (index.html) wird jedes Mal geprüft
const REVALIDATE: &str = "no-cache";
/// Inhaltstyp der Oberfläche
const HTML: &str = "text/html; charset=utf-8";

/// Liefert eine Datei, eine Seite oder den 404 als JSON
pub async fn serve(state: Arc<AppState>, request: Request) -> Response {
    let Some(web) = state.config.web_dir.clone() else {
        return without_web(&request);
    };
    let index = web.join("index.html");
    if !index.exists() {
        return without_web(&request);
    }
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let service = ServeDir::new(&web)
        .precompressed_br()
        .precompressed_gzip()
        .append_index_html_on_directories(true);
    let found = match service.oneshot(request).await {
        Ok(response) if response.status() != StatusCode::NOT_FOUND => {
            Some(response.map(axum::body::Body::new))
        }
        _ => None,
    };
    match found {
        Some(response) => with_cache_control(response, &path),
        // Rückfall wie `setNotFoundHandler`: Seitenadressen beantwortet die Oberfläche
        None if method == Method::GET && is_page_request(&path) => page(&index).await,
        None => crate::error::not_found_response(),
    }
}

/// Ohne gebaute Oberfläche: `GET /` weist freundlich darauf hin, alles andere ist 404
fn without_web(request: &Request) -> Response {
    let root = request.method() == Method::GET && request.uri().path() == "/";
    if !root {
        return crate::error::not_found_response();
    }
    ([(header::CONTENT_TYPE, HTML)], HINT).into_response()
}

/// Setzt den Cache-Kopf nach der Herkunft der Datei
fn with_cache_control(mut response: Response, path: &str) -> Response {
    let value = if path.starts_with("/assets/") {
        IMMUTABLE
    } else {
        REVALIDATE
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    response
}

/// Die Oberfläche selbst (`index.html`), immer neu geprüft
async fn page(index: &Path) -> Response {
    match tokio::fs::read(index).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, HTML),
                (header::CACHE_CONTROL, REVALIDATE),
            ],
            bytes,
        )
            .into_response(),
        // Zwischen Prüfung und Lesen verschwunden: dann gibt es die Adresse nicht
        Err(_) => crate::error::not_found_response(),
    }
}

/// Ist die Adresse eine Seite der Oberfläche (und keine Datei)?
///
/// Docs-Adressen enden auf den Dateinamen (z. B. `.md`) und sind trotzdem Seiten.
pub fn is_page_request(path: &str) -> bool {
    if path.starts_with("/api") {
        return false;
    }
    !has_extension(path) || is_docs_path(path)
}

/// Endet die Adresse auf eine Endung aus Buchstaben und Zahlen (`/\.[a-z0-9]+$/i`)?
fn has_extension(path: &str) -> bool {
    let last = path.rsplit('/').next().unwrap_or_default();
    let Some((_, extension)) = last.rsplit_once('.') else {
        return false;
    };
    !extension.is_empty() && extension.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Adresse der Dokumente eines Projekts (`^/p/[^/]+/docs/`)
fn is_docs_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/p/") else {
        return false;
    };
    let Some((project, rest)) = rest.split_once('/') else {
        return false;
    };
    !project.is_empty() && (rest == "docs/" || rest.starts_with("docs/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erkennt_seitenadressen() {
        for page in [
            "/",
            "/docs",
            "/p/abc/board",
            "/p/abc/tickets/T-0001",
            "/p/abc/docs/docs/specs/plan.md",
            "/p/abc/docs/a.b.md",
        ] {
            assert!(is_page_request(page), "{page}");
        }
    }

    #[test]
    fn erkennt_dateien_und_api_pfade() {
        for file in [
            "/fehlt.js",
            "/assets/app.js",
            "/favicon.ico",
            "/api/health",
            "/api/gibtesnicht",
            "/p/abc/bild.png",
        ] {
            assert!(!is_page_request(file), "{file}");
        }
    }

    #[test]
    fn eine_endung_mit_sonderzeichen_zahlt_nicht_als_datei() {
        // `\.[a-z0-9]+$` trifft nicht: der Punkt steht nicht vor einer reinen Endung
        assert!(is_page_request("/p/abc/ticket.T-1"));
        assert!(is_page_request("/stand."));
    }
}
