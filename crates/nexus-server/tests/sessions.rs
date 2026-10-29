//! Hook-Schnittstelle und Sessions: Ereignisse annehmen, Zustände führen, Token prüfen,
//! Projekte trennen, speichern und wieder laden; portiert aus
//! `packages/server/tests/unit/Sessions.test.ts`.

mod common;

use common::{TestServer, TestServerOptions};
use serde_json::{json, Value};

/// Das Startereignis der Tests
fn start() -> Value {
    json!({ "kind": "session-start", "sessionId": "s1", "cwd": "D:/wt", "tickets": ["T-0001"] })
}

/// Server mit angemeldetem Projekt samt Token
async fn setup() -> (TestServer, String, common::TempRepo, String) {
    let server = TestServer::start();
    let (repo, pid) = server.add_project();
    let token = server.token(repo.path());
    (server, pid, repo, token)
}

/// Die Sessions eines Projekts als JSON-Liste
async fn sessions(server: &TestServer, pid: &str) -> Vec<Value> {
    let reply = server.get(&format!("/api/projects/{pid}/sessions")).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    reply
        .json()
        .and_then(|json| json.as_array().cloned())
        .expect("Liste der Sessions")
}

#[tokio::test]
async fn nimmt_ein_ereignis_mit_richtigem_token_an_und_zeigt_die_session() {
    let (server, pid, repo, token) = setup().await;
    let reply = server
        .hook(&repo.path().to_string_lossy(), start(), Some(&token))
        .await;
    assert_eq!(reply.status, 204, "{}", reply.text);
    assert_eq!(reply.header("x-nexus-backend"), Some("rust"));

    let list = sessions(&server, &pid).await;
    assert_eq!(list.len(), 1);
    let session = &list[0];
    assert_eq!(session["id"], "s1");
    assert_eq!(session["projectId"], pid.as_str());
    assert_eq!(session["tickets"], json!(["T-0001"]));
    assert_eq!(session["cwd"], "D:/wt");
    assert_eq!(session["state"], "idle");
    assert_eq!(session["displayState"], "idle");
    assert_eq!(session["listening"], true);
    assert_eq!(session["endedAt"], Value::Null);
    assert!(session["runtimeMs"].is_number(), "{session}");
    assert_eq!(
        session["ticketInfo"],
        json!([{ "id": "T-0001", "title": "(unbekannt)", "status": "backlog" }])
    );
}

#[tokio::test]
async fn fuhrt_zustande_und_subagenten() {
    let (server, pid, repo, token) = setup().await;
    let path = repo.path().to_string_lossy().to_string();
    let hook = |event: Value| server.hook(&path, event, Some(&token));
    hook(start()).await;
    hook(json!({ "kind": "user-prompt", "sessionId": "s1", "cwd": "D:/wt" })).await;
    hook(json!({
        "kind": "subagent-start", "sessionId": "s1", "cwd": "D:/wt",
        "agentId": "a1", "agentType": "Explore",
    }))
    .await;

    let session = sessions(&server, &pid).await.remove(0);
    assert_eq!(session["state"], "running");
    assert_eq!(session["subagents"][0]["id"], "a1");
    assert_eq!(session["subagents"][0]["type"], "Explore");
    assert_eq!(session["subagents"][0]["state"], "running");
    assert_eq!(session["subagents"][0]["endedAt"], Value::Null);

    hook(json!({
        "kind": "subagent-stop", "sessionId": "s1", "cwd": "D:/wt",
        "agentId": "a1", "agentType": "Explore",
    }))
    .await;
    hook(json!({ "kind": "session-end", "sessionId": "s1", "cwd": "D:/wt" })).await;
    let session = sessions(&server, &pid).await.remove(0);
    assert_eq!(session["state"], "ended");
    assert_eq!(session["displayState"], "ended");
    assert_eq!(session["listening"], false);
    assert!(session["endedAt"].is_string(), "{session}");
    assert_eq!(session["subagents"][0]["state"], "ended");
}

#[tokio::test]
async fn lehnt_fehlendes_oder_falsches_token_mit_403_ab_und_andert_nichts() {
    let (server, pid, repo, token) = setup().await;
    let path = repo.path().to_string_lossy().to_string();
    assert_eq!(server.hook(&path, start(), None).await.status, 403);
    let wrong = server.hook(&path, start(), Some("falsch")).await;
    assert_eq!(wrong.status, 403);
    assert_eq!(wrong.code().as_deref(), Some("token-invalid"));
    // Gleiche Länge, ein anderes Zeichen am Ende
    let last = if token.ends_with('0') { '1' } else { '0' };
    let near = format!("{}{last}", &token[..token.len() - 1]);
    assert_eq!(server.hook(&path, start(), Some(&near)).await.status, 403);
    assert!(sessions(&server, &pid).await.is_empty());
}

#[tokio::test]
async fn meldet_fur_ein_nicht_angemeldetes_oder_nicht_eingerichtetes_projekt_verstandliche_fehler()
{
    let (server, _pid, _repo, token) = setup().await;
    // Eingerichtet, aber nicht in der Projektliste
    let fresh = common::TempRepo::new();
    nexus_core::HarnessProject::init(fresh.path()).expect("Projekt einrichten");
    let reply = server
        .hook(&fresh.path().to_string_lossy(), start(), Some(&token))
        .await;
    assert_eq!(reply.status, 404, "{}", reply.text);

    // Gar nicht eingerichtet
    let bare = common::TempRepo::new();
    let reply = server
        .hook(&bare.path().to_string_lossy(), start(), Some(&token))
        .await;
    assert_eq!(reply.status, 422, "{}", reply.text);

    // Kein absoluter Pfad
    let reply = server.hook("relativ", start(), Some(&token)).await;
    assert_eq!(reply.status, 422, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("path-missing"));
}

#[tokio::test]
async fn lehnt_ungultige_ereignisse_mit_422_ab() {
    let (server, _pid, repo, token) = setup().await;
    let path = repo.path().to_string_lossy().to_string();
    for event in [
        json!({}),
        json!({ "kind": "quatsch", "sessionId": "s", "cwd": "x" }),
        json!({ "kind": "stop", "cwd": "x" }),
        json!({ "kind": "stop", "sessionId": "", "cwd": "x" }),
        json!("text"),
        Value::Null,
    ] {
        let reply = server.hook(&path, event.clone(), Some(&token)).await;
        assert_eq!(reply.status, 422, "{event}");
        assert_eq!(reply.code().as_deref(), Some("event-invalid"), "{event}");
    }
}

#[tokio::test]
async fn halt_die_sessions_der_projekte_getrennt() {
    let (server, pid, repo, token) = setup().await;
    let (second_repo, second_pid) = server.add_project();
    let second_token = server.token(second_repo.path());
    server
        .hook(&repo.path().to_string_lossy(), start(), Some(&token))
        .await;
    let mut second_event = start();
    second_event["sessionId"] = json!("s2");
    server
        .hook(
            &second_repo.path().to_string_lossy(),
            second_event,
            Some(&second_token),
        )
        .await;

    let ids = |list: Vec<Value>| -> Vec<String> {
        list.iter()
            .map(|session| session["id"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    assert_eq!(ids(sessions(&server, &pid).await), ["s1"]);
    assert_eq!(ids(sessions(&server, &second_pid).await), ["s2"]);
    // Das Token eines anderen Projekts gilt nicht
    let reply = server
        .hook(&second_repo.path().to_string_lossy(), start(), Some(&token))
        .await;
    assert_eq!(reply.status, 403);
}

#[tokio::test]
async fn speichert_die_sessions_und_ladt_sie_beim_neustart_wieder() {
    let (server, pid, repo, token) = setup().await;
    let path = repo.path().to_string_lossy().to_string();
    server.hook(&path, start(), Some(&token)).await;
    server
        .hook(
            &path,
            json!({
                "kind": "subagent-start", "sessionId": "s1", "cwd": "D:/wt",
                "agentId": "a1", "agentType": "Explore",
            }),
            Some(&token),
        )
        .await;
    // Schreibt ausstehende Änderungen (wie das Beenden des Servers)
    server.state.sessions.close().await;
    assert!(server.home.sessions_file().exists());

    let again = server.restarted_with(TestServerOptions::default());
    let list = sessions(&again, &pid).await;
    assert_eq!(list[0]["id"], "s1");
    assert_eq!(list[0]["subagents"][0]["id"], "a1");
}

#[tokio::test]
async fn startet_nach_beschadigter_sessiondatei_leer_statt_zu_scheitern() {
    let (server, pid, repo, token) = setup().await;
    server
        .hook(&repo.path().to_string_lossy(), start(), Some(&token))
        .await;
    server.state.sessions.close().await;
    std::fs::write(server.home.sessions_file(), "{kaputt").expect("Datei beschädigen");

    let again = server.restarted_with(TestServerOptions::default());
    assert!(sessions(&again, &pid).await.is_empty());
}

#[tokio::test]
async fn antwortet_fur_ein_unbekanntes_projekt_mit_404() {
    let server = TestServer::start();
    let reply = server.get("/api/projects/00000000/sessions").await;
    assert_eq!(reply.status, 404);
}

#[tokio::test]
async fn schreibt_sessions_json_auch_ohne_beenden_entprellt() {
    let (server, _pid, repo, token) = setup().await;
    server
        .hook(&repo.path().to_string_lossy(), start(), Some(&token))
        .await;
    // Der entprellte Schreibauftrag läuft nach 300 ms von selbst
    for _ in 0..30 {
        if server.home.sessions_file().exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let text = std::fs::read_to_string(server.home.sessions_file()).expect("sessions.json");
    let file: Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(file["sessions"][0]["id"], "s1");
}

#[tokio::test]
async fn lasst_die_gruppe_sessions_mit_dem_notschalter_zum_node_server() {
    let server = TestServer::with(TestServerOptions {
        proxy_groups: nexus_server::groups::RouteGroups::from_env(Some("sessions")),
        ..TestServerOptions::default()
    });
    let (_repo, pid) = server.add_project();
    // Ohne Upstream fällt die Gruppe auf 404 des Rückfalls – Rust beantwortet sie nicht mehr
    let reply = server.get(&format!("/api/projects/{pid}/sessions")).await;
    assert_eq!(reply.code().as_deref(), Some("not-found"));
}
