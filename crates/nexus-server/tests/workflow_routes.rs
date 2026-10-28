//! `GET /api/workflow` und `GET /api/customizations`; portiert aus
//! `packages/server/tests/unit/WorkflowRoutes.test.ts` und `CustomizationRoutes.test.ts`.

mod common;

use std::fs;

use common::TestServer;

#[tokio::test]
async fn workflow_liefert_ohne_anpassung_den_standard() {
    let server = TestServer::start();
    let reply = server.get("/api/workflow").await;
    assert_eq!(reply.status, 200);
    let json = reply.json().unwrap();
    assert_eq!(json["source"], "standard");
    let ids: Vec<&str> = json["phases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|phase| phase["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["planen", "umsetzen"]);
}

#[tokio::test]
async fn workflow_liefert_den_angewendeten_workflow() {
    let server = TestServer::start();
    let phases = serde_json::json!([
        { "id": "planen", "title": "P", "hint": "h", "steps": [] },
        { "id": "umsetzen", "title": "U", "hint": "h", "steps": [] },
    ]);
    fs::create_dir_all(server.home.customizations_dir()).unwrap();
    fs::write(
        server.home.customizations_dir().join("workflow.json"),
        serde_json::to_string(&phases).unwrap(),
    )
    .unwrap();
    let reply = server.get("/api/workflow").await;
    assert_eq!(
        reply.json().unwrap(),
        serde_json::json!({ "source": "custom", "phases": phases })
    );
}

#[tokio::test]
async fn customizations_meldet_ohne_uebersicht_den_stand_unbekannt() {
    let server = TestServer::start();
    let reply = server.get("/api/customizations").await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.json().unwrap(),
        serde_json::json!({ "state": "unknown" })
    );
}

#[tokio::test]
async fn customizations_liefert_die_uebersicht_ohne_diffs() {
    let server = TestServer::start();
    let entry = serde_json::json!({
        "point": "skill",
        "name": "harness-grill",
        "kind": "replace",
        "state": "active",
        "base": "outdated",
        "problems": [],
        "warnings": [{ "code": "base-outdated", "message": "m" }],
    });
    let mut entry_mit_diff = entry.clone();
    entry_mit_diff["diff"] = serde_json::json!("+neu");
    let overview = serde_json::json!({
        "version": 1,
        "createdAt": "2026-10-14T10:00:00.000Z",
        "cloneDir": "/h",
        "customDir": "/h/custom",
        "harnessVersion": "0.2.0",
        "workflow": "standard",
        "entries": [entry_mit_diff],
    });
    fs::create_dir_all(server.home.customizations_dir()).unwrap();
    fs::write(
        server.home.customizations_dir().join("overview.json"),
        serde_json::to_string(&overview).unwrap(),
    )
    .unwrap();
    let reply = server.get("/api/customizations").await;
    let mut erwartet = overview;
    erwartet["entries"] = serde_json::json!([entry]);
    assert_eq!(reply.json().unwrap(), erwartet);
}

#[tokio::test]
async fn customizations_behaelt_unbekannte_felder_und_ihre_reihenfolge() {
    let server = TestServer::start();
    let roh =
        r#"{"version":1,"entries":[{"point":"skill","diff":"+x","custom":"y"}],"zukunft":"wert"}"#;
    fs::create_dir_all(server.home.customizations_dir()).unwrap();
    fs::write(server.home.customizations_dir().join("overview.json"), roh).unwrap();
    let reply = server.get("/api/customizations").await;
    assert_eq!(
        reply.text,
        r#"{"version":1,"entries":[{"point":"skill","custom":"y"}],"zukunft":"wert"}"#
    );
}
