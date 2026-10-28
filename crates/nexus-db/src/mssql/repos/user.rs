//! Benutzer des Boards auf SQL Server; portiert aus `packages/db/src/MssqlUserRepository.ts`.
//! Gerüst – gefüllt von P2-12.

use async_trait::async_trait;
use nexus_core::board::database::UserRepository;
use nexus_core::board::types::BoardUser;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl UserRepository for MssqlUnitOfWork<'_> {
    async fn upsert(&mut self, _user: &BoardUser) -> Result<()> {
        Err(not_implemented("users.upsert", "P2-12"))
    }

    async fn by_id(&mut self, _id: &str) -> Result<Option<BoardUser>> {
        Err(not_implemented("users.by_id", "P2-12"))
    }

    async fn list(&mut self) -> Result<Vec<BoardUser>> {
        Err(not_implemented("users.list", "P2-12"))
    }
}
