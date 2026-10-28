//! Papierkorb gelöschter Tickets; Gerüst, gefüllt von P2-7 (Port von
//! `packages/db/src/sqlite/SqliteDeletionRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::DeletionRepository;
use nexus_core::board::types::Deletion;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl DeletionRepository for SqliteUnitOfWork<'_> {
    async fn insert(&mut self, _deletion: &Deletion) -> Result<()> {
        Err(folgt("DeletionRepository.insert"))
    }

    async fn list(&mut self) -> Result<Vec<Deletion>> {
        Err(folgt("DeletionRepository.list"))
    }
}
