//! Routengruppe `events`: `GET /api/projects/{pid}/events` als Ereignisstrom (Server-Sent
//! Events); portiert aus `packages/server/src/routes/EventRoutes.ts`.
//!
//! Der Körper wird von Hand gebaut (`Body::from_stream`) statt mit `axum::response::Sse`:
//! Nur so stehen genau die Bytes des Vertrags auf der Leitung – jedes Ereignis als
//! `data: <JSON>\n\n`, der Herzschlag als `: ping\n\n`. Die Anmeldungen am Verteiler, an den
//! Watchern und am Tab-Verzeichnis liegen im Strom selbst: geht der Browser, fällt der Strom
//! und mit ihm jede Anmeldung.

use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

use crate::error::ApiResult;
use crate::events::hub::LiveEvent;
use crate::events::{Bridge, Subscription, Watched, WatchedDocs};
use crate::extract::QueryMap;
use crate::notices::AttachedTab;
use crate::proxy::proxy_or_static;
use crate::state::AppState;

/// Herzschlag des Stroms; er hält Proxys und Browser bei der Stange
const PING: &[u8] = b": ping\n\n";

/// Meldet die Route an. Der Rückfall der Methoden gehört dazu: hat eine Rust-Route den Pfad,
/// aber nicht die Methode, darf axum kein 405 liefern.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/projects/{pid}/events",
        get(events).fallback(proxy_or_static),
    )
}

/// Der Ereignisstrom eines Projekts; ein unbekanntes Projekt wird mit 404 abgewiesen, **bevor**
/// der Strom beginnt
async fn events(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    state.projects.open(&pid)?;
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let notify = query.first("notify") == Some("1");
    let guards = Guards {
        _hub: state.events.subscribe(&pid, sender.clone()),
        _board: watched_id(&query, "dbBoard")
            .map(|id| state.board_changes.watch(id, sender.clone())),
        _questions: watched_id(&query, "dbProject")
            .map(|id| state.question_changes.watch(id, sender.clone())),
        _docs: state.spec_docs.watch(&pid, sender.clone()),
        _tab: notify.then(|| state.notice_tabs.attach(sender.clone())),
        // Sessions und die Meldungen des Node-Dispatchers entstehen noch dort
        _bridge: state.upstream.clone().map(|upstream| {
            Bridge::start(
                upstream,
                &pid,
                notify,
                state.bridged_events.clone(),
                sender,
                state.log.clone(),
                &state.shutdown,
            )
        }),
    };
    Ok(stream(receiver, guards, &state))
}

/// Die ID eines Abfrage-Parameters, sofern es eine ganze Zahl über null ist
fn watched_id(query: &QueryMap, name: &str) -> Option<i32> {
    query.first(name)?.parse::<i32>().ok().filter(|id| *id > 0)
}

/// Die Antwort mit dem Strom: Köpfe des Vertrags, `hello` und danach Ereignisse und
/// Herzschlag, bis der Browser geht oder der Server endet
fn stream(receiver: UnboundedReceiver<LiveEvent>, guards: Guards, state: &AppState) -> Response {
    let open = Open {
        receiver,
        heartbeat: heartbeat(state.config.heartbeat_ms),
        shutdown: state.shutdown.clone(),
        _guards: guards,
    };
    let frames = futures::stream::unfold(open, |mut open| async move {
        let frame = tokio::select! {
            event = open.receiver.recv() => Bytes::from(event?.frame()),
            _ = open.heartbeat.tick() => Bytes::from_static(PING),
            _ = open.shutdown.cancelled() => return None,
        };
        Some((Ok::<Bytes, std::io::Error>(frame), open))
    });
    let hello = futures::stream::once(async { Ok(Bytes::from(LiveEvent::Hello.frame())) });
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache, no-transform"),
            (header::CONNECTION, "keep-alive"),
            // Kein Puffern in einem Proxy davor
            (header::HeaderName::from_static("x-accel-buffering"), "no"),
        ],
        Body::from_stream(futures::StreamExt::chain(hello, frames)),
    )
        .into_response()
}

/// Der Takt des Herzschlags; der erste Schlag kommt erst nach einem Takt
fn heartbeat(heartbeat_ms: u64) -> tokio::time::Interval {
    let period = Duration::from_millis(heartbeat_ms.max(1));
    tokio::time::interval_at(tokio::time::Instant::now() + period, period)
}

/// Was ein offener Strom braucht
struct Open {
    receiver: UnboundedReceiver<LiveEvent>,
    heartbeat: tokio::time::Interval,
    shutdown: CancellationToken,
    _guards: Guards,
}

/// Alle Anmeldungen eines Stroms. Niemand liest sie – sie wirken erst, wenn sie fallen: mit
/// dem Strom endet jede Anmeldung.
struct Guards {
    _hub: Subscription,
    _board: Option<Watched>,
    _questions: Option<Watched>,
    _docs: WatchedDocs,
    _tab: Option<AttachedTab>,
    _bridge: Option<Bridge>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::QueryValue;

    /// Abfrageteil als Tabelle
    fn query(entries: &[(&str, &str)]) -> QueryMap {
        QueryMap::parse(
            &entries
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("&"),
        )
    }

    #[test]
    fn nimmt_nur_ganze_zahlen_uber_null_als_beobachtete_id() {
        assert_eq!(watched_id(&query(&[("dbBoard", "7")]), "dbBoard"), Some(7));
        for roh in ["0", "-1", "abc", "1.5", "", "1x"] {
            assert_eq!(
                watched_id(&query(&[("dbBoard", roh)]), "dbBoard"),
                None,
                "{roh}"
            );
        }
        assert_eq!(watched_id(&query(&[]), "dbBoard"), None);
        assert_eq!(query(&[]).get("dbBoard"), QueryValue::Missing);
    }
}
