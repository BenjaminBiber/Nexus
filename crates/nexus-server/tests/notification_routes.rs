//! Routengruppe `notifications`: die beiden Testmeldungen der Benutzereinstellungen; portiert
//! aus den Testmeldungs-Fällen von `packages/server/tests/unit/UserSettingsRoutes.test.ts`.
//!
//! Die Systemmeldung geht an die Attrappe von [`Platform::fake`], der Push an einen kleinen
//! HTTP-Server in diesem Prozess – nichts verlässt den Rechner.

mod common;

use std::sync::{Arc, Mutex};

use common::{TestServer, TestServerOptions};
use nexus_core::settings::{UserSettings, UserSettingsStore};
use nexus_platform::Platform;

/// Eine Attrappe des ntfy-Servers: antwortet mit festem Status und merkt sich die Anfragen
struct NtfyStub {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
}

impl NtfyStub {
    /// Startet die Attrappe auf einem freien Port
    async fn start(status: u16) -> NtfyStub {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("Port der Attrappe");
        let port = listener.local_addr().expect("Adresse").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let seen = seen.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
                    let mut buffer = vec![0u8; 8 * 1024];
                    let read = stream.read(&mut buffer).await.unwrap_or(0);
                    seen.lock()
                        .expect("Sperre")
                        .push(String::from_utf8_lossy(&buffer[..read]).into_owned());
                    let answer = format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{{}}"
                    );
                    let _ = stream.write_all(answer.as_bytes()).await;
                });
            }
        });
        NtfyStub { port, requests }
    }

    /// Die Adresse, die in die Einstellungen gehört
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Die empfangenen Anfragen als Rohtext
    fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("Sperre").clone()
    }
}

/// Testserver, dessen Systemmeldungen in der Attrappe landen
fn server() -> TestServer {
    TestServer::with(TestServerOptions {
        platform: Some(Platform::fake()),
        ..TestServerOptions::default()
    })
}

#[tokio::test]
async fn die_testmeldung_antwortet_mit_ok_und_ohne_fehler() {
    let server = server();
    let reply = server
        .post("/api/user-settings/test-notification", "{}")
        .await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.json().expect("JSON"),
        serde_json::json!({ "ok": true, "error": null })
    );
    assert_eq!(reply.header("x-nexus-backend"), Some("rust"));
}

#[tokio::test]
async fn der_push_test_ohne_thema_nennt_das_fehlende_thema() {
    let server = server();
    let reply = server.post("/api/user-settings/test-ntfy", "{}").await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.json().expect("JSON"),
        serde_json::json!({ "ok": false, "error": "Für ntfy ist kein Thema eingetragen." })
    );
}

#[tokio::test]
async fn der_push_test_schickt_die_beispielmeldung_an_den_eingetragenen_server() {
    let stub = NtfyStub::start(200).await;
    let server = server();
    UserSettingsStore::new(server.home.clone())
        .write(&UserSettings {
            ntfy: true,
            ntfy_server: stub.url(),
            ntfy_topic: "harness-test".into(),
            ntfy_token: "tk_test".into(),
            ..UserSettings::default()
        })
        .expect("Einstellungen");

    let reply = server.post("/api/user-settings/test-ntfy", "{}").await;
    assert_eq!(
        reply.json().expect("JSON"),
        serde_json::json!({ "ok": true, "error": null })
    );
    let requests = stub.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    let request = &requests[0];
    assert!(request.starts_with("POST / HTTP/1.1"), "{request}");
    assert!(
        request.contains("authorization: Bearer tk_test"),
        "{request}"
    );
    assert!(request.contains("\"topic\":\"harness-test\""), "{request}");
    assert!(
        request.contains("Beispielprojekt · ARC-0042 Favoriten\\nTestmeldung des Harness"),
        "{request}"
    );
    // Ein Link auf den eigenen Rechner öffnet auf dem Handy nichts und fehlt deshalb
    assert!(!request.contains("\"click\""), "{request}");
}

#[tokio::test]
async fn ein_fehler_des_ntfy_servers_steht_im_fehlertext() {
    let stub = NtfyStub::start(500).await;
    let server = server();
    UserSettingsStore::new(server.home.clone())
        .write(&UserSettings {
            ntfy: true,
            ntfy_server: stub.url(),
            ntfy_topic: "harness-test".into(),
            ..UserSettings::default()
        })
        .expect("Einstellungen");

    let reply = server.post("/api/user-settings/test-ntfy", "{}").await;
    let json = reply.json().expect("JSON");
    assert_eq!(json["ok"], false);
    assert!(
        json["error"]
            .as_str()
            .expect("Fehlertext")
            .starts_with("ntfy antwortete mit 500"),
        "{json}"
    );
}
