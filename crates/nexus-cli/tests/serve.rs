//! `nexus serve` von außen: freier Port, Health, `server.json` mit der Rust-Prozessnummer und
//! dem öffentlichen Port, Weiterleitung an den Node-Server und ein geordnetes Ende mit SIGTERM.
//!
//! Der Test braucht den gebauten Node-Server (`npm run build`); fehlt er, meldet er das und tut
//! nichts. Das Beenden mit Signal gibt es nur unter Unix (`#[cfg(unix)]`).

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::prelude::*;
use nexus_core::registry::ServerInfoFile;
use nexus_core::HarnessHome;

/// Wurzel des Projekts (zwei Ebenen über dem Crate)
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap()
}

/// Programmdatei des gebauten TypeScript-Servers; ohne `npm run build` gibt es sie nicht
fn node_server() -> Option<PathBuf> {
    let entry = repo_root()
        .join("packages")
        .join("server")
        .join("dist")
        .join("main.js");
    entry.is_file().then_some(entry)
}

/// Ein Port, auf dem gerade niemand hört
fn freier_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

/// Startet `nexus serve` mit frischem Benutzerordner
fn serve(home: &Path, port: u16, node: Option<&Path>) -> std::process::Child {
    let mut command = Command::cargo_bin("nexus").unwrap();
    command
        .args(["serve", "--port", &port.to_string()])
        .env("HARNESS_HOME", home)
        .env("NEXUS_HOME", home)
        .env_remove("HARNESS_PORT")
        .env_remove("NEXUS_PORT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match node {
        Some(node) => command.args(["--node-sidecar", &node.to_string_lossy()]),
        None => command.arg("--no-node"),
    };
    command.spawn().expect("nexus serve startet")
}

/// Wartet, bis `GET /api/health` den eigenen Server meldet; liefert die ganze Antwort
fn warte_auf_health(port: u16) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Some(text) = fetch(port, "/api/health") {
            if text.contains("\"name\":\"ai-harness\"") {
                return Some(text);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// Kleiner HTTP-Aufruf von Hand; `None`, wenn niemand antwortet
fn fetch(port: u16, path: &str) -> Option<String> {
    use std::io::{Read, Write};
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut text = String::new();
    stream.read_to_string(&mut text).ok()?;
    Some(text)
}

/// Wartet, bis auf dem Port niemand mehr hört
fn warte_auf_freien_port(port: u16, frist: Duration) -> bool {
    let deadline = Instant::now() + frist;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_err() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    TcpStream::connect(("127.0.0.1", port)).is_err()
}

/// Läuft ein Prozess mit dieser Nummer noch?
#[cfg(unix)]
fn lebt(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[test]
fn serve_startet_rust_und_node_und_schreibt_server_json() {
    let Some(node) = node_server() else {
        eprintln!("übersprungen: packages/server/dist/main.js fehlt – vorher `npm run build`");
        return;
    };
    let home = tempfile::Builder::new()
        .prefix("nexus-serve-")
        .tempdir()
        .unwrap();
    let port = freier_port();
    let mut kind = serve(home.path(), port, Some(&node));

    let health = warte_auf_health(port).expect("Health des Servers");
    // Die Antwort kommt von Rust, nicht vom Node-Server dahinter
    assert!(
        health.to_lowercase().contains("x-nexus-backend: rust"),
        "{health}"
    );

    // `server.json` nennt den Rust-Prozess und den öffentlichen Port, nicht den internen
    let info = ServerInfoFile::new(HarnessHome::at(home.path()))
        .read()
        .expect("server.json");
    assert_eq!(info.port, port);
    assert_eq!(info.pid, kind.id(), "die PID des Rust-Prozesses");
    assert!(
        health.contains(&format!("\"pid\":{}", info.pid)),
        "{health}"
    );

    // Eine noch nicht portierte Route kommt beim Node-Server an
    let tickets = fetch(port, "/api/db/tickets").expect("Antwort auf /api/db/tickets");
    assert!(
        tickets.to_lowercase().contains("x-nexus-backend: node"),
        "{tickets}"
    );

    let _ = kind.kill();
    let _ = kind.wait();
    assert!(warte_auf_freien_port(port, Duration::from_secs(20)));
}

#[test]
fn ein_belegter_port_endet_mit_port_busy_und_code_1() {
    let home = tempfile::Builder::new()
        .prefix("nexus-serve-")
        .tempdir()
        .unwrap();
    let belegt = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = belegt.local_addr().unwrap().port();

    let mut command = Command::cargo_bin("nexus").unwrap();
    let out = command
        .args(["serve", "--port", &port.to_string(), "--no-node"])
        .env("HARNESS_HOME", home.path())
        .env("NEXUS_HOME", home.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr).trim_end(),
        format!("Port {port} ist belegt. [port-busy]")
    );
}

#[test]
#[cfg(unix)]
fn sigterm_beendet_rust_und_node_und_loescht_server_json() {
    let Some(node) = node_server() else {
        eprintln!("übersprungen: packages/server/dist/main.js fehlt – vorher `npm run build`");
        return;
    };
    let home = tempfile::Builder::new()
        .prefix("nexus-serve-")
        .tempdir()
        .unwrap();
    let port = freier_port();
    let mut kind = serve(home.path(), port, Some(&node));
    warte_auf_health(port).expect("Health des Servers");

    // Der interne Port des Node-Servers steht im Protokoll
    let protokoll = std::fs::read_to_string(home.path().join("server.log")).unwrap_or_default();
    let intern: u16 = protokoll
        .lines()
        .find_map(|line| line.rsplit_once("Node-Server hört auf 127.0.0.1:"))
        .and_then(|(_, port)| port.trim().parse().ok())
        .unwrap_or_else(|| panic!("Port des Node-Servers im Protokoll: {protokoll}"));
    assert_ne!(
        intern, port,
        "der Node-Server hört nicht auf dem öffentlichen Port"
    );

    let pid = kind.id();
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .unwrap();
    assert!(status.success());

    let ende = kind.wait().expect("Ende des Rust-Prozesses");
    assert!(ende.success(), "{ende}");
    assert!(!lebt(pid));
    // Beide Ports sind frei und `server.json` ist weg
    assert!(warte_auf_freien_port(port, Duration::from_secs(20)));
    assert!(warte_auf_freien_port(intern, Duration::from_secs(20)));
    assert!(
        !home.path().join("server.json").exists(),
        "server.json ist gelöscht"
    );
}
