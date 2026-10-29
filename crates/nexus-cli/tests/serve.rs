//! `nexus serve` von außen: freier Port, Health, `server.json` mit Prozessnummer und Port und
//! ein geordnetes Ende mit SIGTERM.
//!
//! Das Beenden mit Signal gibt es nur unter Unix (`#[cfg(unix)]`).

use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::prelude::*;
use nexus_core::registry::ServerInfoFile;
use nexus_core::HarnessHome;

/// Ein Port, auf dem gerade niemand hört
fn freier_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

/// Startet `nexus serve` mit frischem Benutzerordner
fn serve(home: &Path, port: u16) -> std::process::Child {
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
fn serve_startet_den_server_und_schreibt_server_json() {
    let home = tempfile::Builder::new()
        .prefix("nexus-serve-")
        .tempdir()
        .unwrap();
    let port = freier_port();
    let mut kind = serve(home.path(), port);

    let health = warte_auf_health(port).expect("Health des Servers");
    // Jede Antwort ist als Rust-Antwort gekennzeichnet
    assert!(
        health.to_lowercase().contains("x-nexus-backend: rust"),
        "{health}"
    );

    // `server.json` nennt den Prozess und den Port des Servers
    let info = ServerInfoFile::new(HarnessHome::at(home.path()))
        .read()
        .expect("server.json");
    assert_eq!(info.port, port);
    assert_eq!(info.pid, kind.id(), "die PID des Serverprozesses");
    assert!(
        health.contains(&format!("\"pid\":{}", info.pid)),
        "{health}"
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
        .args(["serve", "--port", &port.to_string()])
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
fn sigterm_beendet_den_server_und_loescht_server_json() {
    let home = tempfile::Builder::new()
        .prefix("nexus-serve-")
        .tempdir()
        .unwrap();
    let port = freier_port();
    let mut kind = serve(home.path(), port);
    warte_auf_health(port).expect("Health des Servers");

    let pid = kind.id();
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .unwrap();
    assert!(status.success());

    let ende = kind.wait().expect("Ende des Serverprozesses");
    assert!(ende.success(), "{ende}");
    assert!(!lebt(pid));
    // Der Port ist frei und `server.json` ist weg
    assert!(warte_auf_freien_port(port, Duration::from_secs(20)));
    assert!(
        !home.path().join("server.json").exists(),
        "server.json ist gelöscht"
    );
}
