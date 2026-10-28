//! Boards in der Tabelle `Boards`. Löschen entfernt die Notizen per Kaskade; Tickets löscht
//! vorher der Dienst. Portiert aus `packages/db/src/sqlite/SqliteBoardRepository.ts`.

use async_trait::async_trait;

use nexus_core::board::database::{BoardFilter, BoardRepository, NewBoard};
use nexus_core::board::types::Board;
use nexus_core::Result;

use crate::sqlite::runner::{date, guid, iso, missing_row, Params};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;

/// Die Spalten eines Boards in der Reihenfolge der TypeScript-Fassung
const COLUMNS: &str = "Id, ProjectId, OwnerId, Name, RepoPath, CreatedAt";

#[async_trait]
impl BoardRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, filter: &BoardFilter) -> Result<Vec<Board>> {
        let mut where_parts: Vec<&str> = Vec::new();
        let mut params: Params = Vec::new();
        if let Some(project_id) = filter.project_id {
            where_parts.push("ProjectId = @projectId");
            params.push(("projectId".into(), project_id.into()));
        }
        if let Some(owner_id) = &filter.owner_id {
            where_parts.push("OwnerId = @ownerId");
            params.push(("ownerId".into(), owner_id.to_lowercase().into()));
        }
        let clause = if where_parts.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_parts.join(" AND "))
        };
        let sql = format!("SELECT {COLUMNS} FROM Boards {clause} ORDER BY Id");
        self.sql.query_rows(&sql, &params, map_board)
    }

    async fn by_id(&mut self, id: i32) -> Result<Option<Board>> {
        let sql = format!("SELECT {COLUMNS} FROM Boards WHERE Id = @id");
        let rows = self
            .sql
            .query_rows(&sql, &sqlite_params! { "id": id }, map_board)?;
        Ok(rows.into_iter().next())
    }

    async fn insert(&mut self, board: &NewBoard) -> Result<Board> {
        let rows = self.sql.query_rows(
            "INSERT INTO Boards (ProjectId, OwnerId, Name, RepoPath, CreatedAt) VALUES (@projectId, @ownerId, @name, @repoPath, @createdAt) RETURNING Id",
            &sqlite_params! {
                "projectId": board.project_id,
                "ownerId": board.owner_id.to_lowercase(),
                "name": board.name.clone(),
                "repoPath": board.repo_path.clone(),
                "createdAt": date(&board.created_at),
            },
            |row| row.get::<_, i64>(0),
        )?;
        let id = rows.first().copied().ok_or_else(|| missing_row("Boards"))?;
        Ok(Board {
            id: i32::try_from(id).unwrap_or_default(),
            project_id: board.project_id,
            owner_id: board.owner_id.clone(),
            name: board.name.clone(),
            repo_path: board.repo_path.clone(),
            created_at: board.created_at.clone(),
        })
    }

    async fn update(&mut self, board: &Board) -> Result<()> {
        self.sql.execute(
            "UPDATE Boards SET Name = @name, RepoPath = @repoPath WHERE Id = @id",
            &sqlite_params! {
                "id": board.id,
                "name": board.name.clone(),
                "repoPath": board.repo_path.clone(),
            },
        )?;
        Ok(())
    }

    async fn delete(&mut self, id: i32) -> Result<()> {
        self.sql.execute(
            "DELETE FROM Boards WHERE Id = @id",
            &sqlite_params! { "id": id },
        )?;
        Ok(())
    }
}

/// Eine Zeile aus `Boards` als Fachobjekt
fn map_board(row: &rusqlite::Row<'_>) -> rusqlite::Result<Board> {
    Ok(Board {
        id: row.get("Id")?,
        project_id: row.get("ProjectId")?,
        owner_id: guid(&row.get::<_, String>("OwnerId")?),
        name: row.get("Name")?,
        repo_path: row.get("RepoPath")?,
        created_at: iso(row.get("CreatedAt")?).unwrap_or_default(),
    })
}
