//! Anmeldung, Plugin und Abo-Limits je Account; portiert aus
//! `packages/server/tests/unit/AccountUsage.test.ts` des Referenzbranches.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use nexus_core::accounts::{keychain_service, standard_account, AccountPaths, ClaudeAccount};
use nexus_core::clock::Clock;
use nexus_server::accounts::{no_keychain, AccountUsageOptions, AccountUsageService, Keychain};
use nexus_server::usage::{UsageFetch, UsageReply};
use serde_json::json;

/// Antwort des Endpunkts in allen Tests
fn body() -> serde_json::Value {
    json!({
        "five_hour": { "utilization": 42, "resets_at": "2026-10-28T12:00:00Z" },
        "seven_day": { "utilization": 10, "resets_at": null },
    })
}

/// Inhalt einer Anmeldedatei mit diesem Token
fn credentials(token: &str) -> String {
    json!({ "claudeAiOauth": { "accessToken": token, "subscriptionType": "max" } }).to_string()
}

/// Ein Abruf, der immer dieselbe Antwort gibt und die gesehenen Tokens sammelt
fn fetch() -> (UsageFetch, Arc<Mutex<Vec<String>>>) {
    let tokens: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = tokens.clone();
    let fetch: UsageFetch = Arc::new(move |request| {
        let token = request
            .headers
            .iter()
            .find(|(name, _)| name == "Authorization")
            .map(|(_, value)| value.clone())
            .unwrap_or_default();
        seen.lock().unwrap().push(token);
        Box::pin(async { Ok(UsageReply::new(200, Some(body()))) })
    });
    (fetch, tokens)
}

/// Ein Abruf, der immer scheitert
fn failing_fetch() -> UsageFetch {
    Arc::new(|_| Box::pin(async { Ok(UsageReply::new(500, None)) }))
}

/// Dienst über den Accounts `standard` und `max` in diesem Ordner
fn accounts(
    dir: &Path,
    platform: &str,
    keychain: Keychain,
    fetch: Option<UsageFetch>,
) -> (AccountUsageService, ClaudeAccount) {
    let max = ClaudeAccount {
        id: "max".into(),
        name: "Max".into(),
        config_dir: Some(dir.join("max").to_string_lossy().into_owned()),
        enabled: true,
        fallback: false,
        limit_five_hour: None,
        limit_week: None,
    };
    let accounts = vec![standard_account(), max.clone()];
    let service = AccountUsageService::with_options(
        Arc::new(accounts),
        AccountUsageOptions {
            paths: AccountPaths::new(BTreeMap::new().into_iter().collect(), dir.join("user")),
            platform: platform.to_string(),
            keychain,
            fetch: Some(fetch.unwrap_or_else(|| self::fetch().0)),
            clock: Clock::system(),
            tokens: None,
            token_post: None,
        },
    );
    (service, max)
}

/// Ein Ordner mit `max/plugins` darin
fn temp() -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("nexus-accusage-")
        .tempdir()
        .expect("Ordner");
    std::fs::create_dir_all(dir.path().join("max").join("plugins")).expect("Account-Ordner");
    dir
}

#[tokio::test]
async fn liest_unter_macos_das_token_aus_dem_schluesselbund_des_account_ordners() {
    let dir = temp();
    let wanted = keychain_service(Some(&dir.path().join("max")));
    let asked = Arc::new(AtomicUsize::new(0));
    let counted = asked.clone();
    let keychain: Keychain = Arc::new(move |name: String| {
        counted.fetch_add(1, Ordering::SeqCst);
        let wanted = wanted.clone();
        Box::pin(async move { (name == wanted).then(|| credentials("tok-max")) })
    });
    let (fetch, tokens) = fetch();
    let (service, max) = accounts(dir.path(), "darwin", keychain, Some(fetch));

    let usage = service.usage(&max, false).await;

    assert!(usage.available);
    assert_eq!(usage.plan.as_deref(), Some("max"));
    assert_eq!(usage.five_hour.expect("5 h").percent, 42);
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert_eq!(
        tokens.lock().unwrap().first().map(String::as_str),
        Some("Bearer tok-max")
    );
}

#[tokio::test]
async fn faellt_ohne_schluesselbund_auf_die_datei_zurueck_und_fragt_ihn_nur_unter_macos() {
    let dir = temp();
    std::fs::write(
        dir.path().join("max").join(".credentials.json"),
        credentials("tok-file"),
    )
    .expect("Anmeldedatei");
    let asked = Arc::new(AtomicUsize::new(0));
    let counted = asked.clone();
    let empty: Keychain = Arc::new(move |_name| {
        counted.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { None })
    });
    let (service, max) = accounts(dir.path(), "darwin", empty, None);
    assert!(service.usage(&max, false).await.available);
    assert_eq!(asked.load(Ordering::SeqCst), 1);

    let never = Arc::new(AtomicUsize::new(0));
    let counted = never.clone();
    let full: Keychain = Arc::new(move |_name| {
        counted.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Some(credentials("nie")) })
    });
    let (service, max) = accounts(dir.path(), "linux", full, None);
    assert!(service.usage(&max, false).await.available);
    assert_eq!(never.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn info_liest_die_anmeldung_aus_claude_json_und_das_plugin_aus_der_plugin_liste() {
    let dir = temp();
    std::fs::write(
        dir.path().join("max").join(".claude.json"),
        json!({ "oauthAccount": { "emailAddress": "max@example.com" } }).to_string(),
    )
    .expect("Identität");
    std::fs::write(
        dir.path()
            .join("max")
            .join("plugins")
            .join("installed_plugins.json"),
        json!({ "plugins": { "ai-harness@ai-harness-local": [] } }).to_string(),
    )
    .expect("Plugin-Liste");
    let (service, max) = accounts(dir.path(), "linux", no_keychain(), None);

    let info = service
        .info(&max, Some("2026-10-28T12:00:00Z".into()), false)
        .await;

    assert_eq!(info.id, "max");
    assert!(info.logged_in);
    assert_eq!(info.email.as_deref(), Some("max@example.com"));
    assert!(info.plugin);
    assert_eq!(info.effective_dir, dir.path().join("max").to_string_lossy());
    assert_eq!(info.paused_until.as_deref(), Some("2026-10-28T12:00:00Z"));
    assert_eq!(info.usage.reason.as_deref(), Some("disabled"));
}

#[tokio::test]
async fn states_ohne_usage_fragt_anthropic_nicht_und_der_standard_gilt_als_angemeldet() {
    let dir = temp();
    let (fetch, tokens) = fetch();
    let (service, max) = accounts(dir.path(), "linux", no_keychain(), Some(fetch));

    let states = service
        .states(&[standard_account(), max], &BTreeMap::new(), false)
        .await;

    assert!(tokens.lock().unwrap().is_empty());
    let standard = &states["standard"];
    assert_eq!(standard.logged_in, Some(true));
    assert_eq!(standard.plugin, None);
    assert_eq!(standard.five_hour, None);
    let max = &states["max"];
    assert_eq!(max.logged_in, Some(false));
    assert_eq!(max.plugin, Some(false));
}

#[tokio::test]
async fn states_mit_usage_liefert_die_auslastung_je_fenster() {
    let dir = temp();
    std::fs::write(
        dir.path().join("max").join(".credentials.json"),
        credentials("tok"),
    )
    .expect("Anmeldedatei");
    let (service, max) = accounts(dir.path(), "linux", no_keychain(), None);
    let pauses = BTreeMap::from([("max".to_string(), "2026-10-28T12:00:00Z".to_string())]);

    let states = service
        .states(std::slice::from_ref(&max), &pauses, true)
        .await;

    let state = &states["max"];
    assert_eq!(state.logged_in, Some(true));
    assert_eq!(state.five_hour, Some(42));
    assert_eq!(state.seven_day, Some(10));
    assert_eq!(state.paused_until.as_deref(), Some("2026-10-28T12:00:00Z"));

    // Scheitert nur die Anfrage, bleibt der Account angemeldet und ohne Werte
    let (service, max) = accounts(dir.path(), "linux", no_keychain(), Some(failing_fetch()));
    let states = service.states(&[max], &BTreeMap::new(), true).await;
    let state = &states["max"];
    assert_eq!(state.logged_in, Some(true));
    assert_eq!(state.five_hour, None);
    assert_eq!(state.seven_day, None);
}
