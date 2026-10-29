//! Ticketangaben an Sessions aus der Board-Datenbank; portiert aus
//! `packages/server/tests/unit/SessionTicketInfo.test.ts`.

mod common;

use common::{TempRepo, TestServer};
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use serde_json::json;

/// Meldet ein Projekt an und startet eine Session mit diesen Tickets
async fn session_with(server: &TestServer, tickets: Vec<&str>) -> (String, TempRepo) {
    let (repo, pid) = server.add_project();
    let token = server.token(repo.path());
    let reply = server
        .hook(
            &repo.path().to_string_lossy(),
            json!({
                "kind": "session-start", "sessionId": "s1", "cwd": "D:/wt",
                "tickets": tickets,
            }),
            Some(&token),
        )
        .await;
    assert_eq!(reply.status, 204, "{}", reply.text);
    (pid, repo)
}

#[tokio::test]
async fn liefert_titel_und_status_der_db_tickets() {
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
                title: "Login".into(),
                status: Some(TicketStatus::Planned),
                ..NewTicketInput::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");

    let (pid, _repo) = session_with(&server, vec!["ARC-0001", "ARC-0099"]).await;
    let list = server.get(&format!("/api/projects/{pid}/sessions")).await;
    let sessions = list.json().expect("JSON");
    assert_eq!(sessions[0]["tickets"], json!(["ARC-0001", "ARC-0099"]));
    assert_eq!(
        sessions[0]["ticketInfo"],
        json!([
            { "id": "ARC-0001", "title": "Login", "status": "planned" },
            { "id": "ARC-0099", "title": "(unbekannt)", "status": "backlog" },
        ])
    );
}

#[tokio::test]
async fn zeigt_sessions_auch_ohne_eingerichtete_board_db() {
    let server = TestServer::with(common::TestServerOptions {
        board_from_config: true,
        ..common::TestServerOptions::default()
    });
    let (pid, _repo) = session_with(&server, vec!["ARC-0001"]).await;
    let list = server.get(&format!("/api/projects/{pid}/sessions")).await;
    assert_eq!(list.status, 200, "{}", list.text);
    let sessions = list.json().expect("JSON");
    assert_eq!(
        sessions[0]["ticketInfo"],
        json!([{ "id": "ARC-0001", "title": "(unbekannt)", "status": "backlog" }])
    );
}
