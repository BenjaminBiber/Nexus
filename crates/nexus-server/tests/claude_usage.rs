//! Die Abo-Limits von Claude: Anmeldedatei, Zwischenspeicher, Fehlergründe und die Route
//! `GET /api/claude-usage`; portiert aus `packages/server/tests/unit/ClaudeUsage.test.ts`.

mod common;

use std::sync::atomic::{AtomicI64, AtomicU16, AtomicUsize, Ordering};
use std::sync::Arc;

use common::{TestServer, TestServerOptions};
use nexus_core::clock::Clock;
use nexus_core::settings::{UserSettings, UserSettingsStore};
use nexus_server::usage::{ClaudeUsageService, UsageFetch, UsageReply, DEFAULT_TTL_MS};
use serde_json::{json, Value};

/// Zeitpunkt der Tests
const NOW: &str = "2026-10-20T10:00:00Z";

/// Die Antwort des Endpunkts
fn body() -> Value {
    json!({
        "five_hour": { "utilization": 79.4, "resets_at": "2026-10-20T11:19:59+00:00" },
        "seven_day": { "utilization": 29, "resets_at": "2026-10-22T13:59:59+00:00" },
    })
}

/// Jetzt in Millisekunden
fn now_ms() -> i64 {
    NOW.parse::<chrono::DateTime<chrono::Utc>>()
        .expect("Zeitpunkt")
        .timestamp_millis()
}

/// Eine Uhr auf dem Millisekunden-Zähler (Tests stellen die Zeit vor)
fn clock(at: Arc<AtomicI64>) -> Clock {
    Clock::from_fn(move || {
        chrono::DateTime::from_timestamp_millis(at.load(Ordering::SeqCst)).expect("Zeitpunkt")
    })
}

/// Schreibt eine Anmeldedatei mit diesen Feldern
fn login(file: &std::path::Path, oauth: Value) {
    let mut fields = json!({
        "accessToken": "tok-secret",
        "expiresAt": now_ms() + 3_600_000,
        "subscriptionType": "team",
    });
    for (name, value) in oauth.as_object().into_iter().flatten() {
        fields[name] = value.clone();
    }
    std::fs::write(file, json!({ "claudeAiOauth": fields }).to_string())
        .expect("Anmeldedatei schreiben");
}

/// Ein Abruf, der diesen Status und Körper liefert und seine Aufrufe zählt
fn answers(status: u16, body: Value) -> (UsageFetch, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let fetch: UsageFetch = Arc::new(move |request| {
        counted.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.url, nexus_server::usage::USAGE_URL);
        assert_eq!(
            request.headers,
            vec![
                ("Authorization".to_string(), "Bearer tok-secret".to_string()),
                ("anthropic-beta".to_string(), "oauth-2025-04-20".to_string()),
                ("User-Agent".to_string(), "ai-harness".to_string()),
            ]
        );
        let body = body.clone();
        Box::pin(async move { Ok(UsageReply::new(status, Some(body))) })
    });
    (fetch, calls)
}

#[tokio::test]
async fn liest_beide_fenster_und_schickt_das_token_nur_als_bearer_an_den_endpunkt() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let (fetch, calls) = answers(200, body());
    let service = ClaudeUsageService::with(
        file,
        fetch,
        Clock::fixed(NOW.parse().unwrap()),
        DEFAULT_TTL_MS,
    );
    let usage = service.current(false).await;
    assert_eq!(
        serde_json::to_value(&usage).unwrap(),
        json!({
            "available": true, "reason": null, "plan": "team",
            "fetchedAt": "2026-10-20T10:00:00.000Z",
            "fiveHour": { "percent": 79, "resetsAt": "2026-10-20T11:19:59+00:00" },
            "sevenDay": { "percent": 29, "resetsAt": "2026-10-22T13:59:59+00:00" },
        })
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!serde_json::to_string(&usage)
        .unwrap()
        .contains("tok-secret"));
}

#[tokio::test]
async fn fragt_innerhalb_der_gultigkeit_nur_einmal_und_ruft_gleichzeitige_anfragen_zusammen() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let (fetch, calls) = answers(200, body());
    let service = Arc::new(ClaudeUsageService::with(
        file,
        fetch,
        Clock::fixed(NOW.parse().unwrap()),
        DEFAULT_TTL_MS,
    ));
    let first = service.clone();
    let second = service.clone();
    let (left, right) = tokio::join!(
        tokio::spawn(async move { first.current(false).await }),
        tokio::spawn(async move { second.current(false).await })
    );
    assert!(left.unwrap().available);
    assert!(right.unwrap().available);
    service.current(false).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn meldet_fehlende_anmeldung_abgelaufenes_token_und_fehler_des_endpunkts_ohne_werte() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    let service = |fetch: UsageFetch| {
        ClaudeUsageService::with(
            file.clone(),
            fetch,
            Clock::fixed(NOW.parse().unwrap()),
            DEFAULT_TTL_MS,
        )
    };

    let (fetch, never) = answers(200, body());
    let usage = service(fetch.clone()).current(false).await;
    assert_eq!(usage.reason.as_deref(), Some("no-login"));
    assert!(!usage.available);

    login(&file, json!({ "expiresAt": now_ms() - 1 }));
    let usage = service(fetch.clone()).current(false).await;
    assert_eq!(usage.reason.as_deref(), Some("token-expired"));
    assert_eq!(usage.plan.as_deref(), Some("team"));
    assert_eq!(never.load(Ordering::SeqCst), 0);

    login(&file, json!({}));
    let (unauthorized, _) = answers(401, body());
    assert_eq!(
        service(unauthorized).current(false).await.reason.as_deref(),
        Some("token-expired")
    );
    let (broken, _) = answers(500, body());
    assert_eq!(
        service(broken).current(false).await.reason.as_deref(),
        Some("request-failed")
    );
    let offline: UsageFetch = Arc::new(|_| Box::pin(async { Err("offline".to_string()) }));
    assert_eq!(
        service(offline).current(false).await.reason.as_deref(),
        Some("request-failed")
    );
}

#[tokio::test]
async fn merkt_sich_einen_fehlschlag_nur_kurz_und_fragt_danach_wieder_an() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let at = Arc::new(AtomicI64::new(now_ms()));
    let status = Arc::new(AtomicU16::new(500));
    let calls = Arc::new(AtomicUsize::new(0));
    let (answer, counted) = (status.clone(), calls.clone());
    let fetch: UsageFetch = Arc::new(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        let status = answer.load(Ordering::SeqCst);
        Box::pin(async move { Ok(UsageReply::new(status, Some(body()))) })
    });
    let service = ClaudeUsageService::with(file, fetch, clock(at.clone()), DEFAULT_TTL_MS);
    assert_eq!(
        service.current(false).await.reason.as_deref(),
        Some("request-failed")
    );
    at.fetch_add(5_000, Ordering::SeqCst);
    status.store(200, Ordering::SeqCst);
    assert!(service.current(false).await.available);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn fragt_nach_429_mindestens_fuenf_minuten_nicht_neu_auch_nicht_frisch() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let at = Arc::new(AtomicI64::new(now_ms()));
    let status = Arc::new(AtomicU16::new(429));
    let retry_after = Arc::new(std::sync::Mutex::new(Some("0".to_string())));
    let calls = Arc::new(AtomicUsize::new(0));
    let (answer, header, counted) = (status.clone(), retry_after.clone(), calls.clone());
    let fetch: UsageFetch = Arc::new(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        let status = answer.load(Ordering::SeqCst);
        let retry_after = header.lock().unwrap().clone();
        Box::pin(async move {
            Ok(UsageReply {
                retry_after,
                ..UsageReply::new(status, Some(body()))
            })
        })
    });
    let service = ClaudeUsageService::with(file, fetch, clock(at.clone()), DEFAULT_TTL_MS);

    let first = service.current(false).await;
    assert_eq!(first.reason.as_deref(), Some("rate-limited"));
    assert_eq!(first.plan.as_deref(), Some("team"));

    // Auch `fresh` fragt innerhalb der Sperre nicht neu
    at.fetch_add(4 * 60_000, Ordering::SeqCst);
    assert_eq!(
        service.current(true).await.reason.as_deref(),
        Some("rate-limited")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // Nach fünf Minuten fragt er wieder – und ein langes Retry-After sperrt länger
    *retry_after.lock().unwrap() = Some("900".to_string());
    at.fetch_add(61_000, Ordering::SeqCst);
    assert_eq!(
        service.current(false).await.reason.as_deref(),
        Some("rate-limited")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    status.store(200, Ordering::SeqCst);
    at.fetch_add(14 * 60_000, Ordering::SeqCst);
    assert_eq!(
        service.current(false).await.reason.as_deref(),
        Some("rate-limited")
    );
    at.fetch_add(61_000, Ordering::SeqCst);
    assert!(service.current(false).await.available);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn fresh_umgeht_den_zwischenspeicher_einer_erfolgreichen_antwort() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let (fetch, calls) = answers(200, body());
    let service = ClaudeUsageService::with(
        file,
        fetch,
        Clock::fixed(NOW.parse().unwrap()),
        DEFAULT_TTL_MS,
    );

    service.current(false).await;
    service.current(false).await;
    service.current(true).await;

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn stempelt_den_zwischenspeicher_erst_nach_dem_laden() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let at = Arc::new(AtomicI64::new(now_ms()));
    let calls = Arc::new(AtomicUsize::new(0));
    let (ticking, counted) = (at.clone(), calls.clone());
    // Die Anfrage selbst braucht Zeit; die Gültigkeit darf erst nach ihrem Abschluss beginnen
    let fetch: UsageFetch = Arc::new(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        ticking.fetch_add(50_000, Ordering::SeqCst);
        Box::pin(async { Ok(UsageReply::new(200, Some(body()))) })
    });
    let service = ClaudeUsageService::with(file, fetch, clock(at.clone()), DEFAULT_TTL_MS);
    service.current(false).await;
    // 20 s nach dem Abschluss, also noch innerhalb der 60 s
    at.fetch_add(20_000, Ordering::SeqCst);
    service.current(false).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn lasst_ein_fehlendes_fenster_leer() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let (fetch, _) = answers(
        200,
        json!({ "five_hour": { "utilization": 3, "resets_at": null }, "seven_day": null }),
    );
    let usage = ClaudeUsageService::with(
        file,
        fetch,
        Clock::fixed(NOW.parse().unwrap()),
        DEFAULT_TTL_MS,
    )
    .current(false)
    .await;
    assert!(usage.available);
    assert_eq!(
        serde_json::to_value(&usage.five_hour).unwrap(),
        json!({ "percent": 3, "resetsAt": null })
    );
    assert_eq!(usage.seven_day, None);
}

/// Ein Abruf, der jedes Token außer `good` mit 401 abweist und die gesehenen Tokens sammelt
fn accepts(good: &'static str) -> (UsageFetch, Arc<std::sync::Mutex<Vec<String>>>) {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let tokens = seen.clone();
    let fetch: UsageFetch = Arc::new(move |request| {
        let bearer = request
            .headers
            .iter()
            .find(|(name, _)| name == "Authorization")
            .map(|(_, value)| value.trim_start_matches("Bearer ").to_string())
            .unwrap_or_default();
        let status = if bearer == good { 200 } else { 401 };
        tokens.lock().unwrap().push(bearer);
        Box::pin(async move { Ok(UsageReply::new(status, Some(body()))) })
    });
    (fetch, seen)
}

#[tokio::test]
async fn liest_eine_halb_geschriebene_anmeldung_noch_einmal_und_nimmt_sonst_die_letzte() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    // Noch nie gelesen: vorübergehender Fehler statt „nicht angemeldet“
    std::fs::write(&file, "{\"claudeAiOauth\": {\"accessTo").unwrap();
    let at = Arc::new(AtomicI64::new(now_ms()));
    let (fetch, calls) = answers(200, body());
    let service = ClaudeUsageService::with(file.clone(), fetch, clock(at.clone()), 1_000);
    assert_eq!(
        service.current(false).await.reason.as_deref(),
        Some("request-failed")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    login(&file, json!({}));
    at.fetch_add(5_000, Ordering::SeqCst);
    assert!(service.current(false).await.available);

    // Claude Code schreibt gerade: die zuletzt gelesene Anmeldung gilt weiter
    std::fs::write(&file, "").unwrap();
    at.fetch_add(5_000, Ordering::SeqCst);
    assert!(service.current(false).await.available);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn fragt_nach_401_mit_dem_token_noch_einmal_das_ein_anderer_prozess_erneuert_hat() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({ "accessToken": "tok-old" }));
    let renewed = file.clone();
    let (inner, seen) = accepts("tok-new");
    // Während der ersten Anfrage erneuert Claude Code sein Token
    let fetch: UsageFetch = Arc::new(move |request| {
        login(&renewed, json!({ "accessToken": "tok-new" }));
        inner(request)
    });
    let service = ClaudeUsageService::with(
        file,
        fetch,
        Clock::fixed(NOW.parse().unwrap()),
        DEFAULT_TTL_MS,
    );
    assert!(service.current(false).await.available);
    assert_eq!(*seen.lock().unwrap(), vec!["tok-old", "tok-new"]);
}

#[tokio::test]
async fn wartet_nach_fehlschlaegen_in_folge_immer_laenger_bis_zu_einer_minute() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let at = Arc::new(AtomicI64::new(now_ms()));
    let (fetch, calls) = answers(500, body());
    let service = ClaudeUsageService::with(file, fetch, clock(at.clone()), DEFAULT_TTL_MS);
    let mut asked_after = Vec::new();
    let mut waited = 0;
    for _ in 0..80 {
        let before = calls.load(Ordering::SeqCst);
        service.current(false).await;
        if calls.load(Ordering::SeqCst) > before {
            asked_after.push(waited);
            waited = 0;
        }
        at.fetch_add(5_000, Ordering::SeqCst);
        waited += 5_000;
    }
    assert_eq!(
        &asked_after[..7],
        &[0, 5_000, 10_000, 20_000, 40_000, 60_000, 60_000]
    );
}

/// Ein Account-Dienst, dessen Standard-Account seine Anmeldung aus dieser Datei liest
fn standard_usage(
    file: std::path::PathBuf,
    fetch: UsageFetch,
) -> Arc<nexus_server::accounts::AccountUsageService> {
    let dir = file.parent().expect("Ordner der Anmeldung").to_path_buf();
    let paths = nexus_core::accounts::AccountPaths::new(
        [(
            "CLAUDE_CONFIG_DIR".to_string(),
            dir.to_string_lossy().into_owned(),
        )]
        .into_iter()
        .collect(),
        dir,
    );
    Arc::new(nexus_server::accounts::AccountUsageService::with_options(
        Arc::new(vec![nexus_core::accounts::standard_account()]),
        nexus_server::accounts::AccountUsageOptions {
            paths,
            platform: "linux".to_string(),
            keychain: nexus_server::accounts::no_keychain(),
            fetch: Some(fetch),
            clock: Clock::fixed(NOW.parse().unwrap()),
            tokens: None,
            token_post: None,
        },
    ))
}

/// Server, dessen Dienst der Abo-Limits aus dieser Anmeldedatei liest
fn server_with_usage(file: std::path::PathBuf) -> (TestServer, Arc<AtomicUsize>) {
    let (fetch, calls) = answers(200, body());
    let server = TestServer::with(TestServerOptions {
        account_usage: Some(standard_usage(file, fetch)),
        ..TestServerOptions::default()
    });
    (server, calls)
}

#[tokio::test]
async fn bleibt_ohne_eingeschaltete_einstellung_bei_disabled_und_fragt_die_quelle_nie() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let (server, calls) = server_with_usage(file);
    let reply = server.get("/api/claude-usage").await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    assert_eq!(
        reply.json(),
        Some(json!({
            "available": false, "reason": "disabled", "plan": null,
            "fiveHour": null, "sevenDay": null, "fetchedAt": null,
        }))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn liefert_nach_dem_einschalten_was_die_quelle_meldet() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    login(&file, json!({}));
    let (server, calls) = server_with_usage(file);
    UserSettingsStore::new(server.home.clone())
        .write(&UserSettings {
            claude_usage: true,
            ..UserSettings::default()
        })
        .expect("Einstellungen schreiben");

    let reply = server.get("/api/claude-usage").await;
    let usage = reply.json().expect("JSON");
    assert_eq!(usage["available"], true);
    assert_eq!(usage["plan"], "team");
    assert_eq!(usage["fiveHour"]["percent"], 79);

    // Nach dem Ausschalten wieder „disabled“, ohne die Quelle erneut zu fragen
    UserSettingsStore::new(server.home.clone())
        .write(&UserSettings::default())
        .expect("Einstellungen schreiben");
    let reply = server.get("/api/claude-usage").await;
    assert_eq!(reply.json().expect("JSON")["reason"], "disabled");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
