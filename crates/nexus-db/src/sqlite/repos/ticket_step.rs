//! Abhakbare Arbeitsschritte eines Tickets; Gerüst, gefüllt von P2-6 (Port von
//! `packages/db/src/sqlite/SqliteTicketStepRepository.ts`).

use std::collections::BTreeMap;

use async_trait::async_trait;

use nexus_core::board::database::TicketStepRepository;
use nexus_core::board::types::{StepProgress, TicketStep};
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl TicketStepRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, _ticket_id: &str) -> Result<Vec<TicketStep>> {
        Err(folgt("TicketStepRepository.list"))
    }

    async fn replace(&mut self, _ticket_id: &str, _steps: &[TicketStep]) -> Result<()> {
        Err(folgt("TicketStepRepository.replace"))
    }

    async fn set_done(&mut self, _ticket_id: &str, _position: i32, _done: bool) -> Result<bool> {
        Err(folgt("TicketStepRepository.set_done"))
    }

    async fn progress(&mut self, _ticket_ids: &[String]) -> Result<BTreeMap<String, StepProgress>> {
        Err(folgt("TicketStepRepository.progress"))
    }
}
