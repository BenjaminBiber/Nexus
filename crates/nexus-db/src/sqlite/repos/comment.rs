//! Kommentare an Tickets; portiert aus
//! `packages/db/src/sqlite/SqliteCommentRepository.ts`.
//!
//! Nur anhängen; einzig die Lesemarke des Agenten wird nachgetragen. Löschen gibt es nicht im
//! Vertrag – Kommentare verschwinden per Kaskade mit ihrem Ticket.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use async_trait::async_trait;

use nexus_core::board::database::{CommentRepository, NewComment};
use nexus_core::board::types::Comment;
use nexus_core::Result;

use crate::sqlite::runner::{date, guid, iso, iso_millis, missing_row};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite::{Params, SqlValue};
use crate::sqlite_params;
use crate::{SqlChunks, SQL_CHUNK_SIZE};

const COLUMNS: &str = "Id, TicketId, AuthorId, AuthorName, Body, CreatedAt, ReadByAgentAt";

/// Parameternamen `p0`…`p499` für die IN-Listen der Blöcke. Der Runner bindet nur `&'static str`,
/// deshalb entstehen die Namen einmalig und bleiben danach liegen.
fn chunk_names() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        (0..SQL_CHUNK_SIZE)
            .map(|index| &*Box::leak(format!("p{index}").into_boxed_str()))
            .collect()
    })
}

/// Die IN-Liste `@p0, @p1, …` und die Parameter eines Blocks
fn chunk_params<T: Clone + Into<SqlValue>>(values: &[T]) -> (String, Params) {
    let names = &chunk_names()[..values.len()];
    let list = names
        .iter()
        .map(|name| format!("@{name}"))
        .collect::<Vec<_>>()
        .join(", ");
    let params = names
        .iter()
        .zip(values.iter())
        .map(|(name, value)| (*name, value.clone().into()))
        .collect();
    (list, params)
}

/// Zeile → Kommentar
fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Comment> {
    Ok(Comment {
        id: row.get(0)?,
        ticket_id: row.get(1)?,
        author_id: guid(&row.get::<_, String>(2)?),
        author_name: row.get(3)?,
        body: row.get(4)?,
        created_at: iso_millis(&row.get::<_, String>(5)?),
        read_by_agent_at: iso(row.get(6)?),
    })
}

#[async_trait]
impl CommentRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<Comment>> {
        let sql = format!("SELECT {COLUMNS} FROM Comments WHERE TicketId = @ticketId ORDER BY Id");
        self.sql
            .query_rows(&sql, &sqlite_params! { "ticketId": ticket_id }, map_row)
    }

    async fn insert(&mut self, comment: &NewComment) -> Result<Comment> {
        let read_by_agent_at = comment.read_by_agent_at.clone();
        let params: Params = sqlite_params! {
            "ticketId": comment.ticket_id.clone(),
            "authorId": comment.author_id.to_lowercase(),
            "authorName": comment.author_name.clone(),
            "body": comment.body.clone(),
            "createdAt": date(&comment.created_at),
            "readByAgentAt": read_by_agent_at.as_deref().map_or(SqlValue::Null, date),
        };
        let ids = self.sql.query_rows(
            "INSERT INTO Comments (TicketId, AuthorId, AuthorName, Body, CreatedAt, ReadByAgentAt)
       VALUES (@ticketId, @authorId, @authorName, @body, @createdAt, @readByAgentAt) RETURNING Id",
            &params,
            |row| row.get::<_, i64>(0),
        )?;
        let id = ids
            .into_iter()
            .next()
            .ok_or_else(|| missing_row("Comments.insert"))?;
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
        let zeilen = self.sql.query_rows(
            "SELECT c.TicketId, COUNT(*) AS N FROM Comments c JOIN Tickets t ON t.Id = c.TicketId WHERE t.BoardId = @boardId GROUP BY c.TicketId",
            &sqlite_params! { "boardId": board_id },
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )?;
        Ok(zeilen.into_iter().collect())
    }

    async fn unread_by_agent(&mut self, ticket_ids: &[String]) -> Result<Vec<Comment>> {
        let mut result = Vec::new();
        for chunk in SqlChunks::of(ticket_ids) {
            let (list, params) = chunk_params(chunk);
            let sql = format!(
                "SELECT {COLUMNS} FROM Comments WHERE ReadByAgentAt IS NULL AND TicketId IN ({list})"
            );
            result.extend(self.sql.query_rows(&sql, &params, map_row)?);
        }
        // Reihenfolge des Anlegens, unabhängig von den Blöcken (wie in TypeScript nach dem Lesen)
        result.sort_by_key(|comment| comment.id);
        Ok(result)
    }

    async fn mark_read_by_agent(&mut self, ids: &[i64], at: &str) -> Result<()> {
        for chunk in SqlChunks::of(ids) {
            let (list, mut params) = chunk_params(chunk);
            params.push(("at", date(at)));
            let sql = format!(
                "UPDATE Comments SET ReadByAgentAt = @at WHERE ReadByAgentAt IS NULL AND Id IN ({list})"
            );
            self.sql.execute(&sql, &params)?;
        }
        Ok(())
    }
}
