//! Boards auf SQL Server. Löschen entfernt die Notizen per Kaskade; Tickets löscht vorher der
//! Dienst. Portiert aus `packages/db/src/MssqlBoardRepository.ts`.

use async_trait::async_trait;
use nexus_core::board::database::{BoardFilter, BoardRepository, NewBoard};
use nexus_core::board::types::Board;
use nexus_core::{Error, Result};

use crate::mssql::runner::{MsRow, MsValue};
use crate::mssql::unit_of_work::MssqlUnitOfWork;

/// Die Spalten eines Boards in der Reihenfolge der TypeScript-Fassung
const COLUMNS: &str = "Id, ProjectId, OwnerId, Name, RepoPath, CreatedAt";

#[async_trait]
impl BoardRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, filter: &BoardFilter) -> Result<Vec<Board>> {
        let mut where_parts: Vec<&str> = Vec::new();
        let mut params: Vec<(&str, MsValue)> = Vec::new();
        if let Some(project_id) = filter.project_id {
            where_parts.push("ProjectId = @projectId");
            params.push(("projectId", MsValue::Int(project_id)));
        }
        if let Some(owner_id) = &filter.owner_id {
            where_parts.push("OwnerId = @ownerId");
            params.push(("ownerId", MsValue::text(owner_id.as_str())));
        }
        let clause = if where_parts.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_parts.join(" AND "))
        };
        let rows = self
            .sql
            .query_rows(
                &format!("SELECT {COLUMNS} FROM board.Boards {clause} ORDER BY Id"),
                &params,
            )
            .await?;
        rows.iter().map(map_board).collect()
    }

    async fn by_id(&mut self, id: i32) -> Result<Option<Board>> {
        let rows = self
            .sql
            .query_rows(
                &format!("SELECT {COLUMNS} FROM board.Boards WHERE Id = @id"),
                &[("id", MsValue::Int(id))],
            )
            .await?;
        rows.first().map(map_board).transpose()
    }

    async fn insert(&mut self, board: &NewBoard) -> Result<Board> {
        let rows = self
            .sql
            .query_rows(
                "INSERT INTO board.Boards (ProjectId, OwnerId, Name, RepoPath, CreatedAt) OUTPUT inserted.Id VALUES (@projectId, @ownerId, @name, @repoPath, @createdAt)",
                &[
                    ("projectId", MsValue::Int(board.project_id)),
                    ("ownerId", MsValue::text(board.owner_id.as_str())),
                    ("name", MsValue::text(board.name.as_str())),
                    ("repoPath", MsValue::text_opt(board.repo_path.as_deref())),
                    ("createdAt", MsValue::date(&board.created_at)?),
                ],
            )
            .await?;
        let id = rows.first().ok_or_else(|| {
            Error::unexpected_message("Die Tabelle board.Boards liefert keine Zeile.")
        })?;
        Ok(Board {
            id: id.get_i32("Id")?,
            project_id: board.project_id,
            owner_id: board.owner_id.clone(),
            name: board.name.clone(),
            repo_path: board.repo_path.clone(),
            created_at: board.created_at.clone(),
        })
    }

    async fn update(&mut self, board: &Board) -> Result<()> {
        self.sql
            .execute(
                "UPDATE board.Boards SET Name = @name, RepoPath = @repoPath WHERE Id = @id",
                &[
                    ("id", MsValue::Int(board.id)),
                    ("name", MsValue::text(board.name.as_str())),
                    ("repoPath", MsValue::text_opt(board.repo_path.as_deref())),
                ],
            )
            .await?;
        Ok(())
    }

    async fn delete(&mut self, id: i32) -> Result<()> {
        self.sql
            .execute(
                "DELETE FROM board.Boards WHERE Id = @id",
                &[("id", MsValue::Int(id))],
            )
            .await?;
        Ok(())
    }
}

/// Eine Zeile aus `board.Boards` als Fachobjekt
fn map_board(row: &MsRow) -> Result<Board> {
    Ok(Board {
        id: row.get_i32("Id")?,
        project_id: row.get_i32("ProjectId")?,
        owner_id: row.get_guid("OwnerId")?,
        name: row.get_str("Name")?,
        repo_path: row.get_opt_str("RepoPath")?,
        created_at: row.get_iso("CreatedAt")?,
    })
}
