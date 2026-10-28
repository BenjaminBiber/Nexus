//! Verlauf einer Frage auf SQL Server; portiert aus
//! `packages/db/src/MssqlQuestionHistoryRepository.ts`. Gerüst – gefüllt von P2-13.

use async_trait::async_trait;
use nexus_core::board::database::BoardQuestionHistoryRepository;
use nexus_core::board::question_types::BoardQuestionHistoryEntry;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl BoardQuestionHistoryRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionHistoryEntry>> {
        Err(not_implemented("question_history.list", "P2-13"))
    }

    async fn insert(&mut self, _entries: &[BoardQuestionHistoryEntry]) -> Result<()> {
        Err(not_implemented("question_history.insert", "P2-13"))
    }
}
