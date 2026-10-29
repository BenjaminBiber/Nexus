//! Spricht die Dispatcher-Routen des lokalen Servers an; startet den Server bei Bedarf.
//! Portiert aus `packages/cli/src/board/DispatchClient.ts`.
//!
//! Die Antworten bleiben rohes JSON: `--json` gibt sie unverändert weiter, auch Felder, die
//! ältere oder neuere Server zusätzlich liefern. Für die Textausgabe liest der
//! [`crate::output::DispatchStatusFormatter`] die Felder, die er braucht.

use std::time::Duration;

use nexus_core::{Error, HarnessError, Result};
use serde_json::Value;

use crate::http::LocalHttp;
use crate::server::{PortState, ServerLauncher};

/// Wartezeit eines Aufrufs an den Server; ein Durchlauf darf dauern
const TIMEOUT: Duration = Duration::from_secs(120);

/// Ruft die Dispatcher-Routen des lokalen Servers auf.
#[derive(Debug, Clone)]
pub struct DispatchClient {
    port: u16,
    launcher: Option<ServerLauncher>,
    http: LocalHttp,
}

impl DispatchClient {
    /// Client für den Server auf diesem Port; fehlt er, wird er gestartet
    pub fn new(port: u16) -> Self {
        Self::with_launcher(port, Some(ServerLauncher::from_process_env()))
    }

    /// Wie [`DispatchClient::new`]; ohne Starter wird kein Server gestartet (Tests)
    pub fn with_launcher(port: u16, launcher: Option<ServerLauncher>) -> Self {
        Self {
            port,
            launcher,
            http: LocalHttp::new(TIMEOUT),
        }
    }

    /// Zustand, mit Board die Bereitschaft seiner Geplant-Tickets (ohne Board: aktives Board)
    pub fn status(&self, board_id: Option<i32>) -> Result<Value> {
        let path = match board_id {
            Some(id) => format!("/api/dispatch/status?boardId={id}"),
            None => "/api/dispatch/status".to_string(),
        };
        self.get(&path)
    }

    /// Sofort einen Tick ausführen
    pub fn run(&self, board_id: Option<i32>) -> Result<Value> {
        let body = match board_id {
            Some(id) => serde_json::json!({ "boardId": id }),
            None => serde_json::json!({}),
        };
        self.post("/api/dispatch/run", &body)
    }

    /// Neue Starts pausieren
    pub fn pause(&self, minutes: u32) -> Result<Value> {
        self.post(
            "/api/dispatch/pause",
            &serde_json::json!({ "minutes": minutes }),
        )
    }

    /// Pause aufheben
    pub fn resume(&self) -> Result<Value> {
        self.post("/api/dispatch/resume", &serde_json::json!({}))
    }

    /// Ein- oder ausschalten
    pub fn set_enabled(&self, on: bool) -> Result<Value> {
        let path = if on {
            "/api/dispatch/enable"
        } else {
            "/api/dispatch/disable"
        };
        self.post(path, &serde_json::json!({}))
    }

    /// „Erneut mergen“; liefert Zustand und Ergebnis für das Ticket
    pub fn merge(&self, id: &str) -> Result<Value> {
        let path = format!("/api/dispatch/merge/{}", encode(id));
        self.post(&path, &serde_json::json!({}))
    }

    /// „Abnehmen“ eines Human-Interaction-Tickets; liefert Zustand und Ergebnis
    pub fn accept(&self, id: &str, by: &str) -> Result<Value> {
        let path = format!("/api/dispatch/accept/{}", encode(id));
        self.post(&path, &serde_json::json!({ "by": by }))
    }

    /// „Recherche starten“ für eine offene Frage (sofort, unabhängig von Ein/Aus und Zeitfenster)
    pub fn start_research(&self, question_id: &str) -> Result<Value> {
        let path = format!("/api/dispatch/research/{}", encode(question_id));
        self.post(&path, &serde_json::json!({}))
    }

    /// „Planen“ für ein Ticket (sofort, unabhängig von Ein/Aus und Zeitfenster)
    pub fn start_planning(&self, ticket_id: &str) -> Result<Value> {
        let path = format!("/api/dispatch/plan/{}", encode(ticket_id));
        self.post(&path, &serde_json::json!({}))
    }

    /// Lesende Anfrage
    fn get(&self, path: &str) -> Result<Value> {
        self.ensure_server()?;
        let response = self
            .http
            .get(self.port, path)
            .map_err(|error| unreachable(&error.to_string()))?;
        Self::settle(response.status, response.ok(), response.json())
    }

    /// Schreibende Anfrage mit Pflichtkopf
    fn post(&self, path: &str, body: &Value) -> Result<Value> {
        self.ensure_server()?;
        let response = self
            .http
            .post_json(self.port, path, &body.to_string())
            .map_err(|error| unreachable(&error.to_string()))?;
        Self::settle(response.status, response.ok(), response.json())
    }

    /// Macht aus Status und Rumpf das Ergebnis oder einen `HarnessError`
    fn settle(status: u16, ok: bool, data: Option<Value>) -> Result<Value> {
        if !ok {
            let code = data
                .as_ref()
                .and_then(|json| json.get("code"))
                .and_then(Value::as_str)
                .unwrap_or("dispatch-failed");
            let message = data
                .as_ref()
                .and_then(|json| json.get("message"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("Der Server antwortet mit {status}."));
            return Err(HarnessError::new(code, message).into());
        }
        data.ok_or_else(|| {
            HarnessError::new(
                "dispatch-failed",
                "Der Server lieferte keine lesbare Antwort.",
            )
            .into()
        })
    }

    /// Server prüfen und bei freiem Port starten
    fn ensure_server(&self) -> Result<()> {
        let Some(launcher) = &self.launcher else {
            return Ok(());
        };
        match launcher.probe(self.port) {
            PortState::Ours => Ok(()),
            PortState::Foreign => Err(HarnessError::new(
                "port-foreign",
                format!(
                    "Port {} wird von einem anderen Programm benutzt.",
                    self.port
                ),
            )
            .into()),
            PortState::Free => launcher.start(self.port),
        }
    }
}

/// Der Server ist nicht erreichbar; in TypeScript wirft `fetch` hier selbst
fn unreachable(message: &str) -> Error {
    HarnessError::new(
        "dispatch-failed",
        format!("Der Server ist nicht erreichbar ({message})."),
    )
    .into()
}

/// Kodiert einen Pfadteil wie `encodeURIComponent`
fn encode(value: &str) -> String {
    const SAFE: &[u8] = b"-_.!~*'()";
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || SAFE.contains(byte) {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests;
