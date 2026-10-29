//! Portiert aus `packages/cli/tests/unit/DispatchClient.test.ts`. Der Fake-Server läuft in
//! einem eigenen Faden und schreibt mit, was ankommt.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use super::*;

/// Eine aufgezeichnete Anfrage
#[derive(Debug, Clone, PartialEq, Eq)]
struct Aufruf {
    method: String,
    url: String,
    body: String,
    harness: Option<String>,
}

/// Der Fake-Server: liefert feste Antworten und meldet jede Anfrage außer `/api/health`
struct FakeServer {
    port: u16,
    aufrufe: Receiver<Aufruf>,
}

impl FakeServer {
    /// Startet den Server auf einem freien Port
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (sender, aufrufe) = channel();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                if Self::handle(stream, &sender).is_err() {
                    break;
                }
            }
        });
        Self { port, aufrufe }
    }

    /// Alle bisher aufgezeichneten Anfragen
    fn aufrufe(&self) -> Vec<Aufruf> {
        self.aufrufe.try_iter().collect()
    }

    /// Liest eine Anfrage und antwortet wie der Fake-Server im TypeScript-Test
    fn handle(
        mut stream: TcpStream,
        sender: &std::sync::mpsc::Sender<Aufruf>,
    ) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut start = String::new();
        reader.read_line(&mut start)?;
        let mut teile = start.split_whitespace();
        let method = teile.next().unwrap_or_default().to_string();
        let url = teile.next().unwrap_or_default().to_string();

        let mut harness = None;
        let mut laenge = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 || line == "\r\n" {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(value) = lower.strip_prefix("x-harness:") {
                harness = Some(value.trim().to_string());
            }
            if let Some(value) = lower.strip_prefix("content-length:") {
                laenge = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0u8; laenge];
        reader.read_exact(&mut body)?;
        let body = String::from_utf8_lossy(&body).to_string();

        let (status, rumpf) = match url.as_str() {
            "/api/health" => (200, "{\"name\":\"ai-harness\"}".to_string()),
            "/api/dispatch/merge/ARC-0001" => (
                422,
                "{\"code\":\"merge-not-possible\",\"message\":\"ARC-0001 wartet auf keinen Merge.\"}"
                    .to_string(),
            ),
            "/api/dispatch/merge/ARC-0002" => (502, "<html>Bad Gateway</html>".to_string()),
            "/api/dispatch/merge/ARC-0003" => (200, "<html>ok</html>".to_string()),
            _ => (
                200,
                "{\"state\":\"idle\",\"message\":\"bereit · 0/1\"}".to_string(),
            ),
        };
        if url != "/api/health" {
            let _ = sender.send(Aufruf {
                method,
                url,
                body,
                harness,
            });
        }
        let antwort = format!(
            "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nConnection: close\r\n\r\n{rumpf}",
            rumpf.len()
        );
        stream.write_all(antwort.as_bytes())?;
        stream.flush()
    }
}

/// Client ohne Starter: der Fake-Server läuft schon
fn client(server: &FakeServer) -> DispatchClient {
    DispatchClient::with_launcher(server.port, None)
}

#[test]
fn spricht_die_routen_an_schreibende_mit_pflichtkopf() {
    let server = FakeServer::start();
    let client = client(&server);
    assert_eq!(
        client.status(Some(3)).unwrap()["message"],
        serde_json::json!("bereit · 0/1")
    );
    client.status(None).unwrap();
    client.run(Some(3)).unwrap();
    client.pause(15).unwrap();
    client.resume().unwrap();
    client.set_enabled(true).unwrap();
    client.set_enabled(false).unwrap();

    let aufrufe = server.aufrufe();
    let gesehen: Vec<String> = aufrufe
        .iter()
        .map(|a| format!("{} {} {}", a.method, a.url, a.body))
        .collect();
    assert_eq!(
        gesehen,
        vec![
            "GET /api/dispatch/status?boardId=3 ",
            "GET /api/dispatch/status ",
            "POST /api/dispatch/run {\"boardId\":3}",
            "POST /api/dispatch/pause {\"minutes\":15}",
            "POST /api/dispatch/resume {}",
            "POST /api/dispatch/enable {}",
            "POST /api/dispatch/disable {}",
        ]
    );
    assert!(aufrufe
        .iter()
        .filter(|a| a.method == "POST")
        .all(|a| a.harness.as_deref() == Some("1")));
}

#[test]
fn startet_eine_recherche_mit_kodierter_fragen_id_und_pflichtkopf() {
    let server = FakeServer::start();
    client(&server).start_research("ARC-F0007").unwrap();
    assert_eq!(
        server.aufrufe(),
        vec![Aufruf {
            method: "POST".into(),
            url: "/api/dispatch/research/ARC-F0007".into(),
            body: "{}".into(),
            harness: Some("1".into()),
        }]
    );
}

#[test]
fn startet_eine_planung_mit_kodierter_ticket_id() {
    let server = FakeServer::start();
    client(&server).start_planning("ARC-0007").unwrap();
    assert_eq!(
        server.aufrufe(),
        vec![Aufruf {
            method: "POST".into(),
            url: "/api/dispatch/plan/ARC-0007".into(),
            body: "{}".into(),
            harness: Some("1".into()),
        }]
    );
}

#[test]
fn macht_fehlerantworten_zu_harness_error() {
    let server = FakeServer::start();
    let error = client(&server).merge("ARC-0001").unwrap_err();
    assert_eq!(error.code(), Some("merge-not-possible"));
    assert_eq!(error.to_string(), "ARC-0001 wartet auf keinen Merge.");
}

#[test]
fn kodiert_die_ticket_id_und_sendet_den_pflichtkopf_beim_merge() {
    let server = FakeServer::start();
    assert_eq!(
        client(&server).merge("ARC 1").unwrap()["message"],
        serde_json::json!("bereit · 0/1")
    );
    assert_eq!(
        server.aufrufe(),
        vec![Aufruf {
            method: "POST".into(),
            url: "/api/dispatch/merge/ARC%201".into(),
            body: "{}".into(),
            harness: Some("1".into()),
        }]
    );
}

#[test]
fn nimmt_ein_ticket_ab_kodierte_id_name_im_body_pflichtkopf() {
    let server = FakeServer::start();
    client(&server).accept("ARC 4", "Florian").unwrap();
    assert_eq!(
        server.aufrufe(),
        vec![Aufruf {
            method: "POST".into(),
            url: "/api/dispatch/accept/ARC%204".into(),
            body: "{\"by\":\"Florian\"}".into(),
            harness: Some("1".into()),
        }]
    );
}

#[test]
fn meldet_502_mit_html_body_als_dispatch_failed() {
    let server = FakeServer::start();
    let error = client(&server).merge("ARC-0002").unwrap_err();
    assert_eq!(error.code(), Some("dispatch-failed"));
    assert_eq!(error.to_string(), "Der Server antwortet mit 502.");
}

#[test]
fn meldet_2xx_ohne_json_als_dispatch_failed() {
    let server = FakeServer::start();
    let error = client(&server).merge("ARC-0003").unwrap_err();
    assert_eq!(error.code(), Some("dispatch-failed"));
    assert_eq!(
        error.to_string(),
        "Der Server lieferte keine lesbare Antwort."
    );
}

#[test]
fn kodiert_einen_pfadteil_wie_encode_uri_component() {
    assert_eq!(encode("ARC-0001"), "ARC-0001");
    assert_eq!(encode("ARC 1"), "ARC%201");
    assert_eq!(encode("ä"), "%C3%A4");
}
