//! Asset-Proxy der Themes unter `/api/themes/{id}/fetch` mit lokalem Upstream (kein Internet).

mod common;

use std::fs;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::http::header;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use axum::Router;
use common::{TestServer, TestServerOptions};
use sha2::{Digest, Sha256};

const BILD: &[u8] = b"PNG-DATEN";

/// Lokaler Upstream mit Zähler der Anfragen
struct Upstream {
    addr: SocketAddr,
    hits: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Upstream {
    async fn start() -> Upstream {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("Port des Upstreams");
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let zaehler = hits.clone();
        let fremd = format!("http://localhost:{}/bild.png", addr.port());
        let app = Router::new()
            .route(
                "/bild.png",
                get(|| async { ([(header::CONTENT_TYPE, "image/png")], BILD).into_response() }),
            )
            .route("/gross", get(|| async { vec![b'x'; 4096] }))
            .route(
                "/weiter",
                get(move || async move { Redirect::temporary(&fremd) }),
            )
            .layer(axum::middleware::from_fn(
                move |req: axum::extract::Request, next: axum::middleware::Next| {
                    zaehler.fetch_add(1, Ordering::SeqCst);
                    next.run(req)
                },
            ));
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Upstream { addr, hits, task }
    }

    fn url(&self, pfad: &str) -> String {
        format!("http://127.0.0.1:{}{pfad}", self.addr.port())
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

/// Server mit Theme `demo-theme`, das `127.0.0.1` freigibt; Limit 1 KiB
fn server_mit_theme() -> TestServer {
    let server = TestServer::with(TestServerOptions {
        theme_fetch_max_bytes: Some(1024),
        ..Default::default()
    });
    let theme = server.home.themes_dir().join("demo-theme");
    fs::create_dir_all(&theme).unwrap();
    let manifest = serde_json::json!({
        "id": "demo-theme",
        "name": "Demo",
        "version": "1.0.0",
        "author": "Ada",
        "sdkVersion": "^1.0",
        "colorScheme": "dark",
        "slots": [],
        "permissions": { "network": ["127.0.0.1"] }
    });
    fs::write(theme.join("manifest.json"), manifest.to_string()).unwrap();
    server
}

fn cache_leer(server: &TestServer) -> bool {
    let cache = server.home.themes_dir().join("demo-theme").join("cache");
    fs::read_dir(cache).map_or(true, |mut it| it.next().is_none())
}

fn fetch(url: &str) -> String {
    format!("/api/themes/demo-theme/fetch?url={url}")
}

#[tokio::test]
async fn freigegebener_host_liefert_bytes_und_cacht() {
    let upstream = Upstream::start().await;
    let server = server_mit_theme();
    let reply = server.get(&fetch(&upstream.url("/bild.png"))).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    assert_eq!(reply.text.as_bytes(), BILD);
    assert_eq!(reply.header("content-type"), Some("image/png"));
    assert_eq!(reply.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(reply.header("x-nexus-cache"), Some("miss"));
    assert!(!cache_leer(&server));
    assert_eq!(upstream.hits(), 1);

    let zweiter = server.get(&fetch(&upstream.url("/bild.png"))).await;
    assert_eq!(zweiter.status, 200);
    assert_eq!(zweiter.header("x-nexus-cache"), Some("hit"));
    assert_eq!(zweiter.header("content-type"), Some("image/png"));
    assert_eq!(upstream.hits(), 1);

    // Ohne Upstream weiterhin aus dem Cache
    let url = upstream.url("/bild.png");
    upstream.task.abort();
    let _ = upstream.task.await;
    let dritter = server.get(&fetch(&url)).await;
    assert_eq!(dritter.status, 200);
    assert_eq!(dritter.text.as_bytes(), BILD);
}

#[tokio::test]
async fn nicht_freigegebener_host_ist_403_ohne_anfrage() {
    let upstream = Upstream::start().await;
    let server = server_mit_theme();
    let url = format!("http://localhost:{}/bild.png", upstream.addr.port());
    let reply = server.get(&fetch(&url)).await;
    assert_eq!(reply.status, 403);
    assert_eq!(reply.code().as_deref(), Some("theme_fetch_forbidden"));
    assert_eq!(upstream.hits(), 0);
    assert!(cache_leer(&server));
}

#[tokio::test]
async fn weiterleitung_auf_fremden_host_wird_abgelehnt() {
    let upstream = Upstream::start().await;
    let server = server_mit_theme();
    let reply = server.get(&fetch(&upstream.url("/weiter"))).await;
    assert_eq!(reply.status, 403);
    assert_eq!(upstream.hits(), 1);
    assert!(cache_leer(&server));
}

#[tokio::test]
async fn zu_grosse_antwort_ist_413() {
    let upstream = Upstream::start().await;
    let server = server_mit_theme();
    let reply = server.get(&fetch(&upstream.url("/gross"))).await;
    assert_eq!(reply.status, 413);
    assert_eq!(reply.code().as_deref(), Some("theme_fetch_too_large"));
    assert!(cache_leer(&server));
}

#[tokio::test]
async fn pruefsumme_wird_geprueft() {
    let upstream = Upstream::start().await;
    let server = server_mit_theme();
    let falsch = "0".repeat(64);
    let reply = server
        .get(&format!(
            "{}&sha256={falsch}",
            fetch(&upstream.url("/bild.png"))
        ))
        .await;
    assert_eq!(reply.status, 502);
    assert_eq!(reply.code().as_deref(), Some("theme_fetch_checksum"));
    assert!(cache_leer(&server));

    let richtig: String = Sha256::digest(BILD)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let reply = server
        .get(&format!(
            "{}&sha256={richtig}",
            fetch(&upstream.url("/bild.png"))
        ))
        .await;
    assert_eq!(reply.status, 200);
}

#[tokio::test]
async fn unbekanntes_oder_ungueltiges_theme_ist_404() {
    let server = server_mit_theme();
    for url in [
        "/api/themes/gibt-es-nicht/fetch?url=http://127.0.0.1/x",
        "/api/themes/A_B/fetch?url=http://127.0.0.1/x",
    ] {
        assert_eq!(server.get(url).await.status, 404, "{url}");
    }
}

#[tokio::test]
async fn fehlende_oder_falsche_url_ist_400() {
    let server = server_mit_theme();
    for url in [
        "/api/themes/demo-theme/fetch",
        "/api/themes/demo-theme/fetch?url=file:///etc/passwd",
        "/api/themes/demo-theme/fetch?url=kein-url",
    ] {
        assert_eq!(server.get(url).await.status, 400, "{url}");
    }
}
