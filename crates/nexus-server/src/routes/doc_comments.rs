//! Routengruppe `docs`, Teil Kommentare: `…/docs/comments` lesen, anlegen, ändern und löschen
//! (schreibt nur `*.comments.json` neben das Dokument); portiert aus
//! `packages/server/src/routes/DocCommentsRoutes.ts`.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use nexus_core::docs::{DocCommentPatch, DocCommentStatus, DocCommentStore, NewDocComment};
use nexus_core::{HarnessError, Result};
use serde_json::Value;

use crate::error::{json, ApiResult};
use crate::extract::{JsonBody, QueryMap};
use crate::proxy::proxy_or_static;
use crate::routes::docs::source;
use crate::state::AppState;

/// Höchstlänge der markierten Stelle eines Kommentars
const MAX_QUOTE: usize = 500;

/// Meldet die vier Routen der Kommentare an
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/projects/{pid}/docs/comments",
            get(list_comments)
                .post(add_comment)
                .fallback(proxy_or_static),
        )
        .route(
            "/api/projects/{pid}/docs/comments/{id}",
            axum::routing::patch(update_comment)
                .delete(remove_comment)
                .fallback(proxy_or_static),
        )
}

/// Alle Kommentare eines Dokuments
async fn list_comments(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let (store, path) = open(&state, &pid, &query).await?;
    Ok(json(&store.list(&path)?))
}

/// Legt einen offenen Kommentar an
async fn add_comment(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let input = valid_new(&body)?;
    let (store, path) = open(&state, &pid, &query).await?;
    Ok(json(&store.add(&path, &input)?))
}

/// Ändert Text, Status oder Antwort eines Kommentars
async fn update_comment(
    State(state): State<Arc<AppState>>,
    UrlPath((pid, id)): UrlPath<(String, String)>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let (store, path) = open(&state, &pid, &query).await?;
    Ok(json(&store.update(&path, &id, &valid_patch(&body))?))
}

/// Löscht einen Kommentar
async fn remove_comment(
    State(state): State<Arc<AppState>>,
    UrlPath((pid, id)): UrlPath<(String, String)>,
    query: QueryMap,
    // Der Körper wird nicht gelesen, aber wie in Fastify geprüft: leeres JSON ist 400
    JsonBody(_): JsonBody,
) -> ApiResult<Response> {
    let (store, path) = open(&state, &pid, &query).await?;
    store.remove(&path, &id)?;
    Ok(json(&serde_json::json!({ "ok": true })))
}

/// Projekt öffnen, Quelle auflösen, Dokumentpfad prüfen und Store im Repo des Boards
/// bereitstellen
async fn open(
    state: &Arc<AppState>,
    pid: &str,
    query: &QueryMap,
) -> Result<(DocCommentStore, String)> {
    let source = source(state, pid, query).await?;
    // Vor jedem Schreibzugriff: ohne Quelle gibt es kein Dokument, zu dem ein Kommentar
    // gehören könnte
    let Some(root) = source.root.clone() else {
        return Err(HarnessError::new(
            "doc-not-found",
            "Zu diesem Projekt gibt es keine Dokumente.",
        )
        .into());
    };
    let path = state.docs.assert_document(&source, query.first("path"))?;
    Ok((DocCommentStore::new(root), path))
}

/// Prüft den Körper eines neuen Kommentars
fn valid_new(body: &Value) -> Result<NewDocComment> {
    let text = body
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let quote = body
        .get("quote")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if text.is_empty() || quote.is_empty() {
        return Err(HarnessError::new(
            "comment-invalid",
            "Ein Kommentar braucht einen Text und eine markierte Stelle.",
        )
        .into());
    }
    Ok(NewDocComment {
        text: text.to_string(),
        quote: quote.chars().take(MAX_QUOTE).collect(),
        heading: body
            .get("heading")
            .and_then(Value::as_str)
            .map(str::to_string),
        lines: lines(body.get("lines")),
    })
}

/// Genau zwei ganze Zahlen; alles andere bleibt leer
fn lines(value: Option<&Value>) -> Option<[i64; 2]> {
    let items = value?.as_array()?;
    match items.as_slice() {
        [start, end] => Some([start.as_i64()?, end.as_i64()?]),
        _ => None,
    }
}

/// Prüft eine Änderung (nur Text, Status, Antwort)
fn valid_patch(body: &Value) -> DocCommentPatch {
    DocCommentPatch {
        text: body
            .get("text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string),
        status: match body.get("status").and_then(Value::as_str) {
            Some("open") => Some(DocCommentStatus::Open),
            Some("resolved") => Some(DocCommentStatus::Resolved),
            _ => None,
        },
        resolution: match body.get("resolution") {
            Some(Value::Null) => Some(None),
            Some(Value::String(text)) => Some(Some(text.clone())),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verlangt_text_und_zitat() {
        for body in [
            serde_json::json!({}),
            serde_json::json!({ "text": "  ", "quote": "Das Ticket" }),
            serde_json::json!({ "text": "Bitte prüfen", "quote": "" }),
        ] {
            assert_eq!(
                valid_new(&body).unwrap_err().code(),
                Some("comment-invalid"),
                "{body}"
            );
        }
    }

    #[test]
    fn kurzt_das_zitat_und_nimmt_nur_zwei_zeilennummern() {
        let input = valid_new(&serde_json::json!({
            "text": " Bitte prüfen ", "quote": "ä".repeat(600), "heading": "Ziel", "lines": [3, 5],
        }))
        .unwrap();

        assert_eq!(input.text, "Bitte prüfen");
        assert_eq!(input.quote.chars().count(), MAX_QUOTE);
        assert_eq!(input.heading.as_deref(), Some("Ziel"));
        assert_eq!(input.lines, Some([3, 5]));

        for raw in [
            serde_json::json!([1, 2, 3]),
            serde_json::json!([1]),
            serde_json::json!(["a", "b"]),
            serde_json::json!([1.5, 2]),
            serde_json::Value::Null,
        ] {
            let body = serde_json::json!({ "text": "x", "quote": "y", "lines": raw });
            assert_eq!(valid_new(&body).unwrap().lines, None, "{raw}");
        }
    }

    #[test]
    fn ubernimmt_aus_der_anderung_nur_text_status_und_antwort() {
        let patch = valid_patch(&serde_json::json!({
            "text": " neu ", "status": "resolved", "resolution": "erledigt", "id": "K-1",
        }));
        assert_eq!(patch.text.as_deref(), Some("neu"));
        assert_eq!(patch.status, Some(DocCommentStatus::Resolved));
        assert_eq!(patch.resolution, Some(Some("erledigt".into())));

        let leer = valid_patch(&serde_json::json!({ "text": "  ", "status": "wartet" }));
        assert_eq!(leer, DocCommentPatch::default());

        let null = valid_patch(&serde_json::json!({ "resolution": null }));
        assert_eq!(null.resolution, Some(None));
    }
}
