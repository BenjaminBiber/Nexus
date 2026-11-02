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
/// Nach 429 (zu viele Anfragen) fragt der Dienst mindestens so lange nicht neu – länger, wenn
/// `Retry-After` es verlangt
pub const RATE_LIMIT_TTL_MS: i64 = 5 * 60_000;

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
    /// Kopf `Retry-After` einer Antwort 429 (Sekunden oder HTTP-Datum)
    pub retry_after: Option<String>,
}

impl UsageReply {
    /// Antwort mit Status und Körper, ohne `Retry-After`
    pub fn new(status: u16, body: Option<serde_json::Value>) -> Self {
        Self {
            status,
            body,
            retry_after: None,
        }
    }

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

/// Liest den Inhalt der Anmeldung (Datei oder macOS-Schlüsselbund); `None` = keine Anmeldung
pub type ReadCredentials =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Option<String>> + Send>> + Send + Sync>;

/// Liest den Inhalt einer Anmeldedatei
pub fn file_credentials(file: PathBuf) -> ReadCredentials {
    Arc::new(move || {
        let file = file.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || std::fs::read_to_string(file).ok())
                .await
                .ok()
                .flatten()
        })
    })
}

/// Zwischenspeicher: Wert samt dem Zeitpunkt, zu dem er geladen wurde, und seiner Gültigkeit
struct Cached {
    at: i64,
    ttl: i64,
    value: ClaudeUsageDto,
}

/// Eine geladene Antwort samt ihrer Gültigkeit im Zwischenspeicher
struct Loaded {
    value: ClaudeUsageDto,
    ttl: i64,
}

/// Liest die Abo-Limits über den OAuth-Endpunkt von Anthropic und hält sie kurz vor
pub struct ClaudeUsageService {
    read_credentials: ReadCredentials,
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
        Self::with_credentials(file_credentials(file), fetch, clock, ttl_ms)
    }

    /// Dienst mit eigener Quelle der Anmeldung (Schlüsselbund oder Datei)
    pub fn with_credentials(
        read_credentials: ReadCredentials,
        fetch: UsageFetch,
        clock: Clock,
        ttl_ms: i64,
    ) -> Self {
        Self {
            read_credentials,
            fetch,
            clock,
            ttl_ms,
            cached: Mutex::new(None),
            loading: tokio::sync::Mutex::new(()),
        }
    }

    /// Aktuelle Werte (aus dem Zwischenspeicher, solange er frisch ist); `fresh` fragt neu
    /// (z. B. gleich nach einem Nutzungslimit, wenn der Zwischenspeicher das volle Fenster
    /// noch nicht kennt) – nur nicht, solange der Endpunkt nach 429 gesperrt ist
    pub async fn current(&self, fresh: bool) -> ClaudeUsageDto {
        if let Some(value) = self.fresh(fresh) {
            return value;
        }
        let _guard = self.loading.lock().await;
        // Hat der Aufruf davor gerade geladen, ist der Wert jetzt da
        if let Some(value) = self.fresh(fresh) {
            return value;
        }
        let loaded = self.load().await;
        // Erst nach dem Laden stempeln: sonst zählt die Laufzeit der Anfrage (bis zum
        // Zeitlimit) gegen die Gültigkeit
        *self.cached.lock().expect("Sperre der Abo-Limits") = Some(Cached {
            at: self.now_ms(),
            ttl: loaded.ttl,
            value: loaded.value.clone(),
        });
        loaded.value
    }

    /// Der Wert des Zwischenspeichers, solange er gilt; `fresh` übergeht ihn, außer nach 429
    fn fresh(&self, fresh: bool) -> Option<ClaudeUsageDto> {
        let cached = self.cached.lock().expect("Sperre der Abo-Limits");
        let cached = cached.as_ref()?;
        if fresh && cached.value.reason.as_deref() != Some("rate-limited") {
            return None;
        }
        (self.now_ms() - cached.at < cached.ttl).then(|| cached.value.clone())
    }

    /// Liest die Anmeldung und fragt den Endpunkt
    async fn load(&self) -> Loaded {
        let failed = |reason: &str, plan: Option<String>, ttl: i64| Loaded {
            value: ClaudeUsageDto::unavailable(reason, plan),
            ttl,
        };
        let Some(login) = self.read_login().await else {
            return failed("no-login", None, FAILURE_TTL_MS);
        };
        if login.expires_at.is_some_and(|at| at <= self.now_ms()) {
            return failed("token-expired", login.plan, FAILURE_TTL_MS);
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
            Err(_) => return failed("request-failed", login.plan, FAILURE_TTL_MS),
        };
        if reply.status == 429 {
            let ttl = self.retry_after(reply.retry_after.as_deref());
            return failed("rate-limited", login.plan, ttl);
        }
        if !reply.ok() {
            let reason = if reply.status == 401 {
                "token-expired"
            } else {
                "request-failed"
            };
            return failed(reason, login.plan, FAILURE_TTL_MS);
        }
        let body = reply.body.unwrap_or(serde_json::Value::Null);
        Loaded {
            value: ClaudeUsageDto {
                available: true,
                reason: None,
                plan: login.plan,
                five_hour: window(body.get("five_hour")),
                seven_day: window(body.get("seven_day")),
                fetched_at: Some(self.clock.iso()),
            },
            ttl: self.ttl_ms,
        }
    }

    /// Sperre nach 429: `Retry-After` (Sekunden oder HTTP-Datum), aber nie kürzer als
    /// [`RATE_LIMIT_TTL_MS`]
    fn retry_after(&self, header: Option<&str>) -> i64 {
        let Some(header) = header.map(str::trim).filter(|text| !text.is_empty()) else {
            return RATE_LIMIT_TTL_MS;
        };
        let ms = match header.parse::<f64>() {
            Ok(seconds) if seconds.is_finite() => (seconds * 1000.0) as i64,
            _ => match chrono::DateTime::parse_from_rfc2822(header)
                .or_else(|_| chrono::DateTime::parse_from_rfc3339(header))
            {
                Ok(at) => at.timestamp_millis() - self.now_ms(),
                Err(_) => return RATE_LIMIT_TTL_MS,
            },
        };
        ms.max(RATE_LIMIT_TTL_MS)
    }

    /// Token, Ablauf (ms) und Abo-Art aus der Anmeldung; `None` ohne Anmeldung
    async fn read_login(&self) -> Option<Login> {
        let text = (self.read_credentials)().await?;
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

/// Ein Abruf, der nie etwas liefert (Vertragsmodus und Tests ohne Netz)
pub fn no_fetch() -> UsageFetch {
    Arc::new(|_request: UsageRequest| Box::pin(async { Err("ohne Netz".to_string()) }))
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
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            Ok(UsageReply {
                status,
                body: response.json::<serde_json::Value>().await.ok(),
                retry_after,
            })
        })
    })
}
