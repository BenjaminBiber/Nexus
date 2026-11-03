//! Erneuert die Anmeldung von Claude Code in `.credentials.json`, wie Claude Code selbst es tut
//! (Refresh-Token gegen neues Zugriffstoken).
//!
//! Nur für Accounts, deren Läufe ein Langzeit-Token nutzen: dann erneuert kaum noch ein Prozess
//! die Anmeldung im Ordner, und ohne Nexus bliebe das Token nach etwa acht Stunden abgelaufen –
//! die Limits ließen sich nicht mehr lesen. Erneuert wird erst, wenn das Token abgelaufen ist
//! (laufende Claude-Code-Sitzungen erneuern vorher), nie zwei Mal gleichzeitig, und die Datei
//! wird unmittelbar vorher neu gelesen: hat ein anderer Prozess schon erneuert, bleibt es dabei.
//! Endpunkt und Client-ID sind nicht dokumentiert; scheitert etwas, bleibt die Datei, wie sie
//! ist, und die Limits melden wie bisher `token-expired`.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};

use nexus_core::clock::Clock;
use nexus_core::locking::AtomicWriter;
use serde_json::{json, Value};

use crate::usage::{RefreshCredentials, UsageReply, REQUEST_TIMEOUT};

/// Endpunkt, an dem Claude Code seine Anmeldung erneuert
pub const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
/// Client-ID von Claude Code
pub const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// Berechtigungen, falls die Anmeldung keine nennt
const DEFAULT_SCOPES: &str =
    "user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";

/// Je Server höchstens eine Erneuerung zur Zeit (auch über mehrere Dienste hinweg)
static REFRESHING: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Eine Anfrage an den Token-Endpunkt
#[derive(Debug, Clone, PartialEq)]
pub struct TokenRequest {
    pub url: String,
    pub body: Value,
}

/// Der Aufruf des Token-Endpunkts; Tests setzen hier eine Attrappe ohne Netz ein
pub type TokenPost = Arc<
    dyn Fn(TokenRequest) -> Pin<Box<dyn Future<Output = Result<UsageReply, String>> + Send>>
        + Send
        + Sync,
>;

/// Darf gerade erneuert werden? (Wird bei jedem Versuch neu gefragt.)
pub type RefreshAllowed = Arc<dyn Fn() -> bool + Send + Sync>;

/// Erneuert die Anmeldung in `file`, sofern `allowed` es zulässt
pub fn file_refresh(
    file: PathBuf,
    post: TokenPost,
    clock: Clock,
    allowed: RefreshAllowed,
) -> RefreshCredentials {
    Arc::new(move || {
        let (file, post, clock, allowed) =
            (file.clone(), post.clone(), clock.clone(), allowed.clone());
        Box::pin(async move {
            if !allowed() {
                return false;
            }
            refresh_file(file, post, clock).await
        })
    })
}

/// Erneuert, falls das Token abgelaufen ist; `true` = danach liegt ein gültiges in der Datei
async fn refresh_file(file: PathBuf, post: TokenPost, clock: Clock) -> bool {
    let _guard = REFRESHING.lock().await;
    let now = || clock.now().timestamp_millis();
    let Some(json) = read_json(file.clone()).await else {
        return false;
    };
    let Some(oauth) = json.get("claudeAiOauth") else {
        return false;
    };
    // Schon erneuert (anderer Prozess, oder die Sperre hat gewartet)
    if oauth
        .get("expiresAt")
        .and_then(Value::as_i64)
        .is_some_and(|at| at > now())
    {
        return true;
    }
    let Some(refresh_token) = oauth
        .get("refreshToken")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
    else {
        return false;
    };
    let scope = oauth
        .get("scopes")
        .and_then(Value::as_array)
        .map(|scopes| {
            scopes
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|scope| !scope.is_empty())
        .unwrap_or_else(|| DEFAULT_SCOPES.to_string());
    let reply = post(TokenRequest {
        url: TOKEN_URL.to_string(),
        body: json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": CLIENT_ID,
            "scope": scope,
        }),
    })
    .await;
    let Ok(UsageReply {
        status: 200,
        body: Some(body),
        ..
    }) = reply
    else {
        return false;
    };
    let (Some(access_token), Some(expires_in)) = (
        body.get("access_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty()),
        body.get("expires_in").and_then(Value::as_i64),
    ) else {
        return false;
    };
    let new_refresh = body
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .unwrap_or(&refresh_token);
    // Das alte Refresh-Token ist jetzt verbraucht: das neue muss in die Datei, auf den Stand
    // unmittelbar vor dem Schreiben (andere Felder pflegt Claude Code)
    let mut latest = read_json(file.clone()).await.unwrap_or(json);
    let Some(fields) = latest
        .get_mut("claudeAiOauth")
        .and_then(Value::as_object_mut)
    else {
        return false;
    };
    fields.insert("accessToken".into(), json!(access_token));
    fields.insert("refreshToken".into(), json!(new_refresh));
    fields.insert("expiresAt".into(), json!(now() + expires_in * 1000));
    if let Some(scope) = body.get("scope").and_then(Value::as_str) {
        fields.insert(
            "scopes".into(),
            json!(scope.split_whitespace().collect::<Vec<_>>()),
        );
    }
    let Ok(text) = serde_json::to_string(&latest) else {
        return false;
    };
    tokio::task::spawn_blocking(move || AtomicWriter::new().write_private(&file, &text).is_ok())
        .await
        .unwrap_or(false)
}

/// Inhalt der Datei als JSON oder `None`
async fn read_json(file: PathBuf) -> Option<Value> {
    tokio::task::spawn_blocking(move || std::fs::read_to_string(file).ok())
        .await
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
}

/// Ein Aufruf, der nie etwas liefert (Vertragsmodus und Tests ohne Netz)
pub fn no_post() -> TokenPost {
    Arc::new(|_request: TokenRequest| Box::pin(async { Err("ohne Netz".to_string()) }))
}

/// Der echte Aufruf über HTTPS mit Zeitlimit
pub fn http_post() -> TokenPost {
    Arc::new(|request: TokenRequest| {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|error| error.to_string())?;
            let response = client
                .post(&request.url)
                .header("User-Agent", "ai-harness")
                .json(&request.body)
                .send()
                .await
                .map_err(|error| error.to_string())?;
            let status = response.status().as_u16();
            Ok(UsageReply::new(status, response.json::<Value>().await.ok()))
        })
    })
}
