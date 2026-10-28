//! Benutzer des Boards auf SQL Server; portiert aus `packages/db/src/MssqlUserRepository.ts`.

use async_trait::async_trait;
use nexus_core::board::database::UserRepository;
use nexus_core::board::types::BoardUser;
use nexus_core::Result;

use crate::mssql::runner::{MsRow, MsValue};
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl UserRepository for MssqlUnitOfWork<'_> {
    /// Legt an oder aktualisiert den Namen; `UPDLOCK, SERIALIZABLE` sperrt den Schlüsselbereich
    /// gegen gleichzeitiges Erst-Anlegen
    async fn upsert(&mut self, user: &BoardUser) -> Result<()> {
        self.sql
            .execute(
                "UPDATE board.Users WITH (UPDLOCK, SERIALIZABLE) SET Name = @name WHERE Id = @id;
       IF @@ROWCOUNT = 0 INSERT INTO board.Users (Id, Name) VALUES (@id, @name);",
                &[
                    ("id", MsValue::text(user.id.as_str())),
                    ("name", MsValue::text(user.name.as_str())),
                ],
            )
            .await?;
        Ok(())
    }

    async fn by_id(&mut self, id: &str) -> Result<Option<BoardUser>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT Id, Name FROM board.Users WHERE Id = @id",
                &[("id", MsValue::text(id))],
            )
            .await?;
        rows.first().map(map_user).transpose()
    }

    async fn list(&mut self) -> Result<Vec<BoardUser>> {
        let rows = self
            .sql
            .query_rows("SELECT Id, Name FROM board.Users ORDER BY Name", &[])
            .await?;
        rows.iter().map(map_user).collect()
    }
}

/// Eine Zeile aus `board.Users` als Fachobjekt; die GUID gilt kleingeschrieben
fn map_user(row: &MsRow) -> Result<BoardUser> {
    Ok(BoardUser {
        id: row.get_guid("Id")?,
        name: row.get_str("Name")?,
    })
}
