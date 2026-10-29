//! `/api/db/inbox` der Gruppe `db-misc`; portiert aus
//! `packages/server/tests/unit/DbInboxRoutes.test.ts`.

mod common;

use common::{Reply, TestServer, TestServerOptions};
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::inbox::types::MAX_INBOX_KEYS;
use nexus_core::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
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

/// Die Schlüssel der Einträge in ihrer Reihenfolge
fn keys(json: &Value) -> Vec<&str> {
    json["items"]
        .as_array()
        .expect("Einträge")
        .iter()
        .map(|item| item["key"].as_str().expect("Schlüssel"))
        .collect()
}

/// Server mit zwei Boards und drei wartenden Tickets
async fn start() -> (TestServer, i32) {
    let (server, backend) = TestServer::start_with_board().await;
    let s = &backend.services;
    let actor = backend.actor();
    s.projects
        .create("ARC", "Arches", &actor)
        .await
        .expect("Projekt anlegen");
    let board_id = s
        .boards
        .create("ARC", "Sprint", &actor)
        .await
        .expect("Board anlegen")
        .board
        .id;
    let second = s
        .boards
        .create("ARC", "Zweites", &actor)
        .await
        .expect("Board anlegen")
        .board
        .id;
    for (title, reason) in [
        ("Abnahme", "Human Interaction: ansehen"),
        ("Merge", "Merge: Konflikt"),
    ] {
        let ticket = s
            .tickets
            .create(
                board_id,
                &NewTicketInput {
                    status: Some(TicketStatus::Blocked),
                    ..NewTicketInput::titled(title)
                },
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket anlegen");
        s.tickets
            .update(
                &ticket.id,
                &TicketPatch {
                    blocked_reason: Some(Some(reason.into())),
                    ..TicketPatch::default()
                },
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Grund setzen");
    }
    s.tickets
        .create(
            second,
            &NewTicketInput {
                status: Some(TicketStatus::InPlanning),
                ..NewTicketInput::titled("Planen")
            },
            &actor,
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");
    (server, second)
}

#[tokio::test]
async fn liefert_die_eintrage_nach_dringlichkeit_mit_zahlern() {
    let (server, _second) = start().await;
    let reply = server.get("/api/db/inbox").await;
    assert_eq!(reply.status, 200);
    let json = body(&reply);
    assert_eq!(
        keys(&json),
        ["ticket:ARC-0002", "ticket:ARC-0001", "ticket:ARC-0003"]
    );
    assert_eq!(json["total"], 3);
    assert_eq!(json["counts"]["merge"], 1);
    assert_eq!(json["counts"]["accept"], 1);
    assert_eq!(json["counts"]["planning"], 1);
    assert_eq!(json["counts"]["note"], 0);
    assert_eq!(json["items"][0]["kind"], "merge");
    assert_eq!(json["items"][0]["boardName"], "Sprint");
    assert_eq!(json["items"][0]["projectKey"], "ARC");
    assert_eq!(json["items"][0]["ticketId"], "ARC-0002");
}

#[tokio::test]
async fn filtert_nach_board_und_lehnt_ungultige_boards_ab() {
    let (server, second) = start().await;
    let gefiltert = body(&server.get(&format!("/api/db/inbox?board={second}")).await);
    assert_eq!(keys(&gefiltert), ["ticket:ARC-0003"]);
    assert_eq!(server.get("/api/db/inbox?board=abc").await.status, 422);
    assert_eq!(
        code(&server.get("/api/db/inbox?board=999").await),
        "board-not-owned"
    );
}

#[tokio::test]
async fn speichert_die_reihenfolge_wendet_sie_an_und_setzt_sie_zuruck() {
    let (server, _second) = start().await;
    let put = server
        .put(
            "/api/db/inbox/order",
            &json!({ "keys": ["ticket:ARC-0003", "ticket:ARC-0001"] }).to_string(),
        )
        .await;
    assert_eq!(put.status, 200);
    assert_eq!(
        body(&put)["keys"],
        json!(["ticket:ARC-0003", "ticket:ARC-0001"])
    );
    assert!(server.home.inbox_order_file().exists());
    // ARC-0002 hat keine Position und steht als neu oben
    assert_eq!(
        keys(&body(&server.get("/api/db/inbox").await)),
        ["ticket:ARC-0002", "ticket:ARC-0003", "ticket:ARC-0001"]
    );

    let cleared = server.delete("/api/db/inbox/order").await;
    assert_eq!(cleared.status, 200);
    assert_eq!(body(&cleared)["keys"], json!([]));
    assert!(!server.home.inbox_order_file().exists());
    assert_eq!(
        keys(&body(&server.get("/api/db/inbox").await)),
        ["ticket:ARC-0002", "ticket:ARC-0001", "ticket:ARC-0003"]
    );
}

#[tokio::test]
async fn lehnt_ungultige_schlussel_und_zu_lange_listen_ab() {
    let (server, _second) = start().await;
    let lang = format!("ticket:{}", "A".repeat(60));
    let viele: Vec<String> = (0..=MAX_INBOX_KEYS)
        .map(|nr| format!("note:{nr}"))
        .collect();
    let falsche = [
        json!({ "keys": ["quatsch"] }),
        json!({ "keys": [42] }),
        // Kein Array, sondern ein Text
        json!({ "keys": lang }),
        json!({ "keys": [lang] }),
        json!({ "keys": viele }),
        json!({}),
    ];
    for payload in &falsche {
        let reply = server
            .put("/api/db/inbox/order", &payload.to_string())
            .await;
        assert_eq!(reply.status, 422, "{payload}");
        assert_eq!(code(&reply), "input-invalid", "{payload}");
    }
    assert!(!server.home.inbox_order_file().exists());
}

#[tokio::test]
async fn antwortet_ohne_board_datenbank_mit_503() {
    let server = TestServer::with(TestServerOptions {
        board_from_config: true,
        ..TestServerOptions::default()
    });
    let reply = server.get("/api/db/inbox").await;
    assert_eq!(reply.status, 503);
    assert_eq!(code(&reply), "board-config-missing");
}
