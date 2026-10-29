//! Routengruppe `docs-claude`: die sechs Docs-Routen, die Claude im Headless-Modus rufen –
//! Ticket-Entwürfe (erzeugen, abbrechen), Fragen-Entwürfe (erzeugen, abbrechen), die gewählten
//! Fragen anlegen und einen Dokumentbereich umschreiben. Portiert aus dem Claude-Teil von
//! `packages/server/src/routes/DocsRoutes.ts`.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use serde_json::Value;

use crate::docs::doc_revision_service::valid_revise;
use crate::docs::ticket_draft_service::valid_request;
use crate::error::{json, ApiResult};
use crate::extract::{JsonBody, QueryMap};
use crate::routes::docs::source;
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Meldet die Routen der Gruppe an. Der Rückfall der Methoden gehört zu jeder Route: hat eine
/// Rust-Route den Pfad, aber nicht die Methode, darf axum kein 405 liefern.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/projects/{pid}/docs/ticket-drafts",
            post(create_drafts)
                .delete(cancel_drafts)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/docs/question-drafts",
            post(create_question_drafts)
                .delete(cancel_question_drafts)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/docs/questions",
            post(create_questions).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/docs/revise",
            post(revise).fallback(static_or_not_found),
        )
}

/// Lässt Claude das Dokument oder die markierten Stellen in Ticket-Entwürfe zerlegen
async fn create_drafts(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let request = valid_request(&body);
    let result = state
        .ticket_drafts
        .create(
            state.claude.ticket_draft.as_ref(),
            &state.docs,
            &state.doc_drafts,
            &source,
            &pid,
            &request,
        )
        .await?;
    Ok(json(&result))
}

/// Bricht die laufende Erzeugung von Ticket-Entwürfen des Projekts ab (beendet Claude)
async fn cancel_drafts(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    // Der Körper wird nicht gelesen, aber wie in Fastify geprüft: leeres JSON ist 400
    JsonBody(_): JsonBody,
) -> ApiResult<Response> {
    state.projects.open(&pid)?;
    let cancelled = state.ticket_drafts.cancel(&pid);
    Ok(json(&serde_json::json!({ "cancelled": cancelled })))
}

/// Lässt Claude die offenen Fragen des Dokuments herauslesen (Entwürfe, nicht gespeichert)
async fn create_question_drafts(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let result = state
        .doc_questions
        .drafts(
            state.claude.ticket_draft.as_ref(),
            &state.docs,
            &source,
            &pid,
            body.get("path").and_then(Value::as_str),
        )
        .await?;
    Ok(json(&result))
}

/// Bricht die laufende Erzeugung von Fragen-Entwürfen des Projekts ab (beendet Claude)
async fn cancel_question_drafts(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    JsonBody(_): JsonBody,
) -> ApiResult<Response> {
    state.projects.open(&pid)?;
    let cancelled = state.doc_questions.cancel(&pid);
    Ok(json(&serde_json::json!({ "cancelled": cancelled })))
}

/// Legt die gewählten Entwürfe als Fragen im Projekt des Boards an
async fn create_questions(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let drafts = body
        .get("drafts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let result = state
        .doc_questions
        .create(
            &state.board,
            &state.docs,
            &state.doc_tickets,
            &source,
            body.get("path").and_then(Value::as_str),
            &drafts,
        )
        .await?;
    Ok(json(&result))
}

/// Lässt Claude einen Bereich des Dokuments umschreiben (schreibt selbst nichts)
async fn revise(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let input = valid_revise(&body)?;
    let source = source(&state, &pid, &query).await?;
    let result = state
        .doc_revisions
        .revise(state.claude.revise.as_ref(), &state.docs, &source, &input)
        .await?;
    Ok(json(&result))
}
