//! Fachobjekte der Fragen auf der Board-Datenbank: Frage, Recherche mit Optionen,
//! Entscheidung, Kommentar, Verlauf. Portiert aus
//! `packages/core/src/board/BoardQuestionTypes.ts`.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::basics::{BoardActor, TicketStatus};
use crate::actors::AuthorKind;
use crate::questions::QuestionState;

// Höchstlängen der Texte (passen zu den Spalten in board.Questions* aus 002_questions.sql)
pub const MAX_QUESTION_TITLE_LENGTH: usize = 400;
pub const MAX_QUESTION_OCCASION_LENGTH: usize = 400;
pub const MAX_OPTION_TITLE_LENGTH: usize = 400;
pub const MAX_SESSION_LENGTH: usize = 100;

/// Arten von Verlaufseinträgen einer Frage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "question.ts")]
pub enum BoardQuestionHistoryType {
    Created,
    Title,
    Description,
    Tags,
    Occasion,
    Tickets,
    Depends,
    Research,
    Decided,
    Discarded,
    Reopened,
    TicketDeleted,
}

impl BoardQuestionHistoryType {
    /// Alle Arten in der Reihenfolge von TypeScript (`BOARD_QUESTION_HISTORY_TYPES`)
    pub const ALL: [BoardQuestionHistoryType; 12] = [
        BoardQuestionHistoryType::Created,
        BoardQuestionHistoryType::Title,
        BoardQuestionHistoryType::Description,
        BoardQuestionHistoryType::Tags,
        BoardQuestionHistoryType::Occasion,
        BoardQuestionHistoryType::Tickets,
        BoardQuestionHistoryType::Depends,
        BoardQuestionHistoryType::Research,
        BoardQuestionHistoryType::Decided,
        BoardQuestionHistoryType::Discarded,
        BoardQuestionHistoryType::Reopened,
        BoardQuestionHistoryType::TicketDeleted,
    ];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            BoardQuestionHistoryType::Created => "created",
            BoardQuestionHistoryType::Title => "title",
            BoardQuestionHistoryType::Description => "description",
            BoardQuestionHistoryType::Tags => "tags",
            BoardQuestionHistoryType::Occasion => "occasion",
            BoardQuestionHistoryType::Tickets => "tickets",
            BoardQuestionHistoryType::Depends => "depends",
            BoardQuestionHistoryType::Research => "research",
            BoardQuestionHistoryType::Decided => "decided",
            BoardQuestionHistoryType::Discarded => "discarded",
            BoardQuestionHistoryType::Reopened => "reopened",
            BoardQuestionHistoryType::TicketDeleted => "ticket-deleted",
        }
    }
}

impl fmt::Display for BoardQuestionHistoryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Alle Arten in der Reihenfolge von TypeScript
pub const BOARD_QUESTION_HISTORY_TYPES: [BoardQuestionHistoryType; 12] =
    BoardQuestionHistoryType::ALL;

/// Wer bei Fragen handelt: Benutzer-GUID und Name wie bei Tickets, dazu die Art und
/// bei Sessions deren Kennung
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionActor {
    #[serde(flatten)]
    #[ts(flatten)]
    pub actor: BoardActor,
    pub kind: AuthorKind,
    pub session: Option<String>,
}

/// Ein Lösungsvorschlag einer Recherche; Nummer fortlaufend je Frage (O1, O2, …)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionOption {
    pub id: String,
    pub title: String,
    pub text: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub recommended: bool,
}

/// Eine Recherche zu einer Frage mit Kontext und Optionen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionResearch {
    pub id: i64,
    pub question_id: String,
    pub at: String,
    pub by_id: String,
    pub by_name: String,
    pub kind: AuthorKind,
    pub context: String,
    pub options: Vec<BoardQuestionOption>,
}

/// Zahl der Recherchen einer Frage und Zahl der Optionen in der neuesten
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionResearchSummary {
    pub count: i32,
    pub option_count: i32,
}

/// Ausgang einer Entscheidung: beantwortet oder verworfen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "question.ts")]
pub enum BoardQuestionDecisionOutcome {
    Answered,
    Discarded,
}

impl BoardQuestionDecisionOutcome {
    /// Beide Ausgänge in der Reihenfolge von TypeScript
    pub const ALL: [BoardQuestionDecisionOutcome; 2] = [
        BoardQuestionDecisionOutcome::Answered,
        BoardQuestionDecisionOutcome::Discarded,
    ];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            BoardQuestionDecisionOutcome::Answered => "answered",
            BoardQuestionDecisionOutcome::Discarded => "discarded",
        }
    }
}

impl fmt::Display for BoardQuestionDecisionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Entscheidung eines Menschen (beantwortet oder verworfen) mit Begründung
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionDecision {
    pub outcome: BoardQuestionDecisionOutcome,
    pub option: Option<String>,
    pub text: Option<String>,
    pub reason: String,
    pub by_id: String,
    pub by_name: String,
    pub at: String,
}

/// Eine Frage an den Menschen, wie sie gespeichert ist
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestion {
    pub id: String,
    pub project_id: i32,
    pub number: i32,
    pub title: String,
    pub description: String,
    pub state: QuestionState,
    pub occasion: Option<String>,
    pub origin_kind: AuthorKind,
    pub origin_name: String,
    pub origin_session: Option<String>,
    pub tags: Vec<String>,
    /// Betroffene Tickets, sortiert
    pub ticket_ids: Vec<String>,
    /// Fragen oder Tickets, von denen die Klärung abhängt (Reihenfolge der Eingabe)
    pub depends_on: Vec<String>,
    pub decision: Option<BoardQuestionDecision>,
    pub created_at: String,
    pub created_by_id: String,
    pub updated_at: String,
    pub rev: i32,
}

/// Ein Kommentar an einer Frage
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionComment {
    pub id: i64,
    pub question_id: String,
    pub author_id: String,
    pub author_name: String,
    pub author_kind: AuthorKind,
    pub body: String,
    pub created_at: String,
}

/// Ein Eintrag im Verlauf einer Frage; `by_kind` ist null bei Einträgen ohne
/// eigentlichen Autor (z. B. Ticket gelöscht)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionHistoryEntry {
    pub question_id: String,
    pub at: String,
    pub by_id: String,
    pub by_name: String,
    pub by_kind: Option<AuthorKind>,
    /// Heißt in JSON `type`; `type` ist in Rust ein Schlüsselwort
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub entry_type: BoardQuestionHistoryType,
    pub text: String,
}

/// Ein betroffenes Ticket mit Titel und Status zur Anzeige
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionTicketRef {
    pub id: String,
    pub title: String,
    pub status: TicketStatus,
    pub board_id: i32,
}

/// Eine offene Abhängigkeit mit ihrem Zustand (Fragezustand oder Ticketstatus)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionWaiting {
    pub id: String,
    pub state: String,
}

/// Anzeigefassung einer Frage: berechnete Angaben, nicht gespeichert
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionView {
    #[serde(flatten)]
    #[ts(flatten)]
    pub question: BoardQuestion,
    pub project_key: String,
    pub tickets: Vec<BoardQuestionTicketRef>,
    pub waiting_on: Vec<BoardQuestionWaiting>,
    pub ready: bool,
    pub researched: bool,
    pub option_count: i32,
}

/// Anzeigefassung samt Recherchen, Kommentaren und Verlauf
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub view: BoardQuestionView,
    pub research: Vec<BoardQuestionResearch>,
    pub comments: Vec<BoardQuestionComment>,
    pub history: Vec<BoardQuestionHistoryEntry>,
}

/// Offene Frage, auf die ein Ticket wartet (Karte, Panel, CLI)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardOpenQuestion {
    pub id: String,
    pub title: String,
}

/// Herkunft einer zu meldenden Frage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct NotifiableOrigin {
    pub kind: AuthorKind,
}

/// Eine Recherche, wie sie die Meldung braucht: nur Art und Optionen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct NotifiableResearch {
    pub kind: AuthorKind,
    pub options: Vec<BoardQuestionOption>,
}

/// Eine Frage, wie sie die Windows-Meldungen brauchen (offene eigene Fragen:
/// selbst gestellt oder zu Tickets auf eigenen Boards)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct NotifiableBoardQuestion {
    pub id: String,
    pub title: String,
    pub occasion: Option<String>,
    pub origin: NotifiableOrigin,
    pub research: Vec<NotifiableResearch>,
    pub project_key: String,
    pub project_name: String,
    /// Eigene Repo-Pfade der Boards mit betroffenen Tickets, sonst der eigenen Boards
    /// im Projekt (für die Adresse in der Meldung)
    pub repo_paths: Vec<String>,
}
