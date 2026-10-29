//! Routengruppe `sessions`, Teil Favoriten: lesen, markieren (mit Notiz) und entfernen.
//! Portiert aus `packages/server/src/routes/FavoriteRoutes.ts`.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::Router;
use nexus_core::HarnessError;

use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::favorites::FavoriteDto;
use crate::proxy::proxy_or_static;
use crate::state::AppState;

/// Länge, ab der eine Notiz gekürzt wird
const MAX_NOTE: usize = 2000;

/// Meldet die drei Routen der Favoriten an; andere Methoden auf denselben Pfaden fallen in
/// den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/projects/{pid}/favorites",
            get(list).fallback(proxy_or_static),
        )
        .route(
            "/api/projects/{pid}/favorites/{sid}",
            put(mark).delete(unmark).fallback(proxy_or_static),
        )
}

/// Die Favoriten eines Projekts mit Live-Zustand und Ticketangaben
async fn list(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
) -> ApiResult<Response> {
    Ok(json(&favorites(&state, &pid).await?))
}

/// Markiert eine Session oder ändert nur die Notiz eines bekannten Favoriten
async fn mark(
    State(state): State<Arc<AppState>>,
    UrlPath((pid, sid)): UrlPath<(String, String)>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    state.projects.open(&pid)?;
    let note = note(&body)?;
    let live = state.sessions.find(&pid, &sid);
    let known = state.favorites.is_favorite(&pid, &sid);
    match (&live, known) {
        // Bekannte, nicht mehr laufende Session: nur die Notiz
        (None, true) => {
            state
                .favorites
                .set_note(&pid, &sid, note.as_deref().unwrap_or_default())?;
        }
        _ => {
            state.favorites.mark(&pid, live.as_ref(), note.as_deref())?;
        }
    }
    let found = favorites(&state, &pid)
        .await?
        .into_iter()
        .find(|favorite| favorite.session_id == sid);
    Ok(json(&found))
}

/// Entfernt die Markierung
async fn unmark(
    State(state): State<Arc<AppState>>,
    UrlPath((pid, sid)): UrlPath<(String, String)>,
) -> ApiResult<Response> {
    state.projects.open(&pid)?;
    state.favorites.unmark(&pid, &sid)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Die Favoriten eines Projekts im Antwortformat
async fn favorites(state: &Arc<AppState>, pid: &str) -> ApiResult<Vec<FavoriteDto>> {
    state.projects.open(pid)?;
    let live = state.sessions.list(pid);
    let favorites = state.favorites.list(pid);
    let ids: Vec<String> = favorites
        .iter()
        .flat_map(|favorite| {
            live.iter()
                .find(|view| view.id == favorite.session_id)
                .map(|view| view.tickets.clone())
                .unwrap_or_else(|| favorite.tickets.clone())
        })
        .collect();
    let info = state.ticket_info.infos(&ids).await;
    Ok(favorites
        .iter()
        .map(|favorite| {
            let view = live.iter().find(|view| view.id == favorite.session_id);
            FavoriteDto::of(favorite, view, &info)
        })
        .collect())
}

/// Notiz aus dem Körper: Text (auf 2000 Zeichen gekürzt) oder nicht angegeben
fn note(body: &serde_json::Value) -> ApiResult<Option<String>> {
    match body.get("note") {
        None => Ok(None),
        Some(serde_json::Value::String(note)) => {
            Ok(Some(note.chars().take(MAX_NOTE).collect::<String>()))
        }
        Some(_) => Err(HarnessError::new("note-invalid", "Die Notiz muss ein Text sein.").into()),
    }
}
