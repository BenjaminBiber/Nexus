//! Der Node-Server als Kindprozess hinter dem Rust-Server, geprüft mit dem Ersatz
//! `tests/fixtures/fake-sidecar.mjs`: Port-Zeile lesen, Health abwarten, Anfragen weiterleiten,
//! beim Beenden abräumen, und ohne Port-Zeile ein verständlicher Fehler.
//!
//! Die Tests brauchen `node` im PATH; fehlt es, melden sie das und tun nichts.

mod common;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use common::memory_board;
use nexus_core::HarnessHome;
use nexus_platform::Platform;
use nexus_server::config::ServerConfig;
use nexus_server::sidecar::{NodeSidecar, SidecarSpec, PORT_LINE};

/// Frist der Tests für die Port-Zeile: kurz, damit das Zeitlimit schnell zuschlägt
const PORT_WAIT: Duration = Duration::from_secs(2);

/// Frist der Tests für den Health-Aufruf
const HEALTH_WAIT: Duration = Duration::from_secs(10);

/// `node`, wenn es im PATH liegt; sonst `None` samt Meldung
fn node() -> Option<String> {
    let found = std::process::Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if found {
        return Some("node".to_string());
    }
    eprintln!("übersprungen: node liegt nicht im PATH");
    None
}

/// Pfad des Ersatz-Sidecars
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake-sidecar.mjs")
}

/// Startweg des Ersatz-Sidecars, mit den Schaltern der Umgebung für diesen Test
fn spec() -> SidecarSpec {
    SidecarSpec {
        program: PathBuf::from("node"),
        args: vec![fixture().to_string_lossy().to_string()],
    }
}

/// Kleiner HTTP-Aufruf ohne weitere Abhängigkeit; liefert Status und ganze Antwort
async fn fetch(port: u16, path: &str) -> (u16, String) {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Verbindung");
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut text = String::new();
    stream.read_to_string(&mut text).await.unwrap();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);
    (status, text)
}

/// Läuft ein Prozess mit dieser Nummer noch? Gewartet wird höchstens `frist`.
async fn warte_auf_ende(pid: u32, frist: Duration) -> bool {
    let bis = tokio::time::Instant::now() + frist;
    while tokio::time::Instant::now() < bis {
        if !lebt(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    !lebt(pid)
}

/// Prüfung über das Betriebssystem; unter Unix mit `kill -0`, sonst über die Prozessliste
fn lebt(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output();
        match output {
            Ok(output) => String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()),
            Err(_) => false,
        }
    }
}

#[tokio::test]
async fn liest_die_port_zeile_und_wartet_auf_health() {
    let Some(_) = node() else { return };
    let dir = tempfile::tempdir().unwrap();
    let home = HarnessHome::at(dir.path());

    let sidecar = NodeSidecar::start_waiting(&spec(), &home, 4710, PORT_WAIT, HEALTH_WAIT)
        .await
        .expect("Sidecar startet");
    assert!(sidecar.port > 0, "Port {} gelesen", sidecar.port);

    // Der Ersatz nennt die Umgebung, die er bekommen hat: Sidecar-Modus, öffentlicher Port,
    // abgeschaltete Fragen-Beobachtung, Benutzerordner und `--port 0`
    let (status, text) = fetch(sidecar.port, "/api/echo").await;
    assert_eq!(status, 200, "{text}");
    assert!(text.contains(r#""sidecar":"1""#), "{text}");
    assert!(text.contains(r#""publicPort":"4710""#), "{text}");
    // Die Fragen der Board-Datenbank beobachtet nur der Rust-Server
    assert!(text.contains(r#""dbQuestionPollMs":"0""#), "{text}");
    assert!(
        text.contains(&format!("\"{}\"", dir.path().to_string_lossy())),
        "{text}"
    );
    assert!(text.contains(r#""args":["--port","0"]"#), "{text}");

    // Der Start steht im Protokoll des Servers
    let protokoll = std::fs::read_to_string(home.log_file()).unwrap_or_default();
    assert!(protokoll.contains("Node-Server gestartet"), "{protokoll}");

    sidecar.stop().await;
}

#[tokio::test]
async fn der_rust_server_leitet_anfragen_an_den_sidecar_weiter() {
    let Some(_) = node() else { return };
    let dir = tempfile::tempdir().unwrap();
    let home = HarnessHome::at(dir.path());

    let listener = nexus_server::bind("127.0.0.1", 0).await.expect("Port");
    let public = listener.local_addr().unwrap().port();
    let sidecar = NodeSidecar::start_waiting(&spec(), &home, public, PORT_WAIT, HEALTH_WAIT)
        .await
        .expect("Sidecar startet");

    let mut config = ServerConfig::new(public, home.clone());
    config.version = "test".into();
    config.upstream = Some(format!("http://127.0.0.1:{}", sidecar.port));
    let server = nexus_server::serve_on(listener, config, memory_board(), Platform::fake())
        .await
        .expect("Server startet");

    // Nicht portierte Route: kommt beim Ersatz an, mit dem Kopf des Proxys
    let (status, text) = fetch(server.port, "/api/echo").await;
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("x-nexus-backend: node"), "{text}");
    assert!(text.contains(r#""url":"/api/echo""#), "{text}");

    // Portierte Route: beantwortet Rust selbst
    let (status, text) = fetch(server.port, "/api/health").await;
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("x-nexus-backend: rust"), "{text}");

    server.stop().await;
    sidecar.stop().await;
}

#[tokio::test]
async fn das_beenden_raeumt_den_kindprozess_ab() {
    let Some(_) = node() else { return };
    let dir = tempfile::tempdir().unwrap();
    let home = HarnessHome::at(dir.path());

    let sidecar = NodeSidecar::start_waiting(&spec(), &home, 4710, PORT_WAIT, HEALTH_WAIT)
        .await
        .expect("Sidecar startet");
    let port = sidecar.port;
    let pid = sidecar.pid().expect("Prozessnummer");
    assert!(lebt(pid));

    sidecar.stop().await;
    assert!(
        warte_auf_ende(pid, Duration::from_secs(10)).await,
        "Kindprozess beendet"
    );
    // Auf dem Port hört niemand mehr
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err(),
        "Port {port} ist frei"
    );
}

#[tokio::test]
async fn ohne_port_zeile_endet_der_start_mit_einem_verstaendlichen_fehler() {
    let Some(_) = node() else { return };
    let dir = tempfile::tempdir().unwrap();
    let home = HarnessHome::at(dir.path());

    // Der Ersatz schweigt; der Rust-Server kann den Port nicht erfahren
    let mut stumm = spec();
    stumm.args.push("--silent".to_string());
    let error = NodeSidecar::start_waiting(&stumm, &home, 4710, PORT_WAIT, HEALTH_WAIT)
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("server-start-failed"));
    let message = error.to_string();
    assert!(message.contains(PORT_LINE), "{message}");
    assert!(message.contains("server.log"), "{message}");
}

#[tokio::test]
async fn ein_ende_der_stdin_beendet_den_sidecar_von_selbst() {
    let Some(_) = node() else { return };
    // Hier läuft der Ersatz ohne NodeSidecar, damit nur die Pipe das Ende auslöst (so verhält
    // sich der echte Node-Server, wenn der Rust-Prozess hart beendet wird)
    let mut child = tokio::process::Command::new("node")
        .arg(fixture())
        .args(["--port", "0"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Ersatz startet");
    let pid = child.id().expect("Prozessnummer");
    let stdout = child.stdout.take().expect("Ausgabe");
    let mut lines = tokio::io::AsyncBufReadExt::lines(tokio::io::BufReader::new(stdout));
    let zeile = tokio::time::timeout(Duration::from_secs(15), lines.next_line())
        .await
        .expect("Port-Zeile")
        .expect("Ausgabe lesbar")
        .expect("eine Zeile");
    assert!(zeile.starts_with(PORT_LINE), "{zeile}");

    // Die Pipe schließen: der Ersatz beendet sich selbst, ohne Signal
    drop(child.stdin.take());
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("Ende innerhalb von 5 Sekunden")
        .expect("Rückgabecode");
    assert!(status.success(), "{status}");
    assert!(warte_auf_ende(pid, Duration::from_secs(5)).await);
}
