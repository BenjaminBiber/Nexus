//! Fragen an den Nutzer; Gerüst, gefüllt von P2-8 (Port von
//! `packages/db/src/sqlite/SqliteQuestionRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::{BoardQuestionFilter, BoardQuestionRepository};
use nexus_core::board::question_types::BoardQuestion;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl BoardQuestionRepository for SqliteUnitOfWork<'_> {
    async fn by_id(&mut self, _id: &str, _for_update: bool) -> Result<Option<BoardQuestion>> {
        Err(folgt("BoardQuestionRepository.by_id"))
    }

    async fn list(&mut self, _filter: &BoardQuestionFilter) -> Result<Vec<BoardQuestion>> {
        Err(folgt("BoardQuestionRepository.list"))
    }

    async fn insert(&mut self, _question: &BoardQuestion) -> Result<()> {
        Err(folgt("BoardQuestionRepository.insert"))
    }

    async fn update(&mut self, _question: &BoardQuestion) -> Result<()> {
        Err(folgt("BoardQuestionRepository.update"))
    }

    async fn ids_referencing_ticket(&mut self, _ticket_id: &str) -> Result<Vec<String>> {
        Err(folgt("BoardQuestionRepository.ids_referencing_ticket"))
    }

    async fn remove_ticket(&mut self, _ticket_id: &str) -> Result<Vec<String>> {
        Err(folgt("BoardQuestionRepository.remove_ticket"))
    }
}
