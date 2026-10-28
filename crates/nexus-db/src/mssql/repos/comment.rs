//! Kommentare an Tickets auf SQL Server; portiert aus
//! `packages/db/src/MssqlCommentRepository.ts`. Gerüst – gefüllt von P2-13.

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::database::{CommentRepository, NewComment};
use nexus_core::board::types::Comment;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl CommentRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, _ticket_id: &str) -> Result<Vec<Comment>> {
        Err(not_implemented("comments.list", "P2-13"))
    }

    async fn insert(&mut self, _comment: &NewComment) -> Result<Comment> {
        Err(not_implemented("comments.insert", "P2-13"))
    }

    async fn count_by_board(&mut self, _board_id: i32) -> Result<BTreeMap<String, i64>> {
        Err(not_implemented("comments.count_by_board", "P2-13"))
    }

    async fn unread_by_agent(&mut self, _ticket_ids: &[String]) -> Result<Vec<Comment>> {
        Err(not_implemented("comments.unread_by_agent", "P2-13"))
    }

    async fn mark_read_by_agent(&mut self, _ids: &[i64], _at: &str) -> Result<()> {
        Err(not_implemented("comments.mark_read_by_agent", "P2-13"))
    }
}
