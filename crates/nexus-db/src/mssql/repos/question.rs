//! Fragen an den Nutzer auf SQL Server; portiert aus
//! `packages/db/src/MssqlQuestionRepository.ts`. Gerüst – gefüllt von P2-13.

use async_trait::async_trait;
use nexus_core::board::database::{BoardQuestionFilter, BoardQuestionRepository};
use nexus_core::board::question_types::BoardQuestion;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl BoardQuestionRepository for MssqlUnitOfWork<'_> {
    async fn by_id(&mut self, _id: &str, _for_update: bool) -> Result<Option<BoardQuestion>> {
        Err(not_implemented("questions.by_id", "P2-13"))
    }

    async fn list(&mut self, _filter: &BoardQuestionFilter) -> Result<Vec<BoardQuestion>> {
        Err(not_implemented("questions.list", "P2-13"))
    }

    async fn insert(&mut self, _question: &BoardQuestion) -> Result<()> {
        Err(not_implemented("questions.insert", "P2-13"))
    }

    async fn update(&mut self, _question: &BoardQuestion) -> Result<()> {
        Err(not_implemented("questions.update", "P2-13"))
    }

    async fn ids_referencing_ticket(&mut self, _ticket_id: &str) -> Result<Vec<String>> {
        Err(not_implemented("questions.ids_referencing_ticket", "P2-13"))
    }

    async fn remove_ticket(&mut self, _ticket_id: &str) -> Result<Vec<String>> {
        Err(not_implemented("questions.remove_ticket", "P2-13"))
    }
}
