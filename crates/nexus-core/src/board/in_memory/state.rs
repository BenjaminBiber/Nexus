//! Der Zustand der Speicher-Datenbank und die Merkliste der berührten Boards und Projekte;
//! portiert aus `State` und `Touched` in `packages/core/src/board/InMemoryBoardDatabase.ts`.

use std::collections::BTreeSet;

use crate::board::dispatch_run_types::{DispatchConfigVersion, DispatchRun};
use crate::board::question_types::{
    BoardQuestion, BoardQuestionComment, BoardQuestionHistoryEntry, BoardQuestionResearch,
};
use crate::board::types::{
    Board, BoardUser, Comment, Deletion, HistoryEntry, Note, Project, Ticket, TicketStep,
};

use super::search::SearchableState;

/// Die Schritte eines Tickets; in TypeScript ein Objekt `{ ticketId, steps }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TicketSteps {
    pub ticket_id: String,
    pub steps: Vec<TicketStep>,
}

/// Die Verknüpfung einer Claude-Session mit einem Ticket
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TicketSession {
    pub ticket_id: String,
    pub session_id: String,
    pub added_at: String,
}

/// Alle Daten der Speicher-Datenbank. Eine Transaktion arbeitet auf einer Kopie und übernimmt
/// sie nur bei Erfolg – das ersetzt `structuredClone` aus TypeScript.
#[derive(Debug, Clone)]
pub(crate) struct State {
    pub users: Vec<BoardUser>,
    pub projects: Vec<Project>,
    pub boards: Vec<Board>,
    pub tickets: Vec<Ticket>,
    pub comments: Vec<Comment>,
    pub steps: Vec<TicketSteps>,
    pub ticket_sessions: Vec<TicketSession>,
    pub history: Vec<HistoryEntry>,
    pub notes: Vec<Note>,
    pub deletions: Vec<Deletion>,
    pub questions: Vec<BoardQuestion>,
    pub question_research: Vec<BoardQuestionResearch>,
    pub question_comments: Vec<BoardQuestionComment>,
    pub question_history: Vec<BoardQuestionHistoryEntry>,
    pub dispatch_runs: Vec<DispatchRun>,
    pub dispatch_config_versions: Vec<DispatchConfigVersion>,
    /// Fortlaufende Kennung für alles, was die Datenbank vergibt (Projekte, Boards,
    /// Kommentare, Notizen, Recherchen); beginnt wie in TypeScript bei 1
    pub next_id: i64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            users: Vec::new(),
            projects: Vec::new(),
            boards: Vec::new(),
            tickets: Vec::new(),
            comments: Vec::new(),
            steps: Vec::new(),
            ticket_sessions: Vec::new(),
            history: Vec::new(),
            notes: Vec::new(),
            deletions: Vec::new(),
            questions: Vec::new(),
            question_research: Vec::new(),
            question_comments: Vec::new(),
            question_history: Vec::new(),
            dispatch_runs: Vec::new(),
            dispatch_config_versions: Vec::new(),
            next_id: 1,
        }
    }
}

impl State {
    /// Liefert die nächste Kennung und zählt weiter
    pub fn next_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Das Board eines Tickets, falls es das Ticket gibt
    pub fn board_of(&self, ticket_id: &str) -> Option<i32> {
        self.tickets
            .iter()
            .find(|ticket| ticket.id == ticket_id)
            .map(|ticket| ticket.board_id)
    }

    /// Die Sicht, die die Suche liest
    pub fn searchable(&self) -> SearchableState<'_> {
        SearchableState {
            boards: &self.boards,
            tickets: &self.tickets,
            comments: &self.comments,
            questions: &self.questions,
            question_comments: &self.question_comments,
            notes: &self.notes,
        }
    }
}

/// Was eine Transaktion berührt: Boards (`change_marker`) und Projekte mit geänderten Fragen
/// (`question_marker`)
#[derive(Debug, Clone, Default)]
pub(crate) struct Touched {
    pub boards: BTreeSet<i32>,
    pub projects: BTreeSet<i32>,
}
