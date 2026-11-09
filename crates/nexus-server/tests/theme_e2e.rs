//! Ende-zu-Ende mit dem Test-Theme `e2e-shell` (eigene Shell): das eingecheckte `.nexus`-Paket
//! über die API installieren, auflisten, Dateien abrufen und wieder entfernen.

mod common;

use std::fs;
use std::path::PathBuf;

use base64::Engine;
use common::TestServer;
use nexus_core::themes::pack_theme;

/// Ordner der Test-Themes im Web-Paket
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/web/tests/fixtures/themes")
}

/// Bytes des eingecheckten Pakets
fn paket() -> Vec<u8> {
    fs::read(fixtures().join("e2e-shell.nexus")).unwrap()
}

/// Datei der Quelle als Text
fn quelle(name: &str) -> String {
    fs::read_to_string(fixtures().join("e2e-shell").join(name)).unwrap()
}

/// Der JSON-Körper zum Hochladen
fn koerper(paket: &[u8]) -> String {
    let data = base64::engine::general_purpose::STANDARD.encode(paket);
    serde_json::json!({ "data": data }).to_string()
}

/// Server mit installiertem Test-Theme
async fn installiert() -> TestServer {
    let server = TestServer::start();
    let reply = server.post("/api/themes/install", &koerper(&paket())).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    assert_eq!(reply.body()["id"], "e2e-shell");
    assert_eq!(reply.body()["slots"], serde_json::json!(["Shell"]));
    server
}

#[test]
fn paket_entspricht_der_quelle() {
    let gebaut = pack_theme(&fixtures().join("e2e-shell")).unwrap();
    assert_eq!(gebaut.manifest.id, "e2e-shell");
    assert!(
        gebaut.bytes == paket(),
        "e2e-shell.nexus ist veraltet – neu bauen (siehe packages/web/tests/fixtures/themes/README.md)"
    );
}

#[tokio::test]
async fn installiert_testpaket_ueber_die_api() {
    let server = installiert().await;

    let liste = server.get("/api/themes").await;
    assert_eq!(liste.status, 200);
    let ids: Vec<_> = liste
        .body()
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].clone())
        .collect();
    assert_eq!(ids, [serde_json::json!("e2e-shell")]);

    let main = server.get("/themes/e2e-shell/main.js").await;
    assert_eq!(main.status, 200);
    assert!(main
        .header("content-type")
        .unwrap()
        .starts_with("text/javascript"));
    assert_eq!(main.text, quelle("main.js"));

    let css = server.get("/themes/e2e-shell/theme.css").await;
    assert_eq!(css.status, 200);
    assert!(css.header("content-type").unwrap().starts_with("text/css"));
    assert_eq!(css.text, quelle("theme.css"));

    let manifest = server.get("/themes/e2e-shell/manifest.json").await;
    assert_eq!(manifest.status, 200);
    assert_eq!(manifest.body()["sdkVersion"], "^1.1");
}

#[tokio::test]
async fn entfernen_raeumt_auf() {
    let server = installiert().await;
    let ordner = server.home.themes_dir().join("e2e-shell");
    assert!(ordner.join("main.js").is_file());

    let reply = server.delete("/api/themes/e2e-shell").await;
    assert_eq!(reply.status, 204, "{}", reply.text);

    assert_eq!(
        server.get("/api/themes").await.body(),
        serde_json::json!([])
    );
    assert_eq!(server.get("/themes/e2e-shell/main.js").await.status, 404);
    assert_eq!(server.get("/themes/e2e-shell/theme.css").await.status, 404);
    assert!(!ordner.exists());
}
