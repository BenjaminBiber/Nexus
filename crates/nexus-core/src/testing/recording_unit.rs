//! Eine Unit of Work, die nur die Fragen bedient und jeden Zugriff aufzeichnet.
//! Entspricht `recordingUnit` aus `packages/core/tests/unit/board/QuestionTicketCleanup.test.ts`.
//!
//! Alle übrigen Repositories gibt es hier nicht: Wer sie anfasst, bekommt `unimplemented!()`.
//! So lässt sich die Reihenfolge der Sperren prüfen, ohne eine ganze Datenbank aufzubauen.

use async_trait::async_trait;

use crate::board::database::{
    BoardQuestionCommentRepository, BoardQuestionFilter, BoardQuestionHistoryRepository,
    BoardQuestionRepository, BoardQuestionResearchRepository, BoardRepository,
    BoardSearchRepository, BoardUnitOfWork, CommentRepository, DeletionRepository,
    DispatchRunRepository, DispatchStatsRepository, HistoryRepository, NoteRepository,
    ProjectRepository, TicketRepository, TicketSessionRepository, TicketStepRepository,
    UserRepository,
};
use crate::board::question_types::{BoardQuestion, BoardQuestionHistoryEntry};
use crate::error::Result;
use crate::testing::fixtures::question_fixture;

/// Zeichnet die Zugriffe auf Fragen in ihrer Reihenfolge auf.
#[derive(Debug, Default)]
pub struct RecordingUnit {
    /// IDs, die `ids_referencing_ticket` liefert (unsortiert wie aus der Datenbank)
    referencing: Vec<String>,
    calls: Vec<String>,
}

impl RecordingUnit {
    /// Eine Unit of Work, deren Fragen diese Tickets betreffen
    pub fn new(referencing: &[&str]) -> Self {
        Self {
            referencing: referencing.iter().map(|id| id.to_string()).collect(),
            calls: Vec::new(),
        }
    }

    /// Die aufgezeichneten Zugriffe
    pub fn calls(&self) -> &[String] {
        &self.calls
    }
}

#[async_trait]
impl BoardQuestionRepository for RecordingUnit {
    async fn by_id(&mut self, id: &str, for_update: bool) -> Result<Option<BoardQuestion>> {
        self.calls
            .push(format!("{} {id}", if for_update { "lock" } else { "get" }));
        Ok(Some(question_fixture(id, 1, 1)))
    }

    async fn list(&mut self, _filter: &BoardQuestionFilter) -> Result<Vec<BoardQuestion>> {
        unimplemented!("die Aufzeichnung kennt nur die Zugriffe des Aufräumens")
    }

    async fn insert(&mut self, _question: &BoardQuestion) -> Result<()> {
        unimplemented!("die Aufzeichnung kennt nur die Zugriffe des Aufräumens")
    }

    async fn update(&mut self, question: &BoardQuestion) -> Result<()> {
        self.calls.push(format!("update {}", question.id));
        Ok(())
    }

    async fn ids_referencing_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        self.calls.push(format!("read {ticket_id}"));
        Ok(self.referencing.clone())
    }

    async fn remove_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        self.calls.push(format!("remove {ticket_id}"));
        let mut affected = self.referencing.clone();
        affected.sort();
        Ok(affected)
    }
}

#[async_trait]
impl BoardQuestionHistoryRepository for RecordingUnit {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionHistoryEntry>> {
        unimplemented!("die Aufzeichnung kennt nur die Zugriffe des Aufräumens")
    }

    async fn insert(&mut self, _entries: &[BoardQuestionHistoryEntry]) -> Result<()> {
        Ok(())
    }
}

/// Nur `questions` und `question_history` gibt es; alles andere fehlt bewusst
impl BoardUnitOfWork for RecordingUnit {
    fn questions(&mut self) -> &mut dyn BoardQuestionRepository {
        self
    }

    fn question_history(&mut self) -> &mut dyn BoardQuestionHistoryRepository {
        self
    }

    fn users(&mut self) -> &mut dyn UserRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn projects(&mut self) -> &mut dyn ProjectRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn boards(&mut self) -> &mut dyn BoardRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn tickets(&mut self) -> &mut dyn TicketRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn comments(&mut self) -> &mut dyn CommentRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn steps(&mut self) -> &mut dyn TicketStepRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn ticket_sessions(&mut self) -> &mut dyn TicketSessionRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn history(&mut self) -> &mut dyn HistoryRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn notes(&mut self) -> &mut dyn NoteRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn deletions(&mut self) -> &mut dyn DeletionRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn question_research(&mut self) -> &mut dyn BoardQuestionResearchRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn question_comments(&mut self) -> &mut dyn BoardQuestionCommentRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn search(&mut self) -> &mut dyn BoardSearchRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn dispatch_runs(&mut self) -> &mut dyn DispatchRunRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }

    fn dispatch_stats(&mut self) -> &mut dyn DispatchStatsRepository {
        unimplemented!("die Aufzeichnung kennt nur die Fragen")
    }
}
