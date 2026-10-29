//! Der Ereignisstrom am laufenden Server: Begrüßung, Ereignisse je Projekt, Herzschlag,
//! Meldungen an Tabs mit `notify=1`, Aufräumen beim Schließen und das Ende beim Beenden des
//! Servers; portiert aus `packages/server/tests/live/events.test.ts`.

mod common;

use std::time::Duration;

use axum::body::Body;
use common::{TestServer, TestServerOptions};
use futures::StreamExt as _;
use http_body_util::BodyStream;
use nexus_core::HarnessHome;
use nexus_platform::Platform;
use nexus_server::config::ServerConfig;
use nexus_server::events::LiveEvent;
use nexus_server::notices::{BrowserNotice, ToastKind};

/// Ein offener Ereignisstrom, dessen Stücke der Test einsammelt
struct Stream {
    frames: BodyStream<Body>,
    text: String,
}

impl Stream {
    /// Liest, bis der Text vorkommt; `false` heißt: er kam nicht rechtzeitig
    async fn until(&mut self, needle: &str) -> bool {
        if self.text.contains(needle) {
            return true;
        }
        let read = tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(Ok(frame)) = self.frames.next().await {
                if let Ok(data) = frame.into_data() {
                    self.text.push_str(&String::from_utf8_lossy(&data));
                    if self.text.contains(needle) {
                        return true;
                    }
                }
            }
            false
        })
        .await;
        read.unwrap_or(false)
    }

    /// Liest, bis der Text so oft vorkam
    async fn until_count(&mut self, needle: &str, count: usize) -> bool {
        let read = tokio::time::timeout(Duration::from_secs(3), async {
            while self.text.matches(needle).count() < count {
                let Some(Ok(frame)) = self.frames.next().await else {
                    return false;
                };
                if let Ok(data) = frame.into_data() {
                    self.text.push_str(&String::from_utf8_lossy(&data));
                }
            }
            true
        })
        .await;
        read.unwrap_or(false)
    }

    /// Kam dieser Text bis jetzt vor?
    fn saw(&self, needle: &str) -> bool {
        self.text.contains(needle)
    }
}

/// Server mit kurzem Herzschlag und einem angemeldeten Projekt
async fn server() -> (TestServer, String, common::TempRepo) {
    let server = TestServer::with(TestServerOptions {
        heartbeat_ms: Some(200),
        ..TestServerOptions::default()
    });
    let (repo, pid) = server.add_project();
    (server, pid, repo)
}

/// Öffnet den Ereignisstrom eines Projekts
async fn listen(server: &TestServer, pid: &str, query: &str) -> Stream {
    let response = server
        .send(
            "GET",
            &format!("/api/projects/{pid}/events{query}"),
            None,
            &[],
        )
        .await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    Stream {
        frames: BodyStream::new(response.into_body()),
        text: String::new(),
    }
}

#[tokio::test]
async fn meldet_beim_verbinden_hello_mit_den_kopfen_des_vertrags() {
    let (server, pid, _repo) = server().await;
    let response = server
        .send("GET", &format!("/api/projects/{pid}/events"), None, &[])
        .await;
    assert_eq!(response.status(), 200);
    let headers = response.headers().clone();
    assert_eq!(headers.get("content-type").unwrap(), "text/event-stream");
    assert_eq!(
        headers.get("cache-control").unwrap(),
        "no-cache, no-transform"
    );
    assert_eq!(headers.get("connection").unwrap(), "keep-alive");
    assert_eq!(headers.get("x-accel-buffering").unwrap(), "no");
    assert_eq!(headers.get("x-nexus-backend").unwrap(), "rust");
    assert!(headers.get("content-length").is_none());

    let mut stream = Stream {
        frames: BodyStream::new(response.into_body()),
        text: String::new(),
    };
    assert!(stream.until("data: {\"type\":\"hello\"}\n\n").await);
}

#[tokio::test]
async fn meldet_anderungen_als_sessions_nur_im_betroffenen_projekt() {
    let (server, eins, _repo) = server().await;
    let (_zweites_repo, zwei) = server.add_project();
    let mut strom_eins = listen(&server, &eins, "").await;
    let mut strom_zwei = listen(&server, &zwei, "").await;
    assert!(strom_eins.until("hello").await);
    assert!(strom_zwei.until("hello").await);

    server.state.events.publish(&eins, LiveEvent::Sessions);
    assert!(strom_eins.until("data: {\"type\":\"sessions\"}\n\n").await);
    // Der zweite Strom bekommt stattdessen nur seinen Herzschlag
    assert!(strom_zwei.until(": ping\n\n").await);
    assert!(!strom_zwei.saw("sessions"));
}

#[tokio::test]
async fn sendet_regelmassig_einen_herzschlag() {
    let (server, pid, _repo) = server().await;
    let mut stream = listen(&server, &pid, "").await;
    assert!(stream.until("hello").await);
    // Zwei Schläge zeigen, dass der Takt weiterläuft
    assert!(stream.until_count(": ping\n\n", 2).await, "{}", stream.text);
}

#[tokio::test]
async fn raumt_auf_wenn_der_browser_die_verbindung_schliesst() {
    let (server, pid, _repo) = server().await;
    let mut stream = listen(&server, &pid, "?dbBoard=3&dbProject=4&notify=1").await;
    assert!(stream.until("hello").await);
    assert_eq!(server.state.events.subscribers(&pid), 1);
    assert_eq!(server.state.board_changes.watchers(3), 1);
    assert_eq!(server.state.question_changes.watchers(4), 1);
    assert_eq!(server.state.spec_docs.watchers(&pid), 1);
    assert_eq!(server.state.notice_tabs.tabs(), 1);

    drop(stream);
    assert_eq!(server.state.events.subscribers(&pid), 0);
    assert_eq!(server.state.board_changes.watchers(3), 0);
    assert_eq!(server.state.question_changes.watchers(4), 0);
    assert_eq!(server.state.spec_docs.watchers(&pid), 0);
    assert_eq!(server.state.notice_tabs.tabs(), 0);
}

#[tokio::test]
async fn weist_ein_unbekanntes_projekt_mit_404_ab() {
    let (server, _pid, _repo) = server().await;
    let reply = server.get("/api/projects/00000000/events").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.code().as_deref(), Some("project-not-found"));
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8")
    );
}

#[tokio::test]
async fn schickt_meldungen_nur_an_tabs_mit_notify_und_meldet_sie_beim_schliessen_ab() {
    let (server, pid, _repo) = server().await;
    let mut schlicht = listen(&server, &pid, "").await;
    let mut tab = listen(&server, &pid, "?notify=1").await;
    assert!(schlicht.until("hello").await);
    assert!(tab.until("hello").await);

    let notice = BrowserNotice {
        kind: ToastKind::Wartet,
        tag: "attention".into(),
        title: "T".into(),
        body: "B".into(),
        url: "/p/x/sessions".into(),
    };
    assert!(server.state.notice_tabs.deliver(notice.clone()));
    assert!(
        tab.until(r#"data: {"type":"notice","kind":"wartet","tag":"attention","title":"T","body":"B","url":"/p/x/sessions"}"#)
            .await
    );
    assert!(schlicht.until(": ping\n\n").await);
    assert!(!schlicht.saw("notice"));

    drop(tab);
    assert!(!server.state.notice_tabs.deliver(notice));
}

#[tokio::test]
async fn meldet_anderungen_des_beobachteten_boards_als_dbboard() {
    let fixture = common::DocsFixture::start_with(
        TestServerOptions {
            heartbeat_ms: Some(60_000),
            poll_ms: Some(100),
            ..TestServerOptions::default()
        },
        true,
    )
    .await;
    let board = fixture.board_id;
    let mut mit = listen(&fixture.server, &fixture.pid, &format!("?dbBoard={board}")).await;
    let mut ohne = listen(&fixture.server, &fixture.pid, "").await;
    assert!(mit.until("hello").await);
    assert!(ohne.until("hello").await);
    // Der Ausgangswert des Kennwerts steht nach dem ersten Durchlauf fest
    tokio::time::sleep(Duration::from_millis(300)).await;

    let angelegt = fixture
        .server
        .post(
            &format!("/api/db/boards/{board}/tickets"),
            r#"{"title":"Login"}"#,
        )
        .await;
    assert_eq!(angelegt.status, 201, "{}", angelegt.text);
    assert!(
        mit.until(&format!(
            "data: {{\"type\":\"dbboard\",\"boardId\":{board}}}\n\n"
        ))
        .await,
        "{}",
        mit.text
    );
    assert!(!ohne.saw("dbboard"), "{}", ohne.text);
}

#[tokio::test]
async fn meldet_geanderte_superpowers_dokumente_als_docs_changed() {
    let fixture = common::DocsFixture::start_with(
        TestServerOptions {
            heartbeat_ms: Some(60_000),
            poll_ms: Some(100),
            ..TestServerOptions::default()
        },
        true,
    )
    .await;
    let mut stream = listen(&fixture.server, &fixture.pid, "").await;
    assert!(stream.until("hello").await);
    // Der Ausgangswert steht nach dem ersten Durchlauf fest
    tokio::time::sleep(Duration::from_millis(300)).await;

    fixture.write(
        "docs/superpowers/specs",
        "2026-10-22-neu-design.md",
        "# Neu\n",
    );
    assert!(
        stream
            .until(&format!(
                "data: {{\"type\":\"docs-changed\",\"projectId\":\"{}\"}}\n\n",
                fixture.pid
            ))
            .await,
        "{}",
        stream.text
    );
}

#[tokio::test]
async fn beendet_offene_strome_beim_beenden_des_servers() {
    let dir = tempfile::tempdir().unwrap();
    let home = HarnessHome::at(dir.path());
    let mut config = ServerConfig::new(0, home.clone());
    config.version = "test".into();
    config.heartbeat_ms = 200;
    let server = nexus_server::serve(config, common::memory_board(), Platform::fake())
        .await
        .expect("Server startet");
    let port = server.port;
    let state = server.state.clone();

    // Projekt anmelden und einen Strom öffnen
    let repo = common::TempRepo::new();
    nexus_core::HarnessProject::init(repo.path()).expect("Projekt einrichten");
    let pid = nexus_core::registry::ProjectRegistry::new(home.clone())
        .add(repo.path())
        .expect("Projekt anmelden")
        .project
        .id;
    let strom = tokio::spawn(async move { read_stream(port, &pid).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(state.events.subscribers_total() > 0);

    // Das Beenden hängt nicht am offenen Strom und ist in zwei Sekunden durch
    let stopped = tokio::time::timeout(Duration::from_secs(2), server.stop()).await;
    assert!(stopped.is_ok(), "Beenden hing am offenen Ereignisstrom");
    let text = tokio::time::timeout(Duration::from_secs(2), strom)
        .await
        .expect("der Strom endet")
        .unwrap();
    assert!(text.contains("data: {\"type\":\"hello\"}"), "{text}");
    assert_eq!(state.events.subscribers_total(), 0);
}

/// Liest den Ereignisstrom über einen echten Port, bis er endet
async fn read_stream(port: u16, pid: &str) -> String {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Verbindung zum Server");
    let request = format!(
        "GET /api/projects/{pid}/events HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut text = String::new();
    let _ = stream.read_to_string(&mut text).await;
    text
}
