//! Der Spec-Fortschritt eines Projekts; portiert aus
//! `packages/server/tests/unit/SpecRoutes.test.ts`.

mod common;

use common::TestServer;
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use serde_json::json;

#[tokio::test]
async fn kennt_die_liste_der_spec_ordner_nicht_mehr() {
    let server = TestServer::start();
    let (_repo, pid) = server.add_project();

    assert_eq!(
        server
            .get(&format!("/api/projects/{pid}/specs"))
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn meldet_ein_unbekanntes_projekt_mit_404() {
    let server = TestServer::start();
    server.add_project();

    assert_eq!(
        server
            .get("/api/projects/gibtsnicht/spec-progress")
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn liefert_ohne_board_konfiguration_einen_leeren_board_fortschritt() {
    let server = TestServer::start();
    let (_repo, pid) = server.add_project();

    let res = server
        .get(&format!("/api/projects/{pid}/spec-progress"))
        .await;

    assert_eq!(res.status, 200);
    assert_eq!(res.json(), Some(json!([])));
}

#[tokio::test]
async fn zahlt_die_specs_des_db_projekts_dessen_board_zum_repo_des_projekts_gehort() {
    let (server, backend) = TestServer::start_with_board().await;
    let (repo, pid) = server.add_project();
    let actor = backend.actor();
    backend
        .services
        .projects
        .create("ARC", "Arches", &actor)
        .await
        .expect("DB-Projekt");
    let board_id = backend
        .services
        .boards
        .create("ARC", "Sprint", &actor)
        .await
        .expect("Board")
        .board
        .id;
    backend
        .services
        .tickets
        .create(
            board_id,
            &NewTicketInput {
                spec: Some("001-login".into()),
                status: Some(nexus_core::board::basics::TicketStatus::Blocked),
                ..NewTicketInput::titled("A")
            },
            &actor,
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket");
    let url = format!("/api/projects/{pid}/spec-progress");

    assert_eq!(server.get(&url).await.json(), Some(json!([])));

    backend
        .services
        .boards
        .set_repo_path(board_id, Some(&repo.path().to_string_lossy()))
        .await
        .expect("Repo-Pfad des Boards");

    assert_eq!(
        server.get(&url).await.json(),
        Some(json!([{
            "spec": "001-login", "total": 1, "done": 0, "blocked": 1,
            "waiting": 0, "inProgress": 0, "open": 0, "notNeeded": 0,
        }]))
    );
}
