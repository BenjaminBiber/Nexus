//! Boards auf SQL Server; portiert aus `packages/db/src/MssqlBoardRepository.ts`.
//! Gerüst – gefüllt von P2-12.

use async_trait::async_trait;
use nexus_core::board::database::{BoardFilter, BoardRepository, NewBoard};
use nexus_core::board::types::Board;
use nexus_core::Result;

use crate::mssql::repos::not_implemented;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl BoardRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, _filter: &BoardFilter) -> Result<Vec<Board>> {
        Err(not_implemented("boards.list", "P2-12"))
    }

    async fn by_id(&mut self, _id: i32) -> Result<Option<Board>> {
        Err(not_implemented("boards.by_id", "P2-12"))
    }

    async fn insert(&mut self, _board: &NewBoard) -> Result<Board> {
        Err(not_implemented("boards.insert", "P2-12"))
    }

    async fn update(&mut self, _board: &Board) -> Result<()> {
        Err(not_implemented("boards.update", "P2-12"))
    }

    async fn delete(&mut self, _id: i32) -> Result<()> {
        Err(not_implemented("boards.delete", "P2-12"))
    }
}
