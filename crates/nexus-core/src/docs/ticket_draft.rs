//! Vorschläge für Tickets aus einer markierten Stelle eines Dokuments; portiert aus
//! `packages/core/src/docs/TicketDraft.ts`.

use serde::{Deserialize, Serialize};

/// Stelle im Dokument, aus der ein Entwurf stammt; Zeilen 1-basiert und einschließlich
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketDraftSource {
    pub heading: String,
    pub start_line: i64,
    pub end_line: i64,
}

/// Vorschlag für ein Ticket aus einer markierten Stelle; `depends_on` sind 0-basierte Indizes
/// in dieselbe Liste
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketDraft {
    pub title: String,
    pub description: String,
    pub acceptance: String,
    pub steps: Vec<String>,
    pub depends_on: Vec<i64>,
    pub labels: Vec<String>,
    pub source: TicketDraftSource,
}

/// Geprüfte Entwürfe samt kurzen Hinweisen auf Verworfenes
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TicketDraftCheck {
    pub drafts: Vec<TicketDraft>,
    pub dropped: Vec<String>,
}
