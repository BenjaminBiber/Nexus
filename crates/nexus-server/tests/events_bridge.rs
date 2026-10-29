//! Die Brücke zum Ereignisstrom des Node-Servers: `sessions` und `notice` kommen von dort
//! an, `hello` und Herzschlag nicht doppelt, die Upstream-Verbindung endet mit dem Client.
//!
//! Der „Node-Server“ ist hier ein kleiner axum-Server auf `127.0.0.1:0`, der sich merkt, was
//! bei ihm ankommt und wie viele Ströme er offen hat.

mod common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use common::{TestServer, TestServerOptions};
use futures::StreamExt as _;
use http_body_util::BodyStream;

/// Der Test-Upstream: zählt offene Ströme und merkt sich die zuletzt angefragte Adresse
#[derive(Default)]
struct Upstream {
    open: Arc<AtomicUsize>,
    calls: AtomicUsize,
    last: Mutex<String>,
}

/// Startet den Test-Upstream auf einem freien Port
async fn start_upstream() -> (Arc<Upstream>, String, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Upstream::default());
    let router = Router::new()
        .route("/{*rest}", any(handle))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address: SocketAddr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (state, format!("http://{address}"), task)
}

/// Der Ereignisstrom des Test-Upstreams: Begrüßung, Herzschlag, `sessions` und `notice`
async fn handle(State(state): State<Arc<Upstream>>, request: Request) -> Response {
    state.calls.fetch_add(1, Ordering::SeqCst);
    let uri = request.uri();
    *state.last.lock().expect("Sperre") = format!(
        "{}{}",
        uri.path(),
        uri.query().map(|q| format!("?{q}")).unwrap_or_default()
    );
    state.open.fetch_add(1, Ordering::SeqCst);
    // Zählt die offenen Ströme herunter, sobald der Körper fällt (der Client ist weg)
    let offen = Offen(state.open.clone());
    let frames = futures::stream::iter([
        "data: {\"type\":\"hello\"}\n\n",
        ": ping\n\n",
        "data: {\"type\":\"sessions\"}\n\n",
        "data: {\"type\":\"notice\",\"kind\":\"wartet\",\"tag\":\"attention\",\"title\":\"T\",\"body\":\"B\",\"url\":\"/p/x/sessions\"}\n\n",
        "data: {\"type\":\"dbboard\",\"boardId\":9}\n\n",
    ])
    .map(Ok::<_, std::io::Error>)
    // Danach bleibt der Strom offen, bis der Client geht
    .chain(futures::stream::unfold(offen, |offen| async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Some((Ok(": ping\n\n"), offen))
    }));
    (
        [("content-type", "text/event-stream")],
        Body::from_stream(frames),
    )
        .into_response()
}

/// Hält einen offenen Strom des Upstreams fest
struct Offen(Arc<AtomicUsize>);

impl Drop for Offen {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Rust-Server mit Test-Upstream und einem angemeldeten Projekt
async fn with_upstream() -> (
    TestServer,
    String,
    common::TempRepo,
    Arc<Upstream>,
    tokio::task::JoinHandle<()>,
) {
    let (upstream, base, task) = start_upstream().await;
    let server = TestServer::with(TestServerOptions {
        upstream: Some(base),
        // Der eigene Herzschlag darf den des Upstreams im Test nicht verdecken
        heartbeat_ms: Some(60_000),
        ..TestServerOptions::default()
    });
    let (repo, pid) = server.add_project();
    (server, pid, repo, upstream, task)
}

/// Liest den Strom, bis der Text vorkommt oder die Zeit um ist
async fn until(frames: &mut BodyStream<Body>, text: &mut String, needle: &str) -> bool {
    if text.contains(needle) {
        return true;
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(Ok(frame)) = frames.next().await {
            if let Ok(data) = frame.into_data() {
                text.push_str(&String::from_utf8_lossy(&data));
                if text.contains(needle) {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false)
}

#[tokio::test]
async fn reicht_sessions_und_notice_des_node_servers_weiter_ohne_hello_und_herzschlag_zu_doppeln() {
    let (server, pid, _repo, _upstream, task) = with_upstream().await;
    let response = server
        .send("GET", &format!("/api/projects/{pid}/events"), None, &[])
        .await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers().get("x-nexus-backend").unwrap(), "rust");
    let mut frames = BodyStream::new(response.into_body());
    let mut text = String::new();

    assert!(until(&mut frames, &mut text, "data: {\"type\":\"sessions\"}\n\n").await);
    assert!(
        until(
            &mut frames,
            &mut text,
            r#"data: {"type":"notice","kind":"wartet","tag":"attention","title":"T","body":"B","url":"/p/x/sessions"}"#
        )
        .await
    );
    // Genau eine Begrüßung – die des Rust-Servers; `: ping` und `dbboard` des Upstreams
    // bleiben liegen
    assert_eq!(text.matches("\"type\":\"hello\"").count(), 1, "{text}");
    assert!(!text.contains(": ping"), "{text}");
    assert!(!text.contains("dbboard"), "{text}");

    drop(frames);
    task.abort();
}

#[tokio::test]
async fn fragt_beim_node_server_ohne_dbboard_und_dbproject_und_mit_notify_wie_der_client() {
    let (server, pid, _repo, upstream, task) = with_upstream().await;
    let response = server
        .send(
            "GET",
            &format!("/api/projects/{pid}/events?dbBoard=3&dbProject=4&notify=1"),
            None,
            &[],
        )
        .await;
    let mut frames = BodyStream::new(response.into_body());
    let mut text = String::new();
    assert!(until(&mut frames, &mut text, "sessions").await);
    assert_eq!(
        *upstream.last.lock().expect("Sperre"),
        format!("/api/projects/{pid}/events?notify=1")
    );

    drop(frames);
    task.abort();
}

#[tokio::test]
async fn schliesst_die_verbindung_zum_node_server_wenn_der_client_geht() {
    let (server, pid, _repo, upstream, task) = with_upstream().await;
    let response = server
        .send("GET", &format!("/api/projects/{pid}/events"), None, &[])
        .await;
    let mut frames = BodyStream::new(response.into_body());
    let mut text = String::new();
    assert!(until(&mut frames, &mut text, "sessions").await);
    assert_eq!(upstream.open.load(Ordering::SeqCst), 1);

    drop(frames);
    // Der Upstream merkt das Ende erst, wenn er wieder schreibt
    let mut offen = 1;
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        offen = upstream.open.load(Ordering::SeqCst);
        if offen == 0 {
            break;
        }
    }
    assert_eq!(offen, 0, "die Verbindung zum Node-Server blieb offen");
    // Und sie wird nicht wieder aufgebaut
    let calls = upstream.calls.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(upstream.calls.load(Ordering::SeqCst), calls);

    task.abort();
}

#[tokio::test]
async fn ohne_upstream_gibt_es_keine_brucke() {
    let server = TestServer::with(TestServerOptions {
        heartbeat_ms: Some(200),
        ..TestServerOptions::default()
    });
    let (_repo, pid) = server.add_project();
    let response = server
        .send("GET", &format!("/api/projects/{pid}/events"), None, &[])
        .await;
    assert_eq!(response.status(), 200);
    let mut frames = BodyStream::new(response.into_body());
    let mut text = String::new();
    assert!(until(&mut frames, &mut text, ": ping\n\n").await);
    assert!(!text.contains("sessions"), "{text}");
}
