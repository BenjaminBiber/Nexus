//! Verknüpfung von Claude-Sessions mit Tickets auf SQL Server; portiert aus
//! `packages/db/src/MssqlTicketSessionRepository.ts`. Gerüst – gefüllt von P2-12.

use std::collections::BTreeMap;

use async_trait::async_trait;
use nexus_core::board::database::TicketSessionRepository;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl TicketSessionRepository for MssqlUnitOfWork<'_> {
    async fn add(&mut self, _ticket_id: &str, _session_id: &str, _at: &str) -> Result<bool> {
        Err(not_implemented("ticket_sessions.add", "P2-12"))
    }

    async fn by_tickets(
        &mut self,
        _ticket_ids: &[String],
    ) -> Result<BTreeMap<String, Vec<String>>> {
        Err(not_implemented("ticket_sessions.by_tickets", "P2-12"))
    }

    async fn tickets_of(&mut self, _session_id: &str) -> Result<Vec<String>> {
        Err(not_implemented("ticket_sessions.tickets_of", "P2-12"))
    }
}
