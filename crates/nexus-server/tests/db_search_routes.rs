//! `GET /api/db/search` der Gruppe `db-misc`; portiert aus
//! `packages/server/tests/unit/DbSearchRoutes.test.ts`.

mod common;

use common::{Reply, TestServer};
use nexus_core::board::note_service::{AddNoteOptions, NewNoteText};
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

/// Server mit zwei Projekten, zwei Boards und drei durchsuchbaren Gegenständen
async fn start() -> TestServer {
    let (server, backend) = TestServer::start_with_board().await;
    let s = &backend.services;
    let actor = backend.actor();
    for (key, name) in [("ARC", "Arches"), ("SV1", "Service")] {
        s.projects
            .create(key, name, &actor)
            .await
            .expect("Projekt anlegen");
    }
    let a = s
        .boards
        .create("ARC", "Sprint", &actor)
        .await
        .expect("Board anlegen")
        .board
        .id;
    let b = s
        .boards
        .create("SV1", "Incidents", &actor)
        .await
        .expect("Board anlegen")
        .board
        .id;
    for (board, title) in [(a, "Export bauen"), (b, "Export hängt")] {
        s.tickets
            .create(
                board,
                &NewTicketInput::titled(title),
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket anlegen");
    }
    s.notes
        .add(
            a,
            &NewNoteText::titled("Export später prüfen"),
            &actor,
            &AddNoteOptions::default(),
        )
        .await
        .expect("Notiz anlegen");
    server
}

#[tokio::test]
async fn findet_treffer_uber_projekte_und_boards_mit_fundstelle() {
    let server = start().await;
    let reply = server.get("/api/db/search?q=export").await;
    assert_eq!(reply.status, 200);
    let json = body(&reply);
    assert_eq!(json["query"], "export");
    assert_eq!(json["terms"], json!(["export"]));
    assert_eq!(json["truncated"], false);
    let hits = json["hits"].as_array().expect("Treffer");
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0]["id"], "SV1-0001");
    assert_eq!(hits[1]["id"], "ARC-0001");
    assert!(hits[2]["id"].is_string());
    assert_eq!(hits[0]["kind"], "ticket");
    assert_eq!(hits[0]["project"]["key"], "SV1");
    assert_eq!(hits[0]["board"]["name"], "Incidents");
    assert_eq!(hits[0]["snippet"], "Export hängt");
    assert_eq!(hits[0]["matches"], json!([[0, 6]]));
}

#[tokio::test]
async fn filtert_nach_art_und_begrenzt_die_zahl() {
    let server = start().await;
    let notizen = body(&server.get("/api/db/search?q=export&kinds=note").await);
    let arten: Vec<&str> = notizen["hits"]
        .as_array()
        .expect("Treffer")
        .iter()
        .map(|hit| hit["kind"].as_str().expect("Art"))
        .collect();
    assert_eq!(arten, ["note"]);

    let limited = body(&server.get("/api/db/search?q=export&limit=1").await);
    assert_eq!(limited["hits"].as_array().expect("Treffer").len(), 1);
    assert_eq!(limited["truncated"], true);

    assert_eq!(
        code(&server.get("/api/db/search?q=export&kinds=quatsch").await),
        "input-invalid"
    );
    assert_eq!(
        code(&server.get("/api/db/search?q=export&limit=abc").await),
        "input-invalid"
    );
    assert_eq!(
        code(&server.get("/api/db/search?q=export&limit=0").await),
        "input-invalid"
    );
    // Ein leerer Wert zählt wie „nicht angegeben“
    assert_eq!(
        server.get("/api/db/search?q=export&limit=").await.status,
        200
    );
}

#[tokio::test]
async fn lehnt_zu_kurze_und_fehlende_eingaben_mit_422_ab() {
    let server = start().await;
    let short = server.get("/api/db/search?q=e").await;
    assert_eq!(short.status, 422);
    assert_eq!(code(&short), "query-short");
    assert_eq!(code(&server.get("/api/db/search").await), "query-short");
    // Ein wiederholter Parameter kommt wie in Fastify als Liste an und gilt als kein Text
    assert_eq!(
        code(&server.get("/api/db/search?q=export&q=mehr").await),
        "query-short"
    );
}

#[tokio::test]
async fn antwortet_ohne_board_datenbank_mit_503_und_hinweis() {
    let server = TestServer::with(common::TestServerOptions {
        board_from_config: true,
        ..common::TestServerOptions::default()
    });
    let reply = server.get("/api/db/search?q=export").await;
    assert_eq!(reply.status, 503);
    assert_eq!(code(&reply), "board-config-missing");
}

/// Der Notschalter: steht `db-misc` in `NEXUS_PROXY_GROUPS`, meldet Rust die Routen der
/// Gruppe nicht an und der Rückfall übernimmt
#[tokio::test]
async fn lasst_die_gruppe_mit_dem_notschalter_zum_node_server_gehen() {
    let (server, _backend) = TestServer::start_with_board_and(common::TestServerOptions {
        proxy_groups: nexus_server::groups::RouteGroups::from_env(Some(
            nexus_server::groups::RouteGroup::DbMisc.as_str(),
        )),
        ..common::TestServerOptions::default()
    })
    .await;
    // Ohne Node-Server beantwortet der Rückfall die Anfrage – entscheidend ist, dass keine
    // Rust-Route mehr greift
    for url in [
        "/api/db/search?q=export",
        "/api/db/inbox",
        "/api/db/graph?ticket=ARC-0001",
        "/api/db/stats/dispatch?project=ARC",
        "/api/db/boards/1/notes",
        "/api/db/boards/1/spec-progress",
    ] {
        let reply = server.get(url).await;
        assert_eq!(reply.status, 404, "{url}");
        assert_eq!(reply.code().as_deref(), Some("not-found"), "{url}");
    }
}
