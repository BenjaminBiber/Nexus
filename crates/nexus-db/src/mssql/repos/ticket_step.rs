//! Arbeitsschritte eines Tickets auf SQL Server in `board.TicketSteps` (Position ab 0);
//! portiert aus `packages/db/src/MssqlTicketStepRepository.ts`.

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::database::TicketStepRepository;
use nexus_core::board::types::{StepProgress, TicketStep};
use nexus_core::Result;

use crate::mssql::runner::MsValue;
use crate::mssql::unit_of_work::MssqlUnitOfWork;
use crate::SqlChunks;

#[async_trait]
impl TicketStepRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<TicketStep>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT Text, Done, Detail FROM board.TicketSteps WHERE TicketId = @ticketId ORDER BY Position",
                &[("ticketId", MsValue::text(ticket_id))],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(TicketStep {
                    text: row.get_str("Text")?,
                    done: row.get_bool("Done")?,
                    detail: row.get_opt_str("Detail")?,
                })
            })
            .collect()
    }

    async fn replace(&mut self, ticket_id: &str, steps: &[TicketStep]) -> Result<()> {
        self.sql
            .execute(
                "DELETE FROM board.TicketSteps WHERE TicketId = @ticketId",
                &[("ticketId", MsValue::text(ticket_id))],
            )
            .await?;
        for (position, step) in steps.iter().enumerate() {
            self.sql
                .execute(
                    "INSERT INTO board.TicketSteps (TicketId, Position, Text, Done, Detail) VALUES (@ticketId, @position, @text, @done, @detail)",
                    &[
                        ("ticketId", MsValue::text(ticket_id)),
                        ("position", MsValue::Int(i32::try_from(position).unwrap_or_default())),
                        ("text", MsValue::text(step.text.as_str())),
                        ("done", MsValue::Bit(step.done)),
                        ("detail", MsValue::text_opt(step.detail.as_deref())),
                    ],
                )
                .await?;
        }
        Ok(())
    }

    async fn set_done(&mut self, ticket_id: &str, position: i32, done: bool) -> Result<bool> {
        let affected = self
            .sql
            .execute(
                "UPDATE board.TicketSteps SET Done = @done WHERE TicketId = @ticketId AND Position = @position",
                &[
                    ("ticketId", MsValue::text(ticket_id)),
                    ("position", MsValue::Int(position)),
                    ("done", MsValue::Bit(done)),
                ],
            )
            .await?;
        Ok(affected > 0)
    }

    async fn progress(&mut self, ticket_ids: &[String]) -> Result<BTreeMap<String, StepProgress>> {
        let mut result = BTreeMap::new();
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
                        "SELECT TicketId, SUM(CASE WHEN Done = 1 THEN 1 ELSE 0 END) AS Done, COUNT(*) AS Total FROM board.TicketSteps WHERE TicketId IN ({list}) GROUP BY TicketId"
                    ),
                    &borrowed,
                )
                .await?;
            for row in &rows {
                result.insert(
                    row.get_str("TicketId")?,
                    StepProgress {
                        done: row.get_i32("Done")?,
                        total: row.get_i32("Total")?,
                    },
                );
            }
        }
        Ok(result)
    }
}
