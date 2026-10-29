//! Die Abo-Limits von Claude (Fenster von fünf Stunden und Woche), wie sie `/usage` in
//! Claude Code zeigt. Portiert aus `packages/server/src/usage/ClaudeUsageService.ts`.
//!
//! Der OAuth-Endpunkt von Anthropic ist nicht dokumentiert und kann sich ändern; Fehler
//! bleiben darum leise (`available: false`) und gelten nur kurz, damit ein Aussetzer die
//! Anzeige nicht minutenlang leer lässt. Das Token wird nur gelesen, nie erneuert (das
//! übernimmt Claude Code selbst) und nie ausgegeben.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nexus_core::clock::Clock;
use serde::{Deserialize, Serialize};

/// Endpunkt der Abo-Limits
pub const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// Zeitlimit der Anfrage
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// So lange gilt eine erfolgreiche Antwort
pub const DEFAULT_TTL_MS: i64 = 60_000;
/// Fehlschläge gelten nur kurz: erneuert Claude Code gleich danach sein Token, soll der
/// nächste Aufruf es schon sehen
pub const FAILURE_TTL_MS: i64 = 5_000;

/// Auslastung eines Limit-Fensters; `resetsAt` ist ein ISO-Zeitpunkt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindowDto {
    pub percent: i64,
    pub resets_at: Option<String>,
}

/// Antwort von `GET /api/claude-usage`; ohne `available` erklärt `reason`, warum keine Werte
/// da sind
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUsageDto {
    pub available: bool,
    pub reason: Option<String>,
    pub plan: Option<String>,
    pub five_hour: Option<UsageWindowDto>,
    pub seven_day: Option<UsageWindowDto>,
    pub fetched_at: Option<String>,
}

impl ClaudeUsageDto {
    /// Keine Werte, mit Grund (`disabled`, `no-login`, `token-expired`, `request-failed`)
    pub fn unavailable(reason: &str, plan: Option<String>) -> Self {
        Self {
            available: false,
            reason: Some(reason.to_string()),
            plan,
            five_hour: None,
            seven_day: None,
            fetched_at: None,
        }
    }
}

/// Eine Anfrage an den Endpunkt; die Köpfe stehen in der Reihenfolge von TypeScript
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

/// Die Antwort des Endpunkts
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageReply {
    pub status: u16,
    pub body: Option<serde_json::Value>,
}

impl UsageReply {
    /// Hat der Endpunkt mit Erfolg geantwortet?
    pub fn ok(&self) -> bool {
        self.status < 400
    }
}

/// Der Abruf des Endpunkts; Tests setzen hier eine Attrappe ohne Netz ein
pub type UsageFetch = Arc<
    dyn Fn(UsageRequest) -> Pin<Box<dyn Future<Output = Result<UsageReply, String>> + Send>>
        + Send
        + Sync,
>;

/// Zwischenspeicher: Wert samt dem Zeitpunkt, zu dem er geladen wurde
struct Cached {
    at: i64,
    value: ClaudeUsageDto,
}

/// Liest die Abo-Limits über den OAuth-Endpunkt von Anthropic und hält sie kurz vor
pub struct ClaudeUsageService {
    file: PathBuf,
    fetch: UsageFetch,
    clock: Clock,
    ttl_ms: i64,
    cached: Mutex<Option<Cached>>,
    /// Gleichzeitige Aufrufe teilen eine Anfrage: der zweite wartet und findet sie im
    /// Zwischenspeicher
    loading: tokio::sync::Mutex<()>,
}

impl ClaudeUsageService {
    /// Dienst über der Anmeldedatei von Claude Code mit echtem Abruf
    pub fn new(clock: Clock) -> Self {
        Self::with(credentials_file(), http_fetch(), clock, DEFAULT_TTL_MS)
    }

    /// Dienst mit eigener Anmeldedatei, eigenem Abruf und eigener Gültigkeit (Tests)
    pub fn with(file: PathBuf, fetch: UsageFetch, clock: Clock, ttl_ms: i64) -> Self {
        Self {
            file,
            fetch,
            clock,
            ttl_ms,
            cached: Mutex::new(None),
            loading: tokio::sync::Mutex::new(()),
        }
    }

    /// Aktuelle Werte (aus dem Zwischenspeicher, solange er frisch ist)
    pub async fn current(&self) -> ClaudeUsageDto {
        if let Some(value) = self.fresh() {
            return value;
        }
        let _guard = self.loading.lock().await;
        // Hat der Aufruf davor gerade geladen, ist der Wert jetzt da
        if let Some(value) = self.fresh() {
            return value;
        }
        let value = self.load().await;
        // Erst nach dem Laden stempeln: sonst zählt die Laufzeit der Anfrage (bis zum
        // Zeitlimit) gegen die Gültigkeit
        *self.cached.lock().expect("Sperre der Abo-Limits") = Some(Cached {
            at: self.now_ms(),
            value: value.clone(),
        });
        value
    }

    /// Der Wert des Zwischenspeichers, solange er gilt
    fn fresh(&self) -> Option<ClaudeUsageDto> {
        let cached = self.cached.lock().expect("Sperre der Abo-Limits");
        let cached = cached.as_ref()?;
        let ttl = if cached.value.available {
            self.ttl_ms
        } else {
            FAILURE_TTL_MS
        };
        (self.now_ms() - cached.at < ttl).then(|| cached.value.clone())
    }

    /// Liest die Anmeldung und fragt den Endpunkt
    async fn load(&self) -> ClaudeUsageDto {
        let Some(login) = self.read_login() else {
            return ClaudeUsageDto::unavailable("no-login", None);
        };
        if login.expires_at.is_some_and(|at| at <= self.now_ms()) {
            return ClaudeUsageDto::unavailable("token-expired", login.plan);
        }
        let request = UsageRequest {
            url: USAGE_URL.to_string(),
            headers: vec![
                ("Authorization".into(), format!("Bearer {}", login.token)),
                ("anthropic-beta".into(), "oauth-2025-04-20".into()),
                ("User-Agent".into(), "ai-harness".into()),
            ],
        };
        let reply = match (self.fetch)(request).await {
            Ok(reply) => reply,
            Err(_) => return ClaudeUsageDto::unavailable("request-failed", login.plan),
        };
        if !reply.ok() {
            let reason = if reply.status == 401 {
                "token-expired"
            } else {
                "request-failed"
            };
            return ClaudeUsageDto::unavailable(reason, login.plan);
        }
        let body = reply.body.unwrap_or(serde_json::Value::Null);
        ClaudeUsageDto {
            available: true,
            reason: None,
            plan: login.plan,
            five_hour: window(body.get("five_hour")),
            seven_day: window(body.get("seven_day")),
            fetched_at: Some(self.clock.iso()),
        }
    }

    /// Token, Ablauf (ms) und Abo-Art aus der Anmeldedatei; `None` ohne Anmeldung
    fn read_login(&self) -> Option<Login> {
        let text = std::fs::read_to_string(&self.file).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        let oauth = json.get("claudeAiOauth")?;
        let token = oauth
            .get("accessToken")
            .and_then(serde_json::Value::as_str)
            .filter(|token| !token.is_empty())?;
        Some(Login {
            token: token.to_string(),
            expires_at: oauth.get("expiresAt").and_then(serde_json::Value::as_i64),
            plan: oauth
                .get("subscriptionType")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        })
    }

    /// Jetzt in Millisekunden seit der Epoche
    fn now_ms(&self) -> i64 {
        self.clock.now().timestamp_millis()
    }
}

/// Die gelesene Anmeldung
struct Login {
    token: String,
    expires_at: Option<i64>,
    plan: Option<String>,
}

/// Ein Limit-Fenster der Antwort; ohne brauchbare Auslastung bleibt es leer
fn window(raw: Option<&serde_json::Value>) -> Option<UsageWindowDto> {
    let raw = raw?;
    let utilization = raw.get("utilization").and_then(serde_json::Value::as_f64)?;
    if !utilization.is_finite() {
        return None;
    }
    Some(UsageWindowDto {
        percent: (utilization.round() as i64).max(0),
        resets_at: raw
            .get("resets_at")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
    })
}

/// Anmeldedatei von Claude Code: `CLAUDE_CONFIG_DIR` bzw. `~/.claude`
pub fn credentials_file() -> PathBuf {
    let dir = std::env::var("CLAUDE_CONFIG_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".claude"));
    dir.join(".credentials.json")
}

/// Der Benutzerordner des Rechners (wie `os.homedir()`)
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// Der echte Abruf über HTTPS mit Zeitlimit
pub fn http_fetch() -> UsageFetch {
    Arc::new(|request: UsageRequest| {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|error| error.to_string())?;
            let mut call = client.get(&request.url);
            for (name, value) in &request.headers {
                call = call.header(name, value);
            }
            let response = call.send().await.map_err(|error| error.to_string())?;
            let status = response.status().as_u16();
            Ok(UsageReply {
                status,
                body: response.json::<serde_json::Value>().await.ok(),
            })
        })
    })
}
