//! Verlauf eines Tickets auf SQL Server; portiert aus
//! `packages/db/src/MssqlHistoryRepository.ts`. Gerüst – gefüllt von P2-13.

use async_trait::async_trait;
use nexus_core::board::database::HistoryRepository;
use nexus_core::board::types::HistoryEntry;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl HistoryRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, _ticket_id: &str) -> Result<Vec<HistoryEntry>> {
        Err(not_implemented("history.list", "P2-13"))
    }

    async fn insert(&mut self, _entries: &[HistoryEntry]) -> Result<()> {
        Err(not_implemented("history.insert", "P2-13"))
    }
}
