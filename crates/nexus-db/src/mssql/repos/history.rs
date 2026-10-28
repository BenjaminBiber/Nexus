//! Verlauf eines Tickets auf SQL Server; portiert aus
//! `packages/db/src/MssqlHistoryRepository.ts`.
//!
//! Die Spalten `FromValue`/`ToValue` heißen im DTO `from`/`to`.

use async_trait::async_trait;
use nexus_core::board::database::HistoryRepository;
use nexus_core::board::types::HistoryEntry;
use nexus_core::Result;

use crate::mssql::runner::MsValue;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl HistoryRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<HistoryEntry>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT TicketId, At, ById, ByName, Field, FromValue, ToValue, Note FROM board.History WHERE TicketId = @ticketId ORDER BY Id",
                &[("ticketId", MsValue::text(ticket_id))],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(HistoryEntry {
                    ticket_id: row.get_str("TicketId")?,
                    at: row.get_iso("At")?,
                    by_id: row.get_guid("ById")?,
                    by_name: row.get_str("ByName")?,
                    field: row.get_str("Field")?,
                    from: row.get_opt_str("FromValue")?,
                    to: row.get_opt_str("ToValue")?,
                    note: row.get_opt_str("Note")?,
                })
            })
            .collect()
    }

    async fn insert(&mut self, entries: &[HistoryEntry]) -> Result<()> {
        for entry in entries {
            self.sql
                .execute(
                    "INSERT INTO board.History (TicketId, At, ById, ByName, Field, FromValue, ToValue, Note) VALUES (@ticketId, @at, @byId, @byName, @field, @from, @to, @note)",
                    &[
                        ("ticketId", MsValue::text(&entry.ticket_id)),
                        ("at", MsValue::date(&entry.at)?),
                        ("byId", MsValue::text(&entry.by_id)),
                        ("byName", MsValue::text(&entry.by_name)),
                        ("field", MsValue::text(&entry.field)),
                        ("from", MsValue::text_opt(entry.from.as_deref())),
                        ("to", MsValue::text_opt(entry.to.as_deref())),
                        ("note", MsValue::text_opt(entry.note.as_deref())),
                    ],
                )
                .await?;
        }
        Ok(())
    }
}
