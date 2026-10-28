//! Verlauf einer Frage in `QuestionHistory` (nur anhängen); portiert aus
//! `packages/db/src/sqlite/SqliteQuestionHistoryRepository.ts`.

use std::str::FromStr;

use async_trait::async_trait;

use nexus_core::actors::AuthorKind;
use nexus_core::board::database::BoardQuestionHistoryRepository;
use nexus_core::board::question_types::{BoardQuestionHistoryEntry, BoardQuestionHistoryType};
use nexus_core::{Error, Result};

use crate::sqlite::runner::{date, guid, iso_millis};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;

#[async_trait]
impl BoardQuestionHistoryRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionHistoryEntry>> {
        let rows = self.sql.query_rows(
            "SELECT QuestionId, At, ById, ByName, ByKind, \"Type\", \"Text\" FROM QuestionHistory WHERE QuestionId = @questionId ORDER BY Id",
            &sqlite_params! { "questionId": question_id },
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )?;
        rows.into_iter()
            .map(
                |(question_id, at, by_id, by_name, by_kind, entry_type, text)| {
                    Ok(BoardQuestionHistoryEntry {
                        question_id,
                        at: iso_millis(&at),
                        by_id: guid(&by_id),
                        by_name,
                        // ByKind ist NULL bei Einträgen ohne eigentlichen Autor (Ticket gelöscht)
                        by_kind: by_kind.as_deref().map(AuthorKind::from_str).transpose()?,
                        entry_type: history_type_of(&entry_type)?,
                        text,
                    })
                },
            )
            .collect()
    }

    async fn insert(&mut self, entries: &[BoardQuestionHistoryEntry]) -> Result<()> {
        for entry in entries {
            self.sql.execute(
                "INSERT INTO QuestionHistory (QuestionId, At, ById, ByName, ByKind, \"Type\", \"Text\") VALUES (@questionId, @at, @byId, @byName, @byKind, @type, @text)",
                &sqlite_params! {
                    "questionId": entry.question_id.clone(),
                    "at": date(&entry.at),
                    "byId": entry.by_id.to_lowercase(),
                    "byName": entry.by_name.clone(),
                    "byKind": entry.by_kind.map(|kind| kind.as_str()),
                    "type": entry.entry_type.as_str(),
                    "text": entry.text.clone(),
                },
            )?;
        }
        Ok(())
    }
}

/// Art eines Verlaufseintrags aus dem Spaltentext
fn history_type_of(text: &str) -> Result<BoardQuestionHistoryType> {
    BoardQuestionHistoryType::ALL
        .into_iter()
        .find(|entry_type| entry_type.as_str() == text)
        .ok_or_else(|| {
            Error::unexpected_message(format!(
                "Art „{text}“ eines Verlaufseintrags ist unbekannt."
            ))
        })
}
