//! Routengruppe `sessions`, Teil Detailansicht: Kopf- und Laufdaten, Verlauf in Abschnitten,
//! geänderte Dateien und der Diff einer Datei.
//!
//! Unbekannte Session → 404 `session-not-found`; ein fehlendes Transcript ist kein Fehler
//! (`available: false`). Ein Pfad außerhalb des Repos → 400 `input-invalid`.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as UrlPath, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use nexus_core::sessions::detail::git_changes::check_path;
use nexus_core::sessions::detail::{
    read_page, SessionDetailParts, TranscriptPage, DEFAULT_PAGE_LIMIT,
};
use nexus_core::sessions::SessionView;
use nexus_core::HarnessError;

use crate::error::{json, ApiResult};
use crate::extract::QueryMap;
use crate::routes::static_or_not_found;
use crate::sessions::session_details::{self, run_info};
use crate::sessions::SessionDto;
use crate::state::AppState;

/// Höchstzahl der Einträge eines Abschnitts
pub(crate) const MAX_PAGE_LIMIT: usize = 2_000;

/// Meldet die vier Routen an; andere Methoden fallen in den Rückfall
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/projects/{pid}/sessions/{sid}/detail",
            get(detail).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/sessions/{sid}/transcript",
            get(transcript).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/sessions/{sid}/changes",
            get(changes).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/sessions/{sid}/changes/file",
            get(change_file).fallback(static_or_not_found),
        )
}

/// Kopf- und Laufdaten
async fn detail(
    State(state): State<Arc<AppState>>,
    UrlPath((pid, sid)): UrlPath<(String, String)>,
) -> ApiResult<Response> {
    let view = session(&state, &pid, &sid)?;
    let info = state.ticket_info.infos(&view.tickets).await;
    let dto = SessionDto::of(&view, &info);
    let path = transcript_path(&state, &view).await;
    let (facts, size) = match path.clone() {
        Some(path) => {
            let details = state.session_details.clone();
            let cwd = view.cwd.clone();
            tokio::task::spawn_blocking(move || {
                let size = std::fs::metadata(&path).ok().map(|meta| meta.len() as i64);
                (details.facts(&path, &cwd), size)
            })
            .await
            .map_err(nexus_core::Error::unexpected)?
        }
        None => (None, None),
    };
    let run = match &view.run_name {
        Some(name) => Some(run_info(&state, name).await),
        None => None,
    };
    let detail = SessionDetailParts {
        session: dto,
        transcript_path: path.map(|path| path.to_string_lossy().replace('\\', "/")),
        transcript_size: size,
        facts,
        run,
    }
    .build();
    Ok(json(&detail))
}

/// Ein Abschnitt des Verlaufs ab `after` (Byte-Offset) mit höchstens `limit` Einträgen;
/// meldet die Session für Live-Ereignisse als beobachtet
async fn transcript(
    State(state): State<Arc<AppState>>,
    UrlPath((pid, sid)): UrlPath<(String, String)>,
    RawQuery(query): RawQuery,
) -> ApiResult<Response> {
    let view = session(&state, &pid, &sid)?;
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
    let Some(path) = transcript_path(&state, &view).await else {
        return Ok(json(&TranscriptPage {
            entries: Vec::new(),
            cursor: after.to_string(),
            skipped: 0,
            available: false,
        }));
    };
    state.transcript_watch.watch(&pid, &sid, &path);
    let read = tokio::task::spawn_blocking(move || read_page(&path, after, limit))
        .await
        .map_err(nexus_core::Error::unexpected)?;
    Ok(json(&match read {
        Ok(page) => TranscriptPage {
            entries: page.entries,
            cursor: page.cursor.to_string(),
            skipped: page.skipped,
            available: true,
        },
        // Zwischen Finden und Lesen verschwunden (z. B. gelöscht)
        Err(_) => TranscriptPage {
            entries: Vec::new(),
            cursor: after.to_string(),
            skipped: 0,
            available: false,
        },
    }))
}

/// Die geänderten Dateien (Git, sonst Transcript)
async fn changes(
    State(state): State<Arc<AppState>>,
    UrlPath((pid, sid)): UrlPath<(String, String)>,
) -> ApiResult<Response> {
    let view = session(&state, &pid, &sid)?;
    let path = transcript_path(&state, &view).await;
    Ok(json(
        &session_details::changes(&state, &view.cwd, path).await,
    ))
}

/// Diff einer geänderten Datei
async fn change_file(
    State(state): State<Arc<AppState>>,
    UrlPath((pid, sid)): UrlPath<(String, String)>,
    RawQuery(query): RawQuery,
) -> ApiResult<Response> {
    let view = session(&state, &pid, &sid)?;
    let query = QueryMap::parse(query.as_deref().unwrap_or(""));
    let wanted = query.first("path").unwrap_or("").to_string();
    if let Err(error) = check_path(&wanted) {
        return Ok(bad_request(error));
    }
    let path = transcript_path(&state, &view).await;
    let diff = session_details::file_diff(&state, &view.cwd, path, &wanted).await?;
    Ok(json(&diff))
}

/// Die Session oder 404 `session-not-found`
fn session(state: &AppState, pid: &str, sid: &str) -> ApiResult<SessionView> {
    state.projects.open(pid)?;
    state.sessions.find(pid, sid).ok_or_else(|| {
        HarnessError::new("session-not-found", format!("Session {sid} gibt es nicht.")).into()
    })
}

/// Pfad des Transcripts (Suche blockierend, außerhalb der Laufzeit der Anfrage)
async fn transcript_path(state: &Arc<AppState>, view: &SessionView) -> Option<PathBuf> {
    let state = state.clone();
    let known = view.transcript_path.clone();
    let cwd = view.cwd.clone();
    let id = view.id.clone();
    tokio::task::spawn_blocking(move || {
        session_details::locate(&state, known.as_deref(), &cwd, &id)
    })
    .await
    .ok()
    .flatten()
}

/// Eine nicht negative ganze Zahl der Abfrage; anderes → `input-invalid`
pub(crate) fn number(value: Option<&str>, name: &str) -> ApiResult<Option<u64>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    value.parse::<u64>().map(Some).map_err(|_| {
        HarnessError::new(
            "input-invalid",
            format!("{name} muss eine nicht negative ganze Zahl sein."),
        )
        .into()
    })
}

/// 400 mit dem Fehler im Format der übrigen Fehler (`input-invalid` gilt sonst als 422)
pub(crate) fn bad_request(error: nexus_core::Error) -> Response {
    let body = match &error {
        nexus_core::Error::Harness(harness) => serde_json::to_value(harness)
            .unwrap_or_else(|_| crate::error::ErrorMapper::internal_body()),
        _ => crate::error::ErrorMapper::internal_body(),
    };
    (StatusCode::BAD_REQUEST, json(&body)).into_response()
}
