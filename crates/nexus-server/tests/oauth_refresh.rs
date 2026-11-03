//! Nexus erneuert die Anmeldung eines Accounts selbst, wenn dessen Läufe ein Langzeit-Token
//! nutzen: nur abgelaufene Tokens, nur mit Erlaubnis, ohne andere Felder zu verlieren.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use nexus_core::accounts::{standard_account, AccountPaths, AccountTokenStore};
use nexus_core::clock::Clock;
use nexus_core::HarnessHome;
use nexus_server::accounts::{no_keychain, AccountUsageOptions, AccountUsageService};
use nexus_server::oauth_refresh::{file_refresh, TokenPost, TokenRequest, CLIENT_ID, TOKEN_URL};
use nexus_server::usage::{ClaudeUsageService, UsageFetch, UsageReply, DEFAULT_TTL_MS};
use serde_json::{json, Value};

/// Zeitpunkt der Tests
const NOW: &str = "2026-10-20T10:00:00Z";

fn now_ms() -> i64 {
    NOW.parse::<chrono::DateTime<chrono::Utc>>()
        .unwrap()
        .timestamp_millis()
}

fn clock() -> Clock {
    Clock::fixed(NOW.parse().unwrap())
}

/// Anmeldedatei, deren Token seit einer Minute abgelaufen ist
fn expired_login(file: &std::path::Path) {
    std::fs::write(
        file,
        json!({
            "claudeAiOauth": {
                "accessToken": "tok-old",
                "refreshToken": "ref-old",
                "expiresAt": now_ms() - 60_000,
                "scopes": ["user:inference", "user:profile"],
                "subscriptionType": "max",
            },
            "mcpOAuth": { "keep": true },
        })
        .to_string(),
    )
    .unwrap();
}

fn read(file: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

/// Token-Endpunkt, der neue Tokens ausgibt und die Anfragen sammelt
fn token_endpoint(status: u16) -> (TokenPost, Arc<Mutex<Vec<TokenRequest>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let requests = seen.clone();
    let post: TokenPost = Arc::new(move |request| {
        requests.lock().unwrap().push(request);
        Box::pin(async move {
            Ok(UsageReply::new(
                status,
                Some(json!({
                    "access_token": "tok-new",
                    "refresh_token": "ref-new",
                    "expires_in": 28_800,
                    "scope": "user:inference user:profile",
                })),
            ))
        })
    });
    (post, seen)
}

fn allowed(flag: bool) -> Arc<dyn Fn() -> bool + Send + Sync> {
    Arc::new(move || flag)
}

#[tokio::test]
async fn erneuert_ein_abgelaufenes_token_und_behaelt_die_uebrigen_felder() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    expired_login(&file);
    let (post, seen) = token_endpoint(200);

    assert!(file_refresh(file.clone(), post, clock(), allowed(true))().await);

    let requests = seen.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url, TOKEN_URL);
    assert_eq!(
        requests[0].body,
        json!({
            "grant_type": "refresh_token",
            "refresh_token": "ref-old",
            "client_id": CLIENT_ID,
            "scope": "user:inference user:profile",
        })
    );
    let saved = read(&file);
    let oauth = &saved["claudeAiOauth"];
    assert_eq!(oauth["accessToken"], "tok-new");
    assert_eq!(oauth["refreshToken"], "ref-new");
    assert_eq!(oauth["expiresAt"], now_ms() + 28_800_000);
    assert_eq!(oauth["subscriptionType"], "max");
    assert_eq!(saved["mcpOAuth"], json!({ "keep": true }));
}

#[tokio::test]
async fn erneuert_nicht_ohne_erlaubnis_ein_gueltiges_token_oder_nach_einem_fehler() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    expired_login(&file);
    let before = std::fs::read_to_string(&file).unwrap();

    let (post, seen) = token_endpoint(200);
    assert!(!file_refresh(file.clone(), post, clock(), allowed(false))().await);
    assert!(seen.lock().unwrap().is_empty());

    let (rejecting, _) = token_endpoint(400);
    assert!(!file_refresh(file.clone(), rejecting, clock(), allowed(true))().await);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);

    // Ein anderer Prozess hat schon erneuert: nichts zu tun
    let mut fresh = read(&file);
    fresh["claudeAiOauth"]["expiresAt"] = json!(now_ms() + 60_000);
    std::fs::write(&file, fresh.to_string()).unwrap();
    let (post, seen) = token_endpoint(200);
    assert!(file_refresh(file.clone(), post, clock(), allowed(true))().await);
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn die_limits_erneuern_ein_abgelaufenes_token_und_fragen_dann_damit() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".credentials.json");
    expired_login(&file);
    let (post, _) = token_endpoint(200);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let tokens = seen.clone();
    let fetch: UsageFetch = Arc::new(move |request| {
        tokens.lock().unwrap().push(request.headers[0].1.clone());
        Box::pin(async { Ok(UsageReply::new(200, Some(json!({})))) })
    });
    let service =
        ClaudeUsageService::with(file, fetch, clock(), DEFAULT_TTL_MS).with_refresh(file_refresh(
            dir.path().join(".credentials.json"),
            post,
            clock(),
            allowed(true),
        ));

    assert!(service.current(false).await.available);
    assert_eq!(*seen.lock().unwrap(), vec!["Bearer tok-new"]);
}

#[tokio::test]
async fn der_account_dienst_erneuert_nur_fuer_accounts_mit_langzeit_token() {
    let dir = tempfile::tempdir().unwrap();
    let claude = dir.path().join("claude");
    std::fs::create_dir_all(&claude).unwrap();
    let file = claude.join(".credentials.json");
    expired_login(&file);
    let home = HarnessHome::at(dir.path().join("nexus"));
    let tokens = AccountTokenStore::new(home);
    let posted = Arc::new(AtomicUsize::new(0));
    let counted = posted.clone();
    let (inner, _) = token_endpoint(200);
    let post: TokenPost = Arc::new(move |request| {
        counted.fetch_add(1, Ordering::SeqCst);
        inner(request)
    });
    let fetched = Arc::new(AtomicBool::new(false));
    let marker = fetched.clone();
    let fetch: UsageFetch = Arc::new(move |_| {
        marker.store(true, Ordering::SeqCst);
        Box::pin(async { Ok(UsageReply::new(200, Some(json!({})))) })
    });
    let paths = AccountPaths::new(
        BTreeMap::from([(
            "CLAUDE_CONFIG_DIR".to_string(),
            claude.to_string_lossy().into_owned(),
        )])
        .into_iter()
        .collect(),
        dir.path().join("user"),
    );
    let service = AccountUsageService::with_options(
        Arc::new(vec![standard_account()]),
        AccountUsageOptions {
            paths,
            platform: "win32".to_string(),
            keychain: no_keychain(),
            fetch: Some(fetch),
            clock: clock(),
            tokens: Some(tokens.clone()),
            token_post: Some(post),
        },
    );
    let standard = standard_account();

    // Ohne Langzeit-Token: abgelaufen bleibt abgelaufen, die Datei unberührt
    let usage = service.usage(&standard, true).await;
    assert_eq!(usage.reason.as_deref(), Some("token-expired"));
    assert_eq!(posted.load(Ordering::SeqCst), 0);
    let infos = service.infos(&BTreeMap::new(), false).await;
    assert!(!infos[0].run_token);

    tokens.set("standard", "sk-ant-oat01-run").unwrap();
    let usage = service.usage(&standard, true).await;
    assert!(usage.available);
    assert_eq!(posted.load(Ordering::SeqCst), 1);
    assert!(fetched.load(Ordering::SeqCst));
    assert_eq!(read(&file)["claudeAiOauth"]["accessToken"], "tok-new");
    let infos = service.infos(&BTreeMap::new(), false).await;
    assert!(infos[0].run_token);
}
