// Zugang zum Harness-Server: einen schon laufenden Server übernehmen oder den Server im eigenen
// Prozess starten, das Hauptfenster mit der Oberfläche öffnen und den selbst gestarteten Server
// beim Beenden wieder stoppen. Einen fremden Server lässt die App unangetastet.
//
// Seit P5-2 läuft der Server nicht mehr als Kindprozess, sondern als Bibliothek auf einer eigenen
// tokio-Laufzeit im Prozess der App. Ohne App übernimmt `nexus serve` dieselbe Aufgabe.
//
// Seit P5-3 lädt das Fenster die Oberfläche aus den Dateien der App (`tauri://localhost` bzw.
// `http://tauri.localhost`) statt vom Server. Damit die Oberfläche weiß, wen sie fragen darf,
// legt ein Initialisierungsskript vor jedem Skript der Seite `window.__NEXUS__` mit Adresse und
// Start-Token des Servers an. Beides steht fest, bevor das Fenster entsteht: der Port ist
// gebunden, das Token kommt aus den Einstellungen des eigenen Servers bzw. aus der `server.json`
// eines schon laufenden.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use nexus_core::registry::{AppToken, HarnessHome, ServerInfoFile};
use nexus_platform::Platform;
use nexus_server::config::{ServerConfig, ServerConfigLoader};
use nexus_server::ServerHandle;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

// Port, auf dem CLI und Hooks den Server erwarten, wenn nichts anderes gesetzt ist
// (DEFAULT_SERVER_PORT in nexus-core)
pub const DEFAULT_SERVER_PORT: u16 = nexus_core::registry::DEFAULT_SERVER_PORT;
// Daran erkennt sich der eigene Server in der Antwort von /api/health
const HEALTH_NAME: &str = "ai-harness";
// So lange darf ein einzelner Health-Aufruf dauern
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
// So lange bekommt der eigene Server Zeit, um geordnet aufzuhören (Takt anhalten, Läufe abschließen)
const STOP_TIMEOUT: Duration = Duration::from_secs(30);
// So viele Arbeitsfäden bekommt die Laufzeit des Servers; schwere Anfragen blockieren damit nicht
// die Oberfläche, und die App bleibt klein
const SERVER_THREADS: usize = 3;
// Seite, die das Fenster zeigt, wenn die App nicht an den Server kommt. Sie liegt neben der
// Oberfläche (packages/web/public/splash.html) und kommt ohne Schriften, Bilder und Skripte aus.
const STARTSEITE: &str = "splash.html";

// Port, auf dem der genutzte Server antwortet; 0, solange noch keiner gefunden oder gestartet ist
static AKTIVER_PORT: AtomicU16 = AtomicU16::new(0);

// Gründe, aus denen die App nicht an den Server kommt; die Texte stehen so auf der Startseite
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Der Port „{0}“ ist ungültig. Erlaubt sind Zahlen von 1 bis 65535.")]
    PortInvalid(String),
    #[error(
        "Port {0} ist von einem anderen Programm belegt. Bitte das Programm beenden oder HARNESS_PORT auf einen freien Port setzen und Nexus neu starten."
    )]
    PortForeign(u16),
    #[error("Der Server ließ sich nicht starten: {0}")]
    StartFailed(String),
    #[error(
        "Auf Port {0} läuft eine ältere Nexus-Version ohne Start-Token. Bitte sie beenden (nexus stop) und Nexus neu starten."
    )]
    TokenMissing(u16),
}

// Zustand eines Ports: eigener Server, frei oder von einem fremden Programm belegt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    Ours,
    Free,
    Foreign,
}

// Prozessangaben aus server.json; nur ein Hinweis, maßgeblich ist der Health-Aufruf.
// Das Start-Token steht erst seit P5-3 darin; ältere Server schreiben keines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    pub pid: u32,
    pub port: u16,
    pub app_token: Option<AppToken>,
}

// Findet einen laufenden Harness-Server oder startet ihn im eigenen Prozess
pub struct ServerConnector {
    home: PathBuf,
    // Gewünschter Port aus der Umgebung; ohne Wunsch nimmt der Server einen freien (Port 0)
    wunsch_port: Option<u16>,
}

impl ServerConnector {
    // Liest Harness-Ordner und Port aus der Umgebung
    pub fn from_env(env: &HashMap<String, String>) -> Result<Self, ServerError> {
        Ok(Self {
            home: harness_home(env),
            wunsch_port: port_from_env(env)?,
        })
    }

    // Port, auf dem der Server starten soll; 0 heißt „einen freien nehmen“
    pub fn port(&self) -> u16 {
        self.wunsch_port.unwrap_or(0)
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    // Prozessangaben des zuletzt gestarteten Servers
    pub fn server_info(&self) -> Option<ServerInfo> {
        read_server_info(&self.home)
    }

    // Der schon laufende Server dieses Datenordners, falls es ihn gibt: `server.json` nennt ihn,
    // seine Prozessnummer lebt, und unter seinem Port antwortet genau dieser Prozess auf
    // `/api/health`. Die Prozessnummer verknüpft die Antwort mit dem eigenen Datenordner.
    pub async fn vorhandener_server(&self) -> Option<ServerInfo> {
        let info = self.server_info()?;
        if !nexus_core::process::is_alive(info.pid) {
            log::info!(
                "server.json nennt pid {}; der Prozess läuft nicht mehr.",
                info.pid
            );
            return None;
        }
        let client = health_client()?;
        match health(&client, info.port).await {
            Some(pid) if pid == info.pid => Some(info),
            Some(pid) => {
                log::info!(
                    "Auf Port {} antwortet pid {pid}, server.json nennt pid {}.",
                    info.port,
                    info.pid
                );
                None
            }
            None => None,
        }
    }
}

// Nexus-Ordner in derselben Reihenfolge wie `NexusHome` in `crates/nexus-core/src/registry/home.rs`:
// NEXUS_HOME, HARNESS_HOME (veraltet), vorhandenes ~/.nexus, vorhandenes ~/.harness (Rückfall),
// sonst ~/.nexus. Nur so nehmen App, CLI und Server dieselbe board.db.
pub fn harness_home(env: &HashMap<String, String>) -> PathBuf {
    if let Some(dir) = var(env, "HOME") {
        return PathBuf::from(dir);
    }
    let base = dirs::home_dir().unwrap_or_default();
    let nexus = base.join(".nexus");
    if nexus.is_dir() {
        return nexus;
    }
    let harness = base.join(".harness");
    if harness.is_dir() {
        return harness;
    }
    nexus
}

// Wert aus der Umgebung: `NEXUS_<NAME>`, sonst `HARNESS_<NAME>`; leere Werte zählen wie keine
fn var(env: &HashMap<String, String>, name: &str) -> Option<String> {
    [format!("NEXUS_{name}"), format!("HARNESS_{name}")]
        .into_iter()
        .filter_map(|full| env.get(&full).cloned())
        .find(|value| !value.trim().is_empty())
}

// Gewünschter Port des Servers: NEXUS_PORT, sonst HARNESS_PORT. Ohne Angabe nimmt der Server einen
// freien Port (None); CLI und Hooks finden ihn über `server.json`.
pub fn port_from_env(env: &HashMap<String, String>) -> Result<Option<u16>, ServerError> {
    let found = var(env, "PORT");
    let raw = match &found {
        Some(value) => value.trim(),
        None => return Ok(None),
    };
    match raw.parse::<u16>() {
        Ok(port) if port > 0 => Ok(Some(port)),
        _ => Err(ServerError::PortInvalid(raw.to_string())),
    }
}

// Liest server.json; None, wenn es sie nicht gibt oder sie unlesbar ist
pub fn read_server_info(home: &Path) -> Option<ServerInfo> {
    let info = ServerInfoFile::new(HarnessHome::at(home)).read()?;
    Some(ServerInfo {
        pid: info.pid,
        port: info.port,
        app_token: info.app_token,
    })
}

// Port, auf dem der genutzte Server antwortet; solange keiner feststeht, der Wunsch aus der
// Umgebung bzw. der Standardport
pub fn aktiver_port() -> u16 {
    match AKTIVER_PORT.load(Ordering::Relaxed) {
        0 => port_from_env(&std::env::vars().collect())
            .ok()
            .flatten()
            .unwrap_or(DEFAULT_SERVER_PORT),
        port => port,
    }
}

// Merkt den Port des genutzten Servers für Tray, Rückfragen und Deep-Links
fn setze_port(port: u16) {
    AKTIVER_PORT.store(port, Ordering::Relaxed);
}

// Fragt /api/health: antwortet dort der Harness-Server, hört niemand oder ein fremdes Programm?
pub async fn probe(port: u16) -> PortState {
    match health_client() {
        Some(client) => probe_with(&client, port).await,
        None => PortState::Foreign,
    }
}

// Client für den Health-Aufruf; None, wenn er sich nicht bauen lässt
fn health_client() -> Option<reqwest::Client> {
    match reqwest::Client::builder().timeout(PROBE_TIMEOUT).build() {
        Ok(client) => Some(client),
        Err(error) => {
            log::warn!("Der Health-Aufruf ließ sich nicht vorbereiten: {error}");
            None
        }
    }
}

// Ein Health-Aufruf mit vorhandenem Client
async fn probe_with(client: &reqwest::Client, port: u16) -> PortState {
    let url = format!("http://127.0.0.1:{port}/api/health");
    match client.get(url).send().await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(body) if body.get("name").and_then(|name| name.as_str()) == Some(HEALTH_NAME) => {
                PortState::Ours
            }
            _ => PortState::Foreign,
        },
        Err(error) if connection_refused(&error) => PortState::Free,
        Err(_) => PortState::Foreign,
    }
}

// Prozessnummer des Harness-Servers auf dem Port; None, wenn dort niemand oder ein fremdes
// Programm antwortet
async fn health(client: &reqwest::Client, port: u16) -> Option<u32> {
    let url = format!("http://127.0.0.1:{port}/api/health");
    let body: serde_json::Value = client.get(url).send().await.ok()?.json().await.ok()?;
    if body.get("name").and_then(|name| name.as_str()) != Some(HEALTH_NAME) {
        return None;
    }
    u32::try_from(body.get("pid")?.as_u64()?).ok()
}

// Eine abgelehnte Verbindung heißt: auf dem Port hört niemand (ECONNREFUSED wie in der CLI)
fn connection_refused(error: &reqwest::Error) -> bool {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = current {
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            return io.kind() == std::io::ErrorKind::ConnectionRefused;
        }
        current = error.source();
    }
    false
}

// Der von der App gestartete Server samt seiner eigenen Laufzeit; ein fremder Server steht hier nie drin
struct OwnServer {
    handle: ServerHandle,
    runtime: tokio::runtime::Runtime,
}

// Was die App beim Beenden wieder aufräumen muss, was die Startseite noch zeigen soll und
// welcher Deep-Link noch auf den Server wartet
#[derive(Default)]
pub struct ServerState {
    own: Mutex<Option<OwnServer>>,
    message: Mutex<Option<String>>,
    // Pfad eines Deep-Links, der kam, bevor das Fenster die Oberfläche geladen hatte
    pending: Mutex<Option<String>>,
    ready: std::sync::atomic::AtomicBool,
}

impl ServerState {
    // Merkt sich den eigenen Server
    fn keep(&self, server: OwnServer) {
        match self.own.lock() {
            Ok(mut slot) => *slot = Some(server),
            Err(error) => log::warn!("Der Serverzustand ist nicht lesbar: {error}"),
        }
    }

    // Gibt den eigenen Server heraus und vergisst ihn
    fn take(&self) -> Option<OwnServer> {
        self.own.lock().ok().and_then(|mut slot| slot.take())
    }

    // Merkt sich die Meldung für die Startseite
    fn remember(&self, message: &str) {
        if let Ok(mut slot) = self.message.lock() {
            *slot = Some(message.to_string());
        }
    }

    // Die gemerkte Meldung, falls es eine gibt
    fn message(&self) -> Option<String> {
        self.message.lock().ok().and_then(|slot| slot.clone())
    }

    // Merkt den Pfad eines Deep-Links, bis die Oberfläche erreichbar ist
    fn merke_pfad(&self, path: &str) {
        if let Ok(mut slot) = self.pending.lock() {
            *slot = Some(path.to_string());
        }
    }

    // Gibt den ausstehenden Pfad heraus und vergisst ihn
    fn nimm_pfad(&self) -> Option<String> {
        self.pending.lock().ok().and_then(|mut slot| slot.take())
    }
}

// Richtet den Serverzugang ein und öffnet das Hauptfenster.
//
// Erst steht der Zugang fest (Port und Start-Token), dann entsteht das Fenster mit der
// Oberfläche, und erst danach fährt der eigene Server im Hintergrund hoch. Das Fenster wartet
// damit nicht auf den Server: Anfragen, die zu früh kommen, warten am schon gebundenen Port.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    app.manage(ServerState::default());
    watch_signals(app);
    match vorbereiten(app) {
        Ok(Vorbereitet { zugang, eigener }) => {
            setze_port(zugang.port);
            if let Err(error) = fenster_oeffnen(app, Some(&zugang)) {
                log::error!("{error}");
            }
            if let Some(eigener) = eigener {
                eigenen_server_starten(app, eigener);
            }
        }
        Err(error) => {
            log::error!("{error}");
            // Ohne Zugang gibt es keine Oberfläche; das Fenster zeigt dann die Startseite mit
            // der Meldung
            if let Some(state) = app.try_state::<ServerState>() {
                state.remember(&error.to_string());
            }
            if let Err(error) = fenster_oeffnen(app, None) {
                log::error!("{error}");
            }
        }
    }
}

// Zugang zum Server: Adresse und Start-Token, mit denen die Oberfläche ihn anspricht
#[derive(Debug, Clone)]
pub struct Zugang {
    pub port: u16,
    pub token: AppToken,
}

impl Zugang {
    // Adresse des Servers ohne Schrägstrich am Ende; immer 127.0.0.1, nie localhost
    // (localhost zeigt unter Windows auf IPv6)
    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

// Ergebnis der Vorbereitung: der Zugang und – wenn die App den Server selbst mitbringt –
// alles, was zum Hochfahren noch fehlt
struct Vorbereitet {
    zugang: Zugang,
    eigener: Option<EigenerStart>,
}

// Der schon gebundene Port samt Einstellungen und eigener Laufzeit; der Server selbst läuft
// erst, wenn das Fenster offen ist
struct EigenerStart {
    runtime: tokio::runtime::Runtime,
    listener: tokio::net::TcpListener,
    config: ServerConfig,
}

// Sucht den laufenden Server dieses Datenordners oder bereitet den eigenen vor
fn vorbereiten<R: Runtime>(app: &AppHandle<R>) -> Result<Vorbereitet, ServerError> {
    let connector = ServerConnector::from_env(&std::env::vars().collect())?;
    log::info!("Harness-Ordner {}", connector.home().display());
    if let Some(info) = tauri::async_runtime::block_on(connector.vorhandener_server()) {
        log::info!(
            "Auf Port {} läuft schon ein Harness-Server; die App nutzt ihn mit.",
            info.port
        );
        // Sein Start-Token steht in server.json; ohne Token käme die Oberfläche nicht an ihn heran
        let token = info.app_token.ok_or(ServerError::TokenMissing(info.port))?;
        return Ok(Vorbereitet {
            zugang: Zugang {
                port: info.port,
                token,
            },
            eigener: None,
        });
    }
    let eigener = eigenen_vorbereiten(app, &connector)?;
    Ok(Vorbereitet {
        zugang: Zugang {
            port: eigener.config.port,
            token: eigener.config.app_token.clone(),
        },
        eigener: Some(eigener),
    })
}

// Bindet den Port des eigenen Servers und legt seine Einstellungen fest; danach stehen Adresse
// und Token fest, auch wenn der Server noch nicht antwortet
fn eigenen_vorbereiten<R: Runtime>(
    app: &AppHandle<R>,
    connector: &ServerConnector,
) -> Result<EigenerStart, ServerError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(SERVER_THREADS)
        .enable_all()
        .thread_name("nexus-server")
        .build()
        .map_err(|error| ServerError::StartFailed(error.to_string()))?;
    // Ist ein fester Port gewünscht und hört dort ein fremdes Programm, startet die App nicht
    let wunsch = connector.port();
    if wunsch != 0 && runtime.block_on(probe(wunsch)) == PortState::Foreign {
        return Err(ServerError::PortForeign(wunsch));
    }
    let mut config = server_config(app, connector)?;
    let listener = runtime
        .block_on(nexus_server::bind(&config.host, wunsch))
        .map_err(|error| ServerError::StartFailed(error.to_string()))?;
    config.port = listener
        .local_addr()
        .map_err(|error| ServerError::StartFailed(error.to_string()))?
        .port();
    Ok(EigenerStart {
        runtime,
        listener,
        config,
    })
}

// Fährt den eigenen Server auf dem schon gebundenen Port hoch und merkt ihn sich fürs Beenden
fn eigenen_server_starten<R: Runtime>(app: &AppHandle<R>, start: EigenerStart) {
    let EigenerStart {
        runtime,
        listener,
        config,
    } = start;
    let port = config.port;
    let home = config.home.clone();
    let platform = match config.is_contract() {
        true => Platform::contract(),
        false => Platform::local(),
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    runtime.spawn(async move {
        let board = nexus_server::board_from_config(home);
        let _ = sender.send(
            nexus_server::serve_on(listener, config, board, platform)
                .await
                .map_err(|error| error.to_string()),
        );
    });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match receiver.await {
            Ok(Ok(handle)) => {
                log::info!("Server im Prozess der App gestartet (Port {port}).");
                app.state::<ServerState>()
                    .keep(OwnServer { handle, runtime });
            }
            Ok(Err(error)) => {
                log::error!("{}", ServerError::StartFailed(error));
                // Nicht blockierend abbauen: ein Drop der Laufzeit in einer async-Aufgabe
                // bricht den Prozess ab
                runtime.shutdown_background();
            }
            Err(_) => {
                let grund = "Der Server hat nicht geantwortet.".to_string();
                log::error!("{}", ServerError::StartFailed(grund));
                runtime.shutdown_background();
            }
        }
    });
}

// Hat die App den Server selbst gestartet? Dann nimmt sie ihn beim Beenden mit.
// Die Rückfrage beim Beenden sagt danach, was mit dem Server passiert.
pub fn eigener_server<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<ServerState>()
        .and_then(|state| state.own.lock().ok().map(|slot| slot.is_some()))
        .unwrap_or(false)
}

// Beendet den selbst gestarteten Server: Takt anhalten, Board-Laufzeit schließen, server.json
// entfernen. Einen fremden Server lässt die App laufen; losgelöste Worker laufen weiter.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<ServerState>() else {
        return;
    };
    let Some(own) = state.take() else {
        return;
    };
    log::info!("Der eigene Server wird beendet.");
    let OwnServer { handle, runtime } = own;
    let beendet = runtime.block_on(async move {
        tokio::time::timeout(STOP_TIMEOUT, handle.stop())
            .await
            .is_ok()
    });
    if !beendet {
        log::warn!(
            "Der Server war nach {} s nicht fertig; Nexus beendet sich trotzdem.",
            STOP_TIMEOUT.as_secs()
        );
    }
    // Die Laufzeit darf das Beenden nicht aufhalten; was noch läuft, endet mit dem Prozess
    runtime.shutdown_timeout(Duration::from_secs(2));
}

// Label des Hauptfensters; es entsteht erst hier, weil erst jetzt Adresse und Token feststehen
pub const HAUPTFENSTER: &str = "main";

// Öffnet das Hauptfenster mit der Oberfläche aus den Dateien der App. Ohne Zugang zeigt es die
// Startseite, auf der die gemerkte Meldung steht.
fn fenster_oeffnen<R: Runtime>(
    app: &AppHandle<R>,
    zugang: Option<&Zugang>,
) -> Result<(), ServerError> {
    let seite = match zugang {
        Some(_) => "index.html",
        None => STARTSEITE,
    };
    let mut builder = WebviewWindowBuilder::new(app, HAUPTFENSTER, WebviewUrl::App(seite.into()))
        .title("Nexus")
        .inner_size(1280.0, 840.0)
        .min_inner_size(900.0, 600.0)
        // Die Oberfläche hält den Ereignisstrom offen; gedrosselt im Hintergrund bräche er ab
        .background_throttling(tauri::utils::config::BackgroundThrottlingPolicy::Disabled);
    if let Some(zugang) = zugang {
        builder = builder.initialization_script(init_script(zugang));
    }
    builder
        .build()
        .map_err(|error| ServerError::StartFailed(format!("Das Fenster fehlt: {error}")))?;
    log::info!("Das Fenster lädt {seite} aus den Dateien der App.");
    Ok(())
}

// Skript, das vor jedem Skript der Seite läuft: Es legt Adresse und Start-Token des Servers ins
// Fenster. Beide Werte gehen JSON-kodiert hinein, damit sie nie als Code gelesen werden.
fn init_script(zugang: &Zugang) -> String {
    let base = serde_json::to_string(&zugang.base()).unwrap_or_else(|_| "\"\"".to_string());
    let token = serde_json::to_string(zugang.token.as_str()).unwrap_or_else(|_| "\"\"".to_string());
    format!("window.__NEXUS__ = {{ base: {base}, token: {token} }};")
}

// Öffnet einen Pfad der Oberfläche. Die Oberfläche kommt aus den Dateien der App, deshalb wird
// nicht navigiert, sondern im Fenster weitergeschaltet – sonst suchte die App eine Datei mit
// diesem Namen. Steht die Seite noch nicht, wird der Pfad gemerkt.
pub fn open_path<R: Runtime>(app: &AppHandle<R>, path: &str) {
    let Some(state) = app.try_state::<ServerState>() else {
        return;
    };
    if !state.ready.load(std::sync::atomic::Ordering::Relaxed) {
        log::info!("Die Oberfläche steht noch nicht; {path} wird gemerkt.");
        state.merke_pfad(path);
        return;
    }
    let Some(window) = app.get_webview_window(HAUPTFENSTER) else {
        return;
    };
    if let Err(error) = window.eval(pfad_script(path)) {
        log::warn!("{path} ließ sich nicht öffnen: {error}");
    }
}

// Skript, das die Oberfläche auf einen Pfad schaltet: Adresse umschreiben und den Router wecken
// (React Router hört auf `popstate`)
fn pfad_script(path: &str) -> String {
    let encoded = serde_json::to_string(path).unwrap_or_else(|_| "\"/\"".to_string());
    format!(
        "(function(pfad){{\
         if(location.pathname+location.search===pfad)return;\
         history.pushState({{}},'',pfad);\
         window.dispatchEvent(new PopStateEvent('popstate'));\
         }})({encoded});"
    )
}

// Einstellungen des Servers: Port und Ordner wie bei `nexus serve`, dazu die Ressourcen der App
fn server_config<R: Runtime>(
    app: &AppHandle<R>,
    connector: &ServerConnector,
) -> Result<ServerConfig, ServerError> {
    let args = vec!["--port".to_string(), connector.port().to_string()];
    let mut config = ServerConfigLoader::load_from_process(&args)
        .map_err(|error| ServerError::StartFailed(error.to_string()))?;
    // Der aufgelöste Ordner der App gilt; sonst könnte der Server nach einem Umzug auf einer
    // anderen board.db landen als die App
    config.home = HarnessHome::at(connector.home());
    if let Some(base) = resource_dir(app) {
        config.web_dir = Some(base.join("web"));
        config.toast_dir = base.join("toast");
    }
    Ok(config)
}

// Oberfläche und Benachrichtigungsbilder liegen als Ressourcen neben der App; findet die App sie
// nicht, bleibt es bei dem, was der Server selbst neben seinem Programm findet
fn resource_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    let base = match app.path().resource_dir() {
        Ok(dir) => dir,
        Err(error) => {
            log::warn!("Der Ressourcenordner ist nicht bestimmbar: {error}");
            return None;
        }
    };
    if !base.join("web").is_dir() {
        log::warn!(
            "In {} fehlt der Ordner web; der Server sucht die Oberfläche selbst.",
            base.display()
        );
        return None;
    }
    Some(base)
}

// Die Seite ist fertig geladen: ab jetzt nimmt die Oberfläche Pfade an, ein gemerkter Deep-Link
// kommt dran, und eine Meldung aus der Vorbereitung wird nachgetragen
pub fn on_page_load<R: Runtime>(
    webview: &tauri::Webview<R>,
    payload: &tauri::webview::PageLoadPayload<'_>,
) {
    if payload.event() != tauri::webview::PageLoadEvent::Finished {
        return;
    }
    let Some(state) = webview.try_state::<ServerState>() else {
        return;
    };
    state
        .ready
        .store(true, std::sync::atomic::Ordering::Relaxed);
    if let Some(message) = state.message() {
        if let Err(error) = webview.eval(status_script(&message)) {
            log::warn!("Die Meldung ließ sich nicht anzeigen: {error}");
        }
    }
    if let Some(pfad) = state.nimm_pfad() {
        log::info!("Der gemerkte Deep-Link öffnet {pfad}.");
        if let Err(error) = webview.eval(pfad_script(&pfad)) {
            log::warn!("{pfad} ließ sich nicht öffnen: {error}");
        }
    }
}

// Skript, das den Text JSON-kodiert in das Statusfeld der Startseite schreibt, sobald es das Feld gibt
fn status_script(message: &str) -> String {
    let encoded = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "(function(text){{\
         var zeigen=function(){{var feld=document.getElementById('status');if(feld){{feld.textContent=text;}}}};\
         if(document.readyState==='loading'){{document.addEventListener('DOMContentLoaded',zeigen);}}else{{zeigen();}}\
         }})({encoded});"
    )
}

// Ein SIGTERM an die App löst kein RunEvent::Exit aus; abgefangen beendet sie sich geordnet und
// nimmt ihren Server mit
#[cfg(unix)]
fn watch_signals<R: Runtime>(app: &AppHandle<R>) {
    use tokio::signal::unix::{signal, SignalKind};

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let (Ok(mut term), Ok(mut interrupt)) = (
            signal(SignalKind::terminate()),
            signal(SignalKind::interrupt()),
        ) else {
            log::warn!("SIGTERM und SIGINT lassen sich nicht abfangen.");
            return;
        };
        tokio::select! {
            _ = term.recv() => log::info!("SIGTERM erhalten, Nexus wird beendet."),
            _ = interrupt.recv() => log::info!("SIGINT erhalten, Nexus wird beendet."),
        }
        handle.exit(0);
    });
}

// Unter Windows beendet der Benutzer die App über Fenster oder Menü
#[cfg(not(unix))]
fn watch_signals<R: Runtime>(_app: &AppHandle<R>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    // Name des Rust-Programms aus bundle.externalBin
    const NEXUS: &str = "nexus";

    // Umgebung aus Paaren, damit die Tests nur setzen, was sie prüfen
    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect()
    }

    // Kleiner HTTP-Server, der auf jede Anfrage mit demselben Rumpf antwortet
    async fn fake_server(body: impl Into<String>) -> u16 {
        let body = body.into();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });
        port
    }

    // Ein Port, auf dem niemand hört: gebundenen Listener sofort wieder schließen
    async fn free_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap().port()
    }

    // Ein leerer Benutzerordner nur für diesen Test
    fn home_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nexus-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // Schreibt eine server.json mit Prozessnummer und Port
    fn schreibe_server_json(home: &Path, pid: u32, port: u16) {
        std::fs::write(
            home.join("server.json"),
            format!(r#"{{"pid":{pid},"port":{port},"startedAt":"2026-10-28T08:00:00.000Z"}}"#),
        )
        .unwrap();
    }

    #[test]
    fn port_aus_umgebung() {
        // Ohne Angabe wünscht die App keinen festen Port; der Server nimmt einen freien
        assert_eq!(port_from_env(&env(&[])).unwrap(), None);
        assert_eq!(port_from_env(&env(&[("HARNESS_PORT", "")])).unwrap(), None);
        assert_eq!(
            port_from_env(&env(&[("HARNESS_PORT", "4812")])).unwrap(),
            Some(4812)
        );
        assert_eq!(
            port_from_env(&env(&[("HARNESS_PORT", " 4812 ")])).unwrap(),
            Some(4812)
        );
        for invalid in ["0", "-1", "65536", "abc", "47.10"] {
            let error = port_from_env(&env(&[("HARNESS_PORT", invalid)])).unwrap_err();
            assert!(
                matches!(error, ServerError::PortInvalid(_)),
                "{invalid} sollte abgelehnt werden"
            );
        }
    }

    #[test]
    fn harness_home_aus_umgebung() {
        let gesetzt = harness_home(&env(&[("NEXUS_HOME", "/tmp/eigener-ordner")]));
        assert_eq!(gesetzt, PathBuf::from("/tmp/eigener-ordner"));
        let alt = harness_home(&env(&[("HARNESS_HOME", "/tmp/alter-ordner")]));
        assert_eq!(alt, PathBuf::from("/tmp/alter-ordner"));
        let neu_gewinnt = harness_home(&env(&[
            ("NEXUS_HOME", "/tmp/neuer-ordner"),
            ("HARNESS_HOME", "/tmp/alter-ordner"),
        ]));
        assert_eq!(neu_gewinnt, PathBuf::from("/tmp/neuer-ordner"));
        let standard = harness_home(&env(&[]));
        assert!(
            standard.ends_with(".nexus") || standard.ends_with(".harness"),
            "{standard:?} sollte auf .nexus oder (als Rückfall) auf .harness enden"
        );
    }

    #[test]
    fn der_neue_port_gewinnt_gegen_den_alten() {
        assert_eq!(
            port_from_env(&env(&[("NEXUS_PORT", "4813"), ("HARNESS_PORT", "4812")])).unwrap(),
            Some(4813)
        );
        assert_eq!(
            port_from_env(&env(&[("NEXUS_PORT", ""), ("HARNESS_PORT", "4812")])).unwrap(),
            Some(4812)
        );
    }

    #[test]
    fn server_json_wird_gelesen() {
        let home = home_dir("server-json");
        assert_eq!(read_server_info(&home), None);
        schreibe_server_json(&home, 4242, 4812);
        assert_eq!(
            read_server_info(&home),
            Some(ServerInfo {
                pid: 4242,
                port: 4812,
                app_token: None
            })
        );
        std::fs::write(home.join("server.json"), "kein json").unwrap();
        assert_eq!(read_server_info(&home), None);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn probe_erkennt_eigenen_server() {
        let port = fake_server(r#"{"name":"ai-harness","version":"0.1.0"}"#).await;
        assert_eq!(probe(port).await, PortState::Ours);
    }

    #[tokio::test]
    async fn probe_erkennt_fremden_server() {
        let port = fake_server(r#"{"name":"etwas-anderes"}"#).await;
        assert_eq!(probe(port).await, PortState::Foreign);
    }

    #[tokio::test]
    async fn probe_freier_port() {
        let port = free_port().await;
        assert_eq!(probe(port).await, PortState::Free);
    }

    // Steht in server.json ein lebender Server, der sich auf seinem Port mit derselben
    // Prozessnummer meldet, nutzt die App ihn mit und startet keinen zweiten
    #[tokio::test]
    async fn vorhandener_server_wird_uebernommen() {
        let home = home_dir("vorhandener-server");
        let eigene = std::process::id();
        let port = fake_server(format!(r#"{{"name":"ai-harness","pid":{eigene}}}"#)).await;
        schreibe_server_json(&home, eigene, port);
        let connector = ServerConnector {
            home: home.clone(),
            wunsch_port: None,
        };
        let info = connector.vorhandener_server().await.unwrap();
        assert_eq!(info.port, port);
        std::fs::remove_dir_all(&home).unwrap();
    }

    // Ohne server.json, mit toter Prozessnummer oder mit einem fremden Programm auf dem Port
    // startet die App ihren eigenen Server
    #[tokio::test]
    async fn ohne_passenden_server_startet_die_app_selbst() {
        let home = home_dir("kein-server");
        let connector = ServerConnector {
            home: home.clone(),
            wunsch_port: None,
        };
        assert_eq!(connector.vorhandener_server().await, None);

        // Eine Prozessnummer, die nie vergeben ist
        schreibe_server_json(&home, 2_147_483_646, 4710);
        assert_eq!(connector.vorhandener_server().await, None);

        // Lebender Prozess, aber auf dem Port antwortet ein anderer Server
        let fremd = fake_server(r#"{"name":"etwas-anderes"}"#).await;
        schreibe_server_json(&home, std::process::id(), fremd);
        assert_eq!(connector.vorhandener_server().await, None);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn connector_zeigt_auf_die_eigene_adresse() {
        let connector = ServerConnector::from_env(&env(&[
            ("HARNESS_HOME", "/tmp/eigener-ordner"),
            ("HARNESS_PORT", "4812"),
        ]))
        .unwrap();
        assert_eq!(connector.port(), 4812);
        assert_eq!(connector.home(), Path::new("/tmp/eigener-ordner"));

        // Ohne Portwunsch bindet der Server Port 0 und sucht sich selbst einen freien
        let frei =
            ServerConnector::from_env(&env(&[("HARNESS_HOME", "/tmp/eigener-ordner")])).unwrap();
        assert_eq!(frei.port(), 0);
    }

    // Ein Deep-Link schaltet die Oberfläche weiter, statt eine Datei dieses Namens zu laden
    #[test]
    fn pfad_wird_im_fenster_weitergeschaltet() {
        let script = pfad_script("/open/ticket/ARC-0042");
        assert!(script.contains(r#""/open/ticket/ARC-0042""#), "{script}");
        assert!(script.contains("history.pushState"), "{script}");
        assert!(script.contains("PopStateEvent"), "{script}");
        // Ein Pfad mit Anführungszeichen darf die Seite nicht zerschießen: er bleibt kodiert
        let boese = pfad_script("/open/\"); alert(1); //");
        assert!(boese.contains(r#"\"); alert(1)"#), "{boese}");
        assert!(boese.ends_with("//\");"), "{boese}");
    }

    // Adresse und Token gehen JSON-kodiert ins Fenster
    #[test]
    fn init_script_setzt_adresse_und_token() {
        let script = init_script(&Zugang {
            port: 4812,
            token: AppToken::from_text("gehei\"mes"),
        });
        assert!(
            script.contains(r#"base: "http://127.0.0.1:4812""#),
            "{script}"
        );
        assert!(script.contains(r#"token: "gehei\"mes""#), "{script}");
        assert!(script.starts_with("window.__NEXUS__ = "), "{script}");
    }

    // Die Meldung darf die Seite nicht zerschießen, auch wenn sie Anführungszeichen enthält
    #[test]
    fn meldung_wird_kodiert_und_wartet_auf_die_seite() {
        let script = status_script("Port \"4710\" ist belegt\n<script>");
        assert!(
            script.contains(r#""Port \"4710\" ist belegt\n<script>""#),
            "{script}"
        );
        assert!(
            script.contains("document.readyState==='loading'"),
            "{script}"
        );
        assert!(script.contains("getElementById('status')"), "{script}");
    }

    // Die Oberfläche kommt aus den Dateien der App; eine Freigabe für fremde Adressen gibt es
    // nicht mehr, sonst dürfte jede Seite auf einem lokalen Port die Tauri-Funktionen nutzen
    #[test]
    fn capability_gibt_keine_fremde_adresse_frei() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/ui.json")).unwrap();
        assert!(datei.get("remote").is_none(), "{datei}");
        assert_eq!(datei["windows"], serde_json::json!(["main"]));
    }

    // Ordnerauswahl, Öffnen und Systemmeldungen sind freigegeben, aber nur diese Befehle; Adressen nur für Browser und VS Code
    #[test]
    fn capability_gibt_nur_dialog_opener_und_meldungen_frei() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/ui.json")).unwrap();
        let rechte = datei["permissions"].as_array().unwrap();

        let namen: Vec<&str> = rechte
            .iter()
            .map(|recht| {
                recht
                    .as_str()
                    .unwrap_or_else(|| recht["identifier"].as_str().unwrap())
            })
            .collect();
        assert_eq!(
            namen,
            [
                "log:default",
                "notification:default",
                "dialog:allow-open",
                "opener:allow-reveal-item-in-dir",
                "opener:allow-open-url",
            ]
        );

        let adressen: Vec<&str> = rechte
            .iter()
            .find(|recht| recht["identifier"] == "opener:allow-open-url")
            .unwrap()["allow"]
            .as_array()
            .unwrap()
            .iter()
            .map(|eintrag| eintrag["url"].as_str().unwrap())
            .collect();
        assert_eq!(adressen, ["http://*", "https://*", "vscode://*"]);
    }

    // Mitgeliefert wird genau ein Programm: die Kommandozeile, die im Terminal auch ohne App
    // einen Server anbietet (`nexus serve`)
    #[test]
    fn die_kommandozeile_ist_als_externalbin_eingetragen() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let bins: Vec<&str> = datei["bundle"]["externalBin"]
            .as_array()
            .unwrap()
            .iter()
            .map(|eintrag| eintrag.as_str().unwrap())
            .collect();
        assert_eq!(bins, [format!("binaries/{NEXUS}")]);
    }

    // Das Hauptfenster entsteht erst in Rust, wenn Adresse und Token feststehen; in der Konfig
    // steht deshalb keines mehr
    #[test]
    fn die_konfig_legt_kein_fenster_mehr_an() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(datei["app"]["windows"], serde_json::json!([]));
        assert_eq!(datei["build"]["frontendDist"], "../../../packages/web/dist");
    }

    // Die Oberfläche spricht nur den lokalen Server an; alles andere lädt sie aus der App
    #[test]
    fn die_csp_laesst_nur_den_lokalen_server_zu() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let csp = datei["app"]["security"]["csp"].as_str().unwrap();
        assert!(csp.starts_with("default-src 'self';"), "{csp}");
        assert!(
            csp.contains("connect-src 'self' http://127.0.0.1:*"),
            "{csp}"
        );
        assert!(!csp.contains("http://192.168"), "{csp}");
        assert!(!csp.contains("unsafe-eval"), "{csp}");
    }
}
