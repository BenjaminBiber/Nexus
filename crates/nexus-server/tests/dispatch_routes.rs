//! Die Routen unter `/api/dispatch`; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchRoutes.test.ts`.
//!
//! Der Testserver steht auf `NEXUS_DISPATCH=rust`, denn nur dann beantwortet Rust diese
//! Routen. Gestartet wird nie ein echter Worker: die Attrappe des Testservers liefert keine
//! Prozessnummer.

mod common;

use std::fs;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::config::BoardConfig;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::database::BoardDatabase as _;
use nexus_core::board::question_service::NewBoardQuestionInput;
use nexus_core::board::question_types::BoardQuestionActor;
use nexus_core::board::ticket_service::{NewTicketInput, TicketPatch, TicketView, WriteOptions};
use nexus_core::board::types::BoardUser;
use nexus_core::dispatch::{DispatchStateStore, PromptKind};
use nexus_core::tx;
use nexus_server::config::DispatchBackend;
use serde_json::{json, Value};

use common::{
    memory_config, BoardBackendWithServices, Reply, TempRepo, TestServer, TestServerOptions,
    MEMORY_USER_ID, MEMORY_USER_NAME,
};

/// Ein anderer Benutzer, dessen Boards dem Dispatcher nicht gehören
const OTHER_USER_ID: &str = "22222222-2222-2222-2222-222222222222";

/// Knappe eigene work-Vorlage, die alle Pflichtteile erfüllt
const WORK: &str = concat!(
    "{{agent}} setzt {{ticketId}} („{{title}}“) um.\n",
    "Fertig: harness board done {{ticketId}} --by {{agent}}\n",
    "Blockiert: harness board set {{ticketId}} --status blocked\n",
    "Frage: harness board question add \"…\" --ticket {{ticketId}}",
);

/// Testserver mit Board im Speicher, auf dem Rust dispatcht
async fn server() -> (TestServer, std::sync::Arc<BoardBackendWithServices>) {
    TestServer::start_with_board_and(TestServerOptions {
        dispatch: DispatchBackend::Rust,
        ..TestServerOptions::default()
    })
    .await
}

/// Der Handelnde des Testbenutzers
fn actor(backend: &BoardBackendWithServices) -> nexus_core::board::basics::BoardActor {
    backend.actor()
}

/// Projekt und Board des Testbenutzers
async fn board(backend: &BoardBackendWithServices) -> i32 {
    let actor = actor(backend);
    backend
        .services
        .projects
        .create("ARC", "Arches", &actor)
        .await
        .expect("Projekt");
    backend
        .services
        .boards
        .create("ARC", "Sprint", &actor)
        .await
        .expect("Board")
        .board
        .id
}

/// Ein Ticket auf einem Board
async fn ticket(
    backend: &BoardBackendWithServices,
    board_id: i32,
    input: NewTicketInput,
    by: &nexus_core::board::basics::BoardActor,
) -> TicketView {
    backend
        .services
        .tickets
        .create(board_id, &input, by, &WriteOptions::default())
        .await
        .expect("Ticket anlegen")
}

/// Ändert ein Ticket mit Gewalt (Status, Branch …), wie `{ force: true }` in TypeScript
async fn force(
    backend: &BoardBackendWithServices,
    id: &str,
    patch: TicketPatch,
    by: &nexus_core::board::basics::BoardActor,
) {
    backend
        .services
        .tickets
        .update(
            id,
            &patch,
            by,
            &WriteOptions {
                force: true,
                ..WriteOptions::default()
            },
        )
        .await
        .expect("Ticket ändern");
}

/// Trägt ein anderes Board als aktives in `board.yaml` ein
fn set_active_board(server: &TestServer, board_id: i32) {
    BoardConfigStore::new(server.home.clone())
        .write(&BoardConfig {
            active_board: Some(board_id),
            ..memory_config()
        })
        .expect("board.yaml schreiben");
}

/// Ein zweiter Benutzer samt eigenem Board
async fn other_board(backend: &BoardBackendWithServices) -> nexus_core::board::basics::BoardActor {
    let other = BoardUser {
        id: OTHER_USER_ID.into(),
        name: "Kollege".into(),
    };
    tx!(backend.services.db, [other], |u| {
        u.users().upsert(&other).await
    })
    .await
    .expect("Kollegen anlegen");
    nexus_core::board::basics::BoardActor {
        user_id: OTHER_USER_ID.into(),
        name: "Kollege".into(),
    }
}

/// Die rohe Einstellung aus `GET /api/dispatch/config`
async fn raw_config(server: &TestServer) -> Value {
    server.get("/api/dispatch/config").await.json().unwrap()["raw"].clone()
}

/// Eine Einstellung mit zusätzlichen Feldern als Körper
fn with(raw: &Value, patch: Value) -> String {
    let mut merged = raw.clone();
    let object = merged.as_object_mut().expect("Einstellung");
    for (key, value) in patch.as_object().expect("Änderung") {
        object.insert(key.clone(), value.clone());
    }
    merged.to_string()
}

/// Der Körper einer Antwort als JSON
fn body(reply: &Reply) -> Value {
    reply
        .json()
        .unwrap_or_else(|| panic!("kein JSON: {}", reply.text))
}

#[tokio::test]
async fn liefert_ohne_datei_das_beispiel_und_speichert_geprueft() {
    let (server, _backend) = server().await;
    let empty = body(&server.get("/api/dispatch/config").await);
    assert_eq!(empty["exists"], false);
    assert_eq!(empty["error"], Value::Null);
    assert_eq!(empty["config"]["enabled"], false);
    assert_eq!(empty["config"]["optInLabel"], "auto");
    let profiles: Vec<&String> = empty["raw"]["profiles"]
        .as_object()
        .expect("Profile")
        .keys()
        .collect();
    assert_eq!(profiles, ["klein", "standard", "schwer"]);

    let raw = empty["raw"].clone();
    let saved = server
        .put("/api/dispatch/config", &with(&raw, json!({ "wip": 2 })))
        .await;
    assert_eq!(saved.status, 200);
    assert_eq!(body(&saved)["exists"], true);
    assert_eq!(body(&saved)["config"]["wip"], 2);

    let bad = server.put("/api/dispatch/config", r#"{"wip":-1}"#).await;
    assert_eq!(bad.status, 422);
    assert_eq!(bad.code().as_deref(), Some("dispatch-config-invalid"));
    assert_eq!(server.put("/api/dispatch/config", "[]").await.status, 422);
}

#[tokio::test]
async fn speichern_setzt_und_vergisst_den_zeitpunkt_der_recherche_automatik() {
    let (server, _backend) = server().await;
    let state = DispatchStateStore::new(server.home.clone());
    let raw = raw_config(&server).await;
    server
        .put("/api/dispatch/config", &with(&raw, json!({ "wip": 2 })))
        .await;
    assert_eq!(state.read().unwrap().research_auto.since, None);

    server
        .put(
            "/api/dispatch/config",
            &with(&raw, json!({ "research": { "auto": true } })),
        )
        .await;
    let since = state.read().unwrap().research_auto.since;
    assert!(since.is_some());

    server
        .put(
            "/api/dispatch/config",
            &with(&raw, json!({ "research": { "auto": true, "wip": 2 } })),
        )
        .await;
    assert_eq!(state.read().unwrap().research_auto.since, since);

    server
        .put(
            "/api/dispatch/config",
            &with(&raw, json!({ "research": { "auto": false } })),
        )
        .await;
    assert_eq!(state.read().unwrap().research_auto.since, None);

    let invalid = server
        .put(
            "/api/dispatch/config",
            &with(&raw, json!({ "research": { "wip": 6 } })),
        )
        .await;
    assert_eq!(invalid.code().as_deref(), Some("dispatch-config-invalid"));
}

#[tokio::test]
async fn speichern_setzt_und_vergisst_den_zeitpunkt_der_planungs_automatik() {
    let (server, _backend) = server().await;
    let state = DispatchStateStore::new(server.home.clone());
    let raw = raw_config(&server).await;
    let status = body(&server.get("/api/dispatch/status").await);
    assert_eq!(status["planningAuto"]["auto"], false);
    assert_eq!(status["planningAuto"]["wip"], 1);
    assert_eq!(status["planningAuto"]["active"], 0);
    assert_eq!(status["planningAuto"]["since"], Value::Null);

    server
        .put(
            "/api/dispatch/config",
            &with(&raw, json!({ "planning": { "auto": true, "wip": 2 } })),
        )
        .await;
    let since = state.read().unwrap().planning_auto.since;
    assert!(since.is_some());
    let status = body(&server.get("/api/dispatch/status").await);
    assert_eq!(status["planningAuto"]["auto"], true);
    assert_eq!(status["planningAuto"]["wip"], 2);
    assert_eq!(status["planningAuto"]["since"], json!(since));

    server
        .put(
            "/api/dispatch/config",
            &with(&raw, json!({ "planning": { "auto": true, "wip": 3 } })),
        )
        .await;
    assert_eq!(state.read().unwrap().planning_auto.since, since);

    server
        .put(
            "/api/dispatch/config",
            &with(&raw, json!({ "planning": { "auto": false } })),
        )
        .await;
    assert_eq!(state.read().unwrap().planning_auto.since, None);

    let invalid = server
        .put(
            "/api/dispatch/config",
            &with(&raw, json!({ "planning": { "wip": 6 } })),
        )
        .await;
    assert_eq!(invalid.code().as_deref(), Some("dispatch-config-invalid"));
}

#[tokio::test]
async fn schaltet_ein_und_aus_pausiert_und_setzt_fort() {
    let (server, _backend) = server().await;
    let enabled = body(&server.post("/api/dispatch/enable", "{}").await);
    assert_eq!(enabled["enabled"], true);

    let paused = body(
        &server
            .post("/api/dispatch/pause", r#"{"minutes":15}"#)
            .await,
    );
    assert_eq!(paused["state"], "paused");
    assert_ne!(paused["pausedUntil"], Value::Null);
    assert_eq!(
        server
            .post("/api/dispatch/pause", r#"{"minutes":0}"#)
            .await
            .status,
        422
    );

    let resumed = body(&server.post("/api/dispatch/resume", "{}").await);
    assert_eq!(resumed["pausedUntil"], Value::Null);

    let off = body(&server.post("/api/dispatch/disable", "{}").await);
    assert_eq!(off["enabled"], false);
    assert_eq!(off["state"], "off");
    assert_eq!(off["message"], "Dispatcher ist ausgeschaltet");
}

#[tokio::test]
async fn prueft_board_id_und_minuten_vor_der_aktion() {
    let (server, _backend) = server().await;
    let bad = server
        .post("/api/dispatch/enable", r#"{"boardId":"x"}"#)
        .await;
    assert_eq!(bad.status, 422);
    assert_eq!(bad.code().as_deref(), Some("id-invalid"));
    assert_eq!(
        body(&server.get("/api/dispatch/status").await)["enabled"],
        false
    );

    assert_eq!(
        server
            .post("/api/dispatch/pause", r#"{"minutes":15,"boardId":0}"#)
            .await
            .status,
        422
    );
    assert_eq!(
        body(&server.get("/api/dispatch/status").await)["pausedUntil"],
        Value::Null
    );
    assert_eq!(
        server
            .post("/api/dispatch/pause", r#"{"minutes":"viel"}"#)
            .await
            .status,
        422
    );
    assert_eq!(
        body(&server.get("/api/dispatch/status").await)["pausedUntil"],
        Value::Null
    );
}

#[tokio::test]
async fn meldet_ein_unbekanntes_board_bevor_etwas_passiert() {
    let (server, _backend) = server().await;
    let status = server.get("/api/dispatch/status?boardId=99").await;
    assert_eq!(status.status, 404);
    assert_eq!(body(&status)["code"], "board-not-found");
    assert_eq!(body(&status)["message"], "Board 99 gibt es nicht.");

    let run = server.post("/api/dispatch/run", r#"{"boardId":99}"#).await;
    assert_eq!(run.status, 404);
    assert_eq!(run.code().as_deref(), Some("board-not-found"));
    assert_eq!(
        body(&server.get("/api/dispatch/status").await)["lastTick"],
        Value::Null
    );

    assert_eq!(
        server
            .post("/api/dispatch/enable", r#"{"boardId":99}"#)
            .await
            .status,
        404
    );
    assert_eq!(
        body(&server.get("/api/dispatch/status").await)["enabled"],
        false
    );
}

#[tokio::test]
async fn zeigt_die_bereitschaft_der_geplant_tickets_eines_boards() {
    let (server, backend) = server().await;
    let actor = actor(&backend);
    let board_id = board(&backend).await;
    let created = ticket(
        &backend,
        board_id,
        NewTicketInput {
            labels: Some(vec!["auto".into()]),
            needs_planning: Some(false),
            ..NewTicketInput::titled("Ohne Plan")
        },
        &actor,
    )
    .await;
    let id = created.ticket.id.clone();
    force(
        &backend,
        &id,
        TicketPatch {
            status: Some(TicketStatus::Planned),
            ..TicketPatch::default()
        },
        &actor,
    )
    .await;

    let status = body(
        &server
            .get(&format!("/api/dispatch/status?boardId={board_id}"))
            .await,
    );
    assert_eq!(status["boardId"], board_id);
    assert_eq!(status["tickets"][0]["id"], id);
    assert_eq!(status["tickets"][0]["title"], "Ohne Plan");
    assert_eq!(status["tickets"][0]["reason"], "Kein Plan");

    let run = body(
        &server
            .post(
                "/api/dispatch/run",
                &json!({ "boardId": board_id }).to_string(),
            )
            .await,
    );
    assert_ne!(run["lastTick"], Value::Null);

    let merge = server
        .post(&format!("/api/dispatch/merge/{id}"), "{}")
        .await;
    assert_eq!(merge.status, 422);
    assert_eq!(merge.code().as_deref(), Some("merge-not-possible"));
}

#[tokio::test]
async fn erneut_mergen_liefert_das_ergebnis_und_lehnt_fremde_boards_ab() {
    let (server, backend) = server().await;
    let actor = actor(&backend);
    let board_id = board(&backend).await;
    let mine = ticket(
        &backend,
        board_id,
        NewTicketInput {
            acceptance: Some("ok".into()),
            needs_planning: Some(false),
            ..NewTicketInput::titled("Eigenes")
        },
        &actor,
    )
    .await;
    let mine = mine.ticket.id.clone();
    force(
        &backend,
        &mine,
        TicketPatch {
            status: Some(TicketStatus::Done),
            branch: Some(Some("ticket/x".into())),
            ..TicketPatch::default()
        },
        &actor,
    )
    .await;

    let merge = server
        .post(&format!("/api/dispatch/merge/{mine}"), "{}")
        .await;
    assert_eq!(merge.status, 200);
    assert_eq!(
        body(&merge)["result"],
        json!({
            "outcome": "blocked",
            "text": format!("{mine}: Merge blockiert – Das Board hat keinen Repo-Pfad."),
        })
    );
    assert!(body(&merge).get("enabled").is_some());

    let other = other_board(&backend).await;
    let foreign = backend
        .services
        .boards
        .create("ARC", "Fremd", &other)
        .await
        .expect("fremdes Board")
        .board
        .id;
    let theirs = ticket(
        &backend,
        foreign,
        NewTicketInput {
            acceptance: Some("ok".into()),
            needs_planning: Some(false),
            ..NewTicketInput::titled("Fremdes")
        },
        &other,
    )
    .await;
    let theirs = theirs.ticket.id.clone();
    force(
        &backend,
        &theirs,
        TicketPatch {
            status: Some(TicketStatus::Done),
            branch: Some(Some("ticket/y".into())),
            ..TicketPatch::default()
        },
        &other,
    )
    .await;

    let refused = server
        .post(&format!("/api/dispatch/merge/{theirs}"), "{}")
        .await;
    assert_eq!(refused.status, 422);
    assert_eq!(refused.code().as_deref(), Some("merge-not-possible"));
    assert_eq!(
        body(&refused)["message"],
        format!("{theirs} liegt auf dem Board eines anderen Benutzers.")
    );
    let still = backend
        .services
        .tickets
        .get(&theirs)
        .await
        .expect("fremdes Ticket");
    assert_eq!(still.ticket.status, TicketStatus::Done);
}

#[tokio::test]
async fn abnehmen_nur_bei_human_interaction() {
    let (server, backend) = server().await;
    let actor = actor(&backend);
    let board_id = board(&backend).await;
    let created = ticket(
        &backend,
        board_id,
        NewTicketInput {
            acceptance: Some("ok".into()),
            needs_planning: Some(false),
            ..NewTicketInput::titled("Ansehen")
        },
        &actor,
    )
    .await;
    let id = created.ticket.id.clone();

    let refused = server
        .post(&format!("/api/dispatch/accept/{id}"), "{}")
        .await;
    assert_eq!(refused.status, 422);
    assert_eq!(refused.code().as_deref(), Some("accept-not-possible"));

    force(
        &backend,
        &id,
        TicketPatch {
            status: Some(TicketStatus::Blocked),
            blocked_reason: Some(Some("Human Interaction: Oberfläche ansehen".into())),
            branch: Some(Some("ticket/x".into())),
            ..TicketPatch::default()
        },
        &actor,
    )
    .await;

    let accepted = server
        .post(&format!("/api/dispatch/accept/{id}"), r#"{"by":"Florian"}"#)
        .await;
    assert_eq!(accepted.status, 200);
    assert_eq!(body(&accepted)["result"]["outcome"], "accepted");
    let text = body(&accepted)["result"]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        text.contains(&format!("{id} abgenommen und erledigt")),
        "{text}"
    );
    let done = backend.services.tickets.get(&id).await.expect("Ticket");
    assert_eq!(done.ticket.status, TicketStatus::Done);
    assert_eq!(done.ticket.blocked_reason, None);
    assert_eq!(done.ticket.branch.as_deref(), Some("ticket/x"));

    let bad_by = server
        .post(&format!("/api/dispatch/accept/{id}"), r#"{"by":3}"#)
        .await;
    assert_eq!(bad_by.code().as_deref(), Some("input-invalid"));
}

#[tokio::test]
async fn review_und_zurueck_an_den_agenten() {
    let (server, backend) = server().await;
    let repo = TempRepo::new();
    let actor = actor(&backend);
    let board_id = board(&backend).await;
    backend.set_repo_path(board_id, repo.path());
    set_active_board(&server, board_id);
    repo.run_git(&["branch", "ticket/x"]);
    let created = ticket(
        &backend,
        board_id,
        NewTicketInput {
            acceptance: Some("ok".into()),
            needs_planning: Some(false),
            ..NewTicketInput::titled("Ansehen")
        },
        &actor,
    )
    .await;
    let id = created.ticket.id.clone();

    assert_eq!(
        server
            .post(&format!("/api/dispatch/return/{id}"), "{}")
            .await
            .code()
            .as_deref(),
        Some("input-invalid")
    );
    assert_eq!(
        server
            .post(&format!("/api/dispatch/return/{id}"), r#"{"comment":3}"#)
            .await
            .code()
            .as_deref(),
        Some("input-invalid")
    );
    let none = server
        .post(
            &format!("/api/dispatch/return/{id}"),
            r#"{"comment":"Bitte nachbessern"}"#,
        )
        .await;
    assert_eq!(none.status, 422);
    assert_eq!(none.code().as_deref(), Some("return-not-possible"));

    force(
        &backend,
        &id,
        TicketPatch {
            status: Some(TicketStatus::Blocked),
            blocked_reason: Some(Some("Human Interaction: Review vor dem Merge".into())),
            branch: Some(Some("ticket/x".into())),
            ..TicketPatch::default()
        },
        &actor,
    )
    .await;

    let review = server.get(&format!("/api/dispatch/review/{id}")).await;
    assert_eq!(review.status, 200);
    let view = body(&review);
    assert_eq!(view["ticketId"], id);
    assert_eq!(view["branch"], "ticket/x");
    assert_eq!(view["available"], true);
    assert_eq!(view["files"], json!([]));
    assert_eq!(view["tests"], json!([]));
    assert_eq!(view["testsAt"], Value::Null);

    let unknown = server
        .get(&format!(
            "/api/dispatch/review/{id}/file?path={}",
            common::encode("--output=x")
        ))
        .await;
    assert_eq!(unknown.status, 422);
    assert_eq!(unknown.code().as_deref(), Some("review-file-unknown"));
    assert_eq!(
        server
            .get(&format!("/api/dispatch/review/{id}/file"))
            .await
            .code()
            .as_deref(),
        Some("input-invalid")
    );

    let returned = server
        .post(
            &format!("/api/dispatch/return/{id}"),
            r#"{"comment":"Bitte nachbessern","by":"Florian"}"#,
        )
        .await;
    assert_eq!(returned.status, 200);
    assert_eq!(body(&returned)["result"]["outcome"], "returned");
    assert!(body(&returned).get("enabled").is_some());
    let back = backend.services.tickets.get(&id).await.expect("Ticket");
    assert_eq!(back.ticket.status, TicketStatus::Planned);
    assert_eq!(back.ticket.blocked_reason, None);
    assert_eq!(back.ticket.branch.as_deref(), Some("ticket/x"));

    let other = other_board(&backend).await;
    let foreign = backend
        .services
        .boards
        .create("ARC", "Fremd", &other)
        .await
        .expect("fremdes Board")
        .board
        .id;
    let theirs = ticket(
        &backend,
        foreign,
        NewTicketInput {
            acceptance: Some("ok".into()),
            needs_planning: Some(false),
            ..NewTicketInput::titled("Fremdes")
        },
        &other,
    )
    .await;
    let theirs = theirs.ticket.id.clone();
    force(
        &backend,
        &theirs,
        TicketPatch {
            status: Some(TicketStatus::Blocked),
            blocked_reason: Some(Some("Human Interaction: Review vor dem Merge".into())),
            branch: Some(Some("ticket/y".into())),
            ..TicketPatch::default()
        },
        &other,
    )
    .await;
    let foreign_view = body(&server.get(&format!("/api/dispatch/review/{theirs}")).await);
    assert_eq!(foreign_view["available"], false);
    assert!(
        foreign_view["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("anderen Benutzer"),
        "{foreign_view}"
    );
    assert_eq!(
        server
            .post(
                &format!("/api/dispatch/return/{theirs}"),
                r#"{"comment":"x"}"#
            )
            .await
            .code()
            .as_deref(),
        Some("return-not-possible")
    );
}

#[tokio::test]
async fn planen_startet_sofort_und_lehnt_doppelstart_und_boards_ohne_repo_ab() {
    let (server, backend) = server().await;
    let repo = TempRepo::new();
    let actor = actor(&backend);
    let board_id = board(&backend).await;
    let created = ticket(
        &backend,
        board_id,
        NewTicketInput {
            needs_planning: Some(true),
            ..NewTicketInput::titled("Planbar")
        },
        &actor,
    )
    .await;
    let id = created.ticket.id.clone();

    let refused = server
        .post(
            &format!("/api/dispatch/plan/{id}"),
            &json!({ "boardId": board_id }).to_string(),
        )
        .await;
    assert_eq!(refused.status, 422);
    assert_eq!(refused.code().as_deref(), Some("plan-no-repo"));
    assert_eq!(
        server
            .post(
                "/api/dispatch/plan/ARC-0099",
                &json!({ "boardId": board_id }).to_string()
            )
            .await
            .status,
        404
    );

    backend.set_repo_path(board_id, repo.path());
    set_active_board(&server, board_id);
    let before = body(&server.get(&format!("/api/dispatch/plan/{id}")).await);
    assert_eq!(before["ticketId"], id);
    assert_eq!(before["run"], Value::Null);
    assert_eq!(before["pausedUntil"], Value::Null);

    let started = server
        .post(
            &format!("/api/dispatch/plan/{id}"),
            &json!({ "boardId": board_id }).to_string(),
        )
        .await;
    assert_eq!(started.status, 200);
    let json = body(&started);
    assert_eq!(json["started"]["kind"], "plan");
    assert_eq!(json["started"]["ticketId"], id);
    assert_eq!(json["enabled"], false);
    assert_eq!(json["active"], 0);
    assert_eq!(json["planning"], 1);

    let lower = body(
        &server
            .get(&format!("/api/dispatch/plan/{}", id.to_lowercase()))
            .await,
    );
    assert_eq!(lower["run"]["ticketId"], id);

    let again = server
        .post(
            &format!("/api/dispatch/plan/{id}"),
            &json!({ "boardId": board_id }).to_string(),
        )
        .await;
    assert_eq!(again.code().as_deref(), Some("plan-running"));
}

#[tokio::test]
async fn recherche_startet_sofort_und_lehnt_doppelstart_und_fehlende_fragen_ab() {
    let (server, backend) = server().await;
    let repo = TempRepo::new();
    let actor = actor(&backend);
    let board_id = board(&backend).await;
    let asking = BoardQuestionActor {
        actor: actor.clone(),
        kind: nexus_core::actors::AuthorKind::Human,
        session: None,
    };
    let question = backend
        .services
        .questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                title: "Ohne Repo".into(),
                ..NewBoardQuestionInput::default()
            },
            &asking,
        )
        .await
        .expect("Frage");
    let question_id = question.view.question.id.clone();

    let refused = server
        .post(&format!("/api/dispatch/research/{question_id}"), "{}")
        .await;
    assert_eq!(refused.status, 422);
    assert_eq!(refused.code().as_deref(), Some("research-no-repo"));
    assert_eq!(
        server
            .post("/api/dispatch/research/ARC-F0099", "{}")
            .await
            .status,
        404
    );
    assert_eq!(
        server
            .post("/api/dispatch/research/kaputt", "{}")
            .await
            .code()
            .as_deref(),
        Some("question-id-invalid")
    );
    let invalid = server.get("/api/dispatch/research/kaputt").await;
    assert_eq!(invalid.status, 422);
    assert_eq!(invalid.code().as_deref(), Some("question-id-invalid"));
    assert!(
        body(&invalid)["message"]
            .as_str()
            .unwrap_or_default()
            .contains("„kaputt“ ist keine Fragenummer"),
        "{}",
        invalid.text
    );

    backend.set_repo_path(board_id, repo.path());
    set_active_board(&server, board_id);
    let before = body(
        &server
            .get(&format!("/api/dispatch/research/{question_id}"))
            .await,
    );
    assert_eq!(before["questionId"], question_id);
    assert_eq!(before["run"], Value::Null);
    assert_eq!(before["count"], 0);

    let started = server
        .post(&format!("/api/dispatch/research/{question_id}"), "{}")
        .await;
    assert_eq!(started.status, 200);
    let json = body(&started);
    assert_eq!(json["started"]["kind"], "research");
    assert_eq!(json["started"]["questionId"], question_id);
    assert_eq!(json["enabled"], false);
    assert_eq!(json["research"]["active"], 1);

    let lower = body(
        &server
            .get(&format!(
                "/api/dispatch/research/{}",
                question_id.to_lowercase()
            ))
            .await,
    );
    assert_eq!(lower["run"]["questionId"], question_id);
    assert_eq!(lower["count"], 1);

    let again = server
        .post(&format!("/api/dispatch/research/{question_id}"), "{}")
        .await;
    assert_eq!(again.status, 422);
    assert_eq!(again.code().as_deref(), Some("research-running"));
}

#[tokio::test]
async fn listet_und_liest_protokolle_nur_mit_erlaubten_namen() {
    let (server, _backend) = server().await;
    fs::create_dir_all(server.home.dispatch_logs_dir()).unwrap();
    fs::write(
        server
            .home
            .dispatch_logs_dir()
            .join("ARC-0001-20261015-090000.log"),
        "Ausgabe",
    )
    .unwrap();

    assert_eq!(
        body(&server.get("/api/dispatch/logs").await),
        json!(["ARC-0001-20261015-090000.log"])
    );
    let log = server
        .get("/api/dispatch/logs/ARC-0001-20261015-090000.log")
        .await;
    assert_eq!(log.text, "Ausgabe");
    assert!(
        log.header("content-type")
            .unwrap_or_default()
            .contains("text/plain"),
        "{:?}",
        log.header("content-type")
    );
    assert_eq!(
        server
            .get("/api/dispatch/logs/..%2Fboard.yaml")
            .await
            .status,
        422
    );
    assert_eq!(server.get("/api/dispatch/logs/fehlt.log").await.status, 404);
}

#[tokio::test]
async fn listet_die_laeufe_eines_tickets_neueste_zuerst() {
    let (server, _backend) = server().await;
    fs::create_dir_all(server.home.dispatch_logs_dir()).unwrap();
    for name in [
        "ARC-0001-20261015-090000.log",
        "ARC-0001-20261016-101500-merge.log",
        "ARC-00010-20261016-110000.log",
        "ARC-F0002-20261016-120000-research.log",
    ] {
        fs::write(server.home.dispatch_logs_dir().join(name), "x").unwrap();
    }

    let runs = server.get("/api/dispatch/runs?ticket=ARC-0001").await;
    assert_eq!(runs.status, 200);
    let names: Vec<String> = body(&runs)
        .as_array()
        .expect("Läufe")
        .iter()
        .map(|run| run["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        names,
        [
            "ARC-0001-20261016-101500-merge.log",
            "ARC-0001-20261015-090000.log"
        ]
    );
    assert_eq!(body(&runs)[0]["kind"], "merge");
    assert_eq!(body(&runs)[1]["kind"], "work");
    assert_eq!(body(&runs)[0]["running"], false);

    let research = body(&server.get("/api/dispatch/runs?ticket=ARC-F0002").await);
    assert_eq!(research.as_array().expect("Läufe").len(), 1);
    assert_eq!(research[0]["kind"], "research");
    assert_eq!(
        body(&server.get("/api/dispatch/runs?ticket=ARC-0099").await),
        json!([])
    );
    assert_eq!(server.get("/api/dispatch/runs").await.status, 422);
    assert_eq!(
        server
            .get("/api/dispatch/runs?ticket=..%2Fx")
            .await
            .code()
            .as_deref(),
        Some("id-invalid")
    );
}

#[tokio::test]
async fn liefert_ein_lauf_protokoll_zerlegt() {
    let (server, _backend) = server().await;
    fs::create_dir_all(server.home.dispatch_logs_dir()).unwrap();
    fs::write(
        server
            .home
            .dispatch_logs_dir()
            .join("ARC-0001-20261015-090000.log"),
        "[dispatch] Start\n[dispatch] Arbeitsanweisung:\nTu es.\n\n[dispatch] Ausgabe:\n09:00:01  Claude startet (opus)\n",
    )
    .unwrap();

    let parts = body(
        &server
            .get("/api/dispatch/runs/ARC-0001-20261015-090000.log")
            .await,
    );
    assert_eq!(
        parts,
        json!({
            "name": "ARC-0001-20261015-090000.log",
            "header": "[dispatch] Start",
            "prompt": "Tu es.",
            "body": "09:00:01  Claude startet (opus)\n",
        })
    );
    assert_eq!(server.get("/api/dispatch/runs/fehlt.log").await.status, 404);
    assert_eq!(
        server
            .get("/api/dispatch/runs/..%2Fboard.yaml")
            .await
            .status,
        422
    );
}

#[tokio::test]
async fn liefert_je_laufart_standardvorlage_platzhalter_und_pflichtteile() {
    let (server, _backend) = server().await;
    let info = body(&server.get("/api/dispatch/config").await)["promptInfo"].clone();
    let kinds: Vec<String> = info
        .as_array()
        .expect("Laufarten")
        .iter()
        .map(|entry| entry["kind"].as_str().unwrap().to_string())
        .collect();
    let expected: Vec<String> = PromptKind::ALL
        .iter()
        .map(|kind| kind.as_str().to_string())
        .collect();
    assert_eq!(kinds, expected);
    for entry in info.as_array().expect("Laufarten") {
        let kind = PromptKind::parse(entry["kind"].as_str().unwrap()).expect("Laufart");
        assert!(!entry["label"].as_str().unwrap().is_empty());
        assert_eq!(
            entry["default"],
            nexus_core::dispatch::prompt_defaults::default_template(kind)
        );
        let names: Vec<String> = entry["placeholders"]
            .as_array()
            .expect("Platzhalter")
            .iter()
            .map(|placeholder| placeholder["name"].as_str().unwrap().to_string())
            .collect();
        let want: Vec<String> = nexus_core::dispatch::prompt_defaults::placeholders(kind)
            .into_iter()
            .map(|placeholder| placeholder.name)
            .collect();
        assert_eq!(names, want);
        assert!(!entry["required"]
            .as_array()
            .expect("Pflichtteile")
            .is_empty());
    }
    let work = info
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["kind"] == "work")
        .expect("work");
    assert!(work["default"].as_str().unwrap().contains("{{ticketId}}"));
}

#[tokio::test]
async fn lehnt_eine_ungueltige_eigene_vorlage_beim_speichern_ab() {
    let (server, _backend) = server().await;
    let raw = raw_config(&server).await;

    let unknown = server
        .put(
            "/api/dispatch/config",
            &with(
                &raw,
                json!({ "prompts": { "work": format!("{WORK}\n{{{{quatsch}}}}") } }),
            ),
        )
        .await;
    assert_eq!(unknown.status, 422);
    assert_eq!(unknown.code().as_deref(), Some("dispatch-prompt-invalid"));
    assert_eq!(body(&unknown)["details"]["kind"], "work");
    assert_eq!(body(&unknown)["details"]["missing"], json!([]));
    assert_eq!(body(&unknown)["details"]["unknown"], json!(["quatsch"]));

    let missing = server
        .put(
            "/api/dispatch/config",
            &with(
                &raw,
                json!({ "prompts": { "work": "Nur {{agent}} und {{ticketId}}." } }),
            ),
        )
        .await;
    assert_eq!(missing.status, 422);
    assert_eq!(body(&missing)["details"]["kind"], "work");
    assert!(!body(&missing)["details"]["missing"]
        .as_array()
        .expect("fehlende Teile")
        .is_empty());
    assert!(body(&missing)["message"]
        .as_str()
        .unwrap_or_default()
        .contains("work"));

    // nach beiden Fehlversuchen ist nichts geschrieben
    assert_eq!(
        body(&server.get("/api/dispatch/config").await)["exists"],
        false
    );

    let ok = server
        .put(
            "/api/dispatch/config",
            &with(&raw, json!({ "prompts": { "work": WORK } })),
        )
        .await;
    assert_eq!(ok.status, 200);
    assert_eq!(body(&ok)["raw"]["prompts"]["work"], WORK);
}

#[tokio::test]
async fn zeigt_eine_vorschau_mit_beispielwerten_ohne_zu_speichern() {
    let (server, _backend) = server().await;
    let preview = server
        .post("/api/dispatch/prompts/preview", r#"{"kind":"work"}"#)
        .await;
    assert_eq!(preview.status, 200);
    let json = body(&preview);
    assert_eq!(json["kind"], "work");
    assert_eq!(json["id"], Value::Null);
    assert_eq!(json["sample"], true);
    let prompt = json["prompt"].as_str().unwrap();
    assert!(prompt.contains("AIH-0042"), "{prompt}");
    assert!(prompt.contains("junior-AIH-0042"), "{prompt}");
    // Beispielantwort und Beispiel-Rückmeldung machen die optionalen Blöcke sichtbar
    assert!(
        prompt.contains("Entscheidungen zu Fragen dieses Tickets"),
        "{prompt}"
    );
    assert!(prompt.contains("Rückmeldung aus dem Review"), "{prompt}");
    assert!(!prompt.contains("{{"), "noch offene Platzhalter: {prompt}");
}

#[tokio::test]
async fn rendert_die_vorschau_mit_den_echten_werten() {
    let (server, backend) = server().await;
    let actor = actor(&backend);
    let board_id = board(&backend).await;
    let created = ticket(
        &backend,
        board_id,
        NewTicketInput {
            acceptance: Some("ok".into()),
            needs_planning: Some(false),
            ..NewTicketInput::titled("Echtes Ticket")
        },
        &actor,
    )
    .await;
    let id = created.ticket.id.clone();
    let human = BoardQuestionActor {
        actor: nexus_core::board::basics::BoardActor {
            user_id: MEMORY_USER_ID.into(),
            name: MEMORY_USER_NAME.into(),
        },
        kind: nexus_core::actors::AuthorKind::Human,
        session: None,
    };
    let question = backend
        .services
        .questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                title: "Brutto oder netto?".into(),
                ticket_ids: Some(vec![id.clone()]),
                ..NewBoardQuestionInput::default()
            },
            &human,
        )
        .await
        .expect("Frage");
    let question_id = question.view.question.id.clone();
    backend
        .services
        .questions
        .answer(
            &question_id,
            &nexus_core::questions::DecisionInput {
                text: Some("Netto".into()),
                reason: Some("Vertrag".into()),
                ..nexus_core::questions::DecisionInput::default()
            },
            &human,
            &nexus_core::board::question_service::BoardQuestionWriteOptions::default(),
        )
        .await
        .expect("Antwort");

    let work = body(
        &server
            .post(
                "/api/dispatch/prompts/preview",
                &json!({ "kind": "work", "id": id }).to_string(),
            )
            .await,
    );
    assert_eq!(work["kind"], "work");
    assert_eq!(work["id"], id);
    assert_eq!(work["sample"], false);
    let prompt = work["prompt"].as_str().unwrap();
    assert!(prompt.contains("Echtes Ticket"), "{prompt}");
    assert!(prompt.contains(&id), "{prompt}");
    assert!(prompt.contains("Netto"), "{prompt}");

    let research = body(
        &server
            .post(
                "/api/dispatch/prompts/preview",
                &json!({ "kind": "research", "id": question_id }).to_string(),
            )
            .await,
    );
    assert_eq!(research["kind"], "research");
    assert_eq!(research["id"], question_id);
    assert_eq!(research["sample"], false);
    assert!(research["prompt"]
        .as_str()
        .unwrap()
        .contains("Brutto oder netto?"));

    assert_eq!(
        server
            .post(
                "/api/dispatch/prompts/preview",
                r#"{"kind":"work","id":"ARC-0999"}"#
            )
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn rendert_die_eigene_vorlage_prueft_sie_und_speichert_nichts() {
    let (server, _backend) = server().await;
    let own = "Plane {{ticketId}} („{{title}}“) als {{agent}}: harness board plan-json {{ticketId}} --file plan.json";
    let good = server
        .post(
            "/api/dispatch/prompts/preview",
            &json!({ "kind": "plan", "template": own }).to_string(),
        )
        .await;
    assert_eq!(good.status, 200);
    assert!(body(&good)["prompt"]
        .as_str()
        .unwrap()
        .contains("Plane AIH-0042"));

    let bad = server
        .post(
            "/api/dispatch/prompts/preview",
            &json!({ "kind": "plan", "template": format!("{own}\n{{{{quatsch}}}}") }).to_string(),
        )
        .await;
    assert_eq!(bad.status, 422);
    assert_eq!(bad.code().as_deref(), Some("dispatch-prompt-invalid"));
    assert_eq!(body(&bad)["details"]["kind"], "plan");
    assert_eq!(body(&bad)["details"]["unknown"], json!(["quatsch"]));

    assert_eq!(
        server
            .post("/api/dispatch/prompts/preview", r#"{"kind":"quatsch"}"#)
            .await
            .code()
            .as_deref(),
        Some("input-invalid")
    );
    assert_eq!(
        server
            .post(
                "/api/dispatch/prompts/preview",
                r#"{"kind":"plan","template":7}"#
            )
            .await
            .code()
            .as_deref(),
        Some("input-invalid")
    );

    // die Vorschau speichert nichts
    let view = body(&server.get("/api/dispatch/config").await);
    assert_eq!(view["exists"], false);
    assert_eq!(view["config"]["prompts"]["plan"], Value::Null);
}

#[tokio::test]
async fn ohne_umschalter_beantwortet_rust_die_dispatch_routen_nicht() {
    // Vorgabe ist `node`: dann gehen die Routen zum Sidecar; ohne Sidecar bleibt 404
    let (server, _backend) = TestServer::start_with_board().await;
    let status = server.get("/api/dispatch/status").await;
    assert_eq!(status.status, 404);
    assert_eq!(status.code().as_deref(), Some("not-found"));
}
