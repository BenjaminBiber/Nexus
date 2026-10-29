//! Sicherheitsmatrix der Gruppe `sessions`: Hook, Zuordnung und Favoriten nur mit den
//! Pflichtköpfen, von der eigenen Herkunft und mit richtigem Token – und bei abgelehnten
//! Anfragen bleiben Sessions und Favoriten unverändert.
//!
//! Portiert aus den Sitzungs-, Hook- und Favoriten-Fällen von
//! `packages/server/tests/security/Matrix.test.ts`.

mod common;

use common::{Reply, TestServer, PORT};
use serde_json::json;

/// Eine Schreibroute der Gruppe samt Körper
type Route = (&'static str, String, String);

/// Die Schreibrouten der Gruppe für dieses Projekt
fn routes(pid: &str, project: &str) -> Vec<Route> {
    vec![
        (
            "POST",
            "/api/hook".to_string(),
            json!({
                "project": project,
                "event": { "kind": "session-start", "sessionId": "x", "cwd": "D:/x" },
            })
            .to_string(),
        ),
        (
            "POST",
            format!("/api/projects/{pid}/sessions/x/claim"),
            json!({ "ticket": "ARC-0001" }).to_string(),
        ),
        (
            "PUT",
            format!("/api/projects/{pid}/favorites/x"),
            json!({ "note": "Eingeschleust" }).to_string(),
        ),
        (
            "DELETE",
            format!("/api/projects/{pid}/favorites/x"),
            "{}".to_string(),
        ),
    ]
}

/// Schickt eine Anfrage mit genau diesen Köpfen
async fn send(server: &TestServer, route: &Route, headers: &[(&str, Option<&str>)]) -> Reply {
    server
        .request(route.0, &route.1, Some(&route.2), headers)
        .await
}

/// Sessions und Favoriten als Text, um Veränderungen zu erkennen
async fn snapshot(server: &TestServer, pid: &str) -> String {
    let sessions = server.get(&format!("/api/projects/{pid}/sessions")).await;
    let favorites = server.get(&format!("/api/projects/{pid}/favorites")).await;
    format!("{}|{}", sessions.text, favorites.text)
}

/// Server mit angemeldetem Projekt und dessen Pfad
async fn setup() -> (TestServer, String, common::TempRepo) {
    let server = TestServer::start();
    let (repo, pid) = server.add_project();
    (server, pid, repo)
}

#[tokio::test]
async fn lehnt_jede_schreibroute_ohne_pflichtkopf_oder_mit_falschem_inhaltstyp_ab() {
    let (server, pid, repo) = setup().await;
    for route in routes(&pid, &repo.path().to_string_lossy()) {
        let ohne_kopf = send(&server, &route, &[("x-harness", None)]).await;
        assert_eq!(ohne_kopf.status, 403, "{} {}", route.0, route.1);
        assert_eq!(ohne_kopf.code().as_deref(), Some("header-missing"));

        let falscher_typ = send(&server, &route, &[("content-type", Some("text/plain"))]).await;
        assert_eq!(falscher_typ.status, 403, "{} {}", route.0, route.1);
    }
}

#[tokio::test]
async fn lehnt_jede_schreibroute_mit_fremder_herkunft_oder_fremdem_host_ab() {
    let (server, pid, repo) = setup().await;
    for route in routes(&pid, &repo.path().to_string_lossy()) {
        let herkunft = send(&server, &route, &[("origin", Some("http://evil.example"))]).await;
        assert_eq!(herkunft.status, 403, "{} {}", route.0, route.1);
        assert_eq!(herkunft.code().as_deref(), Some("origin-denied"));

        let host = format!("evil.example:{PORT}");
        let fremd = send(&server, &route, &[("host", Some(&host))]).await;
        assert_eq!(fremd.status, 403, "{} {}", route.0, route.1);
        assert_eq!(fremd.code().as_deref(), Some("host-denied"));
    }
}

#[tokio::test]
async fn lehnt_auch_lesezugriffe_mit_fremder_herkunft_oder_fremdem_host_ab() {
    let (server, pid, _repo) = setup().await;
    for path in [
        format!("/api/projects/{pid}/sessions"),
        format!("/api/projects/{pid}/favorites"),
        "/api/claude-usage".to_string(),
    ] {
        let herkunft = server
            .get_with(&path, &[("origin", Some("http://evil.example"))])
            .await;
        assert_eq!(herkunft.status, 403, "{path}");
        let host = server
            .get_with(&path, &[("host", Some("evil.example"))])
            .await;
        assert_eq!(host.status, 403, "{path}");
    }
}

#[tokio::test]
async fn sessions_und_favoriten_bleiben_bei_abgelehnten_anfragen_unverandert() {
    let (server, pid, repo) = setup().await;
    let before = snapshot(&server, &pid).await;
    for route in routes(&pid, &repo.path().to_string_lossy()) {
        send(&server, &route, &[("x-harness", None)]).await;
        send(&server, &route, &[("origin", Some("http://evil.example"))]).await;
    }
    assert_eq!(snapshot(&server, &pid).await, before);
}

#[tokio::test]
async fn lehnt_das_hook_ereignis_mit_falschem_token_ab_und_legt_keine_session_an() {
    let (server, pid, repo) = setup().await;
    let routes = routes(&pid, &repo.path().to_string_lossy());
    let hook = &routes[0];
    let reply = send(&server, hook, &[("x-harness-token", Some("falsch"))]).await;
    assert_eq!(reply.status, 403, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("token-invalid"));
    let sessions = server.get(&format!("/api/projects/{pid}/sessions")).await;
    assert_eq!(sessions.json(), Some(json!([])));
}
