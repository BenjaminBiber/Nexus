//! `nexus board` von außen: echter Node-Server über `HARNESS_SERVER_ENTRY`, Projekt-Anmeldung,
//! Ausgabe als Text und als JSON, belegter fremder Port und unbekannter Unterbefehl.
//! Entspricht den Fällen „board …“ aus `contract/cli/board/boards-projects.scenario.json`.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use assert_cmd::prelude::*;
use nexus_core::registry::ServerInfoFile;
use nexus_core::{HarnessHome, HarnessProject};
use tempfile::TempDir;

/// Wurzel des Projekts (zwei Ebenen über dem Crate)
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap()
}

/// Programmdatei des gebauten TypeScript-Servers; ohne `npm run build` gibt es sie nicht
fn server_entry() -> Option<PathBuf> {
    let entry = repo_root()
        .join("packages")
        .join("server")
        .join("dist")
        .join("main.js");
    entry.is_file().then_some(entry)
}

/// Ein eingerichtetes Projekt in einem temporären Git-Repo
fn projekt() -> TempDir {
    let dir = tempfile::Builder::new()
        .prefix("nexus-board-repo-")
        .tempdir()
        .unwrap();
    let path = dir.path().canonicalize().unwrap();
    git(&path, &["init", "-q", "-b", "main"]);
    git(&path, &["config", "user.email", "vertrag@example.invalid"]);
    git(&path, &["config", "user.name", "Vertrag"]);
    git(&path, &["commit", "--allow-empty", "-q", "-m", "init"]);
    HarnessProject::init(&path).unwrap();
    dir
}

/// Führt Git im Repo aus
fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {status:?}");
}

/// Ein Port, auf dem gerade niemand hört
fn freier_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

/// Der Aufruf von `nexus` mit frischem Benutzerordner, eigenem Port und ohne Nexus-App
fn nexus(home: &Path, port: u16, entry: Option<&Path>) -> Command {
    let mut command = Command::cargo_bin("nexus").unwrap();
    command
        .env("NEXUS_HOME", home)
        .env("NEXUS_PORT", port.to_string())
        .env_remove("HARNESS_HOME")
        .env_remove("HARNESS_PORT")
        // Eine lokal installierte Nexus-App darf den Test nicht an sich ziehen
        .env("NEXUS_APP_PATH", home.join("keine-app"))
        .env_remove("NEXUS_SESSION")
        .env_remove("NEXUS_TICKET")
        .env_remove("HARNESS_SESSION")
        .env_remove("HARNESS_TICKET");
    match entry {
        Some(entry) => command.env("HARNESS_SERVER_ENTRY", entry),
        None => command.env_remove("HARNESS_SERVER_ENTRY"),
    };
    command
}

/// Beendet den Server, den der Test gestartet hat (Angaben aus `server.json`)
fn server_beenden(home: &Path) {
    let Some(info) = ServerInfoFile::new(HarnessHome::at(home)).read() else {
        return;
    };
    #[cfg(unix)]
    let _ = Command::new("kill")
        .args(["-TERM", &info.pid.to_string()])
        .status();
    #[cfg(windows)]
    let _ = Command::new("taskkill")
        .args(["/pid", &info.pid.to_string(), "/T", "/F"])
        .status();
    // Warten, bis der Port wieder frei ist, damit der nächste Test ihn benutzen kann
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", info.port)).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn board_startet_den_server_meldet_das_projekt_an_und_nennt_die_adresse() {
    let Some(entry) = server_entry() else {
        eprintln!("übersprungen: packages/server/dist/main.js fehlt – vorher `npm run build`");
        return;
    };
    let home = tempfile::Builder::new()
        .prefix("nexus-board-home-")
        .tempdir()
        .unwrap();
    let repo = projekt();
    let port = freier_port();

    let json = nexus(home.path(), port, Some(&entry))
        .args(["--json", "board", "--no-open"])
        .current_dir(repo.path().canonicalize().unwrap())
        .output()
        .unwrap();
    let ergebnis = String::from_utf8_lossy(&json.stdout).to_string();
    assert!(
        json.status.success(),
        "stdout: {ergebnis}\nstderr: {}",
        String::from_utf8_lossy(&json.stderr)
    );
    let ergebnis: serde_json::Value = serde_json::from_str(&ergebnis).unwrap();
    let id = ergebnis["project"]["id"].as_str().unwrap().to_string();
    assert_eq!(ergebnis["port"], serde_json::json!(port));
    assert_eq!(ergebnis["started"], serde_json::json!(true));
    assert_eq!(
        ergebnis["url"],
        serde_json::json!(format!("http://127.0.0.1:{port}/p/{id}/board"))
    );
    assert_eq!(
        ergebnis["project"]["path"],
        serde_json::json!(repo.path().canonicalize().unwrap().to_string_lossy())
    );

    // Der zweite Aufruf findet den laufenden Server und startet nichts mehr
    let text = nexus(home.path(), port, Some(&entry))
        .args(["board", "--no-open"])
        .current_dir(repo.path().canonicalize().unwrap())
        .output()
        .unwrap();
    assert!(text.status.success(), "{text:?}");
    assert_eq!(
        String::from_utf8_lossy(&text.stdout).trim_end(),
        format!(
            "Server läuft auf http://127.0.0.1:{port}\nBoard: http://127.0.0.1:{port}/p/{id}/board"
        )
    );

    server_beenden(home.path());
}

#[test]
fn ein_fremdes_programm_auf_dem_port_endet_mit_port_foreign() {
    let home = tempfile::Builder::new()
        .prefix("nexus-board-home-")
        .tempdir()
        .unwrap();
    let repo = projekt();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    // Ein Programm, das kein JSON des Harness liefert
    let fremd = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhallo");
        }
    });

    let out = nexus(home.path(), port, None)
        .args(["board", "--no-open"])
        .current_dir(repo.path().canonicalize().unwrap())
        .output()
        .unwrap();
    fremd.join().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr).trim_end(),
        format!("Port {port} wird von einem anderen Programm benutzt. Mit --port einen anderen Port wählen. [port-foreign]")
    );
}

#[test]
fn ein_unbekannter_unterbefehl_endet_mit_command_unknown() {
    let home = tempfile::Builder::new()
        .prefix("nexus-board-home-")
        .tempdir()
        .unwrap();
    // Kein Projekt nötig: die Abweisung kommt vor jedem Datenbankzugriff
    let out = nexus(home.path(), freier_port(), None)
        .args(["board", "quatsch"])
        .current_dir(home.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr).trim_end(),
        "Unbekannter Befehl „board quatsch“. Hilfe: nexus board --help [command-unknown]"
    );
}

#[test]
fn ein_kaputter_port_endet_mit_port_invalid() {
    let home = tempfile::Builder::new()
        .prefix("nexus-board-home-")
        .tempdir()
        .unwrap();
    let repo = projekt();
    let out = nexus(home.path(), freier_port(), None)
        .args(["--json", "board", "--port", "abc", "--no-open"])
        .current_dir(repo.path().canonicalize().unwrap())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let ergebnis: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).unwrap();
    assert_eq!(
        ergebnis,
        serde_json::json!({
            "error": {
                "code": "port-invalid",
                "message": "Der Port „abc“ ist ungültig. Erlaubt sind Zahlen von 1 bis 65535."
            }
        })
    );
}
