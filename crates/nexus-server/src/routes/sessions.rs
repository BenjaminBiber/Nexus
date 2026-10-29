//! Routengruppe `sessions`, Teil Sitzungen: Liste eines Projekts und die Zuordnung zu einem
//! Ticket der Board-Datenbank. Portiert aus `packages/server/src/routes/SessionRoutes.ts`.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use nexus_core::HarnessError;

use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::routes::static_or_not_found;
use crate::sessions::{SessionDto, TicketInfo};
use crate::state::AppState;

/// Meldet die beiden Routen der Sitzungen an; andere Methoden auf denselben Pfaden fallen in
/// den Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/projects/{pid}/sessions",
            get(list).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/sessions/{sid}/claim",
            post(claim).fallback(static_or_not_found),
        )
}

/// Die Sessions eines Projekts (letzte sieben Tage) mit den Ticketangaben der Board-Datenbank
async fn list(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
) -> ApiResult<Response> {
    state.projects.open(&pid)?;
    let list = state.sessions.list(&pid);
    let ids: Vec<String> = list.iter().flat_map(|view| view.tickets.clone()).collect();
    let info = state.ticket_info.infos(&ids).await;
    let dtos: Vec<SessionDto> = list
        .iter()
        .map(|view| SessionDto::of(view, &info))
        .collect();
    Ok(json(&dtos))
}

/// Ordnet eine Session einem Ticket zu und verknüpft sie dort
async fn claim(
    State(state): State<Arc<AppState>>,
    UrlPath((pid, sid)): UrlPath<(String, String)>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    state.projects.open(&pid)?;
    let wanted = body
        .get("ticket")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|ticket| !ticket.is_empty())
        .ok_or_else(|| {
            HarnessError::new("ticket-missing", "Bitte das Ticket angeben (Feld ticket).")
        })?
        .to_string();
    let ticket = state
        .board
        .run(move |backend| async move { backend.services().tickets.get(&wanted).await })
        .await?;
    state.sessions.claim(&pid, &sid, &ticket.id)?;
    let id = ticket.id.clone();
    let session = sid.clone();
    state
        .board
        .run(move |backend| async move {
            backend.services().tickets.link_session(&id, &session).await
        })
        .await?;
    let view = state.sessions.find(&pid, &sid).ok_or_else(|| {
        HarnessError::new("session-not-found", format!("Session {sid} gibt es nicht."))
    })?;
    let info = HashMap::from([(
        ticket.id.clone(),
        TicketInfo {
            id: ticket.id.clone(),
            title: ticket.title.clone(),
            status: ticket.status.as_str().to_string(),
        },
    )]);
    Ok(json(&SessionDto::of(&view, &info)))
}
