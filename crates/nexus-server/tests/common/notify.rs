//! Helfer der Benachrichtigungs-Tests: ein [`NotificationService`] mit Attrappen für
//! Systemmeldung und ntfy, dazu kleine Bauer für Sessions und Meldungen.
//!
//! Vorbild sind die `setup`-Funktionen in `packages/server/tests/unit/Notifications.test.ts`.
//! Keine Attrappe ruft etwas außerhalb des Prozesses auf.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use nexus_core::clock::Clock;
use nexus_core::sessions::session::{DisplayState, Session, SessionState, SessionView};
use nexus_core::settings::{UserSettings, UserSettingsStore};
use nexus_core::telegram::{TelegramConfig, TelegramConfigStore};
use nexus_core::HarnessHome;
use nexus_platform::{FakeNotifier, Notifier};
use nexus_server::log::ServerLog;
use nexus_server::notices::BrowserNoticeHub;
use nexus_server::notifications::model::{ToastKind, ToastModel};
use nexus_server::notifications::ntfy::{NtfyFetch, NtfyResponse, NtfySender};
use nexus_server::notifications::{NotificationService, NotificationSetup};
use nexus_server::telegram::api::TelegramApi;
use nexus_server::telegram::fake::FakeTelegram;
use nexus_server::telegram::TelegramNotifier;

/// Host und Port der Links in den Meldungen der Tests
pub const HOST: &str = "127.0.0.1";
pub const PORT: u16 = 7777;

/// Bildordner der Tests; die Bilder selbst braucht niemand, nur ihre Pfade im XML
pub const ASSETS_DIR: &str = "/a/toast";

/// Der Dienst samt seinen Attrappen
pub struct ServiceParts {
    pub service: Arc<NotificationService>,
    pub tabs: Arc<BrowserNoticeHub>,
    pub notifier: Arc<FakeNotifier>,
    pub ntfy: Arc<FakeNtfy>,
    /// Die Attrappe der Bot-API; sie zeichnet auf, was nach Telegram ginge
    pub telegram: Arc<FakeTelegram>,
    /// Der Benutzerordner des Dienstes (dort liegt auch `telegram.yaml`)
    pub home: HarnessHome,
    /// Benutzerordner mit der `settings.json`; er muss den Dienst überleben
    _dir: tempfile::TempDir,
}

impl ServiceParts {
    /// Das Toast-XML jeder gezeigten Systemmeldung, in der Reihenfolge des Aufrufs
    pub fn shown(&self) -> Vec<String> {
        self.notifier
            .shown()
            .into_iter()
            .map(|toast| toast.xml.unwrap_or_default())
            .collect()
    }
}

/// Ein Dienst mit diesen Einstellungen; Systemmeldungen, ntfy und Telegram laufen in
/// Attrappen
pub fn service(settings: UserSettings) -> ServiceParts {
    service_at(settings, Clock::fixed(now()))
}

/// Der Zeitpunkt der Tests; die Nachtruhe rechnet nach der Ortszeit dieses Zeitpunkts
pub fn now() -> chrono::DateTime<chrono::Utc> {
    "2026-11-03T12:00:00Z".parse().expect("Zeitpunkt")
}

/// Wie [`service`], mit eigener Uhr (die Nachtruhe von Telegram geht nach ihr)
pub fn service_at(settings: UserSettings, clock: Clock) -> ServiceParts {
    let dir = tempfile::Builder::new()
        .prefix("nexus-notify-")
        .tempdir()
        .expect("Benutzerordner");
    let home = HarnessHome::at(dir.path());
    UserSettingsStore::new(home.clone())
        .write(&settings)
        .expect("Einstellungen schreiben");
    let notifier = Arc::new(FakeNotifier::new());
    let tabs = BrowserNoticeHub::new();
    let ntfy = FakeNtfy::new();
    let telegram = Arc::new(FakeTelegram::new());
    let api = telegram.clone();
    let service = Arc::new(NotificationService::new(NotificationSetup {
        settings: UserSettingsStore::new(home.clone()),
        notifier: notifier.clone() as Arc<dyn Notifier>,
        assets_dir: PathBuf::from(ASSETS_DIR),
        host: HOST.to_string(),
        port: PORT,
        log: ServerLog::new(&home),
        tabs: Some(tabs.clone()),
        ntfy: NtfySender::new(Box::new(SharedNtfy(ntfy.clone()))),
        telegram: Some(Arc::new(TelegramNotifier::new(
            home.clone(),
            clock,
            Some(Arc::new(move |_config: &TelegramConfig| {
                api.clone() as Arc<dyn TelegramApi>
            })),
        ))),
    }));
    ServiceParts {
        service,
        tabs,
        notifier,
        ntfy,
        telegram,
        home,
        _dir: dir,
    }
}

/// Schreibt `telegram.yaml` des Dienstes
pub fn write_telegram(home: &HarnessHome, config: &TelegramConfig) {
    TelegramConfigStore::new(home.clone())
        .write(config)
        .expect("Telegram-Einstellung schreiben");
}

/// Eine Session des Projekts `p1` mit Titel und Modell, wie `view(...)` in TypeScript
pub fn view(id: &str, display_state: DisplayState) -> SessionView {
    SessionView {
        session: Session {
            id: id.to_string(),
            project_id: "p1".to_string(),
            tickets: Vec::new(),
            cwd: "/repo".to_string(),
            state: SessionState::Idle,
            started_at: "2026-01-01T00:00:00.000Z".to_string(),
            last_event_at: "2026-01-01T00:00:00.000Z".to_string(),
            ended_at: None,
            title: Some("Favoriten als Seitenfenster".to_string()),
            model: Some("Sonnet 5.5".to_string()),
            subagents: Vec::new(),
        },
        display_state,
        listening: false,
        runtime_ms: 0,
    }
}

/// Eine Meldung dieser Art mit festen Texten
pub fn model(kind: ToastKind) -> ToastModel {
    ToastModel {
        kind,
        title: "t".to_string(),
        line1: "x".to_string(),
        line2: "y".to_string(),
        open_url: "http://127.0.0.1:7777/p/p1".to_string(),
    }
}

/// Abruf für ntfy, der nur aufzeichnet und immer mit 200 antwortet
#[derive(Default)]
pub struct FakeNtfy {
    bodies: Mutex<Vec<String>>,
}

impl FakeNtfy {
    /// Attrappe ohne aufgezeichnete Anfragen
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Die Körper der gesendeten Anfragen als JSON
    pub fn pushed(&self) -> Vec<serde_json::Value> {
        self.bodies
            .lock()
            .expect("Sperre der Anfragen")
            .iter()
            .map(|body| serde_json::from_str(body).expect("Körper als JSON"))
            .collect()
    }
}

/// Gibt dieselbe Attrappe an den Sender, die der Test ausliest (ein eigener Typ, weil ein
/// `impl` für `Arc<…>` einem fremden Typ gehören würde)
struct SharedNtfy(Arc<FakeNtfy>);

#[async_trait]
impl NtfyFetch for SharedNtfy {
    async fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<NtfyResponse, String> {
        self.0.post(url, headers, body).await
    }
}

#[async_trait]
impl NtfyFetch for FakeNtfy {
    async fn post(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        body: &str,
    ) -> Result<NtfyResponse, String> {
        self.bodies
            .lock()
            .expect("Sperre der Anfragen")
            .push(body.to_string());
        Ok(NtfyResponse {
            ok: true,
            status: 200,
            text: String::new(),
        })
    }
}
