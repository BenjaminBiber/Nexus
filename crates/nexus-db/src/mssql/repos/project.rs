//! Projekte auf SQL Server; Ticket- und Fragenummern werden atomar per `UPDATE … OUTPUT`
//! vergeben. Portiert aus `packages/db/src/MssqlProjectRepository.ts`.

use async_trait::async_trait;
use nexus_core::board::database::{NewProject, ProjectRepository};
use nexus_core::board::types::Project;
use nexus_core::{Error, HarnessError, Result};

use crate::mssql::error_map::MssqlErrorMap;
use crate::mssql::runner::{MsRow, MsValue};
use crate::mssql::unit_of_work::MssqlUnitOfWork;

/// Die Spalten eines Projekts in der Reihenfolge der TypeScript-Fassung
const COLUMNS: &str = "Id, [Key], Name, NextTicketNumber, NextQuestionNumber, CreatedAt, CreatedBy";

#[async_trait]
impl ProjectRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self) -> Result<Vec<Project>> {
        let rows = self
            .sql
            .query_rows(
                &format!("SELECT {COLUMNS} FROM board.Projects ORDER BY [Key]"),
                &[],
            )
            .await?;
        rows.iter().map(map_project).collect()
    }

    async fn by_id(&mut self, id: i32) -> Result<Option<Project>> {
        let rows = self
            .sql
            .query_rows(
                &format!("SELECT {COLUMNS} FROM board.Projects WHERE Id = @id"),
                &[("id", MsValue::Int(id))],
            )
            .await?;
        rows.first().map(map_project).transpose()
    }

    async fn by_key(&mut self, key: &str) -> Result<Option<Project>> {
        let rows = self
            .sql
            .query_rows(
                &format!("SELECT {COLUMNS} FROM board.Projects WHERE [Key] = @key"),
                &[("key", MsValue::text(key))],
            )
            .await?;
        rows.first().map(map_project).transpose()
    }

    /// Ein doppelter Schlüssel (SQL-Fehler 2627/2601) wird zu `project-exists`
    async fn insert(&mut self, project: &NewProject) -> Result<Project> {
        let rows = self
            .sql
            .query_rows(
                "INSERT INTO board.Projects ([Key], Name, CreatedAt, CreatedBy) OUTPUT inserted.Id VALUES (@key, @name, @createdAt, @createdBy)",
                &[
                    ("key", MsValue::text(project.key.as_str())),
                    ("name", MsValue::text(project.name.as_str())),
                    ("createdAt", MsValue::date(&project.created_at)?),
                    ("createdBy", MsValue::text(project.created_by.as_str())),
                ],
            )
            .await
            .map_err(|error| {
                if MssqlErrorMap::is_unique_violation(&error) {
                    HarnessError::new(
                        "project-exists",
                        format!("Projekt {} gibt es schon.", project.key),
                    )
                    .into()
                } else {
                    error
                }
            })?;
        let id = rows.first().ok_or_else(|| fehlende_zeile("Projects"))?;
        Ok(Project {
            id: id.get_i32("Id")?,
            key: project.key.clone(),
            name: project.name.clone(),
            next_ticket_number: 1,
            next_question_number: 1,
            created_at: project.created_at.clone(),
            created_by: project.created_by.clone(),
        })
    }

    async fn rename(&mut self, id: i32, name: &str) -> Result<()> {
        self.sql
            .execute(
                "UPDATE board.Projects SET Name = @name WHERE Id = @id",
                &[("id", MsValue::Int(id)), ("name", MsValue::text(name))],
            )
            .await?;
        Ok(())
    }

    /// Liefert die bisherige Ticketnummer und zählt in derselben Anweisung weiter
    async fn take_ticket_number(&mut self, id: i32) -> Result<i32> {
        self.take(
            "UPDATE board.Projects SET NextTicketNumber = NextTicketNumber + 1 OUTPUT deleted.NextTicketNumber AS N WHERE Id = @id",
            id,
        )
        .await
    }

    /// Liefert die bisherige Fragenummer und zählt in derselben Anweisung weiter
    async fn take_question_number(&mut self, id: i32) -> Result<i32> {
        self.take(
            "UPDATE board.Projects SET NextQuestionNumber = NextQuestionNumber + 1 OUTPUT deleted.NextQuestionNumber AS N WHERE Id = @id",
            id,
        )
        .await
    }
}

impl MssqlUnitOfWork<'_> {
    /// Der gemeinsame Teil der beiden Nummernvergaben
    async fn take(&mut self, sql: &str, id: i32) -> Result<i32> {
        let rows = self
            .sql
            .query_rows(sql, &[("id", MsValue::Int(id))])
            .await?;
        rows.first()
            .ok_or_else(|| fehlende_zeile("Projects"))?
            .get_i32("N")
    }
}

/// Fehler, wenn eine Anweisung ihre Zeile nicht liefert (gelöschtes Projekt)
fn fehlende_zeile(table: &str) -> Error {
    Error::unexpected_message(format!("Die Tabelle board.{table} liefert keine Zeile."))
}

/// Eine Zeile aus `board.Projects` als Fachobjekt
fn map_project(row: &MsRow) -> Result<Project> {
    Ok(Project {
        id: row.get_i32("Id")?,
        key: row.get_str("Key")?,
        name: row.get_str("Name")?,
        next_ticket_number: row.get_i32("NextTicketNumber")?,
        next_question_number: row.get_i32("NextQuestionNumber")?,
        created_at: row.get_iso("CreatedAt")?,
        created_by: row.get_guid("CreatedBy")?,
    })
}
