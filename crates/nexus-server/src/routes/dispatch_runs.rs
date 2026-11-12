//! Routengruppe `dispatch`, Teil Lauf-Ablage: der Verlauf eines Laufs aus dem Transcript von
//! Claude Code und die Screenshots, die neben dem Protokoll liegen.
//!
//! `{name}` ist der Dateiname des Protokolls; ein ungültiger Name → 422 `log-name-invalid`.
//! Fehlende Ablage, fehlende Session und fehlendes Transcript sind kein Fehler
//! (`available: false` bzw. leere Liste). Ein unbekanntes Bild → 404 `shot-not-found`.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as UrlPath, RawQuery, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use nexus_core::dispatch::state::RunFiles;
use nexus_core::sessions::detail::{
    read_page, TranscriptLocator, TranscriptPage, DEFAULT_PAGE_LIMIT,
};
use nexus_core::HarnessError;

use crate::error::{json, ApiResult};
use crate::extract::QueryMap;
use crate::routes::session_detail::{bad_request, number, MAX_PAGE_LIMIT};
use crate::routes::static_or_not_found;
use crate::sessions::session_details::config_dirs;
use crate::state::AppState;

/// Meldet die drei Routen an; andere Methoden fallen in den Rückfall
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/dispatch/runs/{name}/transcript",
            get(transcript).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/runs/{name}/shots",
            get(shots).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/runs/{name}/shots/{file}",
            get(shot).fallback(static_or_not_found),
        )
}

/// Ein Abschnitt des Verlaufs ab `after` (Byte-Offset) mit höchstens `limit` Einträgen
async fn transcript(
    State(state): State<Arc<AppState>>,
    UrlPath(name): UrlPath<String>,
    RawQuery(query): RawQuery,
) -> ApiResult<Response> {
    let files = RunFiles::for_run(&state.home, &name)?;
    let query = QueryMap::parse(query.as_deref().unwrap_or(""));
    let (after, limit) = match (
        number(query.first("after"), "after"),
        number(query.first("limit"), "limit"),
    ) {
        (Ok(after), Ok(limit)) => (after.unwrap_or(0), limit),
        (Err(error), _) | (_, Err(error)) => return Ok(bad_request(error.0)),
    };
    let limit = limit
        .map_or(DEFAULT_PAGE_LIMIT, |limit| limit as usize)
        .clamp(1, MAX_PAGE_LIMIT);
    let dirs = config_dirs(&state);
    let page = tokio::task::spawn_blocking(move || read_transcript(&files, dirs, after, limit))
        .await
        .map_err(nexus_core::Error::unexpected)?;
    Ok(json(&page.unwrap_or_else(|| TranscriptPage {
        entries: Vec::new(),
        cursor: after.to_string(),
        skipped: 0,
        available: false,
    })))
}

/// Liest eine Seite des Verlaufs zu einem Lauf; nichts, wenn Session, Transcript oder Datei fehlen
fn read_transcript(
    files: &RunFiles,
    config_dirs: Vec<PathBuf>,
    after: u64,
    limit: usize,
) -> Option<TranscriptPage> {
    let session = files.session()?;
    let path =
        TranscriptLocator::new(config_dirs).locate(None, &session.cwd, &session.session_id)?;
    let page = read_page(&path, after, limit).ok()?;
    Some(TranscriptPage {
        entries: page.entries,
        cursor: page.cursor.to_string(),
        skipped: page.skipped,
        available: true,
    })
}

/// Die Screenshots des Laufs in der Reihenfolge ihrer Aufnahme
async fn shots(
    State(state): State<Arc<AppState>>,
    UrlPath(name): UrlPath<String>,
) -> ApiResult<Response> {
    let files = RunFiles::for_run(&state.home, &name)?;
    let list = tokio::task::spawn_blocking(move || files.shots())
        .await
        .map_err(nexus_core::Error::unexpected)?;
    Ok(json(&list))
}

/// Ein Screenshot als Bild; geliefert wird nur, was im Index der Ablage steht
async fn shot(
    State(state): State<Arc<AppState>>,
    UrlPath((name, file)): UrlPath<(String, String)>,
) -> ApiResult<Response> {
    let files = RunFiles::for_run(&state.home, &name)?;
    let found = tokio::task::spawn_blocking(move || {
        let (shot, path) = files.find_shot(&file)?;
        let bytes = std::fs::read(path).ok()?;
        Some((shot.file, bytes))
    })
    .await
    .map_err(nexus_core::Error::unexpected)?;
    let Some((file, bytes)) = found else {
        return Err(HarnessError::new("shot-not-found", "Diesen Screenshot gibt es nicht.").into());
    };
    Ok((
        [
            (header::CONTENT_TYPE, media_type(&file)),
            // Der Name eines Bildes ändert sich nie: der Browser darf es behalten
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
}

/// Medientyp nach der Endung der Bilddatei (dem Index wird dafür nicht vertraut)
fn media_type(file: &str) -> &'static str {
    match file.rsplit('.').next() {
        Some("png") => "image/png",
        Some("jpg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use nexus_core::dispatch::state::RunSession;
    use nexus_core::sessions::detail::encode_cwd;
    use serde_json::json;

    use super::*;

    /// Arbeitsordner der Beispiel-Session (existiert nicht)
    const CWD: &str = "D:/Projekte/demo";

    /// Ein kurzes Transcript: Anweisung und Antwort
    fn transcript() -> String {
        let lines = [
            json!({ "type": "user", "uuid": "u1", "timestamp": "2026-11-04T08:00:00.000Z", "cwd": CWD,
                "isSidechain": false, "message": { "role": "user", "content": "Bitte ändern" } }),
            json!({ "type": "assistant", "uuid": "a1", "timestamp": "2026-11-04T08:00:01.000Z", "cwd": CWD,
                "isSidechain": false, "message": { "id": "m1", "content": [{ "type": "text", "text": "Mache ich." }] } }),
        ];
        lines.iter().map(|line| format!("{line}\n")).collect()
    }

    #[test]
    fn liest_den_verlauf_ueber_die_abgelegte_session() {
        let dir = tempfile::tempdir().expect("Ordner");
        let config = dir.path().join("claude");
        let projekt = config.join("projects").join(encode_cwd(CWD));
        std::fs::create_dir_all(&projekt).expect("Projektordner");
        std::fs::write(projekt.join("sitzung-1.jsonl"), transcript()).expect("Transcript");
        let files = RunFiles::beside(&dir.path().join("x.log"));
        files
            .write_session(&RunSession {
                session_id: "sitzung-1".to_string(),
                cwd: CWD.to_string(),
            })
            .expect("Session");

        let page = read_transcript(&files, vec![config.clone()], 0, 500).expect("Verlauf");
        assert!(page.available);
        assert!(!page.entries.is_empty(), "Einträge aus dem Transcript");

        // Ab dem Cursor kommt nichts Neues, der Cursor bleibt stehen
        let cursor: u64 = page.cursor.parse().expect("Cursor ist eine Zahl");
        assert!(cursor > 0);
        let next = read_transcript(&files, vec![config], cursor, 500).expect("Verlauf");
        assert!(next.entries.is_empty());
        assert_eq!(next.cursor, page.cursor);
    }

    #[test]
    fn ohne_session_oder_transcript_gibt_es_keinen_verlauf() {
        let dir = tempfile::tempdir().expect("Ordner");
        let config = dir.path().join("claude");
        let files = RunFiles::beside(&dir.path().join("x.log"));
        assert!(
            read_transcript(&files, vec![config.clone()], 0, 500).is_none(),
            "ohne Session"
        );

        files
            .write_session(&RunSession {
                session_id: "sitzung-1".to_string(),
                cwd: CWD.to_string(),
            })
            .expect("Session");
        assert!(
            read_transcript(&files, vec![config], 0, 500).is_none(),
            "ohne Transcript"
        );
    }

    #[test]
    fn der_medientyp_folgt_der_endung() {
        assert_eq!(media_type("001.png"), "image/png");
        assert_eq!(media_type("002.jpg"), "image/jpeg");
        assert_eq!(media_type("003.gif"), "image/gif");
        assert_eq!(media_type("004.webp"), "image/webp");
        assert_eq!(media_type("005.html"), "application/octet-stream");
    }
}
