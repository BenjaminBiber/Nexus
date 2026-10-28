//! Boards eines Projekts; Gerüst, gefüllt von P2-6 (Port von
//! `packages/db/src/sqlite/SqliteBoardRepository.ts`).

use async_trait::async_trait;

use nexus_core::board::database::{BoardFilter, BoardRepository, NewBoard};
use nexus_core::board::types::Board;
use nexus_core::Result;

use super::folgt;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;

#[async_trait]
impl BoardRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, _filter: &BoardFilter) -> Result<Vec<Board>> {
        Err(folgt("BoardRepository.list"))
    }

    async fn by_id(&mut self, _id: i32) -> Result<Option<Board>> {
        Err(folgt("BoardRepository.by_id"))
    }

    async fn insert(&mut self, _board: &NewBoard) -> Result<Board> {
        Err(folgt("BoardRepository.insert"))
    }

    async fn update(&mut self, _board: &Board) -> Result<()> {
        Err(folgt("BoardRepository.update"))
    }

    async fn delete(&mut self, _id: i32) -> Result<()> {
        Err(folgt("BoardRepository.delete"))
    }
}
