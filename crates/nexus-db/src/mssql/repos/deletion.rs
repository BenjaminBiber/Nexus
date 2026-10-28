//! Papierkorb gelöschter Tickets auf SQL Server; portiert aus
//! `packages/db/src/MssqlDeletionRepository.ts`. Gerüst – gefüllt von P2-13.

use async_trait::async_trait;
use nexus_core::board::database::DeletionRepository;
use nexus_core::board::types::Deletion;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl DeletionRepository for MssqlUnitOfWork<'_> {
    async fn insert(&mut self, _deletion: &Deletion) -> Result<()> {
        Err(not_implemented("deletions.insert", "P2-13"))
    }

    async fn list(&mut self) -> Result<Vec<Deletion>> {
        Err(not_implemented("deletions.list", "P2-13"))
    }
}
