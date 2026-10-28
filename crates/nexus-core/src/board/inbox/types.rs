//! Einträge der persönlichen Arbeitsliste „Meine Aufgaben“: was gerade auf den Benutzer
//! wartet, aus vorhandenen Board-Daten. Portiert aus
//! `packages/core/src/board/inbox/InboxTypes.ts`.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Art eines Eintrags; die Reihenfolge ist zugleich die Dringlichkeit (merge zuerst)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "inbox.ts")]
pub enum InboxKind {
    Merge,
    Setup,
    Question,
    Accept,
    Blocked,
    Planning,
    Note,
}

impl InboxKind {
    /// Alle Arten in der Reihenfolge von TypeScript (`INBOX_KINDS`)
    pub const ALL: [InboxKind; 7] = [
        InboxKind::Merge,
        InboxKind::Setup,
        InboxKind::Question,
        InboxKind::Accept,
        InboxKind::Blocked,
        InboxKind::Planning,
        InboxKind::Note,
    ];

    /// Schreibweise in JSON und im gespeicherten Schlüssel
    pub fn as_str(&self) -> &'static str {
        match self {
            InboxKind::Merge => "merge",
            InboxKind::Setup => "setup",
            InboxKind::Question => "question",
            InboxKind::Accept => "accept",
            InboxKind::Blocked => "blocked",
            InboxKind::Planning => "planning",
            InboxKind::Note => "note",
        }
    }

    /// Anzeigename der Art (`INBOX_KIND_LABELS`)
    pub fn label(&self) -> &'static str {
        match self {
            InboxKind::Merge => "Merge gescheitert",
            InboxKind::Setup => "Einrichtung gescheitert",
            InboxKind::Question => "Frage",
            InboxKind::Accept => "Abnahme",
            InboxKind::Blocked => "Blockiert",
            InboxKind::Planning => "In Planung",
            InboxKind::Note => "Notiz",
        }
    }
}

impl fmt::Display for InboxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Alle Arten in der Reihenfolge der Dringlichkeit
pub const INBOX_KINDS: [InboxKind; 7] = InboxKind::ALL;

/// Ein Eintrag der Arbeitsliste
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "inbox.ts")]
pub struct InboxItem {
    /// Stabiler Schlüssel für die gespeicherte Reihenfolge:
    /// `ticket:AIH-0001`, `question:AIH-F0002`, `note:7`
    pub key: String,
    pub kind: InboxKind,
    pub title: String,
    /// Blockiert-Grund, Anlass der Frage oder Auszug der Notiz
    pub detail: Option<String>,
    pub board_id: Option<i32>,
    pub board_name: Option<String>,
    pub project_key: String,
    pub ticket_id: Option<String>,
    pub question_id: Option<String>,
    pub note_id: Option<i32>,
    /// Nur bei Fragen: Zahl der offenen Tickets, die auf die Frage warten
    pub waiting_tickets: i32,
    /// Priorität des Tickets (nur bei Ticket-Einträgen), sortiert Einträge gleicher Art
    pub priority: Option<String>,
    /// ISO-Zeitpunkt: letzte Änderung des Tickets bzw. Anlage von Frage und Notiz
    pub since: String,
    /// Dringlichkeit, kleiner = dringender
    pub rank: i32,
}

/// Zähler je Art; ein Struct statt einer Map, damit in TypeScript jede Art fest
/// vorkommt (`Record<InboxKind, number>`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "inbox.ts")]
pub struct InboxCounts {
    pub merge: i32,
    pub setup: i32,
    pub question: i32,
    pub accept: i32,
    pub blocked: i32,
    pub planning: i32,
    pub note: i32,
}

impl InboxCounts {
    /// Zähler einer Art lesen
    pub fn get(&self, kind: InboxKind) -> i32 {
        match kind {
            InboxKind::Merge => self.merge,
            InboxKind::Setup => self.setup,
            InboxKind::Question => self.question,
            InboxKind::Accept => self.accept,
            InboxKind::Blocked => self.blocked,
            InboxKind::Planning => self.planning,
            InboxKind::Note => self.note,
        }
    }
}

/// Liste samt Zählern je Art
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "inbox.ts")]
pub struct InboxList {
    pub items: Vec<InboxItem>,
    pub counts: InboxCounts,
    pub total: i32,
}

/// Format gespeicherter Schlüssel (auch für spätere eigene ToDos `todo:`)
pub const INBOX_KEY_PATTERN: &str = r"^(ticket|question|note|todo):[A-Za-z0-9-]+$";
pub const MAX_INBOX_KEYS: usize = 1000;
pub const MAX_INBOX_KEY_LENGTH: usize = 60;
