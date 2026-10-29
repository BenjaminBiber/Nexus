//! Die Routen unter `/api/dispatch/suggestions` samt Claude-Analyse; portiert aus
//! `packages/server/tests/unit/DispatchSuggestionRoutes.test.ts` und
//! `packages/server/tests/unit/DispatchSuggestionAnalysis.test.ts`.
//!
//! Der Testserver steht auf `NEXUS_DISPATCH=rust`, denn nur dann beantwortet Rust diese
//! Routen. Claude startet nie: an seiner Stelle antwortet eine Attrappe.

mod common;

use std::sync::Arc;

use nexus_core::dispatch::{DispatchConfigNormalizer, DispatchConfigStore};
use nexus_core::testing::fixtures::run_fixture;
use nexus_server::config::DispatchBackend;
use nexus_server::dispatch::RunResult;
use serde_json::{json, Value};

use common::{
    fake_runners, reply, BoardBackendWithServices, FakeClaude, TestServer, TestServerOptions,
};

/// Die Einstellung der Tests: zwei Profile, keine Regeln
fn raw() -> Value {
    let mut raw = DispatchConfigNormalizer::new().example();
    let object = raw.as_object_mut().expect("Objekt");
    object.insert(
        "profiles".to_string(),
        json!({
            "standard": { "model": "opus", "maxTurns": 30, "timeoutMinutes": 60 },
            "gross": { "model": "opus", "maxTurns": 80, "timeoutMinutes": 120 },
        }),
    );
    object.insert("defaultProfile".to_string(), json!("standard"));
    object.insert("escalate".to_string(), json!(true));
    object.insert("escalation".to_string(), json!(["standard", "gross"]));
    object.insert("rules".to_string(), json!([]));
    object.insert(
        "research".to_string(),
        json!({ "auto": false, "profile": null }),
    );
    object.insert("planning".to_string(), json!({ "profile": null }));
    raw
}

/// Testserver mit Board, auf dem Rust dispatcht, und dieser Claude-Attrappe
async fn start(claude: Option<Arc<FakeClaude>>) -> (TestServer, Arc<BoardBackendWithServices>) {
    let claude = claude.map(fake_runners);
    let (server, backend) = TestServer::start_with_board_and(TestServerOptions {
        dispatch: DispatchBackend::Rust,
        claude,
        ..TestServerOptions::default()
    })
    .await;
    DispatchConfigStore::new(server.home.clone())
        .write(&raw())
        .expect("Einstellung schreiben");
    (server, backend)
}

/// 5 Läufe mit „standard“ in zwei Projekten, 2 davon an den Schritten gescheitert
async fn runs(backend: &BoardBackendWithServices) {
    let actor = backend.actor();
    let arc = backend
        .services
        .projects
        .create("ARC", "Arches", &actor)
        .await
        .expect("Projekt")
        .id;
    let oth = backend
        .services
        .projects
        .create("OTH", "Andere", &actor)
        .await
        .expect("Projekt")
        .id;
    let now = chrono::Utc::now();
    for i in 0..5 {
        let mut run = run_fixture(
            &format!("r{i}"),
            &format!("T{i}"),
            if i % 2 == 1 { oth } else { arc },
        );
        run.started_at = (now - chrono::Duration::minutes(i as i64))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        run.max_turns = Some(30);
        run.timeout_minutes = Some(60);
        if i < 2 {
            run.outcome = nexus_core::board::DispatchRunOutcome::Abort;
            run.abort_category = Some("max-turns".to_string());
        }
        backend
            .services
            .dispatch_runs
            .record(&run)
            .await
            .expect("Lauf");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn wertet_laufe_aller_projekte_aus_und_schlagt_mehr_schritte_vor() {
    let (server, backend) = start(None).await;
    runs(&backend).await;
    let reply = server.get("/api/dispatch/suggestions").await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body()["basis"]["runs"], json!(5));
    assert_eq!(reply.body()["basis"]["projects"], json!(2));
    let first = &reply.body()["suggestions"][0];
    assert_eq!(first["kind"], json!("turns-too-low"));
    assert_eq!(
        first["patch"],
        json!([{ "path": "profiles.standard.maxTurns", "from": 30, "to": 45 }])
    );
    assert_eq!(
        reply.body()["insufficient"],
        json!([{ "profile": "gross", "runs": 0, "needed": 5 }])
    );
    assert_eq!(reply.body()["analysis"], Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn ubernimmt_einen_vorschlag_in_dispatch_yaml_und_meldet_ihn_danach_nicht_mehr() {
    let (server, backend) = start(None).await;
    runs(&backend).await;
    let list = server
        .get("/api/dispatch/suggestions")
        .await
        .json()
        .expect("JSON");
    let id = list["suggestions"][0]["id"].as_str().expect("ID");
    let reply = server
        .post(
            &format!("/api/dispatch/suggestions/{id}/apply"),
            &json!({ "configHash": list["configHash"] }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.body()["view"]["raw"]["profiles"]["standard"]["maxTurns"],
        json!(45)
    );
    assert_eq!(
        DispatchConfigStore::new(server.home.clone())
            .read()
            .expect("lesen")
            .config
            .profiles["standard"]
            .max_turns,
        45
    );
    let body = reply.body();
    let kinds: Vec<&str> = body["list"]["suggestions"]
        .as_array()
        .expect("Liste")
        .iter()
        .filter_map(|s| s["kind"].as_str())
        .collect();
    assert!(!kinds.contains(&"turns-too-low"), "{kinds:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn lehnt_das_ubernehmen_mit_veralteter_einstellung_oder_unbekannter_id_ab() {
    let (server, backend) = start(None).await;
    runs(&backend).await;
    let list = server
        .get("/api/dispatch/suggestions")
        .await
        .json()
        .expect("JSON");
    let id = list["suggestions"][0]["id"].as_str().expect("ID");
    let stale = server
        .post(
            &format!("/api/dispatch/suggestions/{id}/apply"),
            &json!({ "configHash": "veraltet" }).to_string(),
        )
        .await;
    assert_eq!(stale.status, 409);
    assert_eq!(stale.body()["code"], json!("conflict"));
    assert_eq!(
        stale.body()["details"],
        json!({ "reason": "config-changed" })
    );
    let gone = server
        .post(
            "/api/dispatch/suggestions/0000000000000000/apply",
            &json!({ "configHash": list["configHash"] }).to_string(),
        )
        .await;
    assert_eq!(gone.status, 404);
    assert_eq!(gone.body()["code"], json!("suggestion-not-found"));
}

#[tokio::test(flavor = "multi_thread")]
async fn blendet_vorschlage_aus_und_wieder_ein() {
    let (server, backend) = start(None).await;
    runs(&backend).await;
    let list = server
        .get("/api/dispatch/suggestions")
        .await
        .json()
        .expect("JSON");
    let id = list["suggestions"][0]["id"]
        .as_str()
        .expect("ID")
        .to_string();
    let hidden = server
        .post(&format!("/api/dispatch/suggestions/{id}/dismiss"), "{}")
        .await;
    assert_eq!(hidden.body()["suggestions"], json!([]));
    assert_eq!(
        hidden.body()["dismissed"]
            .as_array()
            .expect("Liste")
            .iter()
            .filter_map(|s| s["id"].as_str())
            .collect::<Vec<&str>>(),
        vec![id.as_str()]
    );
    let back = server
        .delete(&format!("/api/dispatch/suggestions/{id}/dismiss"))
        .await;
    assert_eq!(
        back.body()["suggestions"]
            .as_array()
            .expect("Liste")
            .iter()
            .filter_map(|s| s["id"].as_str())
            .collect::<Vec<&str>>(),
        vec![id.as_str()]
    );
    assert_eq!(
        server
            .post("/api/dispatch/suggestions/kaputt!/dismiss", "{}")
            .await
            .status,
        422
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn ubernimmt_gultige_claude_vorschlage_und_macht_ungultige_zu_hinweisen() {
    let antwort = json!([
        { "title": "Mehr Zeit für gross", "reason": "weil", "target": { "profile": "gross" },
          "patch": [{ "path": "profiles.gross.timeoutMinutes", "from": 120, "to": 180 }] },
        { "title": "Befehl ändern", "reason": "x",
          "patch": [{ "path": "claude.command", "from": "claude", "to": "rm" }] },
        { "title": "Unsinniger Wert", "reason": "y",
          "patch": [{ "path": "profiles.gross.maxTurns", "from": 80, "to": -5 }] },
        { "reason": "ohne Titel – unbrauchbar" },
    ])
    .to_string();
    let claude = FakeClaude::with_replies(vec![reply(&antwort, 0.04)]);
    let (server, _backend) = start(Some(claude)).await;
    let res = server.post("/api/dispatch/suggestions/analyze", "{}").await;
    assert_eq!(res.status, 200);
    let body = res.body();
    let claude_suggestions: Vec<(&str, bool)> = body["suggestions"]
        .as_array()
        .expect("Liste")
        .iter()
        .filter(|s| s["source"] == json!("claude"))
        .map(|s| {
            (
                s["title"].as_str().unwrap_or_default(),
                s["patch"].is_null(),
            )
        })
        .collect();
    assert_eq!(
        claude_suggestions,
        vec![
            ("Mehr Zeit für gross", false),
            ("Befehl ändern", true),
            ("Unsinniger Wert", true),
        ]
    );
    assert_eq!(body["analysis"]["costUsd"], json!(0.04));
    assert_eq!(body["analysis"]["stale"], json!(false));
    assert_eq!(body["analysis"]["count"], json!(3));
    assert_eq!(body["analysis"]["skipped"], json!(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn merkt_auch_ein_leeres_ergebnis_mit_zeitpunkt_und_kosten() {
    let claude = FakeClaude::with_replies(vec![reply("[]", 0.04)]);
    let (server, _backend) = start(Some(claude)).await;
    let res = server.post("/api/dispatch/suggestions/analyze", "{}").await;
    assert_eq!(res.status, 200);
    assert_eq!(res.body()["analysis"]["costUsd"], json!(0.04));
    assert_eq!(res.body()["analysis"]["stale"], json!(false));
    assert_eq!(res.body()["analysis"]["count"], json!(0));
    assert_eq!(res.body()["analysis"]["skipped"], json!(0));
    assert!(res.body()["analysis"]["at"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn kennzeichnet_ein_ergebnis_zu_einer_alteren_einstellung_als_veraltet() {
    let antwort = json!([{ "title": "A", "reason": "r",
        "patch": [{ "path": "profiles.gross.timeoutMinutes", "from": 120, "to": 180 }] }])
    .to_string();
    let claude = FakeClaude::with_replies(vec![reply(&antwort, 0.04)]);
    let (server, _backend) = start(Some(claude)).await;
    server.post("/api/dispatch/suggestions/analyze", "{}").await;
    let mut geandert = raw();
    geandert
        .as_object_mut()
        .expect("Objekt")
        .insert("defaultProfile".to_string(), json!("gross"));
    DispatchConfigStore::new(server.home.clone())
        .write(&geandert)
        .expect("Einstellung schreiben");
    let list = server
        .get("/api/dispatch/suggestions")
        .await
        .json()
        .expect("JSON");
    assert_eq!(list["analysis"]["stale"], json!(true));
    let a = list["suggestions"]
        .as_array()
        .expect("Liste")
        .iter()
        .find(|s| s["title"] == json!("A"))
        .expect("Vorschlag A");
    assert_eq!(a["patch"], Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn meldet_fehlendes_claude_zeitlimit_und_unlesbare_antworten_als_fehler() {
    let fehlt = FakeClaude::with_replies(vec![RunResult {
        code: Some(127),
        timed_out: false,
        output: "spawn claude ENOENT".to_string(),
    }]);
    let (server, _backend) = start(Some(fehlt)).await;
    let missing = server.post("/api/dispatch/suggestions/analyze", "{}").await;
    assert_eq!(missing.status, 422);
    assert_eq!(missing.body()["code"], json!("analysis-failed"));
    assert!(
        missing.body()["message"]
            .as_str()
            .unwrap_or_default()
            .contains("nicht gefunden"),
        "{}",
        missing.body()
    );

    let zeitlimit = FakeClaude::with_replies(vec![RunResult {
        code: None,
        timed_out: true,
        output: String::new(),
    }]);
    let (server, _backend) = start(Some(zeitlimit)).await;
    let timed_out = server.post("/api/dispatch/suggestions/analyze", "{}").await;
    assert!(
        timed_out.body()["message"]
            .as_str()
            .unwrap_or_default()
            .contains("5 Minuten"),
        "{}",
        timed_out.body()
    );

    let unlesbar = FakeClaude::with_replies(vec![reply(
        "Ich schlage vor, mehr Schritte zu geben.",
        0.04,
    )]);
    let (server, _backend) = start(Some(unlesbar)).await;
    let broken = server.post("/api/dispatch/suggestions/analyze", "{}").await;
    assert!(
        broken.body()["message"]
            .as_str()
            .unwrap_or_default()
            .contains("nicht lesbar"),
        "{}",
        broken.body()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn lasst_nur_eine_analyse_gleichzeitig_laufen() {
    let claude = FakeClaude::with_replies(vec![reply("[]", 0.04)]).hold_first();
    let (server, _backend) = start(Some(claude.clone())).await;
    let server = Arc::new(server);
    let first = {
        let server = server.clone();
        tokio::spawn(async move { server.post("/api/dispatch/suggestions/analyze", "{}").await })
    };
    claude.wait_for_calls(1).await;
    let second = server.post("/api/dispatch/suggestions/analyze", "{}").await;
    assert_eq!(second.status, 409);
    assert_eq!(
        second.body()["details"],
        json!({ "reason": "analysis-running" })
    );
    claude.release();
    assert_eq!(first.await.expect("erste Analyse").status, 200);
}
