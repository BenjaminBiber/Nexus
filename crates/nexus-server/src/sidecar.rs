//! Der Node-Server als Kindprozess hinter dem Rust-Server.
//!
//! Der Rust-Server nimmt den öffentlichen Port an und beantwortet, was in Rust schon umgesetzt
//! ist; alles andere geht über den Proxy ([`crate::proxy`]) an diesen Kindprozess. Der Node-Server
//! läuft dafür im Sidecar-Modus: Er wählt sich einen freien Port, meldet ihn auf stdout, schreibt
//! keine eigene `server.json` und beendet sich, sobald seine stdin endet – also sobald der
//! Rust-Prozess weg ist, auch bei einem harten Ende.
//!
//! Abgeräumt wird auf dem jeweils üblichen Weg (siehe [`nexus_core::process::ProcessGroup`]):
//! unter Unix eine eigene Prozessgruppe mit SIGTERM und nach einer Frist SIGKILL, unter Windows
//! ein Job-Objekt mit „Kill on job close“.

use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::body::Body;
use http_body_util::BodyExt;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nexus_core::process::ProcessGroup;
use nexus_core::{HarnessError, HarnessHome, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::config::DispatchBackend;
use crate::log::ServerLog;

/// Zeile, mit der der Node-Server seinen gewählten Port meldet
pub const PORT_LINE: &str = "HARNESS_SIDECAR_PORT=";

/// Name, mit dem sich der Node-Server in `GET /api/health` meldet
const OWN_NAME: &str = "ai-harness";

/// Dateiname der gebündelten Node-Programmdatei (`bundle.externalBin` in `tauri.conf.json`)
const BUNDLED: &str = "harness-sidecar";

/// So lange darf der Node-Server brauchen, bis er seinen Port meldet
const PORT_WAIT: Duration = Duration::from_secs(15);

/// So lange darf er danach brauchen, bis `GET /api/health` antwortet
const HEALTH_WAIT: Duration = Duration::from_secs(10);

/// Abstand zweier Health-Aufrufe beim Warten
const HEALTH_STEP: Duration = Duration::from_millis(150);

/// So viel Zeit bekommt der Node-Server nach SIGTERM, um selbst aufzuräumen
const STOP_WAIT: Duration = Duration::from_secs(5);

/// Womit der Node-Server gestartet wird: Programm und die Argumente vor `--port 0`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl SidecarSpec {
    /// Sucht die Programmdatei: Angabe aus `--node-sidecar`, sonst `NEXUS_NODE_SIDECAR`, sonst
    /// die gebündelte Datei neben dem laufenden Programm.
    ///
    /// `exe_dir` ist der Ordner des laufenden Programms; Tests geben ihn selbst vor.
    pub fn resolve(
        explicit: Option<&str>,
        env: &dyn Fn(&str) -> Option<String>,
        exe_dir: Option<&Path>,
    ) -> Result<Self> {
        let given = explicit
            .map(ToString::to_string)
            .or_else(|| env("NEXUS_NODE_SIDECAR"))
            .filter(|path| !path.is_empty());
        if let Some(path) = given {
            return Ok(Self::for_path(Path::new(&path), env));
        }
        match exe_dir.and_then(bundled_beside) {
            Some(path) => Ok(Self::for_path(&path, env)),
            None => Err(HarnessError::new(
                "node-sidecar-missing",
                "Der Node-Server wurde nicht gefunden. Bitte seinen Pfad mit --node-sidecar angeben.",
            )
            .into()),
        }
    }

    /// Startweg für eine Programmdatei: ein JavaScript-Modul läuft unter `node` (bzw.
    /// `NEXUS_NODE`), eine gebündelte Programmdatei startet selbst und wählt mit `serve` ihren
    /// Serverteil
    fn for_path(path: &Path, env: &dyn Fn(&str) -> Option<String>) -> Self {
        let script = matches!(
            path.extension().and_then(|end| end.to_str()),
            Some("js" | "mjs" | "cjs")
        );
        if script {
            let node = env("NEXUS_NODE")
                .filter(|node| !node.is_empty())
                .unwrap_or_else(|| "node".to_string());
            return Self {
                program: PathBuf::from(node),
                args: vec![text(path)],
            };
        }
        Self {
            program: path.to_path_buf(),
            args: vec!["serve".to_string()],
        }
    }
}

/// Der laufende Node-Server hinter dem Rust-Server
#[derive(Debug)]
pub struct NodeSidecar {
    group: ProcessGroup,
    /// Der interne Port, auf dem der Node-Server hört
    pub port: u16,
    log: ServerLog,
}

impl NodeSidecar {
    /// Startet den Node-Server, liest seinen Port und wartet, bis er antwortet.
    ///
    /// `public_port` ist der Port des Rust-Servers; der Node-Server braucht ihn für die Links in
    /// seinen Meldungen. Antwortet er nicht rechtzeitig, kommt `server-start-failed` und der
    /// Kindprozess wird abgeräumt.
    pub async fn start(
        spec: &SidecarSpec,
        home: &HarnessHome,
        public_port: u16,
        dispatch: DispatchBackend,
    ) -> Result<Self> {
        Self::start_waiting(spec, home, public_port, dispatch, PORT_WAIT, HEALTH_WAIT).await
    }

    /// Wie [`Self::start`], aber mit eigenen Fristen (Tests)
    pub async fn start_waiting(
        spec: &SidecarSpec,
        home: &HarnessHome,
        public_port: u16,
        dispatch: DispatchBackend,
        port_wait: Duration,
        health_wait: Duration,
    ) -> Result<Self> {
        let log = ServerLog::new(home);
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .args(["--port", "0"])
            .env("HARNESS_SIDECAR", "1")
            .env("HARNESS_PUBLIC_PORT", public_port.to_string())
            // Rust löst `NEXUS_HOME` und `HARNESS_HOME` auf und gibt den Ordner unter beiden
            // Namen weiter; der Node-Server kennt nur `HARNESS_HOME`
            .env("NEXUS_HOME", home.dir())
            .env("HARNESS_HOME", home.dir())
            // Die Fragen der Board-Datenbank beobachtet nur der Rust-Server; sonst käme
            // jede neue Frage zweimal als Meldung
            .env("HARNESS_DB_QUESTION_POLL_MS", "0")
            // stdin bleibt offen und wird nie beschrieben: endet die Pipe, endet der Node-Server
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        // Dispatcht der Rust-Server, bleibt der Takt des Node-Servers aus; sonst schrieben
        // zwei Dispatcher dieselben Läufe
        if dispatch.is_rust() {
            command.env("HARNESS_DISPATCH_TICK_MS", "0");
        }
        let mut group = ProcessGroup::spawn(&mut command).map_err(|error| {
            HarnessError::new(
                "server-start-failed",
                format!(
                    "Der Node-Server ist nicht gestartet ({}: {error}).",
                    text(&spec.program)
                ),
            )
        })?;
        log.write(&format!(
            "Node-Server gestartet: {} (pid {})",
            text(&spec.program),
            group.id().unwrap_or_default()
        ));
        if let Some(stderr) = group.take_stderr() {
            tokio::spawn(forward_lines(stderr, log.clone(), "Node"));
        }

        let mut sidecar = Self {
            group,
            port: 0,
            log: log.clone(),
        };
        match sidecar.await_ready(port_wait, health_wait).await {
            Ok(port) => {
                sidecar.port = port;
                log.write(&format!("Node-Server hört auf 127.0.0.1:{port}"));
                Ok(sidecar)
            }
            Err(error) => {
                log.write(&error.to_string());
                sidecar.stop().await;
                Err(error.into())
            }
        }
    }

    /// Prozessnummer des Node-Servers, solange er läuft
    pub fn pid(&self) -> Option<u32> {
        self.group.id()
    }

    /// Beendet den Node-Server samt allem, was er selbst gestartet hat: erst die Bitte um ein
    /// geordnetes Ende, nach [`STOP_WAIT`] der harte Abbruch
    pub async fn stop(mut self) {
        let pid = self.group.id();
        self.group.terminate_tree();
        if tokio::time::timeout(STOP_WAIT, self.group.wait())
            .await
            .is_err()
        {
            self.log.write(&format!(
                "Der Node-Server (pid {}) hat nicht von allein aufgehört; er wird hart beendet.",
                pid.unwrap_or_default()
            ));
            self.group.kill_tree();
            let _ = tokio::time::timeout(STOP_WAIT, self.group.wait()).await;
        }
        self.log.write("Der Node-Server ist beendet.");
    }

    /// Liest die Port-Zeile und wartet dann, bis `GET /api/health` den eigenen Server meldet
    async fn await_ready(
        &mut self,
        port_wait: Duration,
        health_wait: Duration,
    ) -> std::result::Result<u16, HarnessError> {
        let stdout = self
            .group
            .take_stdout()
            .ok_or_else(|| self.failed("Die Ausgabe des Node-Servers ist nicht lesbar."))?;
        let mut lines = BufReader::new(stdout).lines();
        let port = match tokio::time::timeout(port_wait, read_port(&mut lines, &self.log)).await {
            Ok(Some(port)) => port,
            Ok(None) => {
                return Err(self.failed(&format!(
                    "Der Node-Server hat sich beendet, ohne seinen Port zu melden (erwartet: eine Zeile „{PORT_LINE}<port>“)."
                )))
            }
            Err(_) => {
                return Err(self.failed(&format!(
                    "Der Node-Server hat in {} Sekunden keinen Port gemeldet (erwartet: eine Zeile „{PORT_LINE}<port>“).",
                    port_wait.as_secs()
                )))
            }
        };
        // Der Rest der Ausgabe gehört ins Protokoll, damit die Pipe nicht volläuft
        let log = self.log.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                log.write(&format!("Node: {}", line.trim_end()));
            }
        });
        if wait_for_health(port, health_wait).await {
            return Ok(port);
        }
        Err(self.failed(&format!(
            "Der Node-Server auf Port {port} hat in {} Sekunden nicht geantwortet.",
            health_wait.as_secs()
        )))
    }

    /// Fehler `server-start-failed` mit dem Hinweis auf das Protokoll
    fn failed(&self, reason: &str) -> HarnessError {
        HarnessError::new(
            "server-start-failed",
            format!("{reason} Details stehen in {}", text(self.log.file())),
        )
    }
}

/// Liest Zeilen, bis die Port-Zeile kommt; alles davor geht ins Protokoll. `None`, wenn die
/// Ausgabe endet, ohne dass der Port gemeldet wurde.
async fn read_port<R>(lines: &mut tokio::io::Lines<BufReader<R>>, log: &ServerLog) -> Option<u16>
where
    R: tokio::io::AsyncRead + Unpin,
{
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim_end();
        match line
            .strip_prefix(PORT_LINE)
            .and_then(|raw| raw.trim().parse().ok())
        {
            Some(port) => return Some(port),
            None => log.write(&format!("Node: {line}")),
        }
    }
    None
}

/// Schreibt jede Zeile eines Prozessstroms ins Protokoll
async fn forward_lines<R>(stream: R, log: ServerLog, name: &'static str)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        log.write(&format!("{name}: {}", line.trim_end()));
    }
}

/// Fragt `GET /api/health` im Takt von [`HEALTH_STEP`], bis der eigene Server antwortet
async fn wait_for_health(port: u16, health_wait: Duration) -> bool {
    let client: Client<HttpConnector, Body> =
        Client::builder(TokioExecutor::new()).build(HttpConnector::new());
    let deadline = tokio::time::Instant::now() + health_wait;
    loop {
        if is_ours(&client, port).await {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(HEALTH_STEP).await;
    }
}

/// Ein Health-Aufruf: antwortet dort der Node-Server dieses Projekts?
async fn is_ours(client: &Client<HttpConnector, Body>, port: u16) -> bool {
    let Ok(uri) = format!("http://127.0.0.1:{port}/api/health").parse() else {
        return false;
    };
    let Ok(response) = client.get(uri).await else {
        return false;
    };
    let Ok(body) = response.into_body().collect().await else {
        return false;
    };
    serde_json::from_slice::<serde_json::Value>(&body.to_bytes())
        .ok()
        .and_then(|json| json["name"].as_str().map(ToString::to_string))
        .as_deref()
        == Some(OWN_NAME)
}

/// Die gebündelte Programmdatei neben dem laufenden Programm: erst der Name, den Tauri beim
/// Paketieren vergibt, dann der Name mit Zielsuffix aus `npm run build:sidecar`
fn bundled_beside(dir: &Path) -> Option<PathBuf> {
    let plain = dir.join(if cfg!(windows) {
        format!("{BUNDLED}.exe")
    } else {
        BUNDLED.to_string()
    });
    if plain.is_file() {
        return Some(plain);
    }
    let prefix = format!("{BUNDLED}-");
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found.into_iter().next()
}

/// Pfad als Text für Argumente und Meldungen
fn text(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Umgebung aus einer Tabelle
    fn env(entries: &[(&str, &str)]) -> HashMap<String, String> {
        crate::config::env_map(entries)
    }

    #[test]
    fn ein_javascript_modul_laeuft_unter_node() {
        let map = env(&[]);
        let spec = SidecarSpec::resolve(
            Some("packages/server/dist/main.js"),
            &|name| map.get(name).cloned(),
            None,
        )
        .unwrap();
        assert_eq!(spec.program, Path::new("node"));
        assert_eq!(spec.args, vec!["packages/server/dist/main.js"]);
    }

    #[test]
    fn das_node_programm_ist_wahlbar() {
        let map = env(&[("NEXUS_NODE", "/opt/node22/bin/node")]);
        let spec = SidecarSpec::resolve(Some("/s/main.mjs"), &|name| map.get(name).cloned(), None)
            .unwrap();
        assert_eq!(spec.program, Path::new("/opt/node22/bin/node"));
        assert_eq!(spec.args, vec!["/s/main.mjs"]);
    }

    #[test]
    fn eine_gebuendelte_programmdatei_startet_selbst_mit_serve() {
        let map = env(&[("NEXUS_NODE_SIDECAR", "/app/harness-sidecar")]);
        let spec = SidecarSpec::resolve(None, &|name| map.get(name).cloned(), None).unwrap();
        assert_eq!(spec.program, Path::new("/app/harness-sidecar"));
        assert_eq!(spec.args, vec!["serve"]);
    }

    #[test]
    fn die_angabe_geht_vor_der_umgebung() {
        let map = env(&[("NEXUS_NODE_SIDECAR", "/aus/der/umgebung")]);
        let spec = SidecarSpec::resolve(
            Some("/aus/dem/argument"),
            &|name| map.get(name).cloned(),
            None,
        )
        .unwrap();
        assert_eq!(spec.program, Path::new("/aus/dem/argument"));
    }

    #[test]
    fn ohne_angabe_wird_neben_dem_programm_gesucht() {
        let dir = tempfile::tempdir().unwrap();
        let name = if cfg!(windows) {
            "harness-sidecar.exe"
        } else {
            "harness-sidecar"
        };
        std::fs::write(dir.path().join(name), b"x").unwrap();
        let map = env(&[]);
        let spec =
            SidecarSpec::resolve(None, &|name| map.get(name).cloned(), Some(dir.path())).unwrap();
        assert_eq!(spec.program, dir.path().join(name));
        assert_eq!(spec.args, vec!["serve"]);
    }

    #[test]
    fn auch_der_name_mit_zielsuffix_wird_gefunden() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("harness-sidecar-aarch64-apple-darwin"),
            b"x",
        )
        .unwrap();
        let map = env(&[]);
        let spec =
            SidecarSpec::resolve(None, &|name| map.get(name).cloned(), Some(dir.path())).unwrap();
        assert_eq!(
            spec.program,
            dir.path().join("harness-sidecar-aarch64-apple-darwin")
        );
    }

    #[test]
    fn ohne_programmdatei_fehlt_der_node_server() {
        let dir = tempfile::tempdir().unwrap();
        let map = env(&[]);
        let error = SidecarSpec::resolve(None, &|name| map.get(name).cloned(), Some(dir.path()))
            .unwrap_err();
        assert_eq!(error.code(), Some("node-sidecar-missing"));
        assert!(error.to_string().contains("--node-sidecar"), "{error}");
    }
}
