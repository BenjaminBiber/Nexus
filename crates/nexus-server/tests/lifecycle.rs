//! Start und Stopp des Servers mit echtem Port: `server.json`, Protokoll, belegter Port und
//! das Ende offener Ströme beim Beenden; portiert aus `packages/server/src/main.ts`.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use common::{memory_board, MemoryBoardBackend};
use nexus_core::registry::ServerInfoFile;
use nexus_core::HarnessHome;
use nexus_platform::Platform;
use nexus_server::board_runtime::{factory, BoardBackend, BoardBackendFuture};
use nexus_server::config::ServerConfig;

/// Einstellungen für einen Server auf einem freien Port
fn config(home: &HarnessHome) -> ServerConfig {
    let mut config = ServerConfig::new(0, home.clone());
    config.version = "test".into();
    config
}

/// Kleiner HTTP-Aufruf ohne weitere Abhängigkeit: Anfrage von Hand, Antwort als Text samt
/// Status
async fn fetch(port: u16, path: &str) -> (u16, String) {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Verbindung zum Server");
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut text = String::new();
    stream.read_to_string(&mut text).await.unwrap();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);
    (status, text)
}

#[tokio::test]
async fn schreibt_server_json_mit_pid_und_port_und_loscht_sie_beim_beenden() {
    let dir = tempfile::tempdir().unwrap();
    let home = HarnessHome::at(dir.path());
    let server = nexus_server::serve(config(&home), memory_board(), Platform::fake())
        .await
        .expect("Server startet");
    let port = server.port;
    assert!(port > 0);

    let info = ServerInfoFile::new(home.clone())
        .read()
        .expect("server.json");
    assert_eq!(info.pid, std::process::id());
    assert_eq!(info.port, port);
    assert!(info.started_at.ends_with('Z'), "{}", info.started_at);

    // Der Schutz kennt den wirklich belegten Port, sonst käme 403
    let (status, _text) = fetch(port, "/api/health").await;
    assert_eq!(status, 200);

    let log = std::fs::read_to_string(home.log_file()).unwrap();
    assert!(
        log.contains(&format!("Server läuft auf http://127.0.0.1:{port}")),
        "{log}"
    );

    server.stop().await;
    assert!(!home.server_file().exists());
    let log = std::fs::read_to_string(home.log_file()).unwrap();
    assert!(log.contains("Server wird beendet."), "{log}");
}

#[tokio::test]
async fn meldet_einen_belegten_port_mit_der_meldung_von_heute() {
    let dir = tempfile::tempdir().unwrap();
    let home = HarnessHome::at(dir.path());
    let first = nexus_server::serve(config(&home), memory_board(), Platform::fake())
        .await
        .unwrap();

    let mut second = config(&home);
    second.port = first.port;
    let error = match nexus_server::serve(second, memory_board(), Platform::fake()).await {
        Ok(_) => panic!("der zweite Server darf nicht starten"),
        Err(error) => error,
    };
    assert_eq!(error.code(), Some("port-busy"));
    assert_eq!(
        error.to_string(),
        format!("Port {} ist belegt.", first.port)
    );
    let log = std::fs::read_to_string(home.log_file()).unwrap();
    assert!(log.contains("ist belegt."), "{log}");
    first.stop().await;
}

#[tokio::test]
async fn schliesst_die_board_laufzeit_beim_beenden() {
    let dir = tempfile::tempdir().unwrap();
    let home = HarnessHome::at(dir.path());
    let backend = MemoryBoardBackend::new("laufzeit");
    let geteilt = backend.clone();
    let board = factory(move || -> BoardBackendFuture {
        let backend = geteilt.clone();
        Box::pin(async move { Ok(backend as Arc<dyn BoardBackend>) })
    });
    let server = nexus_server::serve(config(&home), board, Platform::fake())
        .await
        .unwrap();
    server.state.board.get().await.unwrap();
    assert!(!backend.closed());
    server.stop().await;
    assert!(backend.closed());
}

#[tokio::test]
async fn beendet_offene_strome_des_upstreams_beim_beenden() {
    // Upstream mit einem Ereignisstrom, der von selbst nie endet
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_port = listener.local_addr().unwrap().port();
    let upstream = tokio::spawn(async move {
        let router = Router::new().route(
            "/api/events",
            get(|| async {
                let stream = futures::stream::unfold((), |_| async {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Some((Ok::<_, std::io::Error>("data: tick\n\n"), ()))
                });
                (
                    [("content-type", "text/event-stream")],
                    Body::from_stream(stream),
                )
                    .into_response()
            }),
        );
        let _ = axum::serve(listener, router).await;
    });

    let dir = tempfile::tempdir().unwrap();
    let home = HarnessHome::at(dir.path());
    let mut config = config(&home);
    config.upstream = Some(format!("http://127.0.0.1:{upstream_port}"));
    let server = nexus_server::serve(config, memory_board(), Platform::fake())
        .await
        .unwrap();
    let port = server.port;

    // Einen Strom öffnen und das erste Ereignis abwarten
    let reader = tokio::spawn(async move { fetch(port, "/api/events").await });
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Das Beenden darf nicht am offenen Strom hängen
    let stopped = tokio::time::timeout(Duration::from_secs(5), server.stop()).await;
    assert!(stopped.is_ok(), "Beenden hing am offenen Ereignisstrom");
    let (_status, text) = tokio::time::timeout(Duration::from_secs(5), reader)
        .await
        .expect("der Strom endet")
        .unwrap();
    assert!(text.contains("data: tick"), "{text}");
    upstream.abort();
}
