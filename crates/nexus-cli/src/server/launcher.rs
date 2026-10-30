//! Erkennt, ob der eigene Server läuft, und startet ihn bei Bedarf als abgelöstes
//! Hintergrundprogramm. Portiert aus `packages/cli/src/board/ServerLauncher.ts`; dazu kommt
//! der Start der Nexus-App, die ihren Port selbst wählt.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use nexus_core::registry::ServerInfoFile;
use nexus_core::{HarnessError, HarnessHome, Result};

use crate::http::LocalHttp;
use crate::server::app_locator::{AppLocator, DesktopApp, Launch};

/// Name, mit dem sich der eigene Server in `GET /api/health` meldet
const OWN_NAME: &str = "ai-harness";

/// Wie lange auf einen gestarteten Server gewartet wird
const WAIT: Duration = Duration::from_secs(10);

/// Abstand zweier Prüfungen beim Warten
const STEP: Duration = Duration::from_millis(150);

/// Wartezeit eines Aufrufs an den Server
const TIMEOUT: Duration = Duration::from_secs(2);

/// Zustand eines Ports
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    /// Der eigene Server antwortet
    Ours,
    /// Niemand hört auf dem Port
    Free,
    /// Ein fremdes Programm hört auf dem Port
    Foreign,
}

/// Ein Startweg: Programm, Argumente und zusätzliche Umgebungsvariablen
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// Startet den Server bzw. die App und wartet, bis sie antworten.
#[derive(Debug, Clone)]
pub struct ServerLauncher {
    env: HashMap<String, String>,
    home: HarnessHome,
    /// Das laufende `nexus`-Binary; es startet den Server mit `serve`
    exe: Option<PathBuf>,
    http: LocalHttp,
}

impl ServerLauncher {
    /// Starter mit der Umgebung dieses Prozesses
    pub fn from_process_env() -> Self {
        Self::new(
            std::env::vars().collect(),
            HarnessHome::from_process_env(),
            std::env::current_exe().ok(),
        )
    }

    /// Starter mit frei wählbarer Umgebung (Tests)
    pub fn new(env: HashMap<String, String>, home: HarnessHome, exe: Option<PathBuf>) -> Self {
        Self {
            env,
            home,
            exe,
            http: LocalHttp::new(TIMEOUT),
        }
    }

    /// Prüft den Port: eigener Server, frei oder von einem fremden Programm belegt
    pub fn probe(&self, port: u16) -> PortState {
        match self.http.get(port, "/api/health") {
            Ok(response) => {
                let name = response
                    .json()
                    .and_then(|json| json["name"].as_str().map(ToString::to_string));
                if name.as_deref() == Some(OWN_NAME) {
                    PortState::Ours
                } else {
                    PortState::Foreign
                }
            }
            Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => PortState::Free,
            // Windows lehnt eine Verbindung zu einem geschlossenen Port erst nach etwa 2 s ab, dann
            // läuft die Wartezeit vorher ab. Lässt sich der Port binden, lauscht dort niemand.
            Err(error) if error.kind() == io::ErrorKind::TimedOut && Self::bindable(port) => {
                PortState::Free
            }
            Err(_) => PortState::Foreign,
        }
    }

    /// Ob sich der Port auf 127.0.0.1 binden lässt, also kein Programm auf ihm lauscht
    fn bindable(port: u16) -> bool {
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    /// Startet den Server auf dem Port und wartet, bis er antwortet
    pub fn start(&self, port: u16) -> Result<()> {
        self.start_waiting(port, WAIT)
    }

    /// Wie `start`, aber mit eigener Wartezeit (Tests)
    pub fn start_waiting(&self, port: u16, wait: Duration) -> Result<()> {
        let start = self.start_command(port).ok_or_else(Self::missing)?;
        self.spawn(&start)?;
        self.wait_for(|| (self.probe(port) == PortState::Ours).then_some(()), wait)
            .ok_or_else(|| self.start_failed())?;
        Ok(())
    }

    /// Startet die installierte Nexus-App und liefert den Port, den sie sich gewählt hat.
    /// Die App schreibt ihn wie der Server in `server.json`.
    pub fn start_app(&self, app: &DesktopApp) -> Result<u16> {
        self.spawn(&Self::app_command(app))?;
        let info = ServerInfoFile::new(self.home.clone());
        self.wait_for(
            || {
                info.read()
                    .filter(|info| self.probe(info.port) == PortState::Ours)
                    .map(|info| info.port)
            },
            WAIT,
        )
        .ok_or_else(|| self.start_failed())
    }

    /// Die installierte App, falls es eine gibt
    pub fn app(&self) -> Option<DesktopApp> {
        AppLocator::new(self.env.clone(), self.user_home()).locate()
    }

    /// Startweg der App: ein macOS-Bündel über `open -a`, sonst die Datei selbst
    pub fn app_command(app: &DesktopApp) -> StartCommand {
        match app.launch {
            Launch::Bundle => StartCommand {
                program: PathBuf::from("open"),
                args: vec!["-a".into(), app.path.to_string_lossy().to_string()],
                env: Vec::new(),
            },
            Launch::Program => StartCommand {
                program: app.path.clone(),
                args: Vec::new(),
                env: Vec::new(),
            },
        }
    }

    /// Der Startweg des Servers: immer dieses Binary mit `serve`
    pub fn start_command(&self, port: u16) -> Option<StartCommand> {
        let program = self.exe.clone()?;
        let args = vec!["serve".to_string(), "--port".to_string(), port.to_string()];
        Some(StartCommand {
            program,
            args,
            env: Vec::new(),
        })
    }

    /// Startet das Programm abgelöst: eigene Prozessgruppe unter Unix, unter Windows ohne
    /// Konsolenfenster und ohne Bindung an dieses Terminal; Ein- und Ausgaben nach `null`.
    /// Die Standard-Handles von `nexus` selbst erbt es nicht, `main` schaltet ihre Vererbung
    /// beim Start ab (`nexus_platform::stdio`)
    fn spawn(&self, start: &StartCommand) -> Result<()> {
        let mut command = Command::new(&start.program);
        command
            .args(&start.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (name, value) in &start.env {
            command.env(name, value);
        }
        detach(&mut command);
        command.spawn().map(|_| ()).map_err(|error| {
            HarnessError::new(
                "server-start-failed",
                format!(
                    "Der Server ist nicht gestartet ({}: {error}). Details stehen in {}",
                    Self::text(&start.program),
                    Self::text(&self.home.log_file())
                ),
            )
            .into()
        })
    }

    /// Wartet in Schritten von 150 ms, bis die Prüfung einen Wert liefert
    fn wait_for<T>(&self, mut check: impl FnMut() -> Option<T>, wait: Duration) -> Option<T> {
        let deadline = Instant::now() + wait;
        loop {
            if let Some(value) = check() {
                return Some(value);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(STEP);
        }
    }

    /// Fehler, wenn der Server nicht antwortet
    fn start_failed(&self) -> nexus_core::Error {
        HarnessError::new(
            "server-start-failed",
            format!(
                "Der Server ist nicht gestartet. Details stehen in {}",
                Self::text(&self.home.log_file())
            ),
        )
        .into()
    }

    /// Fehler, wenn es überhaupt keinen Startweg gibt
    fn missing() -> nexus_core::Error {
        HarnessError::new(
            "server-missing",
            "Kein Server gefunden. Bitte `nexus install` ausführen oder die Nexus-App installieren.",
        )
        .into()
    }

    /// Benutzerordner für die Suche nach der App
    fn user_home(&self) -> PathBuf {
        #[allow(deprecated)]
        std::env::home_dir().unwrap_or_default()
    }

    /// Pfad als Text für Argumente und Meldungen
    fn text(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }
}

/// Löst den Kindprozess von diesem Terminal: eigene Prozessgruppe unter Unix
#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// Löst den Kindprozess unter Windows: kein Konsolenfenster, keine Bindung an dieses Terminal
#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    /// DETACHED_PROCESS | CREATE_NO_WINDOW
    const FLAGS: u32 = 0x0000_0008 | 0x0800_0000;
    command.creation_flags(FLAGS);
}

/// Hält `wait_for`, `probe` und `env` auch ohne Netz prüfbar
#[cfg(test)]
mod tests {
    use super::*;

    /// Das laufende Binary in den Tests; es wird nie gestartet, nur im Startweg erwartet
    fn exe(dir: &Path) -> PathBuf {
        dir.join(if cfg!(windows) { "nexus.exe" } else { "nexus" })
    }

    fn starter(paare: &[(&str, &str)], home: &Path, exe: Option<PathBuf>) -> ServerLauncher {
        ServerLauncher::new(
            paare
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            HarnessHome::at(home),
            exe,
        )
    }

    #[test]
    fn gestartet_wird_immer_dieses_binary_mit_serve() {
        let starter = starter(&[], Path::new("/h"), Some(exe(Path::new("/opt/nexus"))));
        let start = starter.start_command(4800).unwrap();
        assert_eq!(start.program, exe(Path::new("/opt/nexus")));
        assert_eq!(start.args, vec!["serve", "--port", "4800"]);
        assert!(start.env.is_empty());
    }

    #[test]
    fn ohne_eigenes_binary_fehlt_der_server() {
        let dir = tempfile::Builder::new()
            .prefix("nexus-h-")
            .tempdir()
            .unwrap();
        let starter = starter(&[], dir.path(), None);
        assert_eq!(starter.start_command(4710), None);

        let error = starter.start_waiting(4710, Duration::ZERO).unwrap_err();
        assert_eq!(error.code(), Some("server-missing"));
        assert_eq!(
            error.to_string(),
            "Kein Server gefunden. Bitte `nexus install` ausführen oder die Nexus-App installieren."
        );
    }

    #[test]
    fn ein_freier_port_gilt_als_frei() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert_eq!(
            starter(&[], Path::new("/h"), None).probe(port),
            PortState::Free
        );
    }

    #[test]
    fn ein_programm_das_lauscht_aber_nicht_antwortet_gilt_als_fremd() {
        // Die Wartezeit läuft ab wie bei einem geschlossenen Port unter Windows; der Port ist
        // aber belegt und darf nicht als frei gelten
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_eq!(
            starter(&[], Path::new("/h"), None).probe(port),
            PortState::Foreign
        );
        drop(listener);
    }

    #[test]
    fn ein_macos_buendel_wird_mit_open_gestartet() {
        let app = DesktopApp {
            path: PathBuf::from("/Applications/Nexus.app"),
            launch: Launch::Bundle,
        };
        let start = ServerLauncher::app_command(&app);
        assert_eq!(start.program, Path::new("open"));
        assert_eq!(start.args, vec!["-a", "/Applications/Nexus.app"]);
    }

    #[test]
    fn eine_ausfuehrbare_app_wird_direkt_gestartet() {
        let app = DesktopApp {
            path: PathBuf::from("/opt/Nexus"),
            launch: Launch::Program,
        };
        let start = ServerLauncher::app_command(&app);
        assert_eq!(start.program, Path::new("/opt/Nexus"));
        assert!(start.args.is_empty());
    }

    #[test]
    fn das_warten_endet_mit_server_start_failed() {
        let starter = starter(&[], Path::new("/h"), None);
        assert_eq!(starter.wait_for(|| None::<u8>, Duration::ZERO), None);
        let error = starter.start_failed();
        assert_eq!(error.code(), Some("server-start-failed"));
        assert!(error.to_string().ends_with("server.log"), "{}", error);
    }
}
