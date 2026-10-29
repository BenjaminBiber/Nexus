//! `POST …/sessions/{sid}/claim`: Session einem Ticket der Board-Datenbank zuordnen;
//! portiert aus `packages/server/tests/unit/SessionClaim.test.ts`.

mod common;

use std::sync::Arc;

use common::{BoardBackendWithServices, TempRepo, TestServer};
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use serde_json::json;

/// Server mit Board, einem Ticket `ARC-0001` und einer laufenden Session `s1`
async fn setup() -> (TestServer, Arc<BoardBackendWithServices>, String, TempRepo) {
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
                title: "Eins".into(),
                ..NewTicketInput::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");
    let (repo, pid) = server.add_project();
    let token = server.token(repo.path());
    server
        .hook(
            &repo.path().to_string_lossy(),
            json!({ "kind": "session-start", "sessionId": "s1", "cwd": "D:/wt" }),
            Some(&token),
        )
        .await;
    (server, backend, pid, repo)
}

#[tokio::test]
async fn ordnet_die_session_dem_db_ticket_zu_und_verknupft_sie_in_der_db() {
    let (server, backend, pid, _repo) = setup().await;
    let reply = server
        .post(
            &format!("/api/projects/{pid}/sessions/s1/claim"),
            &json!({ "ticket": "arc-1" }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    let session = reply.json().expect("JSON");
    assert_eq!(session["tickets"], json!(["ARC-0001"]));
    assert_eq!(
        session["ticketInfo"],
        json!([{ "id": "ARC-0001", "title": "Eins", "status": "backlog" }])
    );
    let ticket = backend
        .services
        .tickets
        .get("ARC-0001")
        .await
        .expect("Ticket");
    assert_eq!(ticket.sessions, ["s1"]);
}

#[tokio::test]
async fn meldet_unbekannte_session_und_unbekanntes_ticket_mit_404() {
    let (server, backend, pid, _repo) = setup().await;
    let no_session = server
        .post(
            &format!("/api/projects/{pid}/sessions/gibtsnicht/claim"),
            &json!({ "ticket": "ARC-0001" }).to_string(),
        )
        .await;
    assert_eq!(no_session.status, 404, "{}", no_session.text);
    assert_eq!(no_session.code().as_deref(), Some("session-not-found"));
    let ticket = backend
        .services
        .tickets
        .get("ARC-0001")
        .await
        .expect("Ticket");
    assert!(ticket.sessions.is_empty());

    let no_ticket = server
        .post(
            &format!("/api/projects/{pid}/sessions/s1/claim"),
            &json!({ "ticket": "ARC-0099" }).to_string(),
        )
        .await;
    assert_eq!(no_ticket.status, 404, "{}", no_ticket.text);
    assert_eq!(no_ticket.code().as_deref(), Some("ticket-not-found"));
}

#[tokio::test]
async fn verlangt_die_angabe_des_tickets() {
    let (server, _backend, pid, _repo) = setup().await;
    let reply = server
        .post(&format!("/api/projects/{pid}/sessions/s1/claim"), "{}")
        .await;
    assert_eq!(reply.status, 422, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("ticket-missing"));
}

#[tokio::test]
async fn ubernimmt_die_kanonische_db_id_in_die_sessions_ansicht() {
    let (server, backend, pid, _repo) = setup().await;
    let board = backend
        .services
        .tickets
        .get("ARC-0001")
        .await
        .expect("Ticket")
        .board_id;
    backend
        .services
        .tickets
        .create(
            board,
            &NewTicketInput {
                title: "Zwei".into(),
                status: Some(nexus_core::board::basics::TicketStatus::Planned),
                ..NewTicketInput::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");
    let reply = server
        .post(
            &format!("/api/projects/{pid}/sessions/s1/claim"),
            &json!({ "ticket": "ARC-0002" }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text);

    let list = server.get(&format!("/api/projects/{pid}/sessions")).await;
    let sessions = list.json().expect("JSON");
    assert_eq!(sessions[0]["tickets"], json!(["ARC-0002"]));
    assert_eq!(
        sessions[0]["ticketInfo"],
        json!([{ "id": "ARC-0002", "title": "Zwei", "status": "planned" }])
    );
}
