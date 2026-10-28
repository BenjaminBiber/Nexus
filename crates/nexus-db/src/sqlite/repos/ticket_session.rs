//! Verknüpfung von Claude-Sessions mit Tickets in `TicketSessions`; portiert aus
//! `packages/db/src/sqlite/SqliteTicketSessionRepository.ts`.

use std::collections::BTreeMap;

use async_trait::async_trait;

use nexus_core::board::database::TicketSessionRepository;
use nexus_core::Result;

use crate::sqlite::runner::{date, Params};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;
use crate::SqlChunks;

#[async_trait]
impl TicketSessionRepository for SqliteUnitOfWork<'_> {
    /// Legt die Verknüpfung nur an, wenn es sie noch nicht gibt
    /// (Primärschlüssel TicketId+SessionId)
    async fn add(&mut self, ticket_id: &str, session_id: &str, at: &str) -> Result<bool> {
        let affected = self.sql.execute(
            "INSERT INTO TicketSessions (TicketId, SessionId, AddedAt)
       VALUES (@ticketId, @sessionId, @at)
       ON CONFLICT(TicketId, SessionId) DO NOTHING",
            &sqlite_params! {
                "ticketId": ticket_id,
                "sessionId": session_id,
                "at": date(at),
            },
        )?;
        Ok(affected > 0)
    }

    async fn by_tickets(&mut self, ticket_ids: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
        let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for chunk in SqlChunks::of(ticket_ids) {
            let (list, chunk_params) = SqlChunks::params("t", chunk);
            let params: Params = chunk_params
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect();
            let sql = format!(
                "SELECT TicketId, SessionId FROM TicketSessions WHERE TicketId IN ({list}) ORDER BY AddedAt, SessionId"
            );
            let rows = self.sql.query_rows(&sql, &params, |row| {
                Ok((
                    row.get::<_, String>("TicketId")?,
                    row.get::<_, String>("SessionId")?,
                ))
            })?;
            for (ticket_id, session_id) in rows {
                result.entry(ticket_id).or_default().push(session_id);
            }
        }
        Ok(result)
    }

    /// Sortiert in Rust, damit die Reihenfolge nicht von der Collation abhängt
    async fn tickets_of(&mut self, session_id: &str) -> Result<Vec<String>> {
        let mut ids = self.sql.query_rows(
            "SELECT TicketId FROM TicketSessions WHERE SessionId = @sessionId",
            &sqlite_params! { "sessionId": session_id },
            |row| row.get::<_, String>("TicketId"),
        )?;
        ids.sort();
        Ok(ids)
    }
}
