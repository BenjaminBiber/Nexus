//! `GET /api/db/graph` und `GET /api/db/boards/{id}/spec-progress` der Gruppe `db-misc`;
//! portiert aus `packages/server/tests/unit/DbGraphRoutes.test.ts`.

mod common;

use common::{Reply, TestServer};
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use serde_json::{json, Value};

/// Der Körper einer Antwort als JSON
fn body(reply: &Reply) -> Value {
    reply
        .json()
        .unwrap_or_else(|| panic!("kein JSON: {}", reply.text))
}

/// Der Fehlercode einer Antwort
fn code(reply: &Reply) -> String {
    reply
        .code()
        .unwrap_or_else(|| panic!("kein Fehlercode: {}", reply.text))
}

/// Server mit Board „Sprint“, zwei Tickets einer Spec und einem Ticket ohne Kanten
async fn start() -> (TestServer, i32) {
    let (server, backend) = TestServer::start_with_board().await;
    let actor = backend.actor();
    backend
        .services
        .projects
        .create("ARC", "Arches", &actor)
        .await
        .expect("Projekt anlegen");
    let board_id = backend
        .services
        .boards
        .create("ARC", "Sprint", &actor)
        .await
        .expect("Board anlegen")
        .board
        .id;
    let tickets = [
        NewTicketInput {
            spec: Some("001-login".into()),
            ..NewTicketInput::titled("A")
        },
        NewTicketInput {
            spec: Some("001-login".into()),
            depends_on: Some(vec!["ARC-0001".into()]),
            ..NewTicketInput::titled("B")
        },
        NewTicketInput::titled("Allein"),
    ];
    for input in &tickets {
        backend
            .services
            .tickets
            .create(board_id, input, &actor, &WriteOptions::default())
            .await
            .expect("Ticket anlegen");
    }
    (server, board_id)
}

#[tokio::test]
async fn liefert_den_graph_fur_ticket_board_und_spec() {
    let (server, board_id) = start().await;
    let ticket = server.get("/api/db/graph?ticket=arc-2").await;
    assert_eq!(ticket.status, 200);
    let ticket = body(&ticket);
    assert_eq!(
        ticket["scope"],
        json!({ "kind": "ticket", "id": "ARC-0002" })
    );
    assert_eq!(
        ticket["edges"],
        json!([{ "from": "ARC-0001", "to": "ARC-0002" }])
    );
    assert_eq!(ticket["criticalPath"], json!(["ARC-0001", "ARC-0002"]));
    assert_eq!(ticket["nodes"][1]["id"], "ARC-0002");
    assert_eq!(ticket["nodes"][1]["column"], "waiting");
    assert_eq!(ticket["nodes"][1]["boardName"], "Sprint");
    assert_eq!(ticket["nodes"][1]["external"], false);

    let board = body(&server.get(&format!("/api/db/graph?board={board_id}")).await);
    assert_eq!(board["hiddenWithoutEdges"], 1);
    let ids: Vec<&str> = board["nodes"]
        .as_array()
        .expect("Knoten")
        .iter()
        .map(|node| node["id"].as_str().expect("Nummer"))
        .collect();
    assert_eq!(ids, ["ARC-0001", "ARC-0002"]);

    let spec = body(
        &server
            .get("/api/db/graph?project=ARC&spec=001-login&done=0")
            .await,
    );
    assert_eq!(
        spec["scope"],
        json!({ "kind": "spec", "project": "ARC", "spec": "001-login" })
    );
}

#[tokio::test]
async fn prufft_die_parameter() {
    let (server, board_id) = start().await;
    assert_eq!(code(&server.get("/api/db/graph").await), "input-invalid");
    assert_eq!(
        code(
            &server
                .get(&format!("/api/db/graph?ticket=ARC-0001&board={board_id}"))
                .await
        ),
        "input-invalid"
    );
    assert_eq!(
        code(&server.get("/api/db/graph?project=ARC").await),
        "input-invalid"
    );
    assert_eq!(
        code(&server.get("/api/db/graph?board=abc").await),
        "id-invalid"
    );
    assert_eq!(
        server.get("/api/db/graph?ticket=ARC-0099").await.status,
        404
    );
    assert_eq!(server.get("/api/db/graph?board=999").await.status, 404);
}

#[tokio::test]
async fn liefert_den_fortschritt_der_specs_eines_boards() {
    let (server, board_id) = start().await;
    let reply = server
        .get(&format!("/api/db/boards/{board_id}/spec-progress"))
        .await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        body(&reply),
        json!([{
            "spec": "001-login", "total": 2, "done": 0, "blocked": 0,
            "waiting": 1, "inProgress": 0, "open": 1, "notNeeded": 0,
        }])
    );
    assert_eq!(
        server.get("/api/db/boards/999/spec-progress").await.status,
        404
    );
}
