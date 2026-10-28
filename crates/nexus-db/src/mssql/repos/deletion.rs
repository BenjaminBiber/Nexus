//! Papierkorb gelöschter Tickets auf SQL Server; portiert aus
//! `packages/db/src/MssqlDeletionRepository.ts`.
//!
//! Ein unbekannter Wert in `FromStatus` ist hier ein Fehler: TypeScript castet die Spalte blind,
//! in Rust scheitert das Lesen lieber laut (wie in der SQLite-Fassung).

use std::str::FromStr;

use async_trait::async_trait;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::database::DeletionRepository;
use nexus_core::board::types::Deletion;
use nexus_core::{Error, Result};

use crate::mssql::runner::MsValue;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl DeletionRepository for MssqlUnitOfWork<'_> {
    async fn insert(&mut self, deletion: &Deletion) -> Result<()> {
        self.sql
            .execute(
                "INSERT INTO board.Deletions (At, ById, ByName, TicketId, TicketTitle, FromStatus) VALUES (@at, @byId, @byName, @ticketId, @ticketTitle, @fromStatus)",
                &[
                    ("at", MsValue::date(&deletion.at)?),
                    ("byId", MsValue::text(&deletion.by_id)),
                    ("byName", MsValue::text(&deletion.by_name)),
                    ("ticketId", MsValue::text(&deletion.ticket_id)),
                    ("ticketTitle", MsValue::text(&deletion.ticket_title)),
                    (
                        "fromStatus",
                        MsValue::text_opt(deletion.from_status.map(|status| status.as_str())),
                    ),
                ],
            )
            .await?;
        Ok(())
    }

    async fn list(&mut self) -> Result<Vec<Deletion>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT At, ById, ByName, TicketId, TicketTitle, FromStatus FROM board.Deletions ORDER BY Id",
                &[],
            )
            .await?;
        rows.iter()
            .map(|row| {
                let ticket_id = row.get_str("TicketId")?;
                let from_status = match row.get_opt_str("FromStatus")? {
                    None => None,
                    Some(text) => Some(TicketStatus::from_str(&text).map_err(|_| {
                        Error::unexpected_message(format!(
                            "Löschprotokoll {ticket_id}: Status „{text}“ ist unbekannt."
                        ))
                    })?),
                };
                Ok(Deletion {
                    at: row.get_iso("At")?,
                    by_id: row.get_guid("ById")?,
                    by_name: row.get_str("ByName")?,
                    ticket_id,
                    ticket_title: row.get_str("TicketTitle")?,
                    from_status,
                })
            })
            .collect()
    }
}
