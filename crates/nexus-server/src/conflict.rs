//! Die gemeinsame Konfliktbehandlung der schreibenden `/api/db`-Routen.
//!
//! Alle Routen, die auf einer gelesenen Version (`rev`) schreiben, antworten gleich: geht die
//! Arbeit durch, steht ihr Ergebnis im Körper; meldet der Dienst `conflict`, wird daraus 409
//! mit dem aktuellen Stand des Gegenstands unter `current`. Welcher Gegenstand das ist – ein
//! Ticket, eine Frage –, sagt die Lesefunktion des Aufrufers.

use std::future::Future;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use nexus_core::{Error, Result};
use serde::Serialize;

use crate::board_runtime::BoardBackend;
use crate::error::{json, ApiResult};
use crate::state::AppState;

/// Führt die Arbeit auf der Board-Laufzeit aus und antwortet mit `status`. Meldet der Dienst
/// `conflict` (veraltete Version), liest `current` den aktuellen Stand und die Antwort ist 409
/// `{ code, message, current }`.
pub async fn with_conflict<T, V, W, WFut, C, CFut>(
    state: &Arc<AppState>,
    status: StatusCode,
    work: W,
    current: C,
) -> ApiResult<Response>
where
    T: Serialize + Send + 'static,
    V: Serialize + Send + 'static,
    W: FnOnce(Arc<dyn BoardBackend>) -> WFut + Send,
    WFut: Future<Output = Result<T>> + Send,
    C: FnOnce(Arc<dyn BoardBackend>) -> CFut + Send,
    CFut: Future<Output = Result<V>> + Send,
{
    let (status, body) = state
        .board
        .run(|b| async move {
            match work(b.clone()).await {
                Ok(value) => Ok((status, serde_json::to_value(value).map_err(Error::unexpected)?)),
                Err(error) if error.code() == Some("conflict") => {
                    let message = error.to_string();
                    let current = current(b).await?;
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
