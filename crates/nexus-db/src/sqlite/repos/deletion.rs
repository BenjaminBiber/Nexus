//! Papierkorb gelöschter Tickets; portiert aus
//! `packages/db/src/sqlite/SqliteDeletionRepository.ts`.
//!
//! Ein unbekannter Wert in `FromStatus` ist hier ein Fehler: TypeScript castet die Spalte blind,
//! in Rust scheitert das Lesen lieber laut.

use std::str::FromStr;

use async_trait::async_trait;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::database::DeletionRepository;
use nexus_core::board::types::Deletion;
use nexus_core::{Error, Result};

use crate::sqlite::runner::{date, guid, iso_millis};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite::Params;
use crate::sqlite_params;

#[async_trait]
impl DeletionRepository for SqliteUnitOfWork<'_> {
    async fn insert(&mut self, deletion: &Deletion) -> Result<()> {
        let params: Params = sqlite_params! {
            "at": date(&deletion.at),
            "byId": deletion.by_id.to_lowercase(),
            "byName": deletion.by_name.clone(),
            "ticketId": deletion.ticket_id.clone(),
            "ticketTitle": deletion.ticket_title.clone(),
            "fromStatus": deletion.from_status.map(|status| status.as_str()),
        };
        self.sql.execute(
            "INSERT INTO Deletions (At, ById, ByName, TicketId, TicketTitle, FromStatus) VALUES (@at, @byId, @byName, @ticketId, @ticketTitle, @fromStatus)",
            &params,
        )?;
        Ok(())
    }

    async fn list(&mut self) -> Result<Vec<Deletion>> {
        let zeilen = self.sql.query_rows(
            "SELECT At, ById, ByName, TicketId, TicketTitle, FromStatus FROM Deletions ORDER BY Id",
            &Vec::new(),
            |row| {
                Ok((
                    Deletion {
                        at: iso_millis(&row.get::<_, String>(0)?),
                        by_id: guid(&row.get::<_, String>(1)?),
                        by_name: row.get(2)?,
                        ticket_id: row.get(3)?,
                        ticket_title: row.get(4)?,
                        from_status: None,
                    },
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )?;
        zeilen
            .into_iter()
            .map(|(deletion, status)| match status {
                None => Ok(deletion),
                Some(text) => TicketStatus::from_str(&text)
                    .map_err(|_| {
                        Error::unexpected_message(format!(
                            "Löschprotokoll {}: Status „{text}“ ist unbekannt.",
                            deletion.ticket_id
                        ))
                    })
                    .map(|status| Deletion {
                        from_status: Some(status),
                        ..deletion
                    }),
            })
            .collect()
    }
}
