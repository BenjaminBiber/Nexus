//! Routengruppe `db-tickets`: Tickets anlegen, lesen, ändern (mit Version), verschieben,
//! löschen, kommentieren, von anderen Boards holen und ihre Schritte bearbeiten; dazu die
//! Zeitleiste. Portiert aus `packages/server/src/routes/DbTicketRoutes.ts`.
//!
//! Schreibaktionen gelten als Aktion des Nutzers (`by_user`): so ist etwa „Nicht mehr nötig“
//! erlaubt. Jede Route, die eine Version prüft, meldet eine veraltete Version als 409 mit dem
//! aktuellen Stand des Tickets ([`with_conflict`]).

use std::future::Future;
use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use nexus_core::board::ticket_service::{
    AddStepOptions, MoveOptions, NewTicketInput, TicketPatch, WriteOptions,
};
use nexus_core::board::ticket_step_edits::{StepChange, StepGuard};
use nexus_core::{Error, HarnessError, Result};
use serde::Serialize;

use crate::board_runtime::BoardBackend;
use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::input::{Body, DbInput, Nullable};
use crate::proxy::proxy_or_static;
use crate::state::AppState;
use crate::timeline::TicketTimelineAssembler;

/// Höchstzahl der Tickets, die ein Aufruf von einem anderen Board holt
const MAX_IMPORT_IDS: usize = 200;

/// Meldet die 13 Routen der Gruppe an. Der Rückfall der Methoden gehört zu jeder Route: hat
/// eine Rust-Route den Pfad, aber nicht die Methode, darf axum kein 405 liefern.
///
/// Die Schrittnummer `{n}` bleibt bewusst ein `String`: als Zahl typisiert würde axum eine
/// falsche Nummer mit 400 abweisen, statt die eigene Meldung zu liefern. Dasselbe gilt für
/// jede Ticket-ID.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/db/boards/{id}/tickets",
            post(create).fallback(proxy_or_static),
        )
        .route(
            "/api/db/boards/{id}/import",
            post(import).fallback(proxy_or_static),
        )
        .route(
            "/api/db/tickets/{id}",
            get(get_ticket)
                .patch(patch_ticket)
                .delete(delete_ticket)
                .fallback(proxy_or_static),
        )
        .route(
            "/api/db/tickets/{id}/timeline",
            get(timeline).fallback(proxy_or_static),
        )
        .route(
            "/api/db/tickets/{id}/move",
            post(move_ticket).fallback(proxy_or_static),
        )
        .route(
            "/api/db/tickets/{id}/comments",
            post(add_comment).fallback(proxy_or_static),
        )
        .route(
            "/api/db/tickets/{id}/steps",
            post(add_step).fallback(proxy_or_static),
        )
        .route(
            "/api/db/tickets/{id}/steps/{n}",
            post(set_step)
                .patch(edit_step)
                .delete(remove_step)
                .fallback(proxy_or_static),
        )
        .route(
            "/api/db/tickets/{id}/steps/{n}/move",
            post(move_step).fallback(proxy_or_static),
        )
}

/// Legt ein Ticket auf einem Board an (201)
async fn create(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let board_id = DbInput::id(&raw_id, "Board")?;
    let body = DbInput::body(&raw);
    let input = NewTicketInput {
        title: DbInput::text(&body, "title")?,
        description: DbInput::optional_text(&body, "description")?,
        status: DbInput::optional_status(&body, "status")?,
        priority: DbInput::optional_priority(&body, "priority")?,
        labels: DbInput::optional_list(&body, "labels")?,
        depends_on: DbInput::optional_list(&body, "dependsOn")?,
        acceptance: DbInput::optional_text(&body, "acceptance")?,
        needs_planning: DbInput::optional_bool(&body, "needsPlanning")?,
        target_branch: nullable(DbInput::nullable_text(&body, "targetBranch")?).flatten(),
        ..NewTicketInput::default()
    };
    let ticket = state
        .board
        .run(|b| async move {
            b.services()
                .tickets
                .create(
                    board_id,
                    &input,
                    &b.actor(None),
                    &WriteOptions {
                        by_user: true,
                        ..WriteOptions::default()
                    },
                )
                .await
        })
        .await?;
    Ok((StatusCode::CREATED, json(&ticket)).into_response())
}

/// Holt Tickets von anderen Boards desselben Projekts auf dieses Board (alles oder nichts)
async fn import(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let board_id = DbInput::id(&raw_id, "Board")?;
    let ticket_ids = DbInput::required_list(&DbInput::body(&raw), "ticketIds", MAX_IMPORT_IDS)?;
    let moved = state
        .board
        .run(|b| async move {
            b.services()
                .tickets
                .move_to_board(&ticket_ids, board_id, &b.actor(None))
                .await
        })
        .await?;
    Ok(json(&serde_json::json!({ "moved": moved })))
}

/// Ein Ticket mit Kommentaren, Verlauf und Schritten
async fn get_ticket(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    let ticket = state
        .board
        .run(|b| async move { b.services().tickets.get(&id).await })
        .await?;
    Ok(json(&ticket))
}

/// Zeitleiste: Verlauf, Kommentare, Fragen, Läufe und Commits in Zeitfolge (lokale Quellen
/// nur auf diesem Rechner)
async fn timeline(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    let dto = TicketTimelineAssembler::new(state.clone()).get(&id).await?;
    Ok(json(&dto))
}

/// Ändert Felder eines Tickets; die gelesene Version (`rev`) gehört dazu
async fn patch_ticket(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let rev = DbInput::optional_int(&body, "rev")?.ok_or_else(|| {
        HarnessError::new(
            "rev-missing",
            "Zum Ändern gehört die gelesene Version (rev).",
        )
    })?;
    let patch = patch_of(&body)?;
    let target = id.clone();
    with_conflict(&state, &id, StatusCode::OK, move |b| async move {
        b.services()
            .tickets
            .update(
                &target,
                &patch,
                &b.actor(None),
                &WriteOptions {
                    rev: Some(rev as i32),
                    by_user: true,
                    ..WriteOptions::default()
                },
            )
            .await?;
        // Nach dem Schreiben neu lesen: die Antwort ist das Ticketdetail, nicht die Ansicht
        b.services().tickets.get(&target).await
    })
    .await
}

/// Löscht ein Ticket; 204 ohne Körper
async fn delete_ticket(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    // Der Körper wird nicht gelesen, aber wie in Fastify geprüft: leeres JSON ist 400
    JsonBody(_): JsonBody,
) -> ApiResult<Response> {
    state
        .board
        .run(|b| async move { b.services().tickets.delete(&id, &b.actor(None)).await })
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Verschiebt ein Ticket in eine Spalte
async fn move_ticket(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let status = DbInput::optional_status(&body, "status")?.ok_or_else(|| {
        HarnessError::new("status-missing", "Für das Verschieben fehlt der Status.")
    })?;
    let options = MoveOptions {
        rev: DbInput::optional_int(&body, "rev")?.map(|rev| rev as i32),
        by_user: true,
        acceptance: DbInput::optional_text(&body, "acceptance")?,
        ..MoveOptions::default()
    };
    let target = id.clone();
    with_conflict(&state, &id, StatusCode::OK, move |b| async move {
        b.services()
            .tickets
            .move_to(&target, status, &b.actor(None), &options)
            .await
    })
    .await
}

/// Hängt einen Kommentar an und liefert das Ticketdetail (201)
async fn add_comment(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let text = DbInput::text(&DbInput::body(&raw), "body")?;
    let detail = state
        .board
        .run(|b| async move {
            b.services()
                .comments
                .add(&id, &text, &b.actor(None), false)
                .await?;
            b.services().tickets.get(&id).await
        })
        .await?;
    Ok((StatusCode::CREATED, json(&detail)).into_response())
}

/// Hakt einen Schritt ab oder nimmt den Haken zurück (Nummer ab 1)
async fn set_step(
    State(state): State<Arc<AppState>>,
    UrlPath((id, raw_number)): UrlPath<(String, String)>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let number = step_number(&raw_number)?;
    let body = DbInput::body(&raw);
    let done = DbInput::optional_bool(&body, "done")?.ok_or_else(|| {
        HarnessError::new("input-invalid", "Feld „done“ fehlt (true oder false).")
    })?;
    let guard = step_guard(&body)?;
    let target = id.clone();
    with_conflict(&state, &id, StatusCode::OK, move |b| async move {
        b.services()
            .tickets
            .set_step(&target, number, done, &b.actor(None), &guard)
            .await
    })
    .await
}

/// Legt einen Schritt an (ans Ende oder an Nummer `at`); Antwort: Ticketdetail (201)
async fn add_step(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let text = DbInput::text(&body, "text")?;
    let guard = step_guard(&body)?;
    let options = AddStepOptions {
        detail: nullable(DbInput::nullable_text(&body, "detail")?).flatten(),
        at: DbInput::optional_int(&body, "at")?.map(|at| at as i32),
        rev: guard.rev,
        expect_text: guard.expect_text,
    };
    let target = id.clone();
    with_conflict(&state, &id, StatusCode::CREATED, move |b| async move {
        b.services()
            .tickets
            .add_step(&target, &text, &b.actor(None), &options)
            .await
    })
    .await
}

/// Ändert Text und/oder Anleitung eines Schritts (`detail: null` entfernt die Anleitung)
async fn edit_step(
    State(state): State<Arc<AppState>>,
    UrlPath((id, raw_number)): UrlPath<(String, String)>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let number = step_number(&raw_number)?;
    let body = DbInput::body(&raw);
    let change = StepChange {
        text: DbInput::optional_text(&body, "text")?,
        detail: nullable(DbInput::nullable_text(&body, "detail")?),
    };
    let guard = step_guard(&body)?;
    let target = id.clone();
    with_conflict(&state, &id, StatusCode::OK, move |b| async move {
        b.services()
            .tickets
            .edit_step(&target, number, &change, &b.actor(None), &guard)
            .await
    })
    .await
}

/// Löscht einen Schritt; die folgenden rücken nach. Der Schutz steht im Körper
async fn remove_step(
    State(state): State<Arc<AppState>>,
    UrlPath((id, raw_number)): UrlPath<(String, String)>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let number = step_number(&raw_number)?;
    let guard = step_guard(&DbInput::body(&raw))?;
    let target = id.clone();
    with_conflict(&state, &id, StatusCode::OK, move |b| async move {
        b.services()
            .tickets
            .remove_step(&target, number, &b.actor(None), &guard)
            .await
    })
    .await
}

/// Verschiebt einen Schritt auf Nummer `to`
async fn move_step(
    State(state): State<Arc<AppState>>,
    UrlPath((id, raw_number)): UrlPath<(String, String)>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let from = step_number(&raw_number)?;
    let body = DbInput::body(&raw);
    let to = DbInput::optional_int(&body, "to")?
        .ok_or_else(|| HarnessError::new("input-invalid", "Feld „to“ fehlt (Zielnummer ab 1)."))?;
    let guard = step_guard(&body)?;
    let target = id.clone();
    with_conflict(&state, &id, StatusCode::OK, move |b| async move {
        b.services()
            .tickets
            .move_step(&target, from, to as i32, &b.actor(None), &guard)
            .await
    })
    .await
}

/// Optionaler Schutz einer Schritt-Änderung: Ticket-Version (`rev`) und bisheriger Text des
/// Schritts (`expectText`)
fn step_guard(body: &Body) -> Result<StepGuard> {
    Ok(StepGuard {
        rev: DbInput::optional_int(body, "rev")?.map(|rev| rev as i32),
        expect_text: DbInput::optional_text(body, "expectText")?,
    })
}

/// Schrittnummer aus der Adresse (ab 1)
fn step_number(raw: &str) -> Result<i32> {
    raw.parse::<i32>()
        .ok()
        .filter(|number| *number >= 1)
        .ok_or_else(|| {
            HarnessError::new(
                "input-invalid",
                format!("„{raw}“ ist keine Schrittnummer (ab 1)."),
            )
            .into()
        })
}

/// Änderungswunsch aus dem Körper; nicht genannte Felder bleiben unverändert. Branch und
/// Worktree lassen sich hier nur gemeinsam leeren (`branch: null` oder `worktree: null`), nie
/// setzen – das tun nur `start` und der Dispatcher
fn patch_of(body: &Body) -> Result<TicketPatch> {
    for key in ["branch", "worktree"] {
        if body.get(key).is_some_and(|value| !value.is_null()) {
            return Err(HarnessError::new(
                "input-invalid",
                "Branch und Worktree lassen sich hier nur leeren (null), nicht setzen.",
            )
            .into());
        }
    }
    let clear = ["branch", "worktree"]
        .iter()
        .any(|key| body.get(*key).is_some_and(serde_json::Value::is_null));
    Ok(TicketPatch {
        branch: clear.then_some(None),
        worktree: clear.then_some(None),
        title: DbInput::optional_text(body, "title")?,
        description: DbInput::optional_text(body, "description")?,
        plan: DbInput::optional_text(body, "plan")?,
        handbook: DbInput::optional_text(body, "handbook")?,
        diagram: DbInput::optional_text(body, "diagram")?,
        acceptance: DbInput::optional_text(body, "acceptance")?,
        status: DbInput::optional_status(body, "status")?,
        priority: DbInput::optional_priority(body, "priority")?,
        labels: DbInput::optional_list(body, "labels")?,
        assignee: nullable(DbInput::nullable_text(body, "assignee")?),
        blocked_reason: nullable(DbInput::nullable_text(body, "blockedReason")?),
        target_branch: nullable(DbInput::nullable_text(body, "targetBranch")?),
        needs_planning: DbInput::optional_bool(body, "needsPlanning")?,
        depends_on: DbInput::optional_list(body, "dependsOn")?,
        depends_on_add: DbInput::optional_list(body, "dependsOnAdd")?,
        depends_on_remove: DbInput::optional_list(body, "dependsOnRemove")?,
        ..TicketPatch::default()
    })
}

/// Ein Feld, das fehlen oder `null` sein darf, als `Option<Option<…>>` der Dienste
fn nullable(value: Nullable<String>) -> Option<Option<String>> {
    match value {
        Nullable::Missing => None,
        Nullable::Null => Some(None),
        Nullable::Value(text) => Some(Some(text)),
    }
}

/// Führt die Arbeit auf der Board-Laufzeit aus und antwortet mit `status`. Meldet der Dienst
/// `conflict` (veraltete Version oder geänderter Schritt), wird daraus 409 mit dem aktuellen
/// Stand des Tickets unter `current`.
async fn with_conflict<T, F, Fut>(
    state: &Arc<AppState>,
    id: &str,
    status: StatusCode,
    work: F,
) -> ApiResult<Response>
where
    T: Serialize + Send + 'static,
    F: FnOnce(Arc<dyn BoardBackend>) -> Fut + Send,
    Fut: Future<Output = Result<T>> + Send,
{
    let ticket = id.to_string();
    let (status, body) = state
        .board
        .run(|b| async move {
            match work(b.clone()).await {
                Ok(value) => Ok((status, serde_json::to_value(value).map_err(Error::unexpected)?)),
                Err(error) if error.code() == Some("conflict") => {
                    let message = error.to_string();
                    let current = b.services().tickets.get(&ticket).await?;
                    Ok((
                        StatusCode::CONFLICT,
                        serde_json::json!({ "code": "conflict", "message": message, "current": current }),
                    ))
                }
                Err(error) => Err(error),
            }
        })
        .await?;
    Ok((status, json(&body)).into_response())
}
