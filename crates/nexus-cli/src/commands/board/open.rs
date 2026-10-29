//! `nexus board` ohne Unterbefehl: Projekt prüfen, Port prüfen, Server oder App starten,
//! Projekt anmelden, Adresse ausgeben und öffnen.
//! Portiert aus `packages/cli/src/commands/BoardCommand.ts`.

use std::time::Duration;

use nexus_core::registry::DEFAULT_SERVER_PORT;
use nexus_core::{HarnessError, Result};
use serde::Serialize;

use crate::board::BrowserOpener;
use crate::context::CliContext;
use crate::http::LocalHttp;
use crate::server::{PortState, ServerLauncher};

/// Wartezeit der Anmeldung beim Server
const TIMEOUT: Duration = Duration::from_secs(10);

/// Umgebungsvariable mit dem Port des Servers
pub const PORT_ENV: &str = "HARNESS_PORT";

/// Das Ergebnis von `nexus board` (`--json`)
#[derive(Debug, Serialize)]
struct BoardResult {
    port: u16,
    url: String,
    started: bool,
    project: serde_json::Value,
}

/// Führt `board` ohne Unterbefehl aus: Server oder App starten und das Board öffnen
pub fn run(context: &CliContext, port: Option<&str>, open: bool) -> Result<u8> {
    context.open_project()?;
    let from_env = std::env::var(PORT_ENV).ok().filter(|raw| !raw.is_empty());
    let port = parse_port(port, from_env.as_deref())?;
    let launcher = ServerLauncher::from_process_env();
    let state = launcher.probe(port);
    if state == PortState::Foreign {
        return Err(HarnessError::new(
            "port-foreign",
            format!(
                "Port {port} wird von einem anderen Programm benutzt. Mit --port einen anderen Port wählen."
            ),
        )
        .into());
    }
    // Ist die App installiert, startet sie statt des Servers und wählt ihren Port selbst
    let port = if state == PortState::Free {
        match launcher.app() {
            Some(app) => launcher.start_app(&app)?,
            None => {
                launcher.start(port)?;
                port
            }
        }
    } else {
        port
    };

    let project = announce_project(context, port)?;
    let id = project["id"].as_str().unwrap_or_default().to_string();
    let url = format!("http://127.0.0.1:{port}/p/{id}/board");
    context.printer().result(
        &BoardResult {
            port,
            url: url.clone(),
            started: state == PortState::Free,
            project,
        },
        &[
            format!("Server läuft auf http://127.0.0.1:{port}"),
            format!("Board: {url}"),
        ],
    );
    if open && !BrowserOpener.open(&url) {
        context
            .printer()
            .warn("Der Browser konnte nicht geöffnet werden. Bitte die Adresse selbst öffnen.");
    }
    Ok(0)
}

/// Port aus Option, Umgebung oder Standard
pub fn parse_port(option: Option<&str>, env: Option<&str>) -> Result<u16> {
    let default = DEFAULT_SERVER_PORT.to_string();
    let raw = option.or(env).unwrap_or(&default);
    raw.parse::<u16>()
        .ok()
        .filter(|port| *port >= 1)
        .ok_or_else(|| {
            HarnessError::new(
                "port-invalid",
                format!("Der Port „{raw}“ ist ungültig. Erlaubt sind Zahlen von 1 bis 65535."),
            )
            .into()
        })
}

/// Meldet das aktuelle Projekt beim Server an
fn announce_project(context: &CliContext, port: u16) -> Result<serde_json::Value> {
    let path = std::fs::canonicalize(context.project_dir())
        .unwrap_or_else(|_| context.project_dir().to_path_buf());
    let body = serde_json::json!({ "path": path.to_string_lossy() }).to_string();
    let response = LocalHttp::new(TIMEOUT)
        .post_json(port, "/api/projects", &body)
        .map_err(|error| register_failed(&error.to_string()))?;
    let json = response.json().unwrap_or(serde_json::Value::Null);
    if !response.ok() || !json["id"].is_string() {
        let message = json["message"]
            .as_str()
            .unwrap_or("Das Projekt konnte nicht angemeldet werden.");
        return Err(register_failed(message));
    }
    Ok(json)
}

/// Fehler, wenn die Anmeldung nicht geklappt hat
fn register_failed(message: &str) -> nexus_core::Error {
    HarnessError::new("register-failed", message).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohne_angabe_gilt_der_standardport() {
        assert_eq!(parse_port(None, None).unwrap(), DEFAULT_SERVER_PORT);
    }

    #[test]
    fn die_option_geht_vor_der_umgebung() {
        assert_eq!(parse_port(Some("4800"), Some("4900")).unwrap(), 4800);
        assert_eq!(parse_port(None, Some("4900")).unwrap(), 4900);
    }

    #[test]
    fn ein_kaputter_port_wird_mit_port_invalid_abgelehnt() {
        let error = parse_port(Some("abc"), None).unwrap_err();
        assert_eq!(error.code(), Some("port-invalid"));
        assert_eq!(
            error.to_string(),
            "Der Port „abc“ ist ungültig. Erlaubt sind Zahlen von 1 bis 65535."
        );
    }

    #[test]
    fn ein_port_ausserhalb_des_bereichs_wird_abgelehnt() {
        for raw in ["0", "99999", "-1", "47.1"] {
            let error = parse_port(Some(raw), None).unwrap_err();
            assert_eq!(error.code(), Some("port-invalid"), "{raw}");
            assert_eq!(
                error.to_string(),
                format!("Der Port „{raw}“ ist ungültig. Erlaubt sind Zahlen von 1 bis 65535.")
            );
        }
    }
}
