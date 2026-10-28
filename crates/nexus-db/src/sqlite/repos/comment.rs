//! Kommentare an Tickets; Gerüst, gefüllt von P2-7 (Port von
//! `packages/db/src/sqlite/SqliteCommentRepository.ts`).

use std::collections::BTreeMap;

use async_trait::async_trait;

use nexus_core::board::database::{CommentRepository, NewComment};
use nexus_core::board::types::Comment;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl CommentRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, _ticket_id: &str) -> Result<Vec<Comment>> {
        Err(folgt("CommentRepository.list"))
    }

    async fn insert(&mut self, _comment: &NewComment) -> Result<Comment> {
        Err(folgt("CommentRepository.insert"))
    }

    async fn count_by_board(&mut self, _board_id: i32) -> Result<BTreeMap<String, i64>> {
        Err(folgt("CommentRepository.count_by_board"))
    }

    async fn unread_by_agent(&mut self, _ticket_ids: &[String]) -> Result<Vec<Comment>> {
        Err(folgt("CommentRepository.unread_by_agent"))
    }

    async fn mark_read_by_agent(&mut self, _ids: &[i64], _at: &str) -> Result<()> {
        Err(folgt("CommentRepository.mark_read_by_agent"))
    }
}
