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

/// Content-Security-Policy der Oberfläche, wortgleich mit `app.security.csp` in
/// `apps/desktop/src-tauri/tauri.conf.json` (Quelle der Wahrheit; ein Test im Desktop-Crate
/// sichert die Gleichheit)
///
/// `script-src` lässt Theme-Module (`main.js`, D1) nur vom Pfad `/themes/` des lokalen Servers
/// zu (der Port ist dynamisch); kein `unsafe-eval`, kein fremder Host.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; connect-src 'self' http://127.0.0.1:* ipc: http://ipc.localhost; img-src 'self' data: blob: http://127.0.0.1:*; style-src 'self' 'unsafe-inline'; font-src 'self' data:; script-src 'self' http://127.0.0.1:*/themes/";

/// Die `script-src`-Direktive der Basis; Hashes von Inline-Skripten kommen hinter sie
const SCRIPT_SRC: &str = "script-src 'self' http://127.0.0.1:*/themes/";

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
        Some(response) => with_headers(response, &path, &index).await,
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
    (
        [
            (header::CONTENT_TYPE, HTML),
            (header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY),
        ],
        HINT,
    )
        .into_response()
}

/// Setzt Cache-Kopf nach der Herkunft der Datei und die CSP
///
/// HTML-Antworten (die `index.html`, auch vorkomprimiert) bekommen die Hashes der
/// Inline-Skripte; gebildet aus der Datei auf der Platte, nicht aus dem Antwortkörper.
async fn with_headers(mut response: Response, path: &str, index: &Path) -> Response {
    let value = if path.starts_with("/assets/") {
        IMMUTABLE
    } else {
        REVALIDATE
    };
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    let policy = if is_html {
        match tokio::fs::read_to_string(index).await {
            Ok(html) => page_policy(&html),
            Err(_) => CONTENT_SECURITY_POLICY.to_string(),
        }
    } else {
        CONTENT_SECURITY_POLICY.to_string()
    };
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    if let Ok(policy) = HeaderValue::from_str(&policy) {
        headers.insert(header::CONTENT_SECURITY_POLICY, policy);
    }
    response
}

/// Die Oberfläche selbst (`index.html`), immer neu geprüft
async fn page(index: &Path) -> Response {
    match tokio::fs::read(index).await {
        Ok(bytes) => {
            let policy = page_policy(&String::from_utf8_lossy(&bytes));
            (
                [
                    (header::CONTENT_TYPE, HTML.to_string()),
                    (header::CACHE_CONTROL, REVALIDATE.to_string()),
                    (header::CONTENT_SECURITY_POLICY, policy),
                ],
                bytes,
            )
                .into_response()
        }
        // Zwischen Prüfung und Lesen verschwunden: dann gibt es die Adresse nicht
        Err(_) => crate::error::not_found_response(),
    }
}

/// CSP einer HTML-Seite: die Basis, deren `script-src` um die Hashes der Inline-Skripte
/// wächst (wie Tauri sie für eingebettete Seiten ergänzt; eine Direktive nie doppelt)
fn page_policy(html: &str) -> String {
    let hashes = inline_script_hashes(html);
    if hashes.is_empty() {
        return CONTENT_SECURITY_POLICY.to_string();
    }
    CONTENT_SECURITY_POLICY.replacen(SCRIPT_SRC, &format!("{SCRIPT_SRC} {}", hashes.join(" ")), 1)
}

/// `'sha256-…'` je `<script>` ohne `src` in der Reihenfolge des Dokuments
fn inline_script_hashes(html: &str) -> Vec<String> {
    use base64::Engine as _;
    use sha2::Digest as _;

    let mut hashes = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<script") {
        rest = &rest[start + "<script".len()..];
        let Some(tag_end) = rest.find('>') else {
            break;
        };
        let attributes = &rest[..tag_end];
        rest = &rest[tag_end + 1..];
        let Some(close) = rest.find("</script") else {
            break;
        };
        let content = &rest[..close];
        rest = &rest[close..];
        let external = attributes
            .split_whitespace()
            .any(|attribute| attribute.starts_with("src="));
        if external {
            continue;
        }
        let digest = sha2::Sha256::digest(content.as_bytes());
        let encoded = base64::engine::general_purpose::STANDARD.encode(digest);
        hashes.push(format!("'sha256-{encoded}'"));
    }
    hashes
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

    #[test]
    fn hasht_nur_inline_skripte() {
        let html = concat!(
            "<script>window.x=1</script>",
            "<script type=\"module\" src=\"/src/main.tsx\"></script>",
            "<script type=\"module\" crossorigin src=\"/assets/a.js\"></script>",
        );
        assert_eq!(
            inline_script_hashes(html),
            ["'sha256-hUmDl3yY/7GaaX+Zuy8aEy8IzxgpBs7xccqYHs2/7kQ='"]
        );
    }

    #[test]
    fn hasht_die_import_map_wie_ein_inline_skript() {
        let map = r#"{"imports":{"react":"/assets/host-react-a.js"}}"#;
        let hashes = inline_script_hashes(&format!("<script type=\"importmap\">{map}</script>"));
        assert_eq!(hashes.len(), 1);
        assert_eq!(
            hashes,
            inline_script_hashes(&format!("<script>{map}</script>"))
        );
    }

    #[test]
    fn ohne_inline_skripte_bleibt_die_basis_csp() {
        assert_eq!(
            page_policy("<script src=\"/a.js\"></script>"),
            CONTENT_SECURITY_POLICY
        );
    }

    /// Wert der Direktive `name` oder `None`; zählt zugleich, wie oft sie vorkommt
    fn directives<'a>(policy: &'a str, name: &str) -> Vec<&'a str> {
        policy
            .split(';')
            .map(str::trim)
            .filter(|d| d.split_whitespace().next() == Some(name))
            .collect()
    }

    #[test]
    fn die_basis_csp_enthaelt_script_src_wortgleich() {
        assert!(CONTENT_SECURITY_POLICY.ends_with(SCRIPT_SRC));
    }

    #[test]
    fn die_basis_csp_laesst_theme_module_nur_vom_lokalen_server_zu() {
        assert_eq!(
            directives(CONTENT_SECURITY_POLICY, "script-src"),
            ["script-src 'self' http://127.0.0.1:*/themes/"]
        );
        assert!(!CONTENT_SECURITY_POLICY.contains("unsafe-eval"));
    }

    #[test]
    fn hashes_landen_in_der_einen_script_src_und_keine_direktive_doppelt() {
        let policy = page_policy("<script>window.x=1</script><script>window.y=2</script>");
        let script_src = directives(&policy, "script-src");
        assert_eq!(script_src.len(), 1, "{policy}");
        let value = script_src[0];
        assert!(
            value.starts_with("script-src 'self' http://127.0.0.1:*/themes/ 'sha256-"),
            "{value}"
        );
        assert_eq!(value.matches("'sha256-").count(), 2, "{value}");
        let mut names: Vec<&str> = policy
            .split(';')
            .filter_map(|d| d.split_whitespace().next())
            .collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "Direktive doppelt: {policy}");
    }
}
