//! Der Hook am laufenden Ereignisstrom: jede Änderung an den Sessions meldet `sessions`, ein
//! Zustandswechsel geht an die Benachrichtigungen und `sessions` steht nicht mehr in der
//! Brücke zum Node-Server.
//!
//! Vorbild sind der Fall „Sessions im Ereignisstrom“ aus
//! `packages/server/tests/unit/Sessions.test.ts` und der Hook-Fall der Meldungen.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use common::{TestServer, TestServerOptions};
use futures::StreamExt as _;
use http_body_util::BodyStream;
use nexus_core::settings::{NotificationMode, UserSettings, UserSettingsStore};
use nexus_platform::{FakeNotifier, Platform};
use serde_json::json;

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
        tokio::time::timeout(Duration::from_secs(3), async {
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
        .await
        .unwrap_or(false)
    }
}

/// Öffnet den Ereignisstrom eines Projekts
async fn listen(server: &TestServer, pid: &str) -> Stream {
    let response = server
        .send("GET", &format!("/api/projects/{pid}/events"), None, &[])
        .await;
    assert_eq!(response.status(), 200);
    Stream {
        frames: BodyStream::new(response.into_body()),
        text: String::new(),
    }
}

#[tokio::test]
async fn meldet_jede_anderung_an_sessions_als_sessions() {
    let server = TestServer::start();
    let (repo, pid) = server.add_project();
    let token = server.token(repo.path());
    let mut stream = listen(&server, &pid).await;
    assert!(stream.until(r#"{"type":"hello"}"#).await);

    let reply = server
        .hook(
            &repo.path().to_string_lossy(),
            json!({ "kind": "session-start", "sessionId": "s1", "cwd": "D:/wt" }),
            Some(&token),
        )
        .await;
    assert_eq!(reply.status, 204, "{}", reply.text);
    assert!(
        stream.until(r#"{"type":"sessions"}"#).await,
        "{}",
        stream.text
    );
}

#[tokio::test]
async fn ein_zustandswechsel_running_waiting_meldet_der_benachrichtigung() {
    let notifier = Arc::new(FakeNotifier::new());
    let server = TestServer::with(TestServerOptions {
        platform: Some(Platform {
            notifier: notifier.clone(),
            ..Platform::fake()
        }),
        ..TestServerOptions::default()
    });
    let (repo, _pid) = server.add_project();
    let token = server.token(repo.path());
    UserSettingsStore::new(server.home.clone())
        .write(&UserSettings {
            notifications: NotificationMode::Windows,
            ..UserSettings::default()
        })
        .expect("Einstellungen schreiben");
    let path = repo.path().to_string_lossy().to_string();

    // running: noch keine Meldung
    server
        .hook(
            &path,
            json!({ "kind": "user-prompt", "sessionId": "s1", "cwd": "D:/wt", "title": "Login" }),
            Some(&token),
        )
        .await;
    assert!(notifier.shown().is_empty());

    // running → waiting: die Session wartet auf den Benutzer
    let reply = server
        .hook(
            &path,
            json!({
                "kind": "notification", "sessionId": "s1", "cwd": "D:/wt",
                "notificationType": "permission_prompt",
            }),
            Some(&token),
        )
        .await;
    assert_eq!(reply.status, 204, "{}", reply.text);
    let shown = notifier.shown();
    assert_eq!(shown.len(), 1, "{shown:?}");
}
