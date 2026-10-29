//! Meldet Projekt, Hook-Ereignisse und Zuordnungen an den Server; alle Aufrufe sind
//! zeitbegrenzt und still bei Fehlern.
//! Portiert aus `packages/cli/src/hooks/ServerClient.ts`.

use std::time::Duration;

use crate::http::LocalHttp;

/// Wartezeit eines Aufrufs an den Server (`TIMEOUT_MS` in TypeScript)
const TIMEOUT: Duration = Duration::from_millis(1500);

/// Kopfzeile, mit der ein Hook-Ereignis das Token seines Projekts mitschickt
const TOKEN_HEADER: &str = "x-harness-token";

/// Ruft den lokalen Server auf; jeder Fehler wird zu „nichts passiert“.
#[derive(Debug, Clone, Copy)]
pub struct ServerClient {
    port: u16,
    http: LocalHttp,
}

impl ServerClient {
    /// Client für den Server auf diesem Port
    pub fn new(port: u16) -> Self {
        Self {
            port,
            http: LocalHttp::new(TIMEOUT),
        }
    }

    /// Meldet das Projekt an (wiederholbar); liefert die Projekt-ID oder `None`
    pub fn register_project(&self, path: &str) -> Option<String> {
        let body = serde_json::json!({ "path": path }).to_string();
        let json = self.post("/api/projects", &body, &[])?;
        json["id"].as_str().map(ToString::to_string)
    }

    /// Sendet ein Ereignis mit dem Token des Projekts
    pub fn send_event(&self, project_path: &str, token: &str, event: &serde_json::Value) -> bool {
        let body = serde_json::json!({ "project": project_path, "event": event }).to_string();
        self.post("/api/hook", &body, &[(TOKEN_HEADER, token)])
            .is_some()
    }

    /// Ordnet eine Session einem Ticket zu
    pub fn claim(&self, project_id: &str, session_id: &str, ticket_id: &str) -> bool {
        let path = format!(
            "/api/projects/{project_id}/sessions/{}/claim",
            encode(session_id)
        );
        let body = serde_json::json!({ "ticket": ticket_id }).to_string();
        self.post(&path, &body, &[]).is_some()
    }

    /// POST mit Pflichtkopf und Zeitlimit; liefert den JSON-Körper (`{}` bei 204) oder `None`
    /// bei Fehlern
    fn post(&self, path: &str, body: &str, headers: &[(&str, &str)]) -> Option<serde_json::Value> {
        let response = self
            .http
            .post_json_with(self.port, path, body, headers)
            .ok()?;
        if !response.ok() {
            return None;
        }
        if response.status == 204 {
            return Some(serde_json::json!({}));
        }
        Some(response.json().unwrap_or(serde_json::json!({})))
    }
}

/// Kodiert einen Pfadteil wie `encodeURIComponent`: alles außer den dort unberührten Zeichen
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
mod tests {
    use super::*;

    #[test]
    fn kodiert_einen_pfadteil_wie_encode_uri_component() {
        assert_eq!(encode("s-1"), "s-1");
        assert_eq!(encode("a/b c"), "a%2Fb%20c");
        assert_eq!(encode("ä"), "%C3%A4");
        assert_eq!(encode("-_.!~*'()"), "-_.!~*'()");
    }

    #[test]
    fn ohne_server_bleibt_jeder_aufruf_still() {
        // Port 1 ist nie belegt; der Aufruf scheitert und wird zu „nichts passiert“
        let client = ServerClient::new(1);
        assert_eq!(client.register_project("/p"), None);
        assert!(!client.claim("p-1", "s-1", "ARC-0001"));
        assert!(!client.send_event("/p", "token", &serde_json::json!({})));
    }
}
