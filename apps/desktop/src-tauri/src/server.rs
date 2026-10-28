// Zugang zum Harness-Server: Port bestimmen, Health prüfen, bei Bedarf den gebündelten Server starten,
// das Hauptfenster auf die Oberfläche schicken und den selbst gestarteten Server beim Beenden wieder stoppen.
// Ein fremder Server auf demselben Port bleibt unangetastet.
//
// Gestartet wird der Rust-Server (`nexus serve`); er nimmt den Port an und hält den Node-Server als
// eigenen Kindprozess dahinter. Beim Beenden genügt deshalb das Signal an `nexus serve` – es räumt
// den Node-Server selbst ab.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::async_runtime::Receiver;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

// Port, auf dem CLI und Hooks den Server erwarten (DEFAULT_SERVER_PORT in packages/core)
pub const DEFAULT_SERVER_PORT: u16 = 4710;
// Name des Rust-Servers aus bundle.externalBin; er nimmt den öffentlichen Port an
const NEXUS: &str = "nexus";
// Name des Node-Servers aus bundle.externalBin; er läuft als Kindprozess hinter dem Rust-Server
const SIDECAR: &str = "harness-sidecar";
// Daran erkennt sich der eigene Server in der Antwort von /api/health
const HEALTH_NAME: &str = "ai-harness";
// Prozessangaben des laufenden Servers im Harness-Ordner
const SERVER_FILE: &str = "server.json";
// Protokoll des Servers, auf das die Fehlermeldung verweist
const SERVER_LOG: &str = "server.log";
// So lange darf ein einzelner Health-Aufruf dauern
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
// So lange wartet die App auf den frisch gestarteten Server (wie ServerLauncher in der CLI)
const START_TIMEOUT: Duration = Duration::from_secs(10);
// Takt der Health-Aufrufe beim Warten (wie ServerLauncher in der CLI)
const POLL: Duration = Duration::from_millis(150);
// So viel Zeit bekommt der eigene Server, um nach SIGTERM selbst aufzuräumen
const STOP_TIMEOUT: Duration = Duration::from_secs(2);

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
    #[error("Der Server hat in {0} Sekunden nicht geantwortet. Einzelheiten stehen in {1}.")]
    NoAnswer(u64, String),
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

// Findet den Harness-Server auf seinem festen Port oder startet ihn als gebündeltes Programm
pub struct ServerConnector {
    home: PathBuf,
    port: u16,
}

impl ServerConnector {
    // Liest Harness-Ordner und Port aus der Umgebung
    pub fn from_env(env: &HashMap<String, String>) -> Result<Self, ServerError> {
        Ok(Self {
            home: harness_home(env),
            port: port_from_env(env)?,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    // Adresse, von der das Fenster die Oberfläche lädt; immer 127.0.0.1, nie localhost (IPv6 unter Windows)
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    // Prüft den eigenen Port
    pub async fn probe(&self) -> PortState {
        probe(self.port).await
    }

    // Wartet, bis auf dem Port der eigene Server antwortet; false, wenn die Frist abläuft.
    // Alle Aufrufe der Schleife teilen sich einen Client.
    pub async fn wait_for_ours(&self, frist: Duration) -> bool {
        let Some(client) = health_client() else {
            return false;
        };
        let deadline = Instant::now() + frist;
        loop {
            if probe_with(&client, self.port).await == PortState::Ours {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(POLL).await;
        }
    }

    // Prozessangaben des zuletzt gestarteten Servers
    pub fn server_info(&self) -> Option<ServerInfo> {
        read_server_info(&self.home)
    }
}

// Harness-Ordner: HARNESS_HOME, sonst ~/.harness (wie HarnessHome in packages/core)
pub fn harness_home(env: &HashMap<String, String>) -> PathBuf {
    match env.get("HARNESS_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => dirs::home_dir().unwrap_or_default().join(".harness"),
    }
}

// Port des Servers: HARNESS_PORT, sonst 4710. CLI und Hooks kennen nur diesen Port, darum kein zufälliger.
pub fn port_from_env(env: &HashMap<String, String>) -> Result<u16, ServerError> {
    let raw = match env.get("HARNESS_PORT") {
        Some(value) if !value.trim().is_empty() => value.trim(),
        _ => return Ok(DEFAULT_SERVER_PORT),
    };
    match raw.parse::<u16>() {
        Ok(port) if port > 0 => Ok(port),
        _ => Err(ServerError::PortInvalid(raw.to_string())),
    }
}

// Liest server.json; None, wenn es sie nicht gibt oder sie unlesbar ist
pub fn read_server_info(home: &Path) -> Option<ServerInfo> {
    let text = std::fs::read_to_string(home.join(SERVER_FILE)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(ServerInfo {
        pid: u32::try_from(value.get("pid")?.as_u64()?).ok()?,
        port: u16::try_from(value.get("port")?.as_u64()?).ok()?,
    })
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

// Der von der App gestartete Server; ein fremder Server steht hier nie drin
struct OwnServer {
    child: CommandChild,
    home: PathBuf,
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

// Beendet den selbst gestarteten Server; einen fremden lässt die App laufen
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<ServerState>() else {
        return;
    };
    let Some(own) = state.take() else {
        return;
    };
    let pid = own.child.pid();
    log::info!("Der selbst gestartete Server (pid {pid}) wird beendet.");
    if !stop_gracefully(pid) {
        log::warn!("Der Server (pid {pid}) hat nicht von allein aufgehört; er wird hart beendet.");
        if let Err(error) = own.child.kill() {
            log::warn!("Der Server (pid {pid}) ließ sich nicht beenden: {error}");
        }
    }
    remove_server_info(&own.home, pid);
}

// Server finden oder starten, dann das Fenster auf die Oberfläche schicken
async fn connect<R: Runtime>(app: &AppHandle<R>) -> Result<(), ServerError> {
    let connector = ServerConnector::from_env(&std::env::vars().collect())?;
    log::info!(
        "Harness-Ordner {}, Port {}",
        connector.home().display(),
        connector.port()
    );
    if let Some(info) = connector.server_info() {
        if info.port != connector.port() {
            log::info!(
                "{SERVER_FILE} nennt Port {} (pid {}); geprüft wird trotzdem zuerst Port {}.",
                info.port,
                info.pid,
                connector.port()
            );
        }
    }
    match connector.probe().await {
        PortState::Ours => log::info!(
            "Auf Port {} läuft schon ein Harness-Server; die App startet keinen zweiten.",
            connector.port()
        ),
        PortState::Free => start(app, &connector).await?,
        PortState::Foreign => return Err(ServerError::PortForeign(connector.port())),
    }
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
    navigate(app, &url_mit_pfad(&connector.url(), pfad.as_deref()))
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

// Startet den gebündelten Rust-Server samt Node-Server dahinter und wartet, bis er antwortet
async fn start<R: Runtime>(
    app: &AppHandle<R>,
    connector: &ServerConnector,
) -> Result<(), ServerError> {
    let mut command = app
        .shell()
        .sidecar(NEXUS)
        .map_err(|error| ServerError::StartFailed(format!("{NEXUS}: {error}")))?
        .args(["serve", "--port", &connector.port().to_string()]);
    // Den Node-Server gibt die App ausdrücklich mit; gefunden wird er wie jedes Sidecar neben dem
    // Programm. Ohne Fund sucht `nexus serve` selbst und meldet sonst einen Fehler.
    match node_sidecar() {
        Some(path) => command = command.args(["--node-sidecar", &path.to_string_lossy()]),
        None => log::warn!("{SIDECAR} liegt nicht neben dem Programm; nexus serve sucht selbst."),
    }
    for (name, dir) in resource_dirs(app) {
        command = command.env(name, dir);
    }
    let (events, child) = command
        .spawn()
        .map_err(|error| ServerError::StartFailed(format!("{NEXUS}: {error}")))?;
    log::info!(
        "Server gestartet (pid {}) auf Port {}.",
        child.pid(),
        connector.port()
    );
    app.state::<ServerState>().keep(OwnServer {
        child,
        home: connector.home().to_path_buf(),
    });
    tauri::async_runtime::spawn(log_output(events));
    if connector.wait_for_ours(START_TIMEOUT).await {
        return Ok(());
    }
    Err(ServerError::NoAnswer(
        START_TIMEOUT.as_secs(),
        connector.home().join(SERVER_LOG).display().to_string(),
    ))
}

// Pfad des Node-Servers, so wie Tauri Sidecars auflöst: die Datei aus bundle.externalBin liegt
// ohne Zielsuffix neben dem Programm. `None`, wenn es sie dort nicht gibt.
fn node_sidecar() -> Option<PathBuf> {
    let exe = tauri::utils::platform::current_exe().ok()?;
    let dir = exe.parent()?;
    let file = dir.join(if cfg!(windows) {
        format!("{SIDECAR}.exe")
    } else {
        SIDECAR.to_string()
    });
    file.is_file().then_some(file)
}

// Oberfläche, Benachrichtigungsbilder und SQL-Skripte liegen als Ressourcen neben der App;
// findet die App sie nicht, suchen die Server selbst neben ihrem Programm
fn resource_dirs<R: Runtime>(app: &AppHandle<R>) -> Vec<(&'static str, PathBuf)> {
    let base = match app.path().resource_dir() {
        Ok(dir) => dir,
        Err(error) => {
            log::warn!("Der Ressourcenordner ist nicht bestimmbar: {error}");
            return Vec::new();
        }
    };
    if !base.join("web").is_dir() {
        log::warn!(
            "In {} fehlt der Ordner web; der Server sucht die Oberfläche selbst.",
            base.display()
        );
        return Vec::new();
    }
    vec![
        ("HARNESS_WEB_DIR", base.join("web")),
        ("HARNESS_TOAST_DIR", base.join("toast")),
        ("HARNESS_SQL_DIR", base.join("sql")),
    ]
}

// Schreibt die Ausgabe des gestarteten Servers ins Protokoll der App
async fn log_output(mut events: Receiver<CommandEvent>) {
    while let Some(event) = events.recv().await {
        match event {
            CommandEvent::Stdout(line) => log::info!("Server: {}", text(&line)),
            CommandEvent::Stderr(line) => log::warn!("Server: {}", text(&line)),
            CommandEvent::Error(message) => log::warn!("Server: {message}"),
            CommandEvent::Terminated(ende) => log::info!(
                "Server beendet (Code {:?}, Signal {:?}).",
                ende.code,
                ende.signal
            ),
            _ => {}
        }
    }
}

// Eine Ausgabezeile des gestarteten Servers als Text
fn text(line: &[u8]) -> String {
    String::from_utf8_lossy(line).trim_end().to_string()
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

// Bittet den Server unter Unix mit SIGTERM um ein geordnetes Ende, damit er server.json selbst aufräumt;
// true, wenn er innerhalb der Frist gegangen ist
#[cfg(unix)]
fn stop_gracefully(pid: u32) -> bool {
    let id = pid as libc::pid_t;
    if unsafe { libc::kill(id, libc::SIGTERM) } != 0 {
        return true; // Es gibt den Prozess nicht mehr
    }
    let deadline = Instant::now() + STOP_TIMEOUT;
    while Instant::now() < deadline {
        if unsafe { libc::kill(id, 0) } != 0 {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

// Windows kennt kein SIGTERM; dort wird der Prozess hart beendet und server.json danach gelöscht
#[cfg(not(unix))]
fn stop_gracefully(_pid: u32) -> bool {
    false
}

// Löscht server.json, wenn sie noch den beendeten Server nennt (der Server selbst tut das nur bei SIGTERM)
fn remove_server_info(home: &Path, pid: u32) {
    if read_server_info(home).map(|info| info.pid) != Some(pid) {
        return;
    }
    let file = home.join(SERVER_FILE);
    match std::fs::remove_file(&file) {
        Ok(()) => log::info!("{} entfernt.", file.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => log::warn!("{} ließ sich nicht löschen: {error}", file.display()),
    }
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

    // Umgebung aus Paaren, damit die Tests nur setzen, was sie prüfen
    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect()
    }

    // Kleiner HTTP-Server, der auf jede Anfrage mit demselben Rumpf antwortet
    async fn fake_server(body: &'static str) -> u16 {
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

    #[test]
    fn port_aus_umgebung() {
        assert_eq!(port_from_env(&env(&[])).unwrap(), DEFAULT_SERVER_PORT);
        assert_eq!(
            port_from_env(&env(&[("HARNESS_PORT", "")])).unwrap(),
            DEFAULT_SERVER_PORT
        );
        assert_eq!(
            port_from_env(&env(&[("HARNESS_PORT", "4812")])).unwrap(),
            4812
        );
        assert_eq!(
            port_from_env(&env(&[("HARNESS_PORT", " 4812 ")])).unwrap(),
            4812
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
        let gesetzt = harness_home(&env(&[("HARNESS_HOME", "/tmp/eigener-ordner")]));
        assert_eq!(gesetzt, PathBuf::from("/tmp/eigener-ordner"));
        let standard = harness_home(&env(&[]));
        assert!(
            standard.ends_with(".harness"),
            "{standard:?} sollte auf .harness enden"
        );
    }

    #[test]
    fn server_json_wird_gelesen() {
        let home = std::env::temp_dir().join(format!("nexus-server-json-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(read_server_info(&home), None);
        std::fs::write(
            home.join(SERVER_FILE),
            r#"{"pid":4242,"port":4812,"startedAt":"2026-10-28T08:00:00.000Z"}"#,
        )
        .unwrap();
        assert_eq!(
            read_server_info(&home),
            Some(ServerInfo {
                pid: 4242,
                port: 4812
            })
        );
        std::fs::write(home.join(SERVER_FILE), "kein json").unwrap();
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

    #[tokio::test]
    async fn warten_endet_mit_der_frist() {
        let port = free_port().await;
        let connector = ServerConnector {
            home: PathBuf::from("/tmp"),
            port,
        };
        assert!(!connector.wait_for_ours(Duration::from_millis(1)).await);
    }

    #[test]
    fn connector_zeigt_auf_die_eigene_adresse() {
        let connector = ServerConnector::from_env(&env(&[
            ("HARNESS_HOME", "/tmp/eigener-ordner"),
            ("HARNESS_PORT", "4812"),
        ]))
        .unwrap();
        assert_eq!(connector.url(), "http://127.0.0.1:4812/");
        assert_eq!(connector.home(), Path::new("/tmp/eigener-ordner"));
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

    // Beide Programme werden mitgeliefert: der Rust-Server, den die App startet, und der Node-Server,
    // den er als Kindprozess dahinter hält
    #[test]
    fn beide_server_sind_als_externalbin_eingetragen() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let bins: Vec<&str> = datei["bundle"]["externalBin"]
            .as_array()
            .unwrap()
            .iter()
            .map(|eintrag| eintrag.as_str().unwrap())
            .collect();
        assert_eq!(
            bins,
            [format!("binaries/{NEXUS}"), format!("binaries/{SIDECAR}")]
        );
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
