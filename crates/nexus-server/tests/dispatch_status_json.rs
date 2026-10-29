//! Die JSON-Form des Dispatcher-Zustands ist Vertrag: Oberfläche und Kommandozeile lesen
//! genau die Schlüssel aus `packages/server/src/dispatch/DispatchStatus.ts`.
//!
//! Die erwarteten Schlüssel stehen in `tests/fixtures/dispatch_status_keys.golden.json`. Sie
//! stammen aus den TypeScript-Typen des alten Stands, sind eingecheckt und werden nie neu
//! erzeugt: Weicht etwas ab, ist der Vertrag gebrochen.

mod common;
mod dispatch_support;

use nexus_core::dispatch::{MergePhase, RunKind, RunLogEntry, RunLogParts};
use nexus_server::dispatch::status::{
    DispatchMergeInfo, DispatchPlanningInfo, DispatchResearchInfo, DispatchRunInfo, DispatchRunLog,
    DispatchRunLogText, DispatchStateKind, DispatchStatus, DispatchTicketRow,
    QuestionResearchStatus, TicketPlanningStatus,
};
use serde::Serialize;
use serde_json::{json, Value};

use dispatch_support::DispatchFixture;

/// Die Schlüssel der JSON-Form, sortiert (die Reihenfolge im Objekt ist kein Vertrag)
fn keys<T: Serialize>(value: &T) -> Vec<String> {
    let json = serde_json::to_value(value).expect("JSON-Form");
    let mut keys: Vec<String> = json
        .as_object()
        .expect("JSON-Objekt")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// Die erwarteten Schlüssel aus der Fixture, sortiert
fn golden(name: &str) -> Vec<String> {
    let text = include_str!("fixtures/dispatch_status_keys.golden.json");
    let all: Value = serde_json::from_str(text).expect("Fixture als JSON");
    let mut keys: Vec<String> = all
        .get(name)
        .unwrap_or_else(|| panic!("{name} fehlt in der Fixture"))
        .as_array()
        .expect("Liste von Schlüsseln")
        .iter()
        .map(|key| key.as_str().expect("Schlüssel als Text").to_string())
        .collect();
    keys.sort();
    keys
}

/// Ein Lauf, wie die Oberfläche ihn sieht
fn run_info() -> DispatchRunInfo {
    DispatchRunInfo {
        id: "ARC-0001-20261029-090000".into(),
        kind: RunKind::Work,
        ticket_id: "ARC-0001".into(),
        title: "T".into(),
        profile: "standard".into(),
        model: Some("opus".into()),
        effort: Some("high".into()),
        started_at: "2026-10-29T09:00:00.000Z".into(),
        log: "ARC-0001-20261029-090000.log".into(),
        question_id: None,
    }
}

fn research_info() -> DispatchResearchInfo {
    DispatchResearchInfo {
        auto: false,
        wip: 1,
        active: 0,
        max_per_day: 10,
        today: 0,
        since: None,
        retry_at: None,
        error: None,
    }
}

fn planning_info() -> DispatchPlanningInfo {
    DispatchPlanningInfo {
        auto: false,
        wip: 1,
        active: 0,
        since: None,
        retry_at: None,
        error: None,
    }
}

#[test]
fn die_json_form_des_zustands_hat_die_schluessel_der_typescript_typen() {
    assert_eq!(keys(&run_info()), golden("dispatchRunInfo"));
    assert_eq!(keys(&research_info()), golden("dispatchResearchInfo"));
    assert_eq!(keys(&planning_info()), golden("dispatchPlanningInfo"));
    assert_eq!(
        keys(&DispatchMergeInfo {
            ticket_id: "ARC-0001".into(),
            phase: MergePhase::Waiting,
            branch: "b".into(),
            target: None,
        }),
        golden("dispatchMergeInfo")
    );
    assert_eq!(
        keys(&DispatchTicketRow {
            id: "ARC-0001".into(),
            title: "T".into(),
            reason: "bereit".into(),
        }),
        golden("dispatchTicketRow")
    );
    assert_eq!(
        keys(&DispatchRunLog::new(
            RunLogEntry {
                name: "ARC-0001-20261029-090000.log".into(),
                kind: RunKind::Work,
                started: Some("2026-10-29T09:00:00.000Z".into()),
            },
            true,
        )),
        golden("dispatchRunLog")
    );
    assert_eq!(
        keys(&DispatchRunLogText {
            name: "ARC-0001-20261029-090000.log".into(),
            parts: RunLogParts {
                header: "h".into(),
                prompt: Some("p".into()),
                body: "b".into(),
            },
        }),
        golden("dispatchRunLogText"),
        "die Teile des Protokolls stehen flach neben dem Namen"
    );
    assert_eq!(
        keys(&QuestionResearchStatus {
            question_id: "ARC-F0001".into(),
            run: None,
            paused_until: None,
            last_started_at: None,
            count: 0,
        }),
        golden("questionResearchStatus")
    );
    assert_eq!(
        keys(&TicketPlanningStatus {
            ticket_id: "ARC-0001".into(),
            run: None,
            paused_until: None,
        }),
        golden("ticketPlanningStatus")
    );
    assert_eq!(
        keys(&DispatchStatus {
            enabled: true,
            state: DispatchStateKind::Idle,
            message: "bereit · 0/1".into(),
            paused_until: None,
            wip: 1,
            active: 0,
            planning: 0,
            runs: vec![run_info()],
            research: research_info(),
            planning_auto: planning_info(),
            merges: Vec::new(),
            board_id: Some(1),
            board_in_scope: true,
            tickets: Vec::new(),
            tickets_error: None,
            last_tick: None,
            last_errors: Vec::new(),
            config_file: "dispatch.yaml".into(),
            config_error: None,
            agent_prefix: "junior-".into(),
        }),
        golden("dispatchStatus")
    );
}

#[tokio::test]
async fn der_zustand_des_dispatchers_serialisiert_genau_diese_schluessel() {
    let f = DispatchFixture::create(json!({})).await;
    let status = f.service.status(Some(f.board.id)).await.expect("Zustand");
    assert_eq!(keys(&status), golden("dispatchStatus"));
    // Die Werte der Aufzählung stehen klein geschrieben im JSON, wie in TypeScript
    let json = serde_json::to_value(&status).expect("JSON-Form");
    assert_eq!(json.get("state"), Some(&json!("idle")));
    assert_eq!(json.get("agentPrefix"), Some(&json!("junior-")));

    let planning = f
        .service
        .planning_status("ARC-0001")
        .await
        .expect("Planungs-Zustand");
    assert_eq!(keys(&planning), golden("ticketPlanningStatus"));
    let research = f
        .service
        .research_status("ARC-F0001")
        .await
        .expect("Recherche-Zustand");
    assert_eq!(keys(&research), golden("questionResearchStatus"));
}
