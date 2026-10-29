//! Favoriten: markieren mit Notiz, Befehl und Ticketangaben, Notiz ändern, entfernen,
//! Trennung je Projekt, Neustart, fehlendes Arbeitsverzeichnis und fortgesetzte Sessions;
//! portiert aus `packages/server/tests/unit/Favorites.test.ts`.

mod common;

use std::sync::Arc;

use common::{BoardBackendWithServices, TempRepo, TestServer, TestServerOptions};
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use nexus_server::board_runtime::{BoardBackend, BoardBackendFuture};
use serde_json::{json, Value};

/// Die Umgebung der Favoriten-Tests
struct Fixture {
    server: TestServer,
    backend: Arc<BoardBackendWithServices>,
    pid: String,
    repo: TempRepo,
    token: String,
    /// Arbeitsverzeichnis der Sessions (ein eigener Ordner, den Tests löschen dürfen)
    cwd: tempfile::TempDir,
}

impl Fixture {
    /// Adresse der Favoriten des Projekts
    fn base(&self) -> String {
        format!("/api/projects/{}/favorites", self.pid)
    }

    /// Schickt ein Hook-Ereignis des Projekts
    async fn hook(&self, event: Value) -> common::Reply {
        self.server
            .hook(
                &self.repo.path().to_string_lossy(),
                event,
                Some(&self.token),
            )
            .await
    }

    /// Startet eine Session mit dem Ticket `ARC-0001`
    async fn start(&self, session_id: &str, extra: Value) -> common::Reply {
        let mut event = json!({
            "kind": "session-start", "sessionId": session_id,
            "cwd": self.cwd.path().to_string_lossy(), "tickets": ["ARC-0001"],
        });
        for (name, value) in extra.as_object().into_iter().flatten() {
            event[name] = value.clone();
        }
        self.hook(event).await
    }

    /// Die Favoriten des Projekts als JSON-Liste
    async fn list(&self) -> Vec<Value> {
        let reply = self.server.get(&self.base()).await;
        assert_eq!(reply.status, 200, "{}", reply.text);
        reply
            .json()
            .and_then(|json| json.as_array().cloned())
            .expect("Liste der Favoriten")
    }
}

/// Server mit Board, Ticket `ARC-0001` und angemeldetem Projekt
async fn setup() -> Fixture {
    let (server, backend) = TestServer::start_with_board().await;
    backend
        .services
        .projects
        .create("ARC", "Arches", &backend.actor())
        .await
        .expect("Projekt anlegen");
    let board = backend
        .services
        .boards
        .create("ARC", "Sprint", &backend.actor())
        .await
        .expect("Board anlegen");
    backend
        .services
        .tickets
        .create(
            board.board.id,
            &NewTicketInput {
                title: "Erstes".into(),
                ..NewTicketInput::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");
    let (repo, pid) = server.add_project();
    let token = server.token(repo.path());
    let cwd = tempfile::Builder::new()
        .prefix("harness fav ")
        .tempdir()
        .expect("Arbeitsverzeichnis");
    Fixture {
        server,
        backend,
        pid,
        repo,
        token,
        cwd,
    }
}

#[tokio::test]
async fn markiert_eine_bekannte_session_mit_notiz_und_liefert_befehl_und_ticketangaben() {
    let fixture = setup().await;
    fixture.start("s1", json!({})).await;
    let reply = fixture
        .server
        .put(
            &format!("{}/s1", fixture.base()),
            &json!({ "note": "Weiter mit Tests" }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    let favorite = reply.json().expect("JSON");
    assert_eq!(favorite["sessionId"], "s1");
    assert_eq!(favorite["note"], "Weiter mit Tests");
    assert_eq!(favorite["tickets"], json!(["ARC-0001"]));
    assert_eq!(favorite["live"], true);
    assert_eq!(favorite["cwdExists"], true);
    assert_eq!(favorite["ticketInfo"][0]["id"], "ARC-0001");
    assert_eq!(favorite["ticketInfo"][0]["title"], "Erstes");
    assert_eq!(
        favorite["command"],
        format!(
            "cd \"{}\" && claude --resume s1",
            fixture.cwd.path().to_string_lossy()
        )
    );
    assert_eq!(fixture.list().await.len(), 1);
}

#[tokio::test]
async fn meldet_eine_unbekannte_session_mit_422() {
    let fixture = setup().await;
    let reply = fixture
        .server
        .put(&format!("{}/gibtsnicht", fixture.base()), "{}")
        .await;
    assert_eq!(reply.status, 422, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("session-unknown"));
}

#[tokio::test]
async fn andert_die_notiz_und_entfernt_den_favoriten_wieder() {
    let fixture = setup().await;
    fixture.start("s1", json!({})).await;
    let base = fixture.base();
    fixture
        .server
        .put(&format!("{base}/s1"), &json!({ "note": "a" }).to_string())
        .await;
    let changed = fixture
        .server
        .put(&format!("{base}/s1"), &json!({ "note": "b" }).to_string())
        .await;
    assert_eq!(changed.json().expect("JSON")["note"], "b");
    assert_eq!(
        fixture.server.delete(&format!("{base}/s1")).await.status,
        204
    );
    assert!(fixture.list().await.is_empty());
    let sessions = fixture
        .server
        .get(&format!("/api/projects/{}/sessions", fixture.pid))
        .await;
    assert_eq!(
        sessions
            .json()
            .and_then(|json| json.as_array().map(Vec::len)),
        Some(1)
    );
}

#[tokio::test]
async fn trennt_favoriten_je_projekt() {
    let fixture = setup().await;
    fixture.start("s1", json!({})).await;
    fixture
        .server
        .put(&format!("{}/s1", fixture.base()), "{}")
        .await;
    let (_other_repo, other_pid) = fixture.server.add_project();
    let other = fixture
        .server
        .get(&format!("/api/projects/{other_pid}/favorites"))
        .await;
    assert_eq!(other.json(), Some(json!([])));
}

#[tokio::test]
async fn behalt_den_favoriten_nach_einem_neustart_und_zeigt_den_letzten_zustand() {
    let fixture = setup().await;
    fixture.start("s1", json!({})).await;
    fixture
        .server
        .put(
            &format!("{}/s1", fixture.base()),
            &json!({ "note": "bleibt" }).to_string(),
        )
        .await;
    fixture
        .hook(json!({
            "kind": "user-prompt", "sessionId": "s1",
            "cwd": fixture.cwd.path().to_string_lossy(),
        }))
        .await;

    // Neustart mit demselben Benutzerordner und derselben Board-Laufzeit: die Sessions sind
    // nur im Speicher gewesen, die Favoriten stehen in der Datei
    let shared = fixture.backend.clone();
    let again = fixture.server.restarted_with(TestServerOptions {
        board: Some(nexus_server::board_runtime::factory(
            move || -> BoardBackendFuture {
                let backend = shared.clone();
                Box::pin(async move { Ok(backend as Arc<dyn BoardBackend>) })
            },
        )),
        ..TestServerOptions::default()
    });
    let reply = again.get(&fixture.base()).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    let list = reply.json().expect("JSON");
    assert_eq!(list[0]["sessionId"], "s1");
    assert_eq!(list[0]["note"], "bleibt");
    assert_eq!(list[0]["live"], false);
    assert_eq!(list[0]["state"], "running");
    assert_eq!(list[0]["tickets"], json!(["ARC-0001"]));
}

#[tokio::test]
async fn weist_ein_fehlendes_arbeitsverzeichnis_aus_und_lasst_den_befehl_bestehen() {
    let fixture = setup().await;
    fixture.start("s1", json!({})).await;
    fixture
        .server
        .put(&format!("{}/s1", fixture.base()), "{}")
        .await;
    std::fs::remove_dir_all(fixture.cwd.path()).expect("Arbeitsverzeichnis löschen");
    let list = fixture.list().await;
    assert_eq!(list[0]["cwdExists"], false);
    assert!(
        list[0]["command"]
            .as_str()
            .is_some_and(|command| command.contains("claude --resume s1")),
        "{}",
        list[0]
    );
}

#[tokio::test]
async fn ordnet_eine_fortgesetzte_session_dem_einzigen_favoriten_zu() {
    let fixture = setup().await;
    fixture.start("s1", json!({})).await;
    fixture
        .server
        .put(&format!("{}/s1", fixture.base()), "{}")
        .await;
    fixture.start("s2", json!({ "source": "resume" })).await;
    assert_eq!(fixture.list().await[0]["resumedAs"], json!(["s2"]));
}

#[tokio::test]
async fn ordnet_einen_normalen_start_nicht_zu() {
    let fixture = setup().await;
    fixture.start("s1", json!({})).await;
    fixture
        .server
        .put(&format!("{}/s1", fixture.base()), "{}")
        .await;
    fixture.start("s2", json!({ "source": "startup" })).await;
    assert_eq!(fixture.list().await[0]["resumedAs"], json!([]));
}

#[tokio::test]
async fn lehnt_eine_notiz_ab_die_kein_text_ist_und_kurzt_lange_notizen() {
    let fixture = setup().await;
    fixture.start("s1", json!({})).await;
    let base = fixture.base();
    let invalid = fixture
        .server
        .put(&format!("{base}/s1"), &json!({ "note": 7 }).to_string())
        .await;
    assert_eq!(invalid.status, 422, "{}", invalid.text);
    assert_eq!(invalid.code().as_deref(), Some("note-invalid"));

    let long = "x".repeat(2500);
    let reply = fixture
        .server
        .put(&format!("{base}/s1"), &json!({ "note": long }).to_string())
        .await;
    assert_eq!(
        reply.json().expect("JSON")["note"].as_str().map(str::len),
        Some(2000)
    );
}

#[tokio::test]
async fn andert_bei_einer_nur_noch_gemerkten_session_allein_die_notiz() {
    let fixture = setup().await;
    fixture.start("s1", json!({})).await;
    let base = fixture.base();
    fixture
        .server
        .put(&format!("{base}/s1"), &json!({ "note": "alt" }).to_string())
        .await;

    // Nach dem Neustart ist die Session nicht mehr bekannt, der Favorit schon
    let again = fixture.server.restarted_with(TestServerOptions::default());
    let reply = again
        .put(&format!("{base}/s1"), &json!({ "note": "neu" }).to_string())
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    let favorite = reply.json().expect("JSON");
    assert_eq!(favorite["note"], "neu");
    assert_eq!(favorite["live"], false);
}
