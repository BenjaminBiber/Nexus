//! Verknüpfung von Claude-Sessions mit Tickets; Gerüst, gefüllt von P2-6 (Port von
//! `packages/db/src/sqlite/SqliteTicketSessionRepository.ts`).

use std::collections::BTreeMap;

use async_trait::async_trait;

use nexus_core::board::database::TicketSessionRepository;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl TicketSessionRepository for SqliteUnitOfWork<'_> {
    async fn add(&mut self, _ticket_id: &str, _session_id: &str, _at: &str) -> Result<bool> {
        Err(folgt("TicketSessionRepository.add"))
    }

    async fn by_tickets(
        &mut self,
        _ticket_ids: &[String],
    ) -> Result<BTreeMap<String, Vec<String>>> {
        Err(folgt("TicketSessionRepository.by_tickets"))
    }

    async fn tickets_of(&mut self, _session_id: &str) -> Result<Vec<String>> {
        Err(folgt("TicketSessionRepository.tickets_of"))
    }
}
