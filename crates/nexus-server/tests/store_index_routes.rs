//! Routengruppe `store_index`: `GET /api/themes/store/index` lädt die Index-Dateien aller
//! Store-Repos. Die Repos sind eine Attrappe aus axum im eigenen Prozess – nichts verlässt den
//! Rechner.

mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use common::{TestServer, TestServerOptions};
use serde_json::{json, Value};

const INDEX: &str = "/api/themes/store/index";
const TIMEOUT: Duration = Duration::from_millis(1000);

fn eintrag(id: &str, name: &str, sdk: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "version": "1.0.0",
        "sdkVersion": sdk,
        "downloadUrl": format!("https://example.com/{id}.nexus"),
        "sha256": "a".repeat(64),
        "description": "Test"
    })
}

/// Startet die Attrappe der Repos
async fn repos_stub() -> SocketAddr {
    let good = json!([
        eintrag("wald", "Wald", "^1.0"),
        eintrag("ozean", "ozean", "^1.0")
    ]);
    let mixed = json!([
        eintrag("berg", "Berg", "^1.0"),
        { "id": "ohne-name", "version": "1.0.0" },
        eintrag("berg", "Berg 2", "^1.0")
    ]);
    let future = json!([eintrag("zukunft", "Zukunft", "^2.0")]);
    let app = Router::new()
        .route("/good.json", get(move || async move { good.to_string() }))
        .route("/mixed.json", get(move || async move { mixed.to_string() }))
        .route(
            "/future-sdk.json",
            get(move || async move { future.to_string() }),
        )
        .route(
            "/slow.json",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "[]"
            }),
        )
        .route(
            "/500.json",
            get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "kaputt") }),
        )
        .route("/garbage.json", get(|| async { "{kein json" }))
        .route("/huge.json", get(|| async { " ".repeat((1 << 20) + 10) }));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("Port der Attrappe");
    let addr = listener.local_addr().expect("Adresse der Attrappe");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn server() -> TestServer {
    TestServer::with(TestServerOptions {
        store_index_timeout: Some(TIMEOUT),
        ..Default::default()
    })
}

/// Trägt die Adresse als Repo ein und liefert die ID
async fn add(server: &TestServer, url: &str) -> String {
    let reply = server
        .post(
            "/api/themes/store/repos",
            &json!({ "url": url }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 201, "{}", reply.text);
    reply.body()["id"].as_str().unwrap().to_string()
}

async fn index(server: &TestServer) -> Value {
    let reply = server.get(INDEX).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    reply.body()
}

fn repo<'a>(body: &'a Value, id: &str) -> &'a Value {
    body["repos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|repo| repo["id"] == json!(id))
        .expect("Repo in der Antwort")
}

#[tokio::test]
async fn ohne_repos_leer() {
    let server = server();
    assert_eq!(index(&server).await, json!({ "entries": [], "repos": [] }));
}

#[tokio::test]
async fn gueltiger_index_wird_gelistet() {
    let addr = repos_stub().await;
    let server = server();
    let id = add(
        &server,
        &format!("http://127.0.0.1:{}/good.json", addr.port()),
    )
    .await;
    let body = index(&server).await;
    let entries = body["entries"].as_array().unwrap();
    let ids: Vec<&str> = entries.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["ozean", "wald"]);
    for entry in entries {
        assert_eq!(entry["repoId"], json!(id));
        assert_eq!(entry["compatible"], json!(true));
    }
    let status = repo(&body, &id);
    assert_eq!(status["ok"], json!(true));
    assert_eq!(status["entryCount"], json!(2));
    assert!(status.get("error").is_none());
}

#[tokio::test]
async fn ungueltige_eintraege_ausgelassen_unpassende_markiert() {
    let addr = repos_stub().await;
    let server = server();
    let mixed = add(
        &server,
        &format!("http://127.0.0.1:{}/mixed.json", addr.port()),
    )
    .await;
    let future = add(
        &server,
        &format!("http://127.0.0.1:{}/future-sdk.json", addr.port()),
    )
    .await;
    let body = index(&server).await;
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["id"], json!("berg"));
    assert_eq!(entries[0]["repoId"], json!(mixed));
    assert_eq!(entries[1]["id"], json!("zukunft"));
    assert_eq!(entries[1]["compatible"], json!(false));
    let skipped = repo(&body, &mixed)["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 2);
    assert_eq!(skipped[0]["index"], json!(1));
    assert!(skipped[0]["message"].as_str().unwrap().contains("name"));
    assert!(skipped[1]["message"].as_str().unwrap().contains("doppelt"));
    assert_eq!(repo(&body, &future)["ok"], json!(true));
}

#[tokio::test]
async fn kaputte_repos_melden_fehler_ohne_die_anderen_zu_blockieren() {
    let addr = repos_stub().await;
    let server = server();
    let base = format!("http://127.0.0.1:{}", addr.port());
    let good = add(&server, &format!("{base}/good.json")).await;
    let cases = [
        (format!("{base}/500.json"), "store-index-http"),
        (format!("{base}/garbage.json"), "store-index-invalid"),
        (format!("{base}/huge.json"), "store-index-too-large"),
        (format!("{base}/slow.json"), "store-index-timeout"),
    ];
    let mut ids = Vec::new();
    for (url, code) in &cases {
        ids.push((add(&server, url).await, *code));
    }
    let start = Instant::now();
    let body = index(&server).await;
    assert!(
        start.elapsed() < TIMEOUT + Duration::from_millis(1500),
        "dauerte {:?}",
        start.elapsed()
    );
    assert_eq!(body["entries"].as_array().unwrap().len(), 2);
    assert_eq!(repo(&body, &good)["ok"], json!(true));
    for (id, code) in ids {
        let status = repo(&body, &id);
        assert_eq!(status["ok"], json!(false), "{code}");
        assert_eq!(status["error"]["code"], json!(code));
        assert!(status["error"]["message"].as_str().is_some());
    }
    let http = repo(&body, &ids_first(&body, "store-index-http"));
    assert!(http["error"]["message"].as_str().unwrap().contains("500"));
}

fn ids_first(body: &Value, code: &str) -> String {
    body["repos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|repo| repo["error"]["code"] == json!(code))
        .and_then(|repo| repo["id"].as_str())
        .unwrap()
        .to_string()
}

/// Eigenes, längeres Zeitlimit: Windows wiederholt eine abgewiesene Verbindung rund 2 s lang
#[tokio::test]
async fn geschlossener_port_ist_nicht_erreichbar() {
    let closed = {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.local_addr().unwrap().port()
    };
    let server = TestServer::with(TestServerOptions {
        store_index_timeout: Some(Duration::from_secs(8)),
        ..Default::default()
    });
    let id = add(&server, &format!("http://127.0.0.1:{closed}/i.json")).await;
    let body = index(&server).await;
    let status = repo(&body, &id);
    assert_eq!(status["ok"], json!(false));
    assert_eq!(status["error"]["code"], json!("store-index-unreachable"));
}
