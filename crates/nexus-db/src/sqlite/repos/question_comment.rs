//! Kommentare an einer Frage; Gerüst, gefüllt von P2-8 (Port von
//! `packages/db/src/sqlite/SqliteQuestionCommentRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::{BoardQuestionCommentRepository, NewQuestionComment};
use nexus_core::board::question_types::BoardQuestionComment;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl BoardQuestionCommentRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionComment>> {
        Err(folgt("BoardQuestionCommentRepository.list"))
    }

    async fn insert(
        &mut self,
        _comment: &NewQuestionComment,
    ) -> Result<BoardQuestionComment> {
        Err(folgt("BoardQuestionCommentRepository.insert"))
    }
}
