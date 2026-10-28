//! Kommentare an einer Frage auf SQL Server; portiert aus
//! `packages/db/src/MssqlQuestionCommentRepository.ts`. Gerüst – gefüllt von P2-13.

use async_trait::async_trait;
use nexus_core::board::database::{BoardQuestionCommentRepository, NewQuestionComment};
use nexus_core::board::question_types::BoardQuestionComment;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl BoardQuestionCommentRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionComment>> {
        Err(not_implemented("question_comments.list", "P2-13"))
    }

    async fn insert(&mut self, _comment: &NewQuestionComment) -> Result<BoardQuestionComment> {
        Err(not_implemented("question_comments.insert", "P2-13"))
    }
}
