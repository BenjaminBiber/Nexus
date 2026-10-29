//! Routengruppe `questions`: Fragen eines Board-Projekts lesen, stellen, ändern, verknüpfen,
//! kommentieren und entscheiden. Portiert aus
//! `packages/server/src/routes/DbQuestionRoutes.ts`.
//!
//! Handelnder ist immer der Mensch der Oberfläche (GUID und Name aus `board.yaml`). Jede
//! Änderung prüft die gelesene Version (`rev`) und meldet eine veraltete Version als 409 mit
//! dem aktuellen Stand der Frage ([`conflict::with_conflict`]).

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::Router;
use nexus_core::actors::AuthorKind;
use nexus_core::board::question_service::{
    BoardQuestionChanges, BoardQuestionListFilter, BoardQuestionWriteOptions, NewBoardQuestionInput,
};
use nexus_core::board::question_types::BoardQuestionActor;
use nexus_core::questions::{DecisionInput, QuestionState};
use nexus_core::{HarnessError, Result};

use crate::board_runtime::BoardBackend;
use crate::conflict;
use crate::error::{json, ApiResult};
use crate::extract::{JsonBody, QueryMap, QueryValue};
use crate::input::{Body, DbInput, Nullable};
use crate::proxy::proxy_or_static;
use crate::state::AppState;

/// Meldet die zehn Routen der Gruppe an. Der Rückfall der Methoden gehört zu jeder Route: hat
/// eine Rust-Route den Pfad, aber nicht die Methode, darf axum kein 405 liefern.
///
/// Die Fragen-ID `{id}` bleibt ein `String`: sie hat die Form `ARC-F0001` und ist keine Zahl.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/db/projects/{key}/questions",
            get(list).post(create).fallback(proxy_or_static),
        )
        .route(
            "/api/db/questions/{id}",
            get(get_question)
                .patch(patch_question)
                .fallback(proxy_or_static),
        )
        .route(
            "/api/db/questions/{id}/tickets",
            put(set_tickets).fallback(proxy_or_static),
        )
        .route(
            "/api/db/questions/{id}/depends",
            put(set_depends).fallback(proxy_or_static),
        )
        .route(
            "/api/db/questions/{id}/comments",
            post(add_comment).fallback(proxy_or_static),
        )
        .route(
            "/api/db/questions/{id}/decision",
            post(decide).fallback(proxy_or_static),
        )
        .route(
            "/api/db/questions/{id}/discard",
            post(discard).fallback(proxy_or_static),
        )
        .route(
            "/api/db/questions/{id}/reopen",
            post(reopen).fallback(proxy_or_static),
        )
}

/// Die Fragen eines Projekts samt Zählern; Betrachter ist der Benutzer der Oberfläche, damit
/// `mine` und `counts.mine` („Eigene“) der Server rechnet
async fn list(
    State(state): State<Arc<AppState>>,
    UrlPath(key): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let filter = BoardQuestionListFilter {
        state: state_filter(param(&query, "state")?)?,
        ticket: set(param(&query, "ticket")?),
        tag: set(param(&query, "tag")?),
        text: set(param(&query, "q")?),
    };
    let list = state
        .board
        .run(|b| async move {
            let viewer = b.actor(None).user_id;
            b.services()
                .questions
                .list(&key, &filter, Some(&viewer))
                .await
        })
        .await?;
    Ok(json(&list))
}

/// Stellt eine Frage im Projekt (201)
async fn create(
    State(state): State<Arc<AppState>>,
    UrlPath(key): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let input = NewBoardQuestionInput {
        title: DbInput::text(&body, "title")?,
        description: DbInput::optional_text(&body, "description")?,
        tags: DbInput::optional_list(&body, "tags")?,
        occasion: value(DbInput::nullable_text(&body, "occasion")?),
        ticket_ids: DbInput::optional_list(&body, "ticketIds")?,
        depends_on: DbInput::optional_list(&body, "dependsOn")?,
    };
    let created = state
        .board
        .run(|b| async move {
            b.services()
                .questions
                .create(&key, &input, &actor(b.as_ref()))
                .await
        })
        .await?;
    Ok((StatusCode::CREATED, json(&created)).into_response())
}

/// Eine Frage mit Recherchen, Kommentaren und Verlauf
async fn get_question(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    let question = state
        .board
        .run(|b| async move { b.services().questions.get(&id).await })
        .await?;
    Ok(json(&question))
}

/// Ändert Titel, Beschreibung, Schlagworte oder Anlass (`occasion: null` löscht den Anlass)
async fn patch_question(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let options = write_options(&body)?;
    let changes = BoardQuestionChanges {
        title: DbInput::optional_text(&body, "title")?,
        description: DbInput::optional_text(&body, "description")?,
        tags: DbInput::optional_list(&body, "tags")?,
        occasion: nullable(DbInput::nullable_text(&body, "occasion")?),
    };
    let target = id.clone();
    with_conflict(&state, &id, move |b| async move {
        b.services()
            .questions
            .update(&target, &changes, &actor(b.as_ref()), &options)
            .await
    })
    .await
}

/// Setzt die Tickets der Frage neu (die Liste darf leer sein)
async fn set_tickets(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let options = write_options(&body)?;
    let ticket_ids = required_list(&body, "ticketIds")?;
    let target = id.clone();
    with_conflict(&state, &id, move |b| async move {
        b.services()
            .questions
            .set_tickets(&target, &ticket_ids, &actor(b.as_ref()), &options)
            .await
    })
    .await
}

/// Setzt die Voraussetzungen der Frage neu (die Liste darf leer sein)
async fn set_depends(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let options = write_options(&body)?;
    let depends_on = required_list(&body, "dependsOn")?;
    let target = id.clone();
    with_conflict(&state, &id, move |b| async move {
        b.services()
            .questions
            .set_depends(&target, &depends_on, &actor(b.as_ref()), &options)
            .await
    })
    .await
}

/// Hängt einen Kommentar an und liefert die Frage (201)
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
                .questions
                .comment(&id, &text, &actor(b.as_ref()))
                .await
        })
        .await?;
    Ok((StatusCode::CREATED, json(&detail)).into_response())
}

/// Entscheidet die Frage: eine Option der neuesten Recherche oder ein eigener Text
async fn decide(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let options = write_options(&body)?;
    let input = DecisionInput {
        option: DbInput::optional_text(&body, "option")?,
        text: DbInput::optional_text(&body, "text")?,
        reason: DbInput::optional_text(&body, "reason")?,
    };
    let target = id.clone();
    with_conflict(&state, &id, move |b| async move {
        b.services()
            .questions
            .answer(&target, &input, &actor(b.as_ref()), &options)
            .await
    })
    .await
}

/// Verwirft eine offene Frage mit Begründung
async fn discard(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let options = write_options(&body)?;
    let reason = DbInput::optional_text(&body, "reason")?;
    let target = id.clone();
    with_conflict(&state, &id, move |b| async move {
        b.services()
            .questions
            .discard(&target, reason.as_deref(), &actor(b.as_ref()), &options)
            .await
    })
    .await
}

/// Öffnet eine entschiedene oder verworfene Frage wieder
async fn reopen(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let options = write_options(&body)?;
    let reason = DbInput::optional_text(&body, "reason")?;
    let target = id.clone();
    with_conflict(&state, &id, move |b| async move {
        b.services()
            .questions
            .reopen(&target, reason.as_deref(), &actor(b.as_ref()), &options)
            .await
    })
    .await
}

/// Der Mensch der Oberfläche
fn actor(b: &dyn BoardBackend) -> BoardQuestionActor {
    BoardQuestionActor {
        actor: b.actor(None),
        kind: AuthorKind::Human,
        session: None,
    }
}

/// Gelesene Version; Pflicht bei allen Änderungen mit Konfliktprüfung
fn write_options(body: &Body) -> Result<BoardQuestionWriteOptions> {
    let rev = DbInput::optional_int(body, "rev")?.ok_or_else(|| {
        HarnessError::new(
            "rev-missing",
            "Zum Ändern gehört die gelesene Version (rev).",
        )
    })?;
    Ok(BoardQuestionWriteOptions::rev(rev as i32))
}

/// Pflichtliste von Texten (darf leer sein)
fn required_list(body: &Body, key: &str) -> Result<Vec<String>> {
    DbInput::optional_list(body, key)?.ok_or_else(|| {
        HarnessError::new(
            "input-invalid",
            format!("Feld „{key}“ fehlt oder ist keine Liste von Texten."),
        )
        .into()
    })
}

/// Einzelner Text-Parameter der Adresse; mehrfach angegeben ist ungültig
fn param(query: &QueryMap, key: &str) -> Result<Option<String>> {
    match query.get(key) {
        QueryValue::Missing => Ok(None),
        QueryValue::One(value) => Ok(Some(value)),
        QueryValue::Many(_) => Err(HarnessError::new(
            "input-invalid",
            format!("Parameter „{key}“ muss genau einmal als Text angegeben werden."),
        )
        .into()),
    }
}

/// Zustandsfilter der Liste; nicht gesetzt oder `all` heißt „alle“
fn state_filter(raw: Option<String>) -> Result<Option<QuestionState>> {
    let value = raw
        .filter(|value| !value.is_empty())
        .unwrap_or("all".into());
    if value == "all" {
        return Ok(None);
    }
    QuestionState::ALL
        .into_iter()
        .find(|state| state.as_str() == value)
        .map(Some)
        .ok_or_else(|| {
            HarnessError::new(
                "input-invalid",
                format!("Filter „{value}“ ist ungültig. Erlaubt: open, answered, discarded, all."),
            )
            .into()
        })
}

/// Ein leerer Text gilt als nicht gesetzt (das `|| undefined` aus TypeScript)
fn set(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

/// Ein Feld, das fehlen oder `null` sein darf, als `Option<Option<…>>` der Dienste
fn nullable(value: Nullable<String>) -> Option<Option<String>> {
    match value {
        Nullable::Missing => None,
        Nullable::Null => Some(None),
        Nullable::Value(text) => Some(Some(text)),
    }
}

/// Ein Feld, das fehlen oder `null` sein darf, als einfacher Wert (beides heißt „nicht
/// gesetzt“)
fn value(value: Nullable<String>) -> Option<String> {
    match value {
        Nullable::Value(text) => Some(text),
        _ => None,
    }
}

/// Arbeit mit Konfliktbehandlung: 409 trägt den aktuellen Stand der Frage
async fn with_conflict<T, F, Fut>(state: &Arc<AppState>, id: &str, work: F) -> ApiResult<Response>
where
    T: serde::Serialize + Send + 'static,
    F: FnOnce(Arc<dyn BoardBackend>) -> Fut + Send,
    Fut: std::future::Future<Output = Result<T>> + Send,
{
    let question = id.to_string();
    conflict::with_conflict(state, StatusCode::OK, work, move |b| async move {
        b.services().questions.get(&question).await
    })
    .await
}
