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

/// Dateiname der gebündelten Programmdatei aus P1-2 (`externalBin` in `tauri.conf.json`)
const SIDECAR: &str = "harness-sidecar";

/// Programmdatei des Servers im Plugin bzw. im Benutzerordner
const SERVER_ENTRY: &str = "server.mjs";

/// Umgebungsvariable, mit der Tests eine eigene Programmdatei vorgeben
pub const SERVER_ENTRY_ENV: &str = "HARNESS_SERVER_ENTRY";

/// Umgebungsvariable mit dem Ordner der Oberfläche
pub const WEB_DIR_ENV: &str = "HARNESS_WEB_DIR";

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

/// Die gefundene Programmdatei des Node-Servers samt der Umgebung, die sie braucht
#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeServer {
    program: PathBuf,
    env: Vec<(String, String)>,
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
            Err(_) => PortState::Foreign,
        }
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

    /// Der Startweg des Servers: immer dieses Binary mit `serve`, also Rust auf dem
    /// öffentlichen Port und der Node-Server als Kindprozess dahinter.
    ///
    /// Gefunden wird nur noch die Programmdatei des Node-Servers; findet sie dieser Starter
    /// nicht, sucht `nexus serve` selbst eine neben dem Programm.
    pub fn start_command(&self, port: u16) -> Option<StartCommand> {
        let program = self.exe.clone()?;
        let mut args = vec!["serve".to_string(), "--port".to_string(), port.to_string()];
        let mut env = Vec::new();
        if let Some(node) = self.node_server() {
            args.push("--node-sidecar".to_string());
            args.push(Self::text(&node.program));
            env = node.env;
        }
        Some(StartCommand { program, args, env })
    }

    /// Programmdatei des Node-Servers: Umgebung (Tests), gebündelte Programmdatei aus P1-2,
    /// `lib/server.mjs` im Plugin oder im Benutzerordner
    fn node_server(&self) -> Option<NodeServer> {
        if let Some(entry) = self.var(SERVER_ENTRY_ENV) {
            return Some(NodeServer {
                program: PathBuf::from(entry),
                env: Vec::new(),
            });
        }
        for root in self.roots() {
            let sidecar = root.join(if cfg!(windows) {
                format!("{SIDECAR}.exe")
            } else {
                SIDECAR.to_string()
            });
            if sidecar.is_file() {
                return Some(NodeServer {
                    program: sidecar,
                    env: Vec::new(),
                });
            }
        }
        for root in self.roots() {
            let entry = root.join("lib").join(SERVER_ENTRY);
            if entry.is_file() {
                // Im Plugin liegt die Oberfläche neben `lib/`; ist sie schon gesetzt, gilt die Einstellung
                let mut env = Vec::new();
                if self.var(WEB_DIR_ENV).is_none() {
                    env.push((WEB_DIR_ENV.to_string(), Self::text(&root.join("web"))));
                }
                return Some(NodeServer {
                    program: entry,
                    env,
                });
            }
        }
        None
    }

    /// Ordner, in denen Programmdatei und Oberfläche liegen können: neben dem Binary, einen
    /// Ordner darüber (Plugin: `lib/` neben `web/`) und `<benutzerordner>/app`
    fn roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(dir) = self.exe.as_ref().and_then(|exe| exe.parent()) {
            roots.push(dir.to_path_buf());
            if let Some(parent) = dir.parent() {
                roots.push(parent.to_path_buf());
            }
        }
        roots.push(self.home.dir().join("app"));
        roots
    }

    /// Startet das Programm abgelöst: eigene Prozessgruppe unter Unix, unter Windows ohne
    /// Konsolenfenster und ohne Bindung an dieses Terminal; Ein- und Ausgaben nach `null`
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

    /// Eine Umgebungsvariable, leere zählen wie keine
    fn var(&self, name: &str) -> Option<String> {
        self.env
            .get(name)
            .filter(|value| !value.is_empty())
            .cloned()
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
    fn die_umgebungsvariable_nennt_den_node_server() {
        let starter = starter(
            &[(SERVER_ENTRY_ENV, "/s/main.js")],
            Path::new("/h"),
            Some(exe(Path::new("/opt/nexus"))),
        );
        let start = starter.start_command(4800).unwrap();
        assert_eq!(start.program, exe(Path::new("/opt/nexus")));
        assert_eq!(
            start.args,
            vec!["serve", "--port", "4800", "--node-sidecar", "/s/main.js"]
        );
        assert!(start.env.is_empty());
    }

    #[test]
    fn danach_kommt_die_gebuendelte_programmdatei() {
        let dir = tempfile::Builder::new()
            .prefix("nexus-s-")
            .tempdir()
            .unwrap();
        let name = if cfg!(windows) {
            "harness-sidecar.exe"
        } else {
            "harness-sidecar"
        };
        std::fs::write(dir.path().join(name), b"x").unwrap();

        let starter = starter(&[], Path::new("/h"), Some(exe(dir.path())));
        let start = starter.start_command(4710).unwrap();
        assert_eq!(start.program, exe(dir.path()));
        assert_eq!(
            start.args,
            vec![
                "serve".to_string(),
                "--port".to_string(),
                "4710".to_string(),
                "--node-sidecar".to_string(),
                dir.path().join(name).to_string_lossy().to_string(),
            ]
        );
    }

    #[test]
    fn danach_kommt_lib_server_mjs_mit_dem_ordner_der_oberflaeche() {
        let dir = tempfile::Builder::new()
            .prefix("nexus-s-")
            .tempdir()
            .unwrap();
        std::fs::create_dir_all(dir.path().join("lib")).unwrap();
        std::fs::write(dir.path().join("lib").join("server.mjs"), b"x").unwrap();

        let starter = starter(&[], Path::new("/h"), Some(exe(dir.path())));
        let start = starter.start_command(4710).unwrap();
        assert_eq!(start.program, exe(dir.path()));
        assert_eq!(
            start.args.last().unwrap(),
            &dir.path()
                .join("lib")
                .join("server.mjs")
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(
            start.env,
            vec![(
                WEB_DIR_ENV.to_string(),
                dir.path().join("web").to_string_lossy().to_string()
            )]
        );
    }

    #[test]
    fn ein_gesetzter_ordner_der_oberflaeche_bleibt_unberuehrt() {
        let dir = tempfile::Builder::new()
            .prefix("nexus-s-")
            .tempdir()
            .unwrap();
        std::fs::create_dir_all(dir.path().join("lib")).unwrap();
        std::fs::write(dir.path().join("lib").join("server.mjs"), b"x").unwrap();

        let starter = starter(
            &[(WEB_DIR_ENV, "/eigen/web")],
            Path::new("/h"),
            Some(exe(dir.path())),
        );
        assert!(starter.start_command(4710).unwrap().env.is_empty());
    }

    #[test]
    fn die_programmdatei_wird_auch_im_benutzerordner_gesucht() {
        let dir = tempfile::Builder::new()
            .prefix("nexus-h-")
            .tempdir()
            .unwrap();
        std::fs::create_dir_all(dir.path().join("app").join("lib")).unwrap();
        std::fs::write(dir.path().join("app").join("lib").join("server.mjs"), b"x").unwrap();

        let starter = starter(&[], dir.path(), Some(exe(Path::new("/opt/nexus"))));
        let start = starter.start_command(4710).unwrap();
        assert!(
            start.args.last().unwrap().ends_with("server.mjs"),
            "{:?}",
            start.args
        );
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
