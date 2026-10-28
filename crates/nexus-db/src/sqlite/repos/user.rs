//! Benutzer in der Tabelle `Users`; portiert aus
//! `packages/db/src/sqlite/SqliteUserRepository.ts`.

use async_trait::async_trait;

use nexus_core::board::database::UserRepository;
use nexus_core::board::types::BoardUser;
use nexus_core::Result;

use crate::sqlite::runner::guid;
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;

#[async_trait]
impl UserRepository for SqliteUnitOfWork<'_> {
    /// Legt an oder aktualisiert den Namen
    async fn upsert(&mut self, user: &BoardUser) -> Result<()> {
        self.sql.execute(
            "INSERT INTO Users (Id, Name) VALUES (@id, @name) ON CONFLICT(Id) DO UPDATE SET Name = excluded.Name",
            &sqlite_params! { "id": user.id.to_lowercase(), "name": user.name.clone() },
        )?;
        Ok(())
    }

    async fn by_id(&mut self, id: &str) -> Result<Option<BoardUser>> {
        let rows = self.sql.query_rows(
            "SELECT Id, Name FROM Users WHERE Id = @id",
            &sqlite_params! { "id": id.to_lowercase() },
            map_user,
        )?;
        Ok(rows.into_iter().next())
    }

    async fn list(&mut self) -> Result<Vec<BoardUser>> {
        self.sql.query_rows(
            "SELECT Id, Name FROM Users ORDER BY Name COLLATE NOCASE, Id",
            &sqlite_params! {},
            map_user,
        )
    }
}

/// Eine Zeile aus `Users` als Fachobjekt; die GUID gilt kleingeschrieben
fn map_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<BoardUser> {
    Ok(BoardUser {
        id: guid(&row.get::<_, String>("Id")?),
        name: row.get("Name")?,
    })
}
