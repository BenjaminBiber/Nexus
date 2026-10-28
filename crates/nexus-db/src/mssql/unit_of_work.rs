//! Alle Repositories über einer laufenden SQL-Transaktion; portiert aus
//! `packages/db/src/MssqlUnitOfWork.ts`.
//!
//! In TypeScript ist jedes Repository ein eigenes Objekt mit dem `SqlRunner` als Feld. In Rust
//! erfüllt **ein** Struct alle 17 Traits – sonst müssten 17 Objekte dieselbe `&mut`-Verbindung
//! teilen. Die Zugriffe geben deshalb jeweils `self` zurück; die Abfragen je Repository liegen
//! trotzdem in eigenen Dateien unter [`crate::mssql::repos`].

use nexus_core::board::database::{
    BoardQuestionCommentRepository, BoardQuestionHistoryRepository, BoardQuestionRepository,
    BoardQuestionResearchRepository, BoardRepository, BoardSearchRepository, BoardUnitOfWork,
    CommentRepository, DeletionRepository, DispatchRunRepository, DispatchStatsRepository,
    HistoryRepository, NoteRepository, ProjectRepository, TicketRepository,
    TicketSessionRepository, TicketStepRepository, UserRepository,
};

use crate::mssql::feature_gate::MssqlFeatureGate;
use crate::mssql::runner::MssqlRunner;

/// Alle Repositories über einer laufenden SQL-Transaktion.
///
/// `gate` ist nur für die Repositories da, die eine additive Erweiterung brauchen (Notizen,
/// Dispatcher-Läufe, Suche, Zielbranch) – in TypeScript bekommen genau diese `missingFeatures`
/// und `database` in den Konstruktor.
pub struct MssqlUnitOfWork<'c> {
    pub(crate) sql: &'c mut dyn MssqlRunner,
    pub(crate) gate: MssqlFeatureGate,
}

impl<'c> MssqlUnitOfWork<'c> {
    /// Unit of Work über diesem Runner; `missing_features` und `database` steuern das Gate
    pub fn new(
        sql: &'c mut dyn MssqlRunner,
        missing_features: Vec<String>,
        database: impl Into<String>,
    ) -> Self {
        Self {
            sql,
            gate: MssqlFeatureGate::new(missing_features, database),
        }
    }

    /// Das Feature-Gate dieser Transaktion
    pub fn gate(&self) -> &MssqlFeatureGate {
        &self.gate
    }

    /// Der Runner dieser Transaktion; die Repositories setzen ihre Abfragen darüber ab
    pub fn sql(&mut self) -> &mut dyn MssqlRunner {
        self.sql
    }
}

impl BoardUnitOfWork for MssqlUnitOfWork<'_> {
    fn users(&mut self) -> &mut dyn UserRepository {
        self
    }

    fn projects(&mut self) -> &mut dyn ProjectRepository {
        self
    }

    fn boards(&mut self) -> &mut dyn BoardRepository {
        self
    }

    fn tickets(&mut self) -> &mut dyn TicketRepository {
        self
    }

    fn comments(&mut self) -> &mut dyn CommentRepository {
        self
    }

    fn steps(&mut self) -> &mut dyn TicketStepRepository {
        self
    }

    fn ticket_sessions(&mut self) -> &mut dyn TicketSessionRepository {
        self
    }

    fn history(&mut self) -> &mut dyn HistoryRepository {
        self
    }

    fn notes(&mut self) -> &mut dyn NoteRepository {
        self
    }

    fn deletions(&mut self) -> &mut dyn DeletionRepository {
        self
    }

    fn questions(&mut self) -> &mut dyn BoardQuestionRepository {
        self
    }

    fn question_research(&mut self) -> &mut dyn BoardQuestionResearchRepository {
        self
    }

    fn question_comments(&mut self) -> &mut dyn BoardQuestionCommentRepository {
        self
    }

    fn question_history(&mut self) -> &mut dyn BoardQuestionHistoryRepository {
        self
    }

    fn search(&mut self) -> &mut dyn BoardSearchRepository {
        self
    }

    fn dispatch_runs(&mut self) -> &mut dyn DispatchRunRepository {
        self
    }

    fn dispatch_stats(&mut self) -> &mut dyn DispatchStatsRepository {
        self
    }
}
