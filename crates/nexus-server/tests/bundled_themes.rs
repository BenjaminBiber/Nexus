//! Mitgelieferte Themes werden beim Start installiert, wenn sie fehlen oder älter sind.

mod common;

use std::fs;
use std::path::Path;

use common::{TestServer, TestServerOptions};
use nexus_core::themes::{install_theme, pack_theme};

/// Baut ein Paket aus einem Mini-Theme-Ordner
fn paket(id: &str, version: &str) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("manifest.json"),
        format!(
            r#"{{"id":"{id}","name":"{id}","version":"{version}","author":"Ada","sdkVersion":"^1.0","colorScheme":"dark"}}"#
        ),
    )
    .unwrap();
    fs::write(dir.path().join("theme.css"), ":root { --bg: #000; }").unwrap();
    pack_theme(dir.path()).unwrap().bytes
}

fn mitgeliefert(dir: &Path, id: &str, version: &str) {
    fs::write(dir.join(format!("{id}.nexus")), paket(id, version)).unwrap();
}

fn server(dir: &Path) -> TestServer {
    TestServer::with(TestServerOptions {
        bundled_themes_dir: Some(dir.to_path_buf()),
        ..Default::default()
    })
}

/// id → Version aus `GET /api/themes`
async fn themes(server: &TestServer) -> Vec<(String, String)> {
    let reply = server.get("/api/themes").await;
    assert_eq!(reply.status, 200);
    reply
        .body()
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["id"].as_str().unwrap().to_string(),
                t["version"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[tokio::test]
async fn fehlendes_mitgeliefertes_theme_wird_installiert_und_geloggt() {
    let dir = tempfile::tempdir().unwrap();
    mitgeliefert(dir.path(), "signal", "1.0.0");
    fs::write(dir.path().join("kaputt.nexus"), b"kein zip").unwrap();
    let server = server(dir.path());
    nexus_server::install_bundled_themes(&server.state).await;

    assert_eq!(
        themes(&server).await,
        vec![("signal".to_string(), "1.0.0".to_string())]
    );
    let log = fs::read_to_string(server.state.log.file()).unwrap();
    assert!(
        log.contains("Theme signal 1.0.0 mitgeliefert installiert"),
        "{log}"
    );
    assert!(log.contains("kaputt.nexus nicht installiert"), "{log}");
}

#[tokio::test]
async fn neuere_installation_und_eigenes_theme_bleiben() {
    let dir = tempfile::tempdir().unwrap();
    mitgeliefert(dir.path(), "signal", "1.0.0");
    let server = server(dir.path());
    let home = server.home.clone();
    install_theme(&home, &paket("signal", "2.0.0")).unwrap();
    install_theme(&home, &paket("eigen", "0.1.0")).unwrap();
    nexus_server::install_bundled_themes(&server.state).await;

    assert_eq!(
        themes(&server).await,
        vec![
            ("eigen".to_string(), "0.1.0".to_string()),
            ("signal".to_string(), "2.0.0".to_string()),
        ]
    );
    let log = fs::read_to_string(server.state.log.file()).unwrap();
    assert!(log.contains("Theme signal bleibt 2.0.0"), "{log}");
}
