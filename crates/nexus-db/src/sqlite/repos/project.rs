//! Projekte in der Tabelle `Projects`; Ticket- und Fragenummern werden atomar per
//! `UPDATE … RETURNING` vergeben. Portiert aus
//! `packages/db/src/sqlite/SqliteProjectRepository.ts`.

use async_trait::async_trait;

use nexus_core::board::database::{NewProject, ProjectRepository};
use nexus_core::board::types::Project;
use nexus_core::{HarnessError, Result};

use crate::sqlite::error_map;
use crate::sqlite::runner::{date, guid, iso, missing_row};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;

/// Die Spalten eines Projekts in der Reihenfolge der TypeScript-Fassung
const COLUMNS: &str =
    "Id, \"Key\", Name, NextTicketNumber, NextQuestionNumber, CreatedAt, CreatedBy";

#[async_trait]
impl ProjectRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self) -> Result<Vec<Project>> {
        let sql = format!("SELECT {COLUMNS} FROM Projects ORDER BY \"Key\"");
        self.sql.query_rows(&sql, &sqlite_params! {}, map_project)
    }

    async fn by_id(&mut self, id: i32) -> Result<Option<Project>> {
        let sql = format!("SELECT {COLUMNS} FROM Projects WHERE Id = @id");
        let rows = self
            .sql
            .query_rows(&sql, &sqlite_params! { "id": id }, map_project)?;
        Ok(rows.into_iter().next())
    }

    async fn by_key(&mut self, key: &str) -> Result<Option<Project>> {
        let sql = format!("SELECT {COLUMNS} FROM Projects WHERE \"Key\" = @key");
        let rows = self
            .sql
            .query_rows(&sql, &sqlite_params! { "key": key }, map_project)?;
        Ok(rows.into_iter().next())
    }

    /// Ein doppelter Schlüssel (Eindeutigkeitsfehler von SQLite) wird zu `project-exists`
    async fn insert(&mut self, project: &NewProject) -> Result<Project> {
        let rows = self
            .sql
            .query_rows(
                "INSERT INTO Projects (\"Key\", Name, CreatedAt, CreatedBy) VALUES (@key, @name, @createdAt, @createdBy) RETURNING Id",
                &sqlite_params! {
                    "key": project.key.clone(),
                    "name": project.name.clone(),
                    "createdAt": date(&project.created_at),
                    "createdBy": project.created_by.to_lowercase(),
                },
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| {
                if error_map::is_unique_violation(&error) {
                    HarnessError::new(
                        "project-exists",
                        format!("Projekt {} gibt es schon.", project.key),
                    )
                    .into()
                } else {
                    error
                }
            })?;
        let id = rows
            .first()
            .copied()
            .ok_or_else(|| missing_row("Projects"))?;
        Ok(Project {
            id: i32::try_from(id).unwrap_or_default(),
            key: project.key.clone(),
            name: project.name.clone(),
            next_ticket_number: 1,
            next_question_number: 1,
            created_at: project.created_at.clone(),
            created_by: project.created_by.clone(),
        })
    }

    async fn rename(&mut self, id: i32, name: &str) -> Result<()> {
        self.sql.execute(
            "UPDATE Projects SET Name = @name WHERE Id = @id",
            &sqlite_params! { "id": id, "name": name },
        )?;
        Ok(())
    }

    /// Liefert die bisherige Ticketnummer und zählt in derselben Anweisung weiter
    async fn take_ticket_number(&mut self, id: i32) -> Result<i32> {
        self.take(
            "UPDATE Projects SET NextTicketNumber = NextTicketNumber + 1 WHERE Id = @id RETURNING NextTicketNumber - 1 AS N",
            id,
        )
    }

    /// Liefert die bisherige Fragenummer und zählt in derselben Anweisung weiter
    async fn take_question_number(&mut self, id: i32) -> Result<i32> {
        self.take(
            "UPDATE Projects SET NextQuestionNumber = NextQuestionNumber + 1 WHERE Id = @id RETURNING NextQuestionNumber - 1 AS N",
            id,
        )
    }
}

impl SqliteUnitOfWork<'_> {
    /// Der gemeinsame Teil der beiden Nummernvergaben
    fn take(&mut self, sql: &str, id: i32) -> Result<i32> {
        let rows = self
            .sql
            .query_rows(sql, &sqlite_params! { "id": id }, |row| {
                row.get::<_, i64>(0)
            })?;
        let number = rows
            .first()
            .copied()
            .ok_or_else(|| missing_row("Projects"))?;
        Ok(i32::try_from(number).unwrap_or_default())
    }
}

/// Eine Zeile aus `Projects` als Fachobjekt
fn map_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("Id")?,
        key: row.get("Key")?,
        name: row.get("Name")?,
        next_ticket_number: row.get("NextTicketNumber")?,
        next_question_number: row.get("NextQuestionNumber")?,
        created_at: iso(row.get("CreatedAt")?).unwrap_or_default(),
        created_by: guid(&row.get::<_, String>("CreatedBy")?),
    })
}
