//! Benutzer des Boards; Gerüst, gefüllt von P2-6 (Port von
//! `packages/db/src/sqlite/SqliteUserRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::UserRepository;
use nexus_core::board::types::BoardUser;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl UserRepository for SqliteUnitOfWork<'_> {
    async fn upsert(&mut self, _user: &BoardUser) -> Result<()> {
        Err(folgt("UserRepository.upsert"))
    }

    async fn by_id(&mut self, _id: &str) -> Result<Option<BoardUser>> {
        Err(folgt("UserRepository.by_id"))
    }

    async fn list(&mut self) -> Result<Vec<BoardUser>> {
        Err(folgt("UserRepository.list"))
    }
}
