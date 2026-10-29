//! Die Notiz-Routen der Gruppe `db-misc`; portiert aus
//! `packages/server/tests/unit/DbNoteRoutes.test.ts`, dazu die Notiz-Zeile aus
//! `packages/server/tests/security/Matrix.test.ts`.

mod common;

use std::sync::Arc;

use common::{BoardBackendWithServices, Reply, TestServer};
use serde_json::{json, Value};

/// Server mit Projekt ARC und einem Board „Sprint“
async fn start() -> (TestServer, Arc<BoardBackendWithServices>, i32) {
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
    let id = board.board.id;
    (server, backend, id)
}

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

/// Eine Notiz über die Route anlegen
async fn add(server: &TestServer, board_id: i32, payload: Value) -> Reply {
    server
        .post(
            &format!("/api/db/boards/{board_id}/notes"),
            &payload.to_string(),
        )
        .await
}

#[tokio::test]
async fn legt_notizen_an_filtert_nach_zustand_ubernimmt_und_verwirft() {
    let (server, _backend, board_id) = start().await;
    let a = add(
        &server,
        board_id,
        json!({ "title": "Cache prüfen", "body": "Details", "labels": ["perf"] }),
    )
    .await;
    assert_eq!(a.status, 201);
    let note = body(&a);
    assert_eq!(note["title"], "Cache prüfen");
    assert_eq!(note["body"], "Details");
    assert_eq!(note["labels"], json!(["perf"]));
    assert_eq!(note["seenCount"], 1);
    assert_eq!(note["state"], "open");
    assert_eq!(note["authorName"], "Florian");
    let b = add(&server, board_id, json!({ "title": "Unwichtig" })).await;

    let taken = server
        .post(
            &format!("/api/db/notes/{}/take", note["id"]),
            &json!({ "title": "Cache prüfen" }).to_string(),
        )
        .await;
    assert_eq!(taken.status, 200);
    let taken = body(&taken);
    assert_eq!(taken["note"]["state"], "taken");
    assert_eq!(taken["note"]["ticketId"], "ARC-0001");
    assert_eq!(taken["ticket"]["id"], "ARC-0001");
    assert_eq!(taken["ticket"]["title"], "Cache prüfen");
    assert_eq!(taken["ticket"]["labels"], json!(["perf"]));

    let zweite = body(&b)["id"].clone();
    let ohne_grund = server
        .post(
            &format!("/api/db/notes/{zweite}/drop"),
            &json!({ "reason": " " }).to_string(),
        )
        .await;
    assert_eq!(code(&ohne_grund), "reason-missing");
    let verworfen = server
        .post(
            &format!("/api/db/notes/{zweite}/drop"),
            &json!({ "reason": "doppelt" }).to_string(),
        )
        .await;
    assert_eq!(body(&verworfen)["state"], "dropped");
    assert_eq!(body(&verworfen)["reason"], "doppelt");

    let offen = server
        .get(&format!("/api/db/boards/{board_id}/notes"))
        .await;
    assert_eq!(body(&offen), json!([]));
    let ubernommen = server
        .get(&format!("/api/db/boards/{board_id}/notes?state=taken"))
        .await;
    assert_eq!(body(&ubernommen).as_array().expect("Liste").len(), 1);
    let alle = server
        .get(&format!("/api/db/boards/{board_id}/notes?state=all"))
        .await;
    assert_eq!(body(&alle).as_array().expect("Liste").len(), 2);
    // Kein Filter der Liste, auch wenn der Name in JavaScript ein Feld jedes Objekts wäre
    let unbekannt = server
        .get(&format!(
            "/api/db/boards/{board_id}/notes?state=constructor"
        ))
        .await;
    assert_eq!(code(&unbekannt), "input-invalid");

    let fehlt = server.post("/api/db/notes/999/take", "{}").await;
    assert_eq!(fehlt.status, 404);
}

#[tokio::test]
async fn lehnt_einen_schon_notierten_befund_ab_legt_mit_force_an_und_zahlt_mit_seen_hoch() {
    let (server, _backend, board_id) = start().await;
    let first = add(
        &server,
        board_id,
        json!({ "title": "Dispatcher legt den Worktree-Branch doppelt an" }),
    )
    .await;
    let id = body(&first)["id"].clone();
    let again = json!({ "title": "Worktree-Branch wird vom Dispatcher doppelt angelegt" });
    let rejected = add(&server, board_id, again.clone()).await;
    assert_eq!(rejected.status, 422);
    let rejected = body(&rejected);
    assert_eq!(rejected["code"], "note-similar");
    assert_eq!(rejected["details"]["notes"][0]["id"], id);
    assert_eq!(rejected["details"]["notes"][0]["state"], "open");

    let forced = add(
        &server,
        board_id,
        json!({ "title": again["title"], "force": true }),
    )
    .await;
    assert_eq!(forced.status, 201);

    let seen = server.post(&format!("/api/db/notes/{id}/seen"), "{}").await;
    assert_eq!(body(&seen)["seenCount"], 2);

    let ohne_titel = add(&server, board_id, json!({ "body": "ohne Titel" })).await;
    assert_eq!(code(&ohne_titel), "input-invalid");
}

#[tokio::test]
async fn nimmt_eine_herkunft_nur_als_existierendes_ticket_an() {
    let (server, _backend, board_id) = start().await;
    let reply = add(
        &server,
        board_id,
        json!({ "title": "x", "source": "ARC-0042" }),
    )
    .await;
    assert_eq!(reply.status, 404);
}

/// Die Notiz-Zeile der Sicherheitsmatrix (`packages/server/tests/security/Matrix.test.ts`):
/// der Schutz greift vor der Route
#[tokio::test]
async fn lehnt_eine_neue_notiz_ohne_pflichtkopf_ab() {
    let (server, backend, board_id) = start().await;
    let reply = server
        .request(
            "POST",
            &format!("/api/db/boards/{board_id}/notes"),
            Some(&json!({ "body": "Eingeschleust" }).to_string()),
            &[("x-harness", None)],
        )
        .await;
    assert_eq!(reply.status, 403);
    assert_eq!(code(&reply), "header-missing");
    // Die Anfrage hat nicht gewirkt
    let notes = backend
        .services
        .notes
        .list(board_id, None, None)
        .await
        .expect("Notizen");
    assert!(notes.is_empty());
}
