//! Verlauf einer Frage; Gerüst, gefüllt von P2-8 (Port von
//! `packages/db/src/sqlite/SqliteQuestionHistoryRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::BoardQuestionHistoryRepository;
use nexus_core::board::question_types::BoardQuestionHistoryEntry;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl BoardQuestionHistoryRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionHistoryEntry>> {
        Err(folgt("BoardQuestionHistoryRepository.list"))
    }

    async fn insert(&mut self, _entries: &[BoardQuestionHistoryEntry]) -> Result<()> {
        Err(folgt("BoardQuestionHistoryRepository.insert"))
    }
}
