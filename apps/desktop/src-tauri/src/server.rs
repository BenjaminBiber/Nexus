// Zugang zum Harness-Server: einen schon laufenden Server übernehmen oder den Server im eigenen
// Prozess starten, das Hauptfenster auf die Oberfläche schicken und den selbst gestarteten Server
// beim Beenden wieder stoppen. Einen fremden Server lässt die App unangetastet.
//
// Seit P5-2 läuft der Server nicht mehr als Kindprozess, sondern als Bibliothek auf einer eigenen
// tokio-Laufzeit im Prozess der App. Ohne App übernimmt `nexus serve` dieselbe Aufgabe.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use nexus_core::registry::{HarnessHome, ServerInfoFile};
use nexus_platform::Platform;
use nexus_server::config::{ServerConfig, ServerConfigLoader};
use nexus_server::ServerHandle;
use tauri::{AppHandle, Manager, Runtime};

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
}

// Zustand eines Ports: eigener Server, frei oder von einem fremden Programm belegt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    Ours,
    Free,
    Foreign,
}

// Prozessangaben aus server.json; nur ein Hinweis, maßgeblich ist der Health-Aufruf
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerInfo {
    pub pid: u32,
    pub port: u16,
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

    // Adresse, von der das Fenster die Oberfläche lädt; immer 127.0.0.1, nie localhost (IPv6 unter Windows)
    pub fn url(&self, port: u16) -> String {
        format!("http://127.0.0.1:{port}/")
    }

    // Prozessangaben des zuletzt gestarteten Servers
    pub fn server_info(&self) -> Option<ServerInfo> {
        read_server_info(&self.home)
    }

    // Der schon laufende Server dieses Datenordners, falls es ihn gibt: `server.json` nennt ihn,
    // seine Prozessnummer lebt, und unter seinem Port antwortet genau dieser Prozess auf
    // `/api/health`. Die Prozessnummer verknüpft die Antwort mit dem eigenen Datenordner.
    pub async fn vorhandener_server(&self) -> Option<u16> {
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
            Some(pid) if pid == info.pid => Some(info.port),
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

// Richtet den Serverzugang ein: Der Start läuft im Hintergrund, damit das Fenster sofort die Startseite zeigt.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    app.manage(ServerState::default());
    watch_signals(app);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = connect(&handle).await {
            log::error!("{error}");
            show_error(&handle, &error.to_string());
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

// Server finden oder starten, dann das Fenster auf die Oberfläche schicken
async fn connect<R: Runtime>(app: &AppHandle<R>) -> Result<(), ServerError> {
    let connector = ServerConnector::from_env(&std::env::vars().collect())?;
    log::info!("Harness-Ordner {}", connector.home().display());
    let port = match connector.vorhandener_server().await {
        Some(port) => {
            log::info!("Auf Port {port} läuft schon ein Harness-Server; die App nutzt ihn mit.");
            port
        }
        None => start(app, &connector).await?,
    };
    setze_port(port);
    // Ab hier nimmt die Oberfläche Deep-Links selbst an; ein vorher gemerkter Pfad kommt jetzt dran
    let pfad = match app.try_state::<ServerState>() {
        Some(state) => {
            state
                .ready
                .store(true, std::sync::atomic::Ordering::Relaxed);
            state.nimm_pfad()
        }
        None => None,
    };
    navigate(app, &url_mit_pfad(&connector.url(port), pfad.as_deref()))
}

// Adresse der Oberfläche mit einem Pfad dahinter; ohne Pfad bleibt es die Startadresse
pub fn url_mit_pfad(base: &str, path: Option<&str>) -> String {
    match path {
        Some(path) => format!("{}{}", base.trim_end_matches('/'), path),
        None => base.to_string(),
    }
}

// Schickt das Fenster auf einen Pfad der Oberfläche. Läuft der Server noch nicht, wird der Pfad
// gemerkt und nach dem ersten Laden angewendet.
pub fn open_path<R: Runtime>(app: &AppHandle<R>, path: &str) {
    let Some(state) = app.try_state::<ServerState>() else {
        return;
    };
    if !state.ready.load(std::sync::atomic::Ordering::Relaxed) {
        log::info!("Der Server ist noch nicht bereit; {path} wird gemerkt.");
        state.merke_pfad(path);
        return;
    }
    let url = url_mit_pfad(&crate::tray::basis_url(), Some(path));
    if let Err(error) = navigate(app, &url) {
        log::warn!("{url} ließ sich nicht öffnen: {error}");
    }
}

// Startet den Server im eigenen Prozess auf einer eigenen Laufzeit und liefert seinen Port
async fn start<R: Runtime>(
    app: &AppHandle<R>,
    connector: &ServerConnector,
) -> Result<u16, ServerError> {
    // Ist ein fester Port gewünscht und hört dort ein fremdes Programm, startet die App nicht
    let wunsch = connector.port();
    if wunsch != 0 && probe(wunsch).await == PortState::Foreign {
        return Err(ServerError::PortForeign(wunsch));
    }
    let config = server_config(app, connector)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(SERVER_THREADS)
        .enable_all()
        .thread_name("nexus-server")
        .build()
        .map_err(|error| ServerError::StartFailed(error.to_string()))?;
    let home = config.home.clone();
    let platform = match config.is_contract() {
        true => Platform::contract(),
        false => Platform::local(),
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    runtime.spawn(async move {
        let board = nexus_server::board_from_config(home);
        let _ = sender.send(
            nexus_server::serve(config, board, platform)
                .await
                .map_err(|error| error.to_string()),
        );
    });
    let handle = receiver
        .await
        .map_err(|_| ServerError::StartFailed("Der Server hat nicht geantwortet.".to_string()))?
        .map_err(ServerError::StartFailed)?;
    let port = handle.port;
    log::info!("Server im Prozess der App gestartet (Port {port}).");
    app.state::<ServerState>()
        .keep(OwnServer { handle, runtime });
    Ok(port)
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

// Schickt das Hauptfenster auf die Oberfläche des Servers
fn navigate<R: Runtime>(app: &AppHandle<R>, url: &str) -> Result<(), ServerError> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| ServerError::StartFailed("Das Hauptfenster gibt es nicht.".to_string()))?;
    let address = url
        .parse()
        .map_err(|error| ServerError::StartFailed(format!("{url} ist keine Adresse: {error}")))?;
    window
        .navigate(address)
        .map_err(|error| ServerError::StartFailed(error.to_string()))?;
    log::info!("Das Fenster lädt {url}");
    Ok(())
}

// Zeigt den Fehler auf der Startseite. Der Fehler kann auftreten, bevor die Seite steht; darum merkt sich
// die App die Meldung und trägt sie in `on_page_load` noch einmal nach.
fn show_error<R: Runtime>(app: &AppHandle<R>, message: &str) {
    if let Some(state) = app.try_state::<ServerState>() {
        state.remember(message);
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.eval(status_script(message)) {
        log::warn!("Die Meldung ließ sich nicht anzeigen: {error}");
    }
}

// Die Seite ist fertig geladen: eine vorher entstandene Meldung nachtragen
pub fn on_page_load<R: Runtime>(
    webview: &tauri::Webview<R>,
    payload: &tauri::webview::PageLoadPayload<'_>,
) {
    if payload.event() != tauri::webview::PageLoadEvent::Finished {
        return;
    }
    let Some(message) = webview
        .try_state::<ServerState>()
        .and_then(|state| state.message())
    else {
        return;
    };
    if let Err(error) = webview.eval(status_script(&message)) {
        log::warn!("Die Meldung ließ sich nicht anzeigen: {error}");
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
                port: 4812
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
        assert_eq!(connector.vorhandener_server().await, Some(port));
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
        assert_eq!(connector.url(4812), "http://127.0.0.1:4812/");
        assert_eq!(connector.home(), Path::new("/tmp/eigener-ordner"));

        // Ohne Portwunsch bindet der Server Port 0 und sucht sich selbst einen freien
        let frei =
            ServerConnector::from_env(&env(&[("HARNESS_HOME", "/tmp/eigener-ordner")])).unwrap();
        assert_eq!(frei.port(), 0);
    }

    // Ein Deep-Link hängt seinen Pfad an die Adresse der Oberfläche, ohne doppelten Schrägstrich
    #[test]
    fn adresse_mit_pfad() {
        assert_eq!(
            url_mit_pfad("http://127.0.0.1:4710/", Some("/open/ticket/ARC-0042")),
            "http://127.0.0.1:4710/open/ticket/ARC-0042"
        );
        assert_eq!(
            url_mit_pfad("http://127.0.0.1:4710", Some("/open/question/ARC-F0001")),
            "http://127.0.0.1:4710/open/question/ARC-F0001"
        );
        assert_eq!(
            url_mit_pfad("http://127.0.0.1:4710/", None),
            "http://127.0.0.1:4710/"
        );
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

    // Die Oberfläche kommt vom lokalen Server; nur diese Herkunft darf Tauri-Funktionen nutzen
    #[test]
    fn capability_gibt_nur_den_lokalen_server_frei() {
        use tauri::utils::acl::RemoteUrlPattern;

        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/remote.json")).unwrap();
        let urls: Vec<String> = datei["remote"]["urls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|url| url.as_str().unwrap().to_string())
            .collect();
        assert_eq!(urls, ["http://127.0.0.1:*", "http://localhost:*"]);

        let patterns: Vec<RemoteUrlPattern> = urls.iter().map(|url| url.parse().unwrap()).collect();
        let passt = |url: &str| {
            let address = url.parse().unwrap();
            patterns.iter().any(|pattern| pattern.test(&address))
        };
        assert!(passt("http://127.0.0.1:4710/"));
        assert!(passt("http://127.0.0.1:4812/board"));
        assert!(passt("http://localhost:4710/"));
        assert!(!passt("http://192.168.0.5:4710/"));
        assert!(!passt("https://example.com/"));
    }

    // Ordnerauswahl, Öffnen und Systemmeldungen sind freigegeben, aber nur diese Befehle; Adressen nur für Browser und VS Code
    #[test]
    fn capability_gibt_nur_dialog_opener_und_meldungen_frei() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/remote.json")).unwrap();
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
                "core:default",
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

    // Das Hauptfenster darf nicht gedrosselt werden, sonst schläft der Ereignisstrom im Hintergrund ein
    #[test]
    fn hauptfenster_wird_im_hintergrund_nicht_gedrosselt() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let fenster = datei["app"]["windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fenster| fenster["label"] == "main")
            .unwrap();
        assert_eq!(fenster["backgroundThrottling"], "disabled");
    }
}
