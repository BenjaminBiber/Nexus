//! Fachobjekte des Boards: Projekte, Boards, Tickets, Kommentare, Verlauf, Notizen.
//! Portiert aus `packages/core/src/board/BoardTypes.ts`; die Enums und Grenzen stehen
//! in [`crate::board::basics`].

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::basics::{NoteState, Priority, TicketStatus};

/// Ein abhakbarer Arbeitsschritt eines Tickets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct TicketStep {
    pub text: String,
    pub done: bool,
    /// Ausführliche Anleitung zum Schritt als Markdown (Code, Befehl, erwartetes Ergebnis)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
}

/// Erledigte und alle Schritte eines Tickets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct StepProgress {
    pub done: i32,
    pub total: i32,
}

/// Ein Benutzer des Boards (Tabelle board.Users)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct BoardUser {
    pub id: String,
    pub name: String,
}

/// Ein Projekt mit den fortlaufenden Nummern für Tickets und Fragen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct Project {
    pub id: i32,
    pub key: String,
    pub name: String,
    pub next_ticket_number: i32,
    pub next_question_number: i32,
    pub created_at: String,
    pub created_by: String,
}

/// Ein Board eines Projekts; `repo_path` ist der Ordner des Repositorys
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct Board {
    pub id: i32,
    pub project_id: i32,
    pub owner_id: String,
    pub name: String,
    pub repo_path: Option<String>,
    pub created_at: String,
}

/// Ein Ticket mit allem, was in der Tabelle board.Tickets steht
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct Ticket {
    pub id: String,
    pub project_id: i32,
    pub board_id: i32,
    pub number: i32,
    pub title: String,
    pub description: String,
    pub status: TicketStatus,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub depends_on: Vec<String>,
    pub sort_order: i32,
    pub assignee: Option<String>,
    pub needs_planning: bool,
    pub acceptance: String,
    pub plan: String,
    pub handbook: String,
    pub diagram: String,
    /// Spec-Dokument, aus dem das Ticket stammt (Pfad unter docs/superpowers);
    /// `spec_task` ist historisch und ungenutzt
    pub spec: Option<String>,
    pub spec_task: Option<String>,
    /// Branch und Worktree, in dem das Ticket umgesetzt wird (nexus board start)
    pub branch: Option<String>,
    pub worktree: Option<String>,
    /// Branch, in den das Ticket am Ende gemergt wird; null = Standardbranch des Projekts.
    /// Nur vor dem Start änderbar
    pub target_branch: Option<String>,
    /// Betroffene Bereiche (Pfade) sowie genutzte und gelieferte Schnittstellen
    pub touches: Vec<String>,
    pub consumes: Vec<String>,
    pub produces: Vec<String>,
    pub blocked_reason: Option<String>,
    pub planned_at: Option<String>,
    pub planned_by: Option<String>,
    pub planning_claim_by: Option<String>,
    pub planning_claim_at: Option<String>,
    pub created_at: String,
    pub created_by_id: String,
    pub created_by_name: String,
    pub updated_at: String,
    pub rev: i32,
}

/// Ein Kommentar an einem Ticket
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct Comment {
    pub id: i64,
    pub ticket_id: String,
    pub author_id: String,
    pub author_name: String,
    pub body: String,
    pub created_at: String,
    /// Wann der Kommentar in eine Session eingespeist wurde; null = noch nicht
    pub read_by_agent_at: Option<String>,
}

/// Ein Eintrag im Verlauf eines Tickets: ein geändertes Feld mit altem und neuem Wert
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct HistoryEntry {
    pub ticket_id: String,
    pub at: String,
    pub by_id: String,
    pub by_name: String,
    pub field: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub note: Option<String>,
}

/// Eine Notiz: ein Befund, der an einem anderen Ticket aufgefallen ist
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct Note {
    pub id: i32,
    pub board_id: i32,
    pub author_id: String,
    pub author_name: String,
    /// Was auffällt, als ganzer Satz; leer bei Notizen von vor der Trennung in Titel und Inhalt
    pub title: String,
    /// Details: wo, woran man es sieht, vermutete Ursache, Vorschlag (darf leer sein)
    pub body: String,
    pub source: Option<String>,
    pub labels: Vec<String>,
    /// Wie oft der Befund aufgefallen ist (1 = nur beim Anlegen)
    pub seen_count: i32,
    /// Weitere Tickets, bei denen er erneut aufgefallen ist (ohne das Herkunftsticket)
    pub seen_in: Vec<String>,
    pub state: NoteState,
    pub ticket_id: Option<String>,
    pub reason: Option<String>,
    pub created_at: String,
}

/// Ein gelöschtes Ticket im Papierkorb des Boards
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct Deletion {
    pub at: String,
    pub by_id: String,
    pub by_name: String,
    pub ticket_id: String,
    pub ticket_title: String,
    pub from_status: Option<TicketStatus>,
}
