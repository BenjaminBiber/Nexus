//! Alle Repositories über einer laufenden SQLite-Transaktion; portiert aus
//! `packages/db/src/sqlite/SqliteUnitOfWork.ts`.
//!
//! In TypeScript ist jedes Repository ein eigenes Objekt über demselben Runner. In Rust erfüllt
//! **ein** Struct alle 17 Repository-Traits – die Zugriffe liefern deshalb `self`. Die `impl`s
//! selbst liegen je Repository in einer eigenen Datei unter [`super::repos`].

use rusqlite::Connection;

use nexus_core::board::database::{
    BoardQuestionCommentRepository, BoardQuestionHistoryRepository, BoardQuestionRepository,
    BoardQuestionResearchRepository, BoardRepository, BoardSearchRepository, BoardUnitOfWork,
    CommentRepository, DeletionRepository, DispatchRunRepository, DispatchStatsRepository,
    HistoryRepository, NoteRepository, ProjectRepository, TicketRepository,
    TicketSessionRepository, TicketStepRepository, UserRepository,
};

use super::runner::SqliteRunner;

/// Die Unit of Work von SQLite: hält den Runner der laufenden Transaktion
pub struct SqliteUnitOfWork<'c> {
    pub(crate) sql: SqliteRunner<'c>,
}

impl<'c> SqliteUnitOfWork<'c> {
    /// Unit of Work über der Verbindung, auf der die Transaktion schon offen ist
    pub fn new(conn: &'c mut Connection) -> Self {
        Self {
            sql: SqliteRunner::new(conn),
        }
    }

    /// Der Runner der laufenden Transaktion; die Repositories arbeiten darüber
    pub fn sql(&mut self) -> &mut SqliteRunner<'c> {
        &mut self.sql
    }
}

impl BoardUnitOfWork for SqliteUnitOfWork<'_> {
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
