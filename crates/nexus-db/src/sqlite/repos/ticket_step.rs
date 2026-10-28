//! Arbeitsschritte je Ticket in `TicketSteps` (Position ab 0); portiert aus
//! `packages/db/src/sqlite/SqliteTicketStepRepository.ts`.

use std::collections::BTreeMap;

use async_trait::async_trait;

use nexus_core::board::database::TicketStepRepository;
use nexus_core::board::types::{StepProgress, TicketStep};
use nexus_core::Result;

use crate::sqlite::runner::{bool_of, Params};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;
use crate::SqlChunks;

#[async_trait]
impl TicketStepRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<TicketStep>> {
        self.sql.query_rows(
            "SELECT Text, Done, Detail FROM TicketSteps WHERE TicketId = @ticketId ORDER BY Position",
            &sqlite_params! { "ticketId": ticket_id },
            |row| {
                Ok(TicketStep {
                    text: row.get("Text")?,
                    done: bool_of(row.get::<_, i64>("Done")?),
                    detail: row.get("Detail")?,
                })
            },
        )
    }

    async fn replace(&mut self, ticket_id: &str, steps: &[TicketStep]) -> Result<()> {
        self.sql.execute(
            "DELETE FROM TicketSteps WHERE TicketId = @ticketId",
            &sqlite_params! { "ticketId": ticket_id },
        )?;
        for (position, step) in steps.iter().enumerate() {
            self.sql.execute(
                "INSERT INTO TicketSteps (TicketId, Position, Text, Done, Detail) VALUES (@ticketId, @position, @text, @done, @detail)",
                &sqlite_params! {
                    "ticketId": ticket_id,
                    "position": i64::try_from(position).unwrap_or_default(),
                    "text": step.text.clone(),
                    "done": step.done,
                    "detail": step.detail.clone(),
                },
            )?;
        }
        Ok(())
    }

    async fn set_done(&mut self, ticket_id: &str, position: i32, done: bool) -> Result<bool> {
        let affected = self.sql.execute(
            "UPDATE TicketSteps SET Done = @done WHERE TicketId = @ticketId AND Position = @position",
            &sqlite_params! { "ticketId": ticket_id, "position": position, "done": done },
        )?;
        Ok(affected > 0)
    }

    async fn progress(&mut self, ticket_ids: &[String]) -> Result<BTreeMap<String, StepProgress>> {
        let mut result = BTreeMap::new();
        for chunk in SqlChunks::of(ticket_ids) {
            let (list, chunk_params) = SqlChunks::params("t", chunk);
            let params: Params = chunk_params
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect();
            let sql = format!(
                "SELECT TicketId, SUM(CASE WHEN Done = 1 THEN 1 ELSE 0 END) AS Done, COUNT(*) AS Total FROM TicketSteps WHERE TicketId IN ({list}) GROUP BY TicketId"
            );
            let rows = self.sql.query_rows(&sql, &params, |row| {
                Ok((
                    row.get::<_, String>("TicketId")?,
                    StepProgress {
                        done: row.get("Done")?,
                        total: row.get("Total")?,
                    },
                ))
            })?;
            result.extend(rows);
        }
        Ok(result)
    }
}
