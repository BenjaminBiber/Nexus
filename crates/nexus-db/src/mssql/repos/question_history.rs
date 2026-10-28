//! Verlauf einer Frage auf SQL Server; portiert aus
//! `packages/db/src/MssqlQuestionHistoryRepository.ts`. Nur anhängen.
//!
//! `Type` und `Text` sind in SQL Server reservierte Wörter und stehen deshalb in Klammern.

use std::str::FromStr;

use async_trait::async_trait;
use nexus_core::actors::AuthorKind;
use nexus_core::board::database::BoardQuestionHistoryRepository;
use nexus_core::board::question_types::{BoardQuestionHistoryEntry, BoardQuestionHistoryType};
use nexus_core::{Error, Result};

use crate::mssql::runner::MsValue;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

/// Art eines Verlaufseintrags aus der Spalte `Type`
fn entry_type_from(text: &str, question_id: &str) -> Result<BoardQuestionHistoryType> {
    BoardQuestionHistoryType::ALL
        .into_iter()
        .find(|art| art.as_str() == text)
        .ok_or_else(|| {
            Error::unexpected_message(format!(
                "Verlauf der Frage {question_id}: Art „{text}“ ist unbekannt."
            ))
        })
}

#[async_trait]
impl BoardQuestionHistoryRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionHistoryEntry>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT QuestionId, At, ById, ByName, ByKind, [Type], [Text] FROM board.QuestionHistory WHERE QuestionId = @questionId ORDER BY Id",
                &[("questionId", MsValue::text(question_id))],
            )
            .await?;
        rows.iter()
            .map(|row| {
                let id = row.get_str("QuestionId")?;
                let art = row.get_str("Type")?;
                let by_kind = match row.get_opt_str("ByKind")? {
                    None => None,
                    Some(kind) => Some(AuthorKind::from_str(&kind).map_err(|_| {
                        Error::unexpected_message(format!(
                            "Verlauf der Frage {id}: Art des Autors „{kind}“ ist unbekannt."
                        ))
                    })?),
                };
                Ok(BoardQuestionHistoryEntry {
                    at: row.get_iso("At")?,
                    by_id: row.get_guid("ById")?,
                    by_name: row.get_str("ByName")?,
                    by_kind,
                    entry_type: entry_type_from(&art, &id)?,
                    text: row.get_str("Text")?,
                    question_id: id,
                })
            })
            .collect()
    }

    async fn insert(&mut self, entries: &[BoardQuestionHistoryEntry]) -> Result<()> {
        for entry in entries {
            self.sql
                .execute(
                    "INSERT INTO board.QuestionHistory (QuestionId, At, ById, ByName, ByKind, [Type], [Text]) VALUES (@questionId, @at, @byId, @byName, @byKind, @type, @text)",
                    &[
                        ("questionId", MsValue::text(&entry.question_id)),
                        ("at", MsValue::date(&entry.at)?),
                        ("byId", MsValue::text(&entry.by_id)),
                        ("byName", MsValue::text(&entry.by_name)),
                        (
                            "byKind",
                            MsValue::text_opt(entry.by_kind.map(|kind| kind.as_str())),
                        ),
                        ("type", MsValue::text(entry.entry_type.as_str())),
                        ("text", MsValue::text(&entry.text)),
                    ],
                )
                .await?;
        }
        Ok(())
    }
}
