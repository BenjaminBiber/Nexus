//! Verlauf eines Tickets; portiert aus `packages/db/src/sqlite/SqliteHistoryRepository.ts`.
//!
//! Die Spalten `FromValue`/`ToValue` heißen im DTO `from`/`to`.

use async_trait::async_trait;

use nexus_core::board::database::HistoryRepository;
use nexus_core::board::types::HistoryEntry;
use nexus_core::Result;

use crate::sqlite::runner::{date, guid, iso_millis};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite::Params;
use crate::sqlite_params;

#[async_trait]
impl HistoryRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<HistoryEntry>> {
        self.sql.query_rows(
            "SELECT TicketId, At, ById, ByName, Field, FromValue, ToValue, Note FROM History WHERE TicketId = @ticketId ORDER BY Id",
            &sqlite_params! { "ticketId": ticket_id },
            |row| {
                Ok(HistoryEntry {
                    ticket_id: row.get(0)?,
                    at: iso_millis(&row.get::<_, String>(1)?),
                    by_id: guid(&row.get::<_, String>(2)?),
                    by_name: row.get(3)?,
                    field: row.get(4)?,
                    from: row.get(5)?,
                    to: row.get(6)?,
                    note: row.get(7)?,
                })
            },
        )
    }

    async fn insert(&mut self, entries: &[HistoryEntry]) -> Result<()> {
        for entry in entries {
            let params: Params = sqlite_params! {
                "ticketId": entry.ticket_id.clone(),
                "at": date(&entry.at),
                "byId": entry.by_id.to_lowercase(),
                "byName": entry.by_name.clone(),
                "field": entry.field.clone(),
                "from": entry.from.clone(),
                "to": entry.to.clone(),
                "note": entry.note.clone(),
            };
            self.sql.execute(
                "INSERT INTO History (TicketId, At, ById, ByName, Field, FromValue, ToValue, Note) VALUES (@ticketId, @at, @byId, @byName, @field, @from, @to, @note)",
                &params,
            )?;
        }
        Ok(())
    }
}
