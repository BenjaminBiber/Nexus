//! Verlauf eines Tickets; Gerüst, gefüllt von P2-7 (Port von
//! `packages/db/src/sqlite/SqliteHistoryRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::HistoryRepository;
use nexus_core::board::types::HistoryEntry;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl HistoryRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, _ticket_id: &str) -> Result<Vec<HistoryEntry>> {
        Err(folgt("HistoryRepository.list"))
    }

    async fn insert(&mut self, _entries: &[HistoryEntry]) -> Result<()> {
        Err(folgt("HistoryRepository.insert"))
    }
}
