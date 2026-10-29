//! Titel und Status von Tickets aus der Board-Datenbank, mit Zeitlimit. Portiert aus
//! `packages/server/src/sessions/TicketInfoLookup.ts`.
//!
//! Hooks laufen sehr oft, und die Sessions-Ansicht soll auch ohne eingerichtete oder
//! erreichbare Board-Datenbank erscheinen: jeder Fehler und jedes Überschreiten des
//! Zeitlimits ergeben deshalb eine leere Zuordnung, nie einen Fehler.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use nexus_core::board::ticket_service::TicketSummary;
use nexus_core::Result;

use super::session_dto::TicketInfo;

/// Standard-Zeitlimit der Abfrage
pub const DEFAULT_TIMEOUT_MS: u64 = 1000;

/// Holt die Kurzangaben der Tickets (in Tests eine Attrappe ohne Datenbank)
pub type TicketSummaries = Arc<
    dyn Fn(Vec<String>) -> Pin<Box<dyn Future<Output = Result<Vec<TicketSummary>>> + Send>>
        + Send
        + Sync,
>;

/// Liest Titel und Status von Tickets für Sessions, Favoriten und Meldungen
pub struct TicketInfoLookup {
    summaries: TicketSummaries,
    timeout: Duration,
}

impl TicketInfoLookup {
    /// Abfrage mit dem Standard-Zeitlimit
    pub fn new(summaries: TicketSummaries) -> Self {
        Self::with_timeout(summaries, DEFAULT_TIMEOUT_MS)
    }

    /// Abfrage mit eigenem Zeitlimit
    pub fn with_timeout(summaries: TicketSummaries, timeout_ms: u64) -> Self {
        Self {
            summaries,
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    /// Angaben je Ticket-ID; ohne eingerichtete, erreichbare oder rechtzeitig antwortende
    /// Board-Datenbank eine leere Zuordnung
    pub async fn infos(&self, ids: &[String]) -> HashMap<String, TicketInfo> {
        let mut wanted: Vec<String> = Vec::new();
        for id in ids {
            if !wanted.contains(id) {
                wanted.push(id.clone());
            }
        }
        if wanted.is_empty() {
            return HashMap::new();
        }
        let found = tokio::time::timeout(self.timeout, (self.summaries)(wanted)).await;
        let Ok(Ok(list)) = found else {
            return HashMap::new();
        };
        list.into_iter()
            .map(|ticket| {
                (
                    ticket.id.clone(),
                    TicketInfo {
                        id: ticket.id,
                        title: ticket.title,
                        status: ticket.status.as_str().to_string(),
                    },
                )
            })
            .collect()
    }
}
