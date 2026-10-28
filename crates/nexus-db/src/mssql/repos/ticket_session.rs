//! Verknüpfung von Claude-Sessions mit Tickets auf SQL Server in `board.TicketSessions`;
//! portiert aus `packages/db/src/MssqlTicketSessionRepository.ts`.

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::database::TicketSessionRepository;
use nexus_core::Result;

use crate::mssql::runner::MsValue;
use crate::mssql::unit_of_work::MssqlUnitOfWork;
use crate::SqlChunks;

#[async_trait]
impl TicketSessionRepository for MssqlUnitOfWork<'_> {
    /// Legt die Verknüpfung nur an, wenn es sie noch nicht gibt (Sperre gegen gleichzeitiges
    /// Anlegen)
    async fn add(&mut self, ticket_id: &str, session_id: &str, at: &str) -> Result<bool> {
        let affected = self
            .sql
            .execute(
                "INSERT INTO board.TicketSessions (TicketId, SessionId, AddedAt)
       SELECT @ticketId, @sessionId, @at
       WHERE NOT EXISTS (SELECT 1 FROM board.TicketSessions WITH (UPDLOCK, HOLDLOCK) WHERE TicketId = @ticketId AND SessionId = @sessionId)",
                &[
                    ("ticketId", MsValue::text(ticket_id)),
                    ("sessionId", MsValue::text(session_id)),
                    ("at", MsValue::date(at)?),
                ],
            )
            .await?;
        Ok(affected > 0)
    }

    async fn by_tickets(&mut self, ticket_ids: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
        let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for chunk in SqlChunks::of(ticket_ids) {
            let (list, chunk_params) = SqlChunks::params("t", chunk);
            let params: Vec<(String, MsValue)> = chunk_params
                .into_iter()
                .map(|(name, value)| (name, MsValue::text(value)))
                .collect();
            let borrowed: Vec<(&str, MsValue)> = params
                .iter()
                .map(|(name, value)| (name.as_str(), value.clone()))
                .collect();
            let rows = self
                .sql
                .query_rows(
                    &format!(
                        "SELECT TicketId, SessionId FROM board.TicketSessions WHERE TicketId IN ({list}) ORDER BY AddedAt, SessionId"
                    ),
                    &borrowed,
                )
                .await?;
            for row in &rows {
                result
                    .entry(row.get_str("TicketId")?)
                    .or_default()
                    .push(row.get_str("SessionId")?);
            }
        }
        Ok(result)
    }

    /// Sortiert in Rust, damit die Reihenfolge nicht von der DB-Collation abhängt
    async fn tickets_of(&mut self, session_id: &str) -> Result<Vec<String>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT TicketId FROM board.TicketSessions WHERE SessionId = @sessionId",
                &[("sessionId", MsValue::text(session_id))],
            )
            .await?;
        let mut ids = rows
            .iter()
            .map(|row| row.get_str("TicketId"))
            .collect::<Result<Vec<_>>>()?;
        ids.sort();
        Ok(ids)
    }
}
