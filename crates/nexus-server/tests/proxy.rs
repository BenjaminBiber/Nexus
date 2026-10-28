//! Reverse-Proxy zum Node-Server: Weiterleitung, Umschreiben der Köpfe, Streaming,
//! nicht erreichbarer Upstream, Schutz vor dem Upstream und der Notschalter der
//! Routengruppen.
//!
//! Der „Node-Server“ ist hier ein kleiner axum-Server auf `127.0.0.1:0`, der mitschreibt, was
//! bei ihm ankommt.

mod common;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use common::{TestServer, TestServerOptions};
use futures::StreamExt as _;
use http_body_util::BodyStream;
use nexus_server::groups::RouteGroups;

/// Was beim Upstream angekommen ist
#[derive(Debug, Clone, Default)]
struct Seen {
    method: String,
    path: String,
    query: String,
    body: String,
    headers: BTreeMap<String, String>,
}

/// Der Test-Upstream: zählt Anfragen, merkt sich die letzte und antwortet nach Pfad
#[derive(Default)]
struct Upstream {
    calls: AtomicUsize,
    last: Mutex<Seen>,
}

impl Upstream {
    /// Zahl der Anfragen, die wirklich angekommen sind
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

/// Startet den Test-Upstream auf einem freien Port
async fn start_upstream() -> (Arc<Upstream>, String, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Upstream::default());
    let router = Router::new()
        .route("/{*rest}", any(handle))
        .route("/", any(handle))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address: SocketAddr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (state, format!("http://{address}"), task)
}

/// Antwort des Test-Upstreams: Ereignisstrom, Fehlerstatus oder eine Auskunft über die Anfrage
async fn handle(State(state): State<Arc<Upstream>>, request: Request) -> Response {
    state.calls.fetch_add(1, Ordering::SeqCst);
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 1024 * 1024).await.unwrap();
    let seen = Seen {
        method: parts.method.to_string(),
        path: parts.uri.path().to_string(),
        query: parts.uri.query().unwrap_or_default().to_string(),
        body: String::from_utf8_lossy(&bytes).into_owned(),
        headers: parts
            .headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect(),
    };
    *state.last.lock().expect("Sperre") = seen.clone();

    match seen.path.as_str() {
        // Ein Strom, der erst nach zwei Sekunden endet; das erste Ereignis kommt sofort
        "/api/events/strom" => {
            let stream = futures::stream::once(async { Ok::<_, std::io::Error>("data: 1\n\n") })
                .chain(futures::stream::once(async {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    Ok("data: 2\n\n")
                }));
            (
                [("content-type", "text/event-stream")],
                Body::from_stream(stream),
            )
                .into_response()
        }
        "/api/kaputt" => (
            axum::http::StatusCode::IM_A_TEAPOT,
            [("content-type", "application/json; charset=utf-8")],
            r#"{"code":"teekanne","message":"Ich bin eine Teekanne."}"#,
        )
            .into_response(),
        _ => (
            [("content-type", "application/json; charset=utf-8")],
            serde_json::to_string(&serde_json::json!({
                "method": seen.method,
                "path": seen.path,
                "query": seen.query,
                "body": seen.body,
                "host": seen.headers.get("host"),
                "origin": seen.headers.get("origin"),
                "harness": seen.headers.get("x-harness"),
            }))
            .unwrap(),
        )
            .into_response(),
    }
}

/// Server mit Upstream
async fn with_upstream() -> (
    TestServer,
    Arc<Upstream>,
    String,
    tokio::task::JoinHandle<()>,
) {
    let (upstream, base, task) = start_upstream().await;
    let server = TestServer::with(TestServerOptions {
        upstream: Some(base.clone()),
        ..TestServerOptions::default()
    });
    (server, upstream, base, task)
}

#[tokio::test]
async fn leitet_methode_pfad_abfrage_und_korper_unverandert_weiter() {
    let (server, upstream, _base, task) = with_upstream().await;
    let reply = server
        .post("/api/db/boards?a=1&a=2", r#"{"path":"/x"}"#)
        .await;
    assert_eq!(reply.status, 200);
    let json = reply.json().unwrap();
    assert_eq!(json["method"], serde_json::json!("POST"));
    assert_eq!(json["path"], serde_json::json!("/api/db/boards"));
    assert_eq!(json["query"], serde_json::json!("a=1&a=2"));
    assert_eq!(json["body"], serde_json::json!(r#"{"path":"/x"}"#));
    assert_eq!(json["harness"], serde_json::json!("1"));
    // Weitergeleitete Antworten sind als solche gekennzeichnet
    assert_eq!(reply.header("x-nexus-backend"), Some("node"));
    assert_eq!(upstream.calls(), 1);
    task.abort();
}

#[tokio::test]
async fn gibt_den_statuscode_des_upstreams_unverandert_zuruck() {
    let (server, _upstream, _base, task) = with_upstream().await;
    let reply = server.get("/api/kaputt").await;
    assert_eq!(reply.status, 418);
    assert_eq!(reply.code().as_deref(), Some("teekanne"));
    task.abort();
}

#[tokio::test]
async fn schreibt_host_und_herkunft_auf_den_upstream_um() {
    let (server, _upstream, base, task) = with_upstream().await;
    let authority = base.trim_start_matches("http://").to_string();
    let reply = server
        .get_with(
            "/api/db/boards",
            &[("origin", Some("http://127.0.0.1:4710"))],
        )
        .await;
    let json = reply.json().unwrap();
    assert_eq!(json["host"], serde_json::json!(authority));
    assert_eq!(json["origin"], serde_json::json!(base));
    task.abort();
}

#[tokio::test]
async fn ein_strom_des_upstreams_kommt_stuck_fur_stuck_an() {
    let (server, _upstream, _base, task) = with_upstream().await;
    let started = Instant::now();
    let response = server.send("GET", "/api/events/strom", None, &[]).await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let mut frames = BodyStream::new(response.into_body());
    let first = frames.next().await.expect("erstes Ereignis").unwrap();
    assert_eq!(
        String::from_utf8_lossy(first.into_data().unwrap().as_ref()),
        "data: 1\n\n"
    );
    // Das erste Ereignis ist da, lange bevor der Strom endet (der Upstream wartet 2 s)
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    task.abort();
}

#[tokio::test]
async fn ein_nicht_erreichbarer_upstream_wird_zu_503_upstream_unavailable() {
    let (upstream, base, task) = start_upstream().await;
    task.abort();
    // Warten, bis der Port wirklich zu ist
    tokio::time::sleep(Duration::from_millis(50)).await;
    let server = TestServer::with(TestServerOptions {
        upstream: Some(base),
        ..TestServerOptions::default()
    });
    let reply = server.get("/api/db/boards").await;
    assert_eq!(reply.status, 503);
    assert_eq!(
        reply.json().unwrap(),
        serde_json::json!({
            "code": "upstream-unavailable",
            "message": "Der Node-Server antwortet nicht.",
        })
    );
    assert_eq!(upstream.calls(), 0);
    // Der Vorfall steht im Protokoll
    let log = std::fs::read_to_string(server.home.log_file()).unwrap_or_default();
    assert!(log.contains("Node-Server antwortet nicht"), "{log}");
}

#[tokio::test]
async fn der_schutz_blockt_vor_dem_upstream() {
    let (server, upstream, _base, task) = with_upstream().await;
    let fremd = server
        .get_with("/api/projects", &[("host", Some("evil.example"))])
        .await;
    assert_eq!(fremd.status, 403);
    assert_eq!(fremd.code().as_deref(), Some("host-denied"));

    let ohne_kopf = server
        .request("POST", "/api/projects", Some("{}"), &[("x-harness", None)])
        .await;
    assert_eq!(ohne_kopf.status, 403);
    assert_eq!(ohne_kopf.code().as_deref(), Some("header-missing"));

    // Keine der beiden Anfragen hat den Upstream erreicht
    assert_eq!(upstream.calls(), 0);
    task.abort();
}

#[tokio::test]
async fn eigene_routen_beantwortet_rust_der_rest_geht_zum_upstream() {
    let (server, upstream, _base, task) = with_upstream().await;
    let health = server.get("/api/health").await;
    assert_eq!(
        health.json().unwrap()["name"],
        serde_json::json!("ai-harness")
    );
    assert_eq!(health.header("x-nexus-backend"), Some("rust"));
    assert_eq!(upstream.calls(), 0);

    // Eine falsche Methode auf einem bekannten Pfad geht weiter, statt 405 zu liefern
    let post = server.post("/api/health", "{}").await;
    assert_eq!(post.status, 200);
    assert_eq!(post.header("x-nexus-backend"), Some("node"));
    assert_eq!(upstream.calls(), 1);
    task.abort();
}

#[tokio::test]
async fn der_notschalter_schickt_eine_routengruppe_zum_node_server() {
    let (upstream, base, task) = start_upstream().await;
    let server = TestServer::with(TestServerOptions {
        upstream: Some(base),
        proxy_groups: RouteGroups::from_env(Some("basis")),
        ..TestServerOptions::default()
    });
    let health = server.get("/api/health").await;
    assert_eq!(health.status, 200);
    assert_eq!(health.header("x-nexus-backend"), Some("node"));
    assert_eq!(
        health.json().unwrap()["path"],
        serde_json::json!("/api/health")
    );
    assert_eq!(upstream.calls(), 1);
    task.abort();
}

#[tokio::test]
async fn ohne_upstream_gibt_es_fur_unbekannte_api_pfade_404() {
    let server = TestServer::start();
    let reply = server.get("/api/db/boards").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.code().as_deref(), Some("not-found"));
    assert_eq!(reply.header("x-nexus-backend"), Some("rust"));
}

#[tokio::test]
async fn statische_adressen_gehen_nie_zum_upstream() {
    let (server, upstream, _base, task) = with_upstream().await;
    // Ohne gebaute Oberfläche: Hinweis bzw. 404, aber kein Proxy
    assert_eq!(server.get("/").await.status, 200);
    assert_eq!(server.get("/fehlt.js").await.status, 404);
    assert_eq!(upstream.calls(), 0);
    task.abort();
}
