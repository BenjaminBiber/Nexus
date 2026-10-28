//! Kommentare an Tickets auf SQL Server; portiert aus
//! `packages/db/src/MssqlCommentRepository.ts`.
//!
//! Nur anhängen; einzig die Lesemarke des Agenten wird nachgetragen. Lange ID-Listen laufen
//! über [`SqlChunks`], damit eine Abfrage unter der Parametergrenze von SQL Server bleibt.

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::database::{CommentRepository, NewComment};
use nexus_core::board::types::Comment;
use nexus_core::Result;

use crate::mssql::repos::{borrow_params, first_row};
use crate::mssql::runner::{MsRow, MsValue};
use crate::mssql::unit_of_work::MssqlUnitOfWork;
use crate::SqlChunks;

const COLUMNS: &str = "Id, TicketId, AuthorId, AuthorName, Body, CreatedAt, ReadByAgentAt";

/// Zeile → Kommentar
fn map_row(row: &MsRow) -> Result<Comment> {
    Ok(Comment {
        id: row.get_i64("Id")?,
        ticket_id: row.get_str("TicketId")?,
        author_id: row.get_guid("AuthorId")?,
        author_name: row.get_str("AuthorName")?,
        body: row.get_str("Body")?,
        created_at: row.get_iso("CreatedAt")?,
        read_by_agent_at: row.get_opt_iso("ReadByAgentAt")?,
    })
}

#[async_trait]
impl CommentRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<Comment>> {
        let sql =
            format!("SELECT {COLUMNS} FROM board.Comments WHERE TicketId = @ticketId ORDER BY Id");
        let rows = self
            .sql
            .query_rows(&sql, &[("ticketId", MsValue::text(ticket_id))])
            .await?;
        rows.iter().map(map_row).collect()
    }

    async fn insert(&mut self, comment: &NewComment) -> Result<Comment> {
        let read_by_agent_at = comment.read_by_agent_at.clone();
        let rows = self
            .sql
            .query_rows(
                "INSERT INTO board.Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt, ReadByAgentAt) OUTPUT inserted.Id
       VALUES (@ticketId, @authorId, @authorName, @body, @createdAt, @readByAgentAt)",
                &[
                    ("ticketId", MsValue::text(&comment.ticket_id)),
                    ("authorId", MsValue::text(&comment.author_id)),
                    ("authorName", MsValue::text(&comment.author_name)),
                    ("body", MsValue::text(&comment.body)),
                    ("createdAt", MsValue::date(&comment.created_at)?),
                    (
                        "readByAgentAt",
                        MsValue::date_opt(read_by_agent_at.as_deref())?,
                    ),
                ],
            )
            .await?;
        let id = first_row(&rows, "comments.insert")?.get_i64("Id")?;
        Ok(Comment {
            id,
            ticket_id: comment.ticket_id.clone(),
            author_id: comment.author_id.clone(),
            author_name: comment.author_name.clone(),
            body: comment.body.clone(),
            created_at: comment.created_at.clone(),
            read_by_agent_at,
        })
    }

    async fn count_by_board(&mut self, board_id: i32) -> Result<BTreeMap<String, i64>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT c.TicketId, COUNT(*) AS N FROM board.Comments c JOIN board.Tickets t ON t.Id = c.TicketId WHERE t.BoardId = @boardId GROUP BY c.TicketId",
                &[("boardId", MsValue::Int(board_id))],
            )
            .await?;
        rows.iter()
            .map(|row| Ok((row.get_str("TicketId")?, row.get_i64("N")?)))
            .collect()
    }

    async fn unread_by_agent(&mut self, ticket_ids: &[String]) -> Result<Vec<Comment>> {
        let mut result: Vec<Comment> = Vec::new();
        for chunk in SqlChunks::of(ticket_ids) {
            let werte: Vec<MsValue> = chunk.iter().map(|id| MsValue::text(id.as_str())).collect();
            let (list, params) = SqlChunks::params("t", &werte);
            let sql = format!(
                "SELECT {COLUMNS} FROM board.Comments WHERE ReadByAgentAt IS NULL AND TicketId IN ({list})"
            );
            let rows = self.sql.query_rows(&sql, &borrow_params(&params)).await?;
            for row in &rows {
                result.push(map_row(row)?);
            }
        }
        // Reihenfolge des Anlegens, unabhängig von den Blöcken (wie in TypeScript nach dem Lesen)
        result.sort_by_key(|comment| comment.id);
        Ok(result)
    }

    async fn mark_read_by_agent(&mut self, ids: &[i64], at: &str) -> Result<()> {
        for chunk in SqlChunks::of(ids) {
            let werte: Vec<MsValue> = chunk.iter().map(|id| MsValue::BigInt(*id)).collect();
            let (list, mut params) = SqlChunks::params("c", &werte);
            params.push(("at".to_string(), MsValue::date(at)?));
            let sql = format!(
                "UPDATE board.Comments SET ReadByAgentAt = @at WHERE ReadByAgentAt IS NULL AND Id IN ({list})"
            );
            self.sql.execute(&sql, &borrow_params(&params)).await?;
        }
        Ok(())
    }
}
