//! `GET /api/db/stats/dispatch` der Gruppe `db-misc`; portiert aus
//! `packages/server/tests/unit/DbStatsRoutes.test.ts`.
//!
//! Die Uhr des Servers steht fest, damit der Zeitraum nachrechenbar ist; die Rechnung selbst
//! prüft `nexus_server::routes::db_misc::stats_range` mit festen Kalendertagen.

mod common;

use std::sync::Arc;

use chrono::{DateTime, Days, Local, Utc};
use common::{BoardBackendWithServices, Reply, TestServer, TestServerOptions};
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::dispatch_run_types::DispatchConfigVersion;
use nexus_core::board::ticket_service::{MoveOptions, NewTicketInput, WriteOptions};
use nexus_core::clock::{iso, Clock};
use nexus_core::testing::run_fixture;
use serde_json::{json, Value};

/// Der feste Augenblick, zu dem der Server in diesen Tests läuft
const NOW: &str = "2026-03-10T12:00:00.000Z";

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

/// Der feste Augenblick als Zeitpunkt
fn now() -> DateTime<Utc> {
    NOW.parse().expect("fester Augenblick")
}

/// Ein Zeitpunkt als Ortszeit-Tag `JJJJ-MM-TT`, wie die Antwort ihn schreibt
fn day_of(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local).format("%Y-%m-%d").to_string()
}

/// Server mit fester Uhr, Projekt ARC und Board „Sprint“
async fn start() -> (TestServer, Arc<BoardBackendWithServices>, i32, i32) {
    let (server, backend) = TestServer::start_with_board_and(TestServerOptions {
        clock: Some(Clock::fixed(now())),
        ..TestServerOptions::default()
    })
    .await;
    let actor = backend.actor();
    let project = backend
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
    (server, backend, project.id, board_id)
}

#[tokio::test]
async fn liefert_die_kennzahlen_eines_projekts_mit_filter_zeitraum_und_laufen() {
    let (server, backend, project_id, board_id) = start().await;
    let actor = backend.actor();
    backend
        .services
        .tickets
        .create(
            board_id,
            &NewTicketInput {
                status: Some(TicketStatus::Planned),
                ..NewTicketInput::titled("A")
            },
            &actor,
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");
    backend
        .services
        .tickets
        .move_to(
            "ARC-0001",
            TicketStatus::Done,
            &actor,
            &MoveOptions {
                acceptance: Some("ok".into()),
                ..MoveOptions::default()
            },
        )
        .await
        .expect("Ticket abschließen");
    let day = day_of(now());
    let lauf = nexus_core::board::dispatch_run_types::DispatchRun {
        board_id: Some(board_id),
        started_at: NOW.into(),
        cost_usd: Some(0.5),
        ..run_fixture("r1", "ARC-0001", project_id)
    };
    backend
        .services
        .dispatch_runs
        .record(&lauf)
        .await
        .expect("Lauf aufzeichnen");

    let reply = server
        .get(&format!(
            "/api/db/stats/dispatch?project=arc&board={board_id}&from={day}&to={day}"
        ))
        .await;
    assert_eq!(reply.status, 200);
    let json = body(&reply);
    assert_eq!(
        json["filter"],
        json!({ "project": "ARC", "boardId": board_id, "from": day, "to": day, "kind": "work" })
    );
    assert_eq!(json["kpis"]["runs"], 1);
    assert_eq!(json["kpis"]["done"], 1);
    assert_eq!(json["kpis"]["successRate"], 1.0);
    let profiles = json["byProfile"].as_array().expect("Profile");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0]["profile"], "standard");
    assert_eq!(profiles[0]["runs"], 1);
    assert_eq!(profiles[0]["done"], 1);

    // Standard-Zeitraum: 8 Wochen bis heute; andere Lauf-Art
    let plain = body(
        &server
            .get("/api/db/stats/dispatch?project=ARC&kind=merge")
            .await,
    );
    assert_eq!(plain["filter"]["to"], day);
    assert_eq!(
        plain["filter"]["from"],
        day_of(now() - Days::new(55)),
        "Standard-Zeitraum sind 56 Tage inkl. heute"
    );
    assert_eq!(plain["filter"]["kind"], "merge");
    assert_eq!(plain["kpis"]["runs"], 0);

    // Seit Konfig-Änderung: ohne Fassung der Standard-Zeitraum
    let ohne_fassung = body(
        &server
            .get("/api/db/stats/dispatch?project=ARC&from=config")
            .await,
    );
    assert_eq!(ohne_fassung["filter"]["from"], plain["filter"]["from"]);

    // Mit Fassung ab deren Tag
    let changed = now() - Days::new(3);
    backend
        .services
        .dispatch_runs
        .record_config_version(&DispatchConfigVersion {
            hash: "h".into(),
            dispatcher: "d".into(),
            first_seen_at: iso(changed),
            content: "{}".into(),
        })
        .await
        .expect("Fassung aufzeichnen");
    let since = body(
        &server
            .get("/api/db/stats/dispatch?project=ARC&from=config")
            .await,
    );
    assert_eq!(since["filter"]["from"], day_of(changed));
    assert_eq!(since["configChangedAt"], iso(changed));
}

#[tokio::test]
async fn prufft_die_parameter() {
    let (server, backend, _project_id, _board_id) = start().await;
    let actor = backend.actor();
    backend
        .services
        .projects
        .create("OTH", "Andere", &actor)
        .await
        .expect("Projekt anlegen");
    let foreign = backend
        .services
        .boards
        .create("OTH", "Fremd", &actor)
        .await
        .expect("Board anlegen")
        .board
        .id;

    assert_eq!(
        code(&server.get("/api/db/stats/dispatch").await),
        "input-invalid"
    );
    assert_eq!(
        server
            .get("/api/db/stats/dispatch?project=XYZ")
            .await
            .status,
        404
    );
    assert_eq!(
        code(
            &server
                .get(&format!(
                    "/api/db/stats/dispatch?project=ARC&board={foreign}"
                ))
                .await
        ),
        "input-invalid"
    );
    assert_eq!(
        server
            .get("/api/db/stats/dispatch?project=ARC&board=999")
            .await
            .status,
        404
    );
    assert_eq!(
        code(
            &server
                .get("/api/db/stats/dispatch?project=ARC&from=2026-02-01&to=2026-01-01")
                .await
        ),
        "input-invalid"
    );
    assert_eq!(
        code(
            &server
                .get("/api/db/stats/dispatch?project=ARC&from=2026-02-30")
                .await
        ),
        "input-invalid"
    );
    let zu_lang = body(
        &server
            .get("/api/db/stats/dispatch?project=ARC&from=2025-01-01&to=2026-01-02")
            .await,
    );
    assert!(
        zu_lang["message"]
            .as_str()
            .expect("Meldung")
            .contains("zu lang"),
        "{zu_lang}"
    );
    assert_eq!(
        server
            .get("/api/db/stats/dispatch?project=ARC&from=2025-01-02&to=2026-01-02")
            .await
            .status,
        200
    );
    assert_eq!(
        code(
            &server
                .get("/api/db/stats/dispatch?project=ARC&kind=plan")
                .await
        ),
        "input-invalid"
    );
}

#[tokio::test]
async fn antwortet_ohne_board_datenbank_mit_503_wie_die_anderen_db_routen() {
    let server = TestServer::with(TestServerOptions {
        board_from_config: true,
        ..TestServerOptions::default()
    });
    assert_eq!(
        server
            .get("/api/db/stats/dispatch?project=ARC")
            .await
            .status,
        503
    );
}
