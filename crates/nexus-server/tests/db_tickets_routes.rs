//! Die Routengruppe `db-tickets`; portiert aus
//! `packages/server/tests/unit/DbTicketRoutes.test.ts`, dazu die Zeilen dieser Gruppe aus
//! `packages/server/tests/security/Matrix.test.ts`.

mod common;

use std::sync::Arc;

use common::{BoardBackendWithServices, Reply, TestServer};
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::database::BoardDatabase as _;
use nexus_core::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
use nexus_core::board::types::TicketStep;
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

/// Ein Ticket über die Route anlegen
async fn create(server: &TestServer, board_id: i32, payload: Value) -> Reply {
    server
        .post(
            &format!("/api/db/boards/{board_id}/tickets"),
            &payload.to_string(),
        )
        .await
}

/// Ein Schritt mit Text, noch nicht abgehakt
fn step(text: &str) -> TicketStep {
    TicketStep {
        text: text.into(),
        done: false,
        detail: None,
    }
}

#[tokio::test]
async fn legt_ein_ticket_an_und_liefert_es_mit_kommentaren_und_verlauf() {
    let (server, _backend, board_id) = start().await;
    let created = create(
        &server,
        board_id,
        json!({ "title": "Login", "description": "Text", "priority": "high", "labels": ["ui"], "needsPlanning": false }),
    )
    .await;
    assert_eq!(created.status, 201);
    let ticket = body(&created);
    assert_eq!(ticket["id"], "ARC-0001");
    assert_eq!(ticket["title"], "Login");
    assert_eq!(ticket["priority"], "high");
    assert_eq!(ticket["labels"], json!(["ui"]));
    assert_eq!(ticket["needsPlanning"], false);
    assert_eq!(ticket["column"], "backlog");
    assert_eq!(ticket["rev"], 1);

    let detail = body(&server.get("/api/db/tickets/ARC-0001").await);
    assert_eq!(detail["id"], "ARC-0001");
    assert_eq!(detail["comments"], json!([]));
    let first = &detail["history"][0];
    assert_eq!(first["note"], "angelegt");
    assert_eq!(first["byName"], common::MEMORY_USER_NAME);

    assert_eq!(server.get("/api/db/tickets/ARC-0099").await.status, 404);
    assert_eq!(
        code(&create(&server, board_id, json!({ "title": " " })).await),
        "title-empty"
    );
    assert_eq!(
        create(&server, 999, json!({ "title": "A" })).await.status,
        404
    );
}

#[tokio::test]
async fn lehnt_ein_offenes_ticket_mit_gleichem_titel_mit_422_ab() {
    let (server, _backend, board_id) = start().await;
    create(&server, board_id, json!({ "title": "Login" })).await;
    let dup = create(&server, board_id, json!({ "title": " login " })).await;
    assert_eq!(dup.status, 422);
    assert_eq!(body(&dup)["code"], "ticket-duplicate");
    assert_eq!(
        body(&dup)["message"],
        "Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0001."
    );
}

#[tokio::test]
async fn leert_branch_und_worktree_gemeinsam_und_lasst_sie_nicht_setzen() {
    let (server, backend, board_id) = start().await;
    create(&server, board_id, json!({ "title": "Login" })).await;
    backend
        .services
        .tickets
        .update(
            "ARC-0001",
            &TicketPatch {
                branch: Some(Some("ticket/ARC-0001-login".into())),
                worktree: Some(Some("C:/w/ARC-0001".into())),
                ..TicketPatch::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Branch setzen");

    let setzen = server
        .patch(
            "/api/db/tickets/ARC-0001",
            &json!({ "rev": 2, "branch": "anderer" }).to_string(),
        )
        .await;
    assert_eq!(code(&setzen), "input-invalid");

    let cleared = server
        .patch(
            "/api/db/tickets/ARC-0001",
            &json!({ "rev": 2, "branch": Value::Null }).to_string(),
        )
        .await;
    assert_eq!(cleared.status, 200);
    let json = body(&cleared);
    assert_eq!(json["branch"], Value::Null);
    assert_eq!(json["worktree"], Value::Null);
    assert_eq!(json["rev"], 3);
    let history = json["history"].as_array().expect("Verlauf").clone();
    for (field, from) in [
        ("branch", "ticket/ARC-0001-login"),
        ("worktree", "C:/w/ARC-0001"),
    ] {
        assert!(
            history.iter().any(|entry| {
                entry["field"] == field && entry["from"] == from && entry["to"] == Value::Null
            }),
            "{field} fehlt im Verlauf: {history:?}"
        );
    }
}

#[tokio::test]
async fn setzt_und_leert_den_zielbranch_und_lehnt_ungultige_namen_ab() {
    let (server, _backend, board_id) = start().await;
    let created = create(
        &server,
        board_id,
        json!({ "title": "Login", "targetBranch": " release/1 " }),
    )
    .await;
    assert_eq!(created.status, 201);
    assert_eq!(body(&created)["id"], "ARC-0001");
    assert_eq!(body(&created)["targetBranch"], "release/1");

    let ohne = create(&server, board_id, json!({ "title": "Ohne" })).await;
    assert_eq!(body(&ohne)["targetBranch"], Value::Null);

    let changed = server
        .patch(
            "/api/db/tickets/ARC-0002",
            &json!({ "rev": 1, "targetBranch": "release/2" }).to_string(),
        )
        .await;
    assert_eq!(body(&changed)["targetBranch"], "release/2");
    assert_eq!(body(&changed)["rev"], 2);
    assert!(body(&changed)["history"]
        .as_array()
        .expect("Verlauf")
        .iter()
        .any(|entry| entry["field"] == "targetBranch"
            && entry["from"] == Value::Null
            && entry["to"] == "release/2"));

    let cleared = server
        .patch(
            "/api/db/tickets/ARC-0001",
            &json!({ "rev": 1, "targetBranch": Value::Null }).to_string(),
        )
        .await;
    assert_eq!(cleared.status, 200);
    assert_eq!(body(&cleared)["targetBranch"], Value::Null);
    assert_eq!(body(&cleared)["rev"], 2);

    let invalid = server
        .patch(
            "/api/db/tickets/ARC-0001",
            &json!({ "rev": 2, "targetBranch": "ticket/x" }).to_string(),
        )
        .await;
    assert_eq!(invalid.status, 422);
    assert_eq!(code(&invalid), "target-branch-invalid");
    assert_eq!(
        code(
            &create(
                &server,
                board_id,
                json!({ "title": "Falsch", "targetBranch": "a b" })
            )
            .await
        ),
        "target-branch-invalid"
    );
    assert_eq!(
        code(
            &create(
                &server,
                board_id,
                json!({ "title": "Zahl", "targetBranch": 5 })
            )
            .await
        ),
        "input-invalid"
    );
}

#[tokio::test]
async fn andert_felder_mit_version_und_meldet_einen_konflikt_mit_dem_aktuellen_stand() {
    let (server, _backend, board_id) = start().await;
    create(&server, board_id, json!({ "title": "Login" })).await;
    let saved = server
        .patch(
            "/api/db/tickets/ARC-0001",
            &json!({ "rev": 1, "title": "Login bauen", "plan": "# Plan", "assignee": "claude", "labels": ["a", "b"] }).to_string(),
        )
        .await;
    assert_eq!(saved.status, 200);
    let json = body(&saved);
    assert_eq!(json["title"], "Login bauen");
    assert_eq!(json["plan"], "# Plan");
    assert_eq!(json["assignee"], "claude");
    assert_eq!(json["labels"], json!(["a", "b"]));
    assert_eq!(json["rev"], 2);
    assert_eq!(json["comments"], json!([]));

    let stale = server
        .patch(
            "/api/db/tickets/ARC-0001",
            &json!({ "rev": 1, "title": "Alt" }).to_string(),
        )
        .await;
    assert_eq!(stale.status, 409);
    let json = body(&stale);
    assert_eq!(json["code"], "conflict");
    assert_eq!(json["current"]["id"], "ARC-0001");
    assert_eq!(json["current"]["title"], "Login bauen");
    assert_eq!(json["current"]["rev"], 2);

    for (payload, expected) in [
        (json!({ "title": "ohne Version" }), "rev-missing"),
        (json!({ "rev": 2 }), "nothing-changed"),
        (json!({ "rev": 2, "status": "quatsch" }), "status-invalid"),
    ] {
        let reply = server
            .patch("/api/db/tickets/ARC-0001", &payload.to_string())
            .await;
        assert_eq!(code(&reply), expected, "{payload}");
    }
    let cleared = server
        .patch(
            "/api/db/tickets/ARC-0001",
            &json!({ "rev": 2, "assignee": Value::Null }).to_string(),
        )
        .await;
    assert_eq!(body(&cleared)["assignee"], Value::Null);
}

#[tokio::test]
async fn verschiebt_als_nutzer_und_pruft_abnahme_und_version() {
    let (server, _backend, board_id) = start().await;
    create(&server, board_id, json!({ "title": "A" })).await;
    let done = server
        .post(
            "/api/db/tickets/ARC-0001/move",
            &json!({ "status": "done" }).to_string(),
        )
        .await;
    assert_eq!(done.status, 422);
    assert_eq!(code(&done), "acceptance-missing");

    let accepted = server
        .post(
            "/api/db/tickets/ARC-0001/move",
            &json!({ "status": "done", "acceptance": "geht" }).to_string(),
        )
        .await;
    let json = body(&accepted);
    assert_eq!(json["status"], "done");
    assert_eq!(json["acceptance"], "geht");
    assert_eq!(json["rev"], 2);

    // „Nicht mehr nötig“ ist nur als Aktion eines Menschen erlaubt (byUser)
    let moved = server
        .post(
            "/api/db/tickets/ARC-0001/move",
            &json!({ "status": "not_needed", "rev": 2 }).to_string(),
        )
        .await;
    let json = body(&moved);
    assert_eq!(json["status"], "not_needed");
    assert_eq!(json["column"], "not_needed");
    assert_eq!(json["rev"], 3);

    let stale = server
        .post(
            "/api/db/tickets/ARC-0001/move",
            &json!({ "status": "backlog", "rev": 1 }).to_string(),
        )
        .await;
    assert_eq!(stale.status, 409);
    assert_eq!(body(&stale)["current"]["rev"], 3);
    assert_eq!(
        code(&server.post("/api/db/tickets/ARC-0001/move", "{}").await),
        "status-missing"
    );
}

#[tokio::test]
async fn hangt_kommentare_an_und_loscht_tickets() {
    let (server, _backend, board_id) = start().await;
    create(&server, board_id, json!({ "title": "A" })).await;
    let commented = server
        .post(
            "/api/db/tickets/ARC-0001/comments",
            &json!({ "body": "Hallo" }).to_string(),
        )
        .await;
    assert_eq!(commented.status, 201);
    let comments = body(&commented)["comments"].as_array().cloned().unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "Hallo");
    assert_eq!(comments[0]["authorName"], common::MEMORY_USER_NAME);
    assert_eq!(
        code(
            &server
                .post(
                    "/api/db/tickets/ARC-0001/comments",
                    &json!({ "body": " " }).to_string()
                )
                .await
        ),
        "comment-empty"
    );

    let deleted = server.delete("/api/db/tickets/ARC-0001").await;
    assert_eq!(deleted.status, 204);
    assert!(deleted.text.is_empty(), "204 ohne Körper: {}", deleted.text);
    assert_eq!(deleted.header("content-type"), None);
    assert_eq!(server.get("/api/db/tickets/ARC-0001").await.status, 404);
}

#[tokio::test]
async fn erganzt_voraussetzungen_und_lehnt_kreise_ab() {
    let (server, _backend, board_id) = start().await;
    create(&server, board_id, json!({ "title": "A" })).await;
    create(
        &server,
        board_id,
        json!({ "title": "B", "dependsOn": ["ARC-0001"] }),
    )
    .await;
    let cycle = server
        .patch(
            "/api/db/tickets/ARC-0001",
            &json!({ "rev": 1, "dependsOn": ["ARC-0002"] }).to_string(),
        )
        .await;
    assert_eq!(cycle.status, 422);
    assert_eq!(code(&cycle), "dependency-cycle");
}

/// Server, Board „Sprint“ und ein zweites Board „Kollege“ mit zwei Tickets
async fn with_source() -> (TestServer, Arc<BoardBackendWithServices>, i32, i32) {
    let (server, backend, board_id) = start().await;
    let source = backend
        .services
        .boards
        .create("ARC", "Kollege", &backend.actor())
        .await
        .expect("zweites Board")
        .board
        .id;
    create(&server, source, json!({ "title": "A" })).await;
    create(
        &server,
        source,
        json!({ "title": "B", "status": "planned" }),
    )
    .await;
    (server, backend, board_id, source)
}

#[tokio::test]
async fn holt_tickets_in_einer_transaktion_und_andert_den_kennwert_beider_boards() {
    let (server, backend, board_id, source) = with_source().await;
    let marker = |id: i32| {
        let db = backend.services.db.clone();
        async move { db.change_marker(id).await.expect("Kennwert") }
    };
    let before = [marker(source).await, marker(board_id).await];
    let reply = server
        .post(
            &format!("/api/db/boards/{board_id}/import"),
            &json!({ "ticketIds": ["ARC-0001", "arc-2"] }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200);
    let moved = body(&reply)["moved"].as_array().cloned().expect("moved");
    assert_eq!(moved.len(), 2);
    assert_eq!(moved[0]["id"], "ARC-0001");
    assert_eq!(moved[0]["boardId"], board_id);
    assert_eq!(moved[0]["rev"], 2);
    assert_eq!(moved[0]["column"], "backlog");
    assert_eq!(moved[1]["id"], "ARC-0002");
    assert_eq!(moved[1]["boardId"], board_id);
    assert_eq!(moved[1]["rev"], 2);
    assert_eq!(moved[1]["column"], "planned");
    assert_ne!(marker(source).await, before[0]);
    assert_ne!(marker(board_id).await, before[1]);

    let detail = body(&server.get("/api/db/tickets/ARC-0001").await);
    let last = detail["history"]
        .as_array()
        .expect("Verlauf")
        .last()
        .cloned()
        .expect("Eintrag");
    assert_eq!(last["field"], "boardId");
    assert_eq!(last["note"], "von Board Kollege geholt");
    assert_eq!(last["byName"], common::MEMORY_USER_NAME);
}

#[tokio::test]
async fn holt_nichts_wenn_ein_ticket_fehlt_schon_da_ist_oder_fremd_ist() {
    let (server, backend, board_id, source) = with_source().await;
    create(&server, board_id, json!({ "title": "Hier" })).await;
    backend
        .services
        .projects
        .create("INC", "Incidents", &backend.actor())
        .await
        .expect("zweites Projekt");
    let foreign = backend
        .services
        .boards
        .create("INC", "Fremd", &backend.actor())
        .await
        .expect("fremdes Board")
        .board
        .id;
    create(&server, foreign, json!({ "title": "Fremd" })).await;

    for (ids, status, expected) in [
        (json!(["ARC-0001", "ARC-0099"]), 404, "ticket-not-found"),
        (
            json!(["ARC-0001", "ARC-0003"]),
            422,
            "ticket-already-on-board",
        ),
        (json!(["ARC-0001", "INC-0001"]), 422, "board-other-project"),
    ] {
        let reply = server
            .post(
                &format!("/api/db/boards/{board_id}/import"),
                &json!({ "ticketIds": ids }).to_string(),
            )
            .await;
        assert_eq!(reply.status, status, "{ids}");
        assert_eq!(code(&reply), expected, "{ids}");
    }
    // Nichts ist gewandert
    let detail = body(&server.get("/api/db/tickets/ARC-0001").await);
    assert_eq!(detail["boardId"], source);
    assert_eq!(detail["rev"], 1);
    assert_eq!(
        server
            .post(
                "/api/db/boards/999/import",
                &json!({ "ticketIds": ["ARC-0001"] }).to_string()
            )
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn pruft_den_korper_des_imports_nicht_leere_liste_von_texten_hochstens_200() {
    let (server, _backend, board_id, source) = with_source().await;
    let too_many: Vec<String> = (1..=201).map(|number| format!("ARC-{number}")).collect();
    for payload in [
        json!({}),
        json!({ "ticketIds": [] }),
        json!({ "ticketIds": "ARC-0001" }),
        json!({ "ticketIds": ["ARC-0001", 7] }),
        json!({ "ticketIds": too_many }),
    ] {
        let reply = server
            .post(
                &format!("/api/db/boards/{board_id}/import"),
                &payload.to_string(),
            )
            .await;
        let kurz = payload.to_string();
        assert_eq!(reply.status, 422, "{}", &kurz[..kurz.len().min(40)]);
        assert_eq!(code(&reply), "input-invalid");
    }
    assert_eq!(
        body(&server.get("/api/db/tickets/ARC-0001").await)["boardId"],
        source
    );
}

#[tokio::test]
async fn hakt_einen_schritt_ab_und_zuruck() {
    let (server, backend, board_id) = start().await;
    backend
        .services
        .tickets
        .create(
            board_id,
            &NewTicketInput {
                title: "Mit Schritten".into(),
                steps: Some(vec![step("A"), step("B")]),
                ..NewTicketInput::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");

    let done = server
        .post(
            "/api/db/tickets/ARC-0001/steps/2",
            &json!({ "done": true }).to_string(),
        )
        .await;
    assert_eq!(done.status, 200);
    let json = body(&done);
    assert_eq!(json["stepsDone"], 1);
    assert_eq!(json["stepsTotal"], 2);
    assert_eq!(json["steps"][0]["done"], false);
    assert_eq!(json["steps"][1]["text"], "B");
    assert_eq!(json["steps"][1]["done"], true);

    let back = server
        .post(
            "/api/db/tickets/ARC-0001/steps/2",
            &json!({ "done": false }).to_string(),
        )
        .await;
    assert_eq!(body(&back)["stepsDone"], 0);
    assert_eq!(
        server
            .post(
                "/api/db/tickets/ARC-0001/steps/3",
                &json!({ "done": true }).to_string()
            )
            .await
            .status,
        404
    );
    assert_eq!(
        code(
            &server
                .post(
                    "/api/db/tickets/ARC-0001/steps/0",
                    &json!({ "done": true }).to_string()
                )
                .await
        ),
        "input-invalid"
    );
    assert_eq!(
        code(&server.post("/api/db/tickets/ARC-0001/steps/1", "{}").await),
        "input-invalid"
    );
}

#[tokio::test]
async fn lehnt_falsche_typen_beim_abhaken_mit_422_ab() {
    let (server, backend, board_id) = start().await;
    backend
        .services
        .tickets
        .create(
            board_id,
            &NewTicketInput {
                title: "Mit Schritten".into(),
                steps: Some(vec![step("A")]),
                ..NewTicketInput::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");
    for (path, payload) in [
        ("steps/1", json!({ "done": "ja" })),
        ("steps/1", json!({ "done": 1 })),
        ("steps/1", json!({ "done": Value::Null })),
        ("steps/x", json!({ "done": true })),
        ("steps/1.5", json!({ "done": true })),
    ] {
        let reply = server
            .post(
                &format!("/api/db/tickets/ARC-0001/{path}"),
                &payload.to_string(),
            )
            .await;
        assert_eq!(reply.status, 422, "{path} {payload}");
        assert_eq!(code(&reply), "input-invalid", "{path} {payload}");
    }
}

#[tokio::test]
async fn legt_schritte_an_bearbeitet_verschiebt_und_loscht_sie() {
    let (server, backend, board_id) = start().await;
    backend
        .services
        .tickets
        .create(
            board_id,
            &NewTicketInput::titled("Ohne Schritte"),
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");

    let added = server
        .post(
            "/api/db/tickets/ARC-0001/steps",
            &json!({ "text": "Test schreiben", "detail": "Erwartet: rot" }).to_string(),
        )
        .await;
    assert_eq!(added.status, 201);
    let json = body(&added);
    assert_eq!(json["id"], "ARC-0001");
    assert_eq!(json["rev"], 2);
    assert_eq!(json["stepsTotal"], 1);
    assert_eq!(
        json["steps"],
        json!([{ "text": "Test schreiben", "done": false, "detail": "Erwartet: rot" }])
    );
    assert!(json["history"].is_array());

    server
        .post(
            "/api/db/tickets/ARC-0001/steps",
            &json!({ "text": "Lesen", "at": 1 }).to_string(),
        )
        .await;
    let edited = server
        .patch(
            "/api/db/tickets/ARC-0001/steps/2",
            &json!({ "text": "Test zuerst", "detail": Value::Null }).to_string(),
        )
        .await;
    assert_eq!(edited.status, 200);
    assert_eq!(
        body(&edited)["steps"],
        json!([
            { "text": "Lesen", "done": false },
            { "text": "Test zuerst", "done": false },
        ])
    );

    let moved = server
        .post(
            "/api/db/tickets/ARC-0001/steps/2/move",
            &json!({ "to": 1 }).to_string(),
        )
        .await;
    let texts: Vec<String> = body(&moved)["steps"]
        .as_array()
        .expect("Schritte")
        .iter()
        .map(|step| step["text"].as_str().expect("Text").to_string())
        .collect();
    assert_eq!(texts, ["Test zuerst", "Lesen"]);

    let removed = server.delete("/api/db/tickets/ARC-0001/steps/1").await;
    assert_eq!(removed.status, 200);
    assert_eq!(body(&removed)["stepsTotal"], 1);
    assert_eq!(body(&removed)["steps"][0]["text"], "Lesen");
}

#[tokio::test]
async fn meldet_fehler_beim_bearbeiten_von_schritten() {
    let (server, backend, board_id) = start().await;
    backend
        .services
        .tickets
        .create(
            board_id,
            &NewTicketInput {
                title: "A".into(),
                steps: Some(vec![step("A")]),
                ..NewTicketInput::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");

    assert_eq!(
        code(
            &server
                .post(
                    "/api/db/tickets/ARC-0001/steps",
                    &json!({ "text": " " }).to_string()
                )
                .await
        ),
        "step-empty"
    );
    assert_eq!(
        server
            .post("/api/db/tickets/ARC-0001/steps", "{}")
            .await
            .status,
        422
    );
    assert_eq!(
        server
            .post(
                "/api/db/tickets/ARC-0001/steps",
                &json!({ "text": "X", "at": 5 }).to_string()
            )
            .await
            .status,
        404
    );
    assert_eq!(
        server
            .post(
                "/api/db/tickets/ARC-0099/steps",
                &json!({ "text": "X" }).to_string()
            )
            .await
            .status,
        404
    );
    assert_eq!(
        server
            .patch(
                "/api/db/tickets/ARC-0001/steps/2",
                &json!({ "text": "X" }).to_string()
            )
            .await
            .status,
        404
    );
    assert_eq!(
        code(&server.patch("/api/db/tickets/ARC-0001/steps/1", "{}").await),
        "nothing-changed"
    );
    assert_eq!(
        code(
            &server
                .patch(
                    "/api/db/tickets/ARC-0001/steps/x",
                    &json!({ "text": "X" }).to_string()
                )
                .await
        ),
        "input-invalid"
    );
    assert_eq!(
        server
            .delete("/api/db/tickets/ARC-0001/steps/2")
            .await
            .status,
        404
    );
    assert_eq!(
        code(
            &server
                .post("/api/db/tickets/ARC-0001/steps/1/move", "{}")
                .await
        ),
        "input-invalid"
    );
    assert_eq!(
        server
            .post(
                "/api/db/tickets/ARC-0001/steps/1/move",
                &json!({ "to": 2 }).to_string()
            )
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn meldet_geanderte_schritte_und_veraltete_versionen_mit_409_und_aktuellem_stand() {
    let (server, backend, board_id) = start().await;
    backend
        .services
        .tickets
        .create(
            board_id,
            &NewTicketInput {
                title: "A".into(),
                steps: Some(vec![step("A"), step("B")]),
                ..NewTicketInput::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");
    // Reihenfolge umdrehen: Schritt 2 nach vorn
    server
        .post(
            "/api/db/tickets/ARC-0001/steps/2/move",
            &json!({ "to": 1 }).to_string(),
        )
        .await;

    let checks = [
        server
            .post(
                "/api/db/tickets/ARC-0001/steps/1",
                &json!({ "done": true, "expectText": "A" }).to_string(),
            )
            .await,
        server
            .post(
                "/api/db/tickets/ARC-0001/steps",
                &json!({ "text": "X", "at": 1, "expectText": "A" }).to_string(),
            )
            .await,
        server
            .patch(
                "/api/db/tickets/ARC-0001/steps/1",
                &json!({ "text": "X", "expectText": "A" }).to_string(),
            )
            .await,
        server
            .request(
                "DELETE",
                "/api/db/tickets/ARC-0001/steps/1",
                Some(&json!({ "expectText": "A" }).to_string()),
                &[],
            )
            .await,
        server
            .post(
                "/api/db/tickets/ARC-0001/steps/1/move",
                &json!({ "to": 2, "expectText": "A" }).to_string(),
            )
            .await,
        server
            .patch(
                "/api/db/tickets/ARC-0001/steps/1",
                &json!({ "text": "X", "rev": 1 }).to_string(),
            )
            .await,
    ];
    for reply in &checks {
        assert_eq!(reply.status, 409, "{}", reply.text);
        let json = body(reply);
        assert_eq!(json["code"], "conflict");
        assert_eq!(json["current"]["id"], "ARC-0001");
        assert_eq!(json["current"]["rev"], 2);
        assert_eq!(json["current"]["steps"][0]["text"], "B");
        assert_eq!(json["current"]["steps"][1]["text"], "A");
    }
    assert_eq!(
        body(&checks[0])["message"],
        "ARC-0001: Schritt 1 ist inzwischen „B“ – die Schritte wurden geändert."
    );

    let ok = server
        .request(
            "DELETE",
            "/api/db/tickets/ARC-0001/steps/1",
            Some(&json!({ "expectText": "B", "rev": 2 }).to_string()),
            &[],
        )
        .await;
    assert_eq!(ok.status, 200);
    assert_eq!(body(&ok)["steps"], json!([{ "text": "A", "done": false }]));

    let added = server
        .post(
            "/api/db/tickets/ARC-0001/steps",
            &json!({ "text": "C", "rev": 3 }).to_string(),
        )
        .await;
    assert_eq!(added.status, 201);
}

/// Die Zeilen dieser Gruppe aus der Sicherheitsmatrix (`packages/server/tests/security/
/// Matrix.test.ts`): der Schutz greift vor jeder Route
#[tokio::test]
async fn lehnt_die_schreibrouten_der_gruppe_ohne_pflichtkopf_ab() {
    let (server, backend, board_id) = start().await;
    create(&server, board_id, json!({ "title": "Login" })).await;
    let routes = [
        (
            "PATCH",
            "/api/db/tickets/ARC-0001".to_string(),
            json!({ "rev": 1, "title": "Überschrieben" }),
        ),
        (
            "POST",
            "/api/db/tickets/ARC-0001/steps/1".to_string(),
            json!({ "done": true }),
        ),
        (
            "POST",
            format!("/api/db/boards/{board_id}/import"),
            json!({ "ticketIds": ["ARC-0001"] }),
        ),
    ];
    for (method, url, payload) in &routes {
        // Ohne den Pflichtkopf `x-harness`
        let reply = server
            .request(
                method,
                url,
                Some(&payload.to_string()),
                &[("x-harness", None)],
            )
            .await;
        assert_eq!(reply.status, 403, "{method} {url}");
        assert_eq!(
            reply.code().as_deref(),
            Some("header-missing"),
            "{method} {url}"
        );
    }
    // Keine Anfrage hat gewirkt
    let ticket = backend
        .services
        .tickets
        .get("ARC-0001")
        .await
        .expect("Ticket");
    assert_eq!(ticket.title, "Login");
    assert_eq!(ticket.rev, 1);
    assert_eq!(ticket.status, TicketStatus::Backlog);
}

/// Wie Fastify: ein leerer JSON-Körper ist 400 `bad-request`, auch wenn die Route den Körper
/// gar nicht liest (`DELETE /api/db/tickets/{id}`)
#[tokio::test]
async fn lehnt_einen_leeren_json_korper_mit_bad_request_ab() {
    let (server, _backend, board_id) = start().await;
    create(&server, board_id, json!({ "title": "Login" })).await;
    let reply = server
        .request("DELETE", "/api/db/tickets/ARC-0001", None, &[])
        .await;
    assert_eq!(reply.status, 400);
    assert_eq!(code(&reply), "bad-request");
}
