//! Meldet dem laufenden Server, dass eine Session jetzt an einem Ticket arbeitet (Anzeige der
//! Sessions sofort aktuell).
//! Portiert aus `packages/cli/src/hooks/ServerSessionNotice.ts`.

use std::path::Path;

use nexus_core::registry::DEFAULT_SERVER_PORT;
use nexus_core::HarnessProject;

use crate::server::client::ServerClient;
use crate::server::launcher::{PortState, ServerLauncher};

/// Umgebungsvariable mit dem Port des Servers (ohne Präfix; `NEXUS_PORT` vor `HARNESS_PORT`)
const PORT_ENV: &str = "PORT";

/// Sagt dem Server, welche Session an welchem Ticket arbeitet.
#[derive(Debug, Clone, Copy, Default)]
pub struct ServerSessionNotice;

impl ServerSessionNotice {
    /// Nach bestem Bemühen: ohne eingerichtetes Projekt oder ohne eigenen Server geschieht
    /// nichts
    pub fn tell(&self, project_dir: &Path, ticket_id: &str, session_id: &str) {
        let Ok(project) = HarnessProject::open(project_dir) else {
            return;
        };
        let port = Self::port();
        if ServerLauncher::from_process_env().probe(port) != PortState::Ours {
            return;
        }
        let client = ServerClient::new(port);
        if let Some(project_id) = client.register_project(&project.root) {
            client.claim(&project_id, session_id, ticket_id);
        }
    }

    /// Port aus `NEXUS_PORT` (sonst `HARNESS_PORT`), sonst der Standardport
    fn port() -> u16 {
        nexus_core::env::var(PORT_ENV)
            .and_then(|raw| raw.parse::<u16>().ok())
            .filter(|port| *port >= 1)
            .unwrap_or(DEFAULT_SERVER_PORT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohne_eingerichtetes_projekt_geschieht_nichts() {
        let dir = tempfile::Builder::new()
            .prefix("harness-notice-")
            .tempdir()
            .unwrap();
        // Kein Panik, kein Fehler: die Meldung ist freiwillig
        ServerSessionNotice.tell(dir.path(), "ARC-0001", "s-1");
    }
}
