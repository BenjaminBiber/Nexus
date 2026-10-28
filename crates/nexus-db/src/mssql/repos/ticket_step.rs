//! Arbeitsschritte eines Tickets auf SQL Server; portiert aus
//! `packages/db/src/MssqlTicketStepRepository.ts`. Gerüst – gefüllt von P2-12.

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::database::TicketStepRepository;
use nexus_core::board::types::{StepProgress, TicketStep};
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl TicketStepRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, _ticket_id: &str) -> Result<Vec<TicketStep>> {
        Err(not_implemented("steps.list", "P2-12"))
    }

    async fn replace(&mut self, _ticket_id: &str, _steps: &[TicketStep]) -> Result<()> {
        Err(not_implemented("steps.replace", "P2-12"))
    }

    async fn set_done(&mut self, _ticket_id: &str, _position: i32, _done: bool) -> Result<bool> {
        Err(not_implemented("steps.set_done", "P2-12"))
    }

    async fn progress(&mut self, _ticket_ids: &[String]) -> Result<BTreeMap<String, StepProgress>> {
        Err(not_implemented("steps.progress", "P2-12"))
    }
}
