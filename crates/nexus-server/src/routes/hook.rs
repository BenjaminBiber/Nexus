//! Routengruppe `sessions`, Teil Hook-Empfang: `POST /api/hook` nimmt die normalisierten
//! Ereignisse der Sessions mit dem Token des Projekts entgegen. Portiert aus
//! `packages/server/src/routes/HookRoutes.ts`.
//!
//! Hooks laufen sehr oft: der Handler wartet nie auf die Board-Datenbank (die Ticketangaben
//! kommen mit Zeitlimit), und Fehler der Favoriten oder der Meldung stören den Hook nicht.

use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use nexus_core::project::TokenStore;
use nexus_core::sessions::{DisplayState, HookEvent, HookKind, SessionView};
use nexus_core::{HarnessError, HarnessProject};

use crate::error::ApiResult;
use crate::extract::JsonBody;
use crate::notifications::{SessionChange, TicketRef};
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Kopf mit dem Token des Projekts
const TOKEN_HEADER: &str = "x-harness-token";

/// Die optionalen Textfelder des Ereignisses
const TEXTS: [&str; 7] = [
    "at",
    "agentId",
    "agentType",
    "notificationType",
    "title",
    "model",
    "source",
];

/// Meldet die Route an; andere Methoden auf demselben Pfad fallen in den Rückfall, damit
/// axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/hook", post(hook).fallback(static_or_not_found))
}

/// Nimmt ein Hook-Ereignis an: Token prüfen, Zustand anwenden, Favoriten nachführen, melden
async fn hook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let path = body
        .get("project")
        .and_then(serde_json::Value::as_str)
        .filter(|path| Path::new(path).is_absolute())
        .ok_or_else(|| {
            HarnessError::new(
                "path-missing",
                "Der Pfad des Projekts fehlt oder ist nicht absolut.",
            )
        })?;
    let opened = state.projects.open_by_path(Path::new(path))?;
    assert_token(&opened.project, headers.get(TOKEN_HEADER))?;
    let event = parse_event(body.get("event"))?;
    let pid = opened.entry.id.clone();
    let before = state
        .sessions
        .find(&pid, &event.session_id)
        .map(|view| view.display_state);
    state.sessions.apply(&pid, &event);
    follow_favorites(&state, &pid, &event);
    notify(&state, &opened.entry.id, &opened.entry.name, &event, before).await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Vergleicht das Token zeitkonstant mit dem des Projekts
fn assert_token(
    project: &HarnessProject,
    given: Option<&axum::http::HeaderValue>,
) -> ApiResult<()> {
    let expected = TokenStore::new(project.layout.clone()).read()?;
    let actual = given.and_then(|value| value.to_str().ok()).unwrap_or("");
    if !equal_in_constant_time(actual.as_bytes(), expected.as_bytes()) {
        return Err(HarnessError::new(
            "token-invalid",
            "Das Token des Projekts fehlt oder ist falsch.",
        )
        .into());
    }
    Ok(())
}

/// Vergleich ohne Abhängigkeit von der Stelle des ersten Unterschieds (Längen zuerst, wie
/// `timingSafeEqual` in Node)
fn equal_in_constant_time(actual: &[u8], expected: &[u8]) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in actual.iter().zip(expected) {
        diff |= left ^ right;
    }
    diff == 0
}

/// Führt die Favoriten nach: Momentaufnahme und Zuordnung fortgesetzter Sessions (Fehler
/// stören den Hook nicht; der Dienst vermerkt sie im Protokoll)
fn follow_favorites(state: &Arc<AppState>, pid: &str, event: &HookEvent) {
    let Some(view) = state.sessions.find(pid, &event.session_id) else {
        return;
    };
    let resumed = event.kind == HookKind::SessionStart && event.source.as_deref() == Some("resume");
    let _ = state.favorites.on_session(pid, &view, resumed);
}

/// Meldet einen Zustandswechsel an die Benachrichtigung (Fehler stören den Hook nicht)
async fn notify(
    state: &Arc<AppState>,
    pid: &str,
    project_name: &str,
    event: &HookEvent,
    before: Option<DisplayState>,
) {
    let Some(view) = state.sessions.find(pid, &event.session_id) else {
        return;
    };
    if Some(view.display_state) == before {
        return;
    }
    let ticket = first_ticket(state, &view).await;
    state
        .notifications
        .on_session_change(&SessionChange {
            project_id: pid.to_string(),
            project_name: project_name.to_string(),
            before,
            view,
            ticket,
        })
        .await;
}

/// ID und Titel des ersten Tickets der Session aus der Board-Datenbank, falls es eines gibt
async fn first_ticket(state: &Arc<AppState>, view: &SessionView) -> Option<TicketRef> {
    let id = view.tickets.first()?.clone();
    let info = state.ticket_info.infos(std::slice::from_ref(&id)).await;
    Some(TicketRef {
        title: info
            .get(&id)
            .map(|found| found.title.clone())
            .unwrap_or_default(),
        id,
    })
}

/// Prüft das Ereignis und übernimmt nur bekannte Felder
fn parse_event(value: Option<&serde_json::Value>) -> ApiResult<HookEvent> {
    let invalid = || {
        HarnessError::new(
            "event-invalid",
            "Das Ereignis ist ungültig (kind und sessionId sind Pflicht).",
        )
    };
    let object = value.and_then(serde_json::Value::as_object);
    let text = |name: &str| {
        object?
            .get(name)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    };
    let kind = text("kind")
        .and_then(|kind| HookKind::from_str_opt(&kind))
        .ok_or_else(invalid)?;
    let session_id = text("sessionId").ok_or_else(invalid)?;
    let mut event = HookEvent::new(kind, session_id, text("cwd").unwrap_or_default());
    if let Some(tickets) = object.and_then(|object| object.get("tickets")) {
        if let Some(list) = tickets.as_array() {
            event.tickets = Some(
                list.iter()
                    .filter_map(serde_json::Value::as_str)
                    .filter(|ticket| is_ticket_id(ticket))
                    .map(str::to_string)
                    .collect(),
            );
        }
    }
    for name in TEXTS {
        let Some(found) = text(name) else { continue };
        match name {
            "at" => event.at = Some(found),
            "agentId" => event.agent_id = Some(found),
            "agentType" => event.agent_type = Some(found),
            "notificationType" => event.notification_type = Some(found),
            "title" => event.title = Some(found),
            "model" => event.model = Some(found),
            _ => event.source = Some(found),
        }
    }
    Ok(event)
}

/// Muster einer Ticket-ID: `^[A-Za-z][A-Za-z0-9]{0,5}-\d+$`
fn is_ticket_id(value: &str) -> bool {
    let Some((prefix, number)) = value.split_once('-') else {
        return false;
    };
    let mut letters = prefix.chars();
    let first = letters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic());
    first
        && prefix.len() <= 6
        && letters.all(|letter| letter.is_ascii_alphanumeric())
        && !number.is_empty()
        && number.chars().all(|digit| digit.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nimmt_nur_ticket_ids_im_muster_an() {
        for gut in ["T-1", "ARC-0001", "ABCDEF-12"] {
            assert!(is_ticket_id(gut), "{gut}");
        }
        for schlecht in ["", "-1", "1-1", "ABCDEFG-1", "ARC-", "ARC-x", "ARC_1"] {
            assert!(!is_ticket_id(schlecht), "{schlecht}");
        }
    }

    #[test]
    fn vergleicht_tokens_zeitkonstant() {
        assert!(equal_in_constant_time(b"abc", b"abc"));
        assert!(!equal_in_constant_time(b"abc", b"abd"));
        assert!(!equal_in_constant_time(b"ab", b"abc"));
        assert!(!equal_in_constant_time(b"", b"abc"));
    }
}
