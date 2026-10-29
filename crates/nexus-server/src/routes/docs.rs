//! Routengruppe `docs`: Dokumentliste, Lesen, Suche, Abschnitt schreiben, Öffnen im Explorer
//! oder in VS Code, gespeicherte Ticket-Entwürfe und Tickets aus Dokumenten; portiert aus den
//! Claude-freien Teilen von `packages/server/src/routes/DocsRoutes.ts`.
//!
//! Die sechs Routen, die Claude aufrufen (`POST`/`DELETE …/docs/ticket-drafts`,
//! `POST`/`DELETE …/docs/question-drafts`, `POST …/docs/questions`, `POST …/docs/revise`),
//! gehören zur Gruppe `docs-claude` ([`crate::routes::docs_claude`]) und werden hier **nicht**
//! angemeldet.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::Router;
use nexus_core::{HarnessError, Result};
use nexus_platform::OpenTarget;
use serde_json::Value;

use crate::docs::doc_ticket_service::DocTicketRequest;
use crate::docs::docs_root_resolver::{DocsRootResolver, DocsSource};
use crate::docs::docs_service::WriteSectionInput;
use crate::error::{json, ApiResult};
use crate::extract::{JsonBody, QueryMap};
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Meldet die Routen der Gruppe an. Der Rückfall der Methoden gehört zu jeder Route: hat eine
/// Rust-Route den Pfad, aber nicht die Methode, darf axum kein 405 liefern.
///
/// `…/docs/ticket-drafts` (POST/DELETE) und `…/docs/ticket-drafts/saved` sind verschiedene
/// Pfade – nur `saved` wird hier angemeldet, der andere läuft in den Rückfall.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/projects/{pid}/docs",
            get(list_docs).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/docs/file",
            get(read_doc).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/docs/search",
            get(search_docs).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/docs/section",
            put(write_section).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/docs/open",
            post(open_doc).fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/docs/ticket-drafts/saved",
            get(saved_drafts)
                .put(save_drafts)
                .delete(discard_drafts)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/docs/tickets",
            get(list_tickets)
                .post(create_tickets)
                .fallback(static_or_not_found),
        )
}

/// Die Dokumente des Boards; gespeicherte Ticket-Entwürfe sind markiert
async fn list_docs(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let list = state.doc_drafts.flag(state.docs.list(&source));
    Ok(json(&list))
}

/// Inhalt, Fingerabdruck und Abschnitte eines freigegebenen Dokuments
async fn read_doc(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    Ok(json(&state.docs.read(&source, query.first("path"))?))
}

/// Teilstringsuche über alle freigegebenen Dokumente
async fn search_docs(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let hits = state
        .docs
        .search(&source, query.first("q").unwrap_or_default())?;
    Ok(json(&hits))
}

/// Ersetzt einen Zeilenbereich einer Spec oder eines Plans
async fn write_section(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let input = valid_section(&body)?;
    let source = source(&state, &pid, &query).await?;
    Ok(json(&state.docs.write_section(&source, &input).await?))
}

/// Öffnet das Dokument auf diesem Rechner im Explorer (markiert) oder in VS Code
async fn open_doc(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let target = match body.get("target").and_then(Value::as_str) {
        Some("explorer") => OpenTarget::Explorer,
        Some("vscode") => OpenTarget::Vscode,
        _ => {
            return Err(HarnessError::new(
                "request-invalid",
                "Ziel muss „explorer“ oder „vscode“ sein.",
            )
            .into())
        }
    };
    let file = state
        .docs
        .full_path(&source, body.get("path").and_then(Value::as_str))?;
    state.platform.files.open(&file, target).await?;
    Ok(json(&serde_json::json!({ "ok": true })))
}

/// Gespeicherte Entwürfe eines Dokuments (`{ result: null }`, wenn es keine gibt)
async fn saved_drafts(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let result = state
        .doc_drafts
        .saved(&state.docs, &source, query.first("path"))?;
    Ok(json(&serde_json::json!({ "result": result })))
}

/// Speichert die im Dialog bearbeiteten Entwürfe
async fn save_drafts(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let result = state
        .doc_drafts
        .update(
            &state.docs,
            &source,
            body.get("path").and_then(Value::as_str),
            body.get("drafts").unwrap_or(&Value::Null),
        )
        .await?;
    Ok(json(&serde_json::json!({ "result": result })))
}

/// Verwirft die gespeicherten Entwürfe eines Dokuments
async fn discard_drafts(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    // Der Körper wird nicht gelesen, aber wie in Fastify geprüft: leeres JSON ist 400
    JsonBody(_): JsonBody,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    state
        .doc_drafts
        .discard(&state.docs, &source, query.first("path"))
        .await?;
    Ok(json(&serde_json::json!({ "ok": true })))
}

/// Tickets, die aus diesem Dokument entstanden sind, samt Überschrift und Zeilen ihrer Quelle
async fn list_tickets(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let tickets = state
        .doc_tickets
        .list_for(&state.board, &state.docs, &source, query.first("path"))
        .await?;
    Ok(json(&tickets))
}

/// Legt die geprüften Entwürfe als Tickets auf dem Board des Ordners an
async fn create_tickets(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let source = source(&state, &pid, &query).await?;
    let request = DocTicketRequest {
        path: body.get("path").and_then(Value::as_str).map(str::to_string),
        drafts: body
            .get("drafts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    };
    let result = state
        .doc_tickets
        .create(&state.board, &state.docs, &source, &request)
        .await?;
    // Angelegt: die gespeicherten Entwürfe haben ihren Zweck erfüllt
    let _ = state
        .doc_drafts
        .discard(&state.docs, &source, request.path.as_deref())
        .await;
    Ok(json(&result))
}

/// Projekt öffnen und die Quelle der Dokumente auflösen (Board aus der Query oder das des
/// Ordners)
pub(crate) async fn source(
    state: &Arc<AppState>,
    pid: &str,
    query: &QueryMap,
) -> Result<DocsSource> {
    let project = state.projects.open(pid)?;
    DocsRootResolver::for_state(state)
        .resolve_for(&project.project.root, query.first("board"))
        .await
}

/// Prüft den Körper der Schreibroute; Zeilenfelder getrennt, damit der Client 400 statt 422
/// sieht
fn valid_section(body: &Value) -> Result<WriteSectionInput> {
    let text = |key: &str| {
        body.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string()
    };
    let path = text("path");
    let expected_hash = text("expectedHash");
    let content = body.get("content").and_then(Value::as_str);
    // Ein leerer Inhalt ist erlaubt: so lässt sich ein Abschnitt leeren
    let (Some(content), false, false) = (content, path.is_empty(), expected_hash.is_empty()) else {
        return Err(HarnessError::new(
            "request-invalid",
            "Zum Schreiben braucht es Pfad, Hash und Inhalt.",
        )
        .into());
    };
    let (Some(start_line), Some(end_line)) = (line(body, "startLine"), line(body, "endLine"))
    else {
        return Err(range_invalid());
    };
    if start_line < 1 || end_line < start_line {
        return Err(range_invalid());
    }
    Ok(WriteSectionInput {
        path,
        start_line,
        end_line,
        content: content.to_string(),
        expected_hash,
    })
}

/// Eine Zeilennummer aus dem Körper; nur ganze Zahlen zählen (wie `Number.isInteger`)
fn line(body: &Value, key: &str) -> Option<i64> {
    body.get(key)?.as_i64()
}

/// „Anfang und Ende müssen ganze Zeilennummern ab 1 sein“ – wortgleich zu TypeScript
fn range_invalid() -> nexus_core::Error {
    HarnessError::new(
        "doc-range-invalid",
        "Anfang und Ende müssen ganze Zeilennummern ab 1 sein, das Ende nicht vor dem Anfang.",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Code eines Fehlers der Prüfung
    fn code(body: serde_json::Value) -> Option<String> {
        valid_section(&body).unwrap_err().code().map(str::to_string)
    }

    #[test]
    fn verlangt_pfad_hash_und_inhalt() {
        for body in [
            serde_json::json!({}),
            serde_json::json!({ "path": "a.md", "startLine": 1, "endLine": 2, "content": "x" }),
            serde_json::json!({ "startLine": 1, "endLine": 2, "content": "x", "expectedHash": "h" }),
            serde_json::json!({ "path": "a.md", "startLine": 1, "endLine": 2, "expectedHash": "h" }),
        ] {
            assert_eq!(code(body.clone()), Some("request-invalid".into()), "{body}");
        }
    }

    #[test]
    fn lehnt_ungultige_zeilenbereiche_ab() {
        for (start, end) in [
            (serde_json::json!(0), serde_json::json!(2)),
            (serde_json::json!(5), serde_json::json!(4)),
            (serde_json::json!(-1), serde_json::json!(1)),
            (serde_json::json!(1.5), serde_json::json!(2)),
            (serde_json::json!("1"), serde_json::json!(2)),
            (serde_json::Value::Null, serde_json::json!(2)),
        ] {
            let body = serde_json::json!({
                "path": "a.md", "expectedHash": "h", "content": "x",
                "startLine": start, "endLine": end,
            });
            assert_eq!(
                code(body.clone()),
                Some("doc-range-invalid".into()),
                "{body}"
            );
        }
    }

    #[test]
    fn nimmt_einen_leeren_inhalt_an() {
        let input = valid_section(&serde_json::json!({
            "path": " a.md ", "expectedHash": " h ", "content": "", "startLine": 3, "endLine": 3,
        }))
        .unwrap();

        assert_eq!(input.path, "a.md");
        assert_eq!(input.expected_hash, "h");
        assert_eq!(input.content, "");
        assert_eq!((input.start_line, input.end_line), (3, 3));
    }
}
