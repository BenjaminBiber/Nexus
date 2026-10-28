//! Recherchen zu einer Frage auf SQL Server; portiert aus
//! `packages/db/src/MssqlQuestionResearchRepository.ts`.
//!
//! Die Recherche steht in `board.QuestionResearch`, ihre Optionen in `board.QuestionOptions`;
//! `Pros` und `Cons` liegen dort als JSON-Listen. Nur anhängen.

use std::collections::BTreeMap;
use std::str::FromStr;

use async_trait::async_trait;
use nexus_core::actors::AuthorKind;
use nexus_core::board::database::{BoardQuestionResearchRepository, NewQuestionResearch};
use nexus_core::board::question_types::{
    BoardQuestionOption, BoardQuestionResearch, BoardQuestionResearchSummary,
};
use nexus_core::{Error, Result};

use crate::mssql::repos::{borrow_params, first_row};
use crate::mssql::runner::MsValue;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

/// JSON-Liste aus der Spalte `Pros`/`Cons`
fn string_list(json: &str, column: &str) -> Result<Vec<String>> {
    serde_json::from_str(json).map_err(|error| {
        Error::unexpected_message(format!(
            "Die Spalte {column} einer Option ist keine Liste von Texten: {error}"
        ))
    })
}

#[async_trait]
impl BoardQuestionResearchRepository for MssqlUnitOfWork<'_> {
    /// Recherchen in Reihenfolge des Anlegens, Optionen nach Recherche und Nummer
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionResearch>> {
        let research = self
            .sql
            .query_rows(
                "SELECT Id, QuestionId, At, ById, ByName, Kind, Context FROM board.QuestionResearch WHERE QuestionId = @questionId ORDER BY Id",
                &[("questionId", MsValue::text(question_id))],
            )
            .await?;
        if research.is_empty() {
            return Ok(Vec::new());
        }
        let options = self
            .sql
            .query_rows(
                "SELECT ResearchId, OptionId, Title, Text, Pros, Cons, Recommended FROM board.QuestionOptions
       WHERE QuestionId = @questionId ORDER BY ResearchId, CAST(SUBSTRING(OptionId, 2, 10) AS int)",
                &[("questionId", MsValue::text(question_id))],
            )
            .await?;
        research
            .iter()
            .map(|row| {
                let id = row.get_i64("Id")?;
                let kind = row.get_str("Kind")?;
                let eigene: Vec<BoardQuestionOption> = options
                    .iter()
                    .filter(|option| option.get_i64("ResearchId").is_ok_and(|rid| rid == id))
                    .map(|option| {
                        Ok(BoardQuestionOption {
                            id: option.get_str("OptionId")?,
                            title: option.get_str("Title")?,
                            text: option.get_str("Text")?,
                            pros: string_list(&option.get_str("Pros")?, "Pros")?,
                            cons: string_list(&option.get_str("Cons")?, "Cons")?,
                            recommended: option.get_bool("Recommended")?,
                        })
                    })
                    .collect::<Result<_>>()?;
                Ok(BoardQuestionResearch {
                    id,
                    question_id: row.get_str("QuestionId")?,
                    at: row.get_iso("At")?,
                    by_id: row.get_guid("ById")?,
                    by_name: row.get_str("ByName")?,
                    kind: AuthorKind::from_str(&kind).map_err(|_| {
                        Error::unexpected_message(format!(
                            "Recherche {id}: Art „{kind}“ ist unbekannt."
                        ))
                    })?,
                    context: row.get_str("Context")?,
                    options: eigene,
                })
            })
            .collect()
    }

    async fn insert(&mut self, research: &NewQuestionResearch) -> Result<BoardQuestionResearch> {
        let rows = self
            .sql
            .query_rows(
                "INSERT INTO board.QuestionResearch (QuestionId, At, ById, ByName, Kind, Context) OUTPUT inserted.Id VALUES (@questionId, @at, @byId, @byName, @kind, @context)",
                &[
                    ("questionId", MsValue::text(&research.question_id)),
                    ("at", MsValue::date(&research.at)?),
                    ("byId", MsValue::text(&research.by_id)),
                    ("byName", MsValue::text(&research.by_name)),
                    ("kind", MsValue::text(research.kind.as_str())),
                    ("context", MsValue::text(&research.context)),
                ],
            )
            .await?;
        let id = first_row(&rows, "question_research.insert")?.get_i64("Id")?;
        for option in &research.options {
            let pros = serde_json::to_string(&option.pros).unwrap_or_else(|_| "[]".to_string());
            let cons = serde_json::to_string(&option.cons).unwrap_or_else(|_| "[]".to_string());
            self.sql
                .execute(
                    "INSERT INTO board.QuestionOptions (QuestionId, ResearchId, OptionId, Title, Text, Pros, Cons, Recommended)
         VALUES (@questionId, @researchId, @optionId, @title, @text, @pros, @cons, @recommended)",
                    &[
                        ("questionId", MsValue::text(&research.question_id)),
                        ("researchId", MsValue::BigInt(id)),
                        ("optionId", MsValue::text(&option.id)),
                        ("title", MsValue::text(&option.title)),
                        ("text", MsValue::text(&option.text)),
                        ("pros", MsValue::text(pros)),
                        ("cons", MsValue::text(cons)),
                        ("recommended", MsValue::Bit(option.recommended)),
                    ],
                )
                .await?;
        }
        Ok(BoardQuestionResearch {
            id,
            question_id: research.question_id.clone(),
            at: research.at.clone(),
            by_id: research.by_id.clone(),
            by_name: research.by_name.clone(),
            kind: research.kind,
            context: research.context.clone(),
            options: research.options.clone(),
        })
    }

    /// Zahl der Recherchen und Optionen der neuesten je Frage
    async fn summaries(
        &mut self,
        question_ids: &[String],
    ) -> Result<BTreeMap<String, BoardQuestionResearchSummary>> {
        if question_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let params: Vec<(String, MsValue)> = question_ids
            .iter()
            .enumerate()
            .map(|(index, id)| (format!("q{index}"), MsValue::text(id.as_str())))
            .collect();
        let list = (0..question_ids.len())
            .map(|index| format!("@q{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let rows = self
            .sql
            .query_rows(
                &format!(
                    "SELECT QuestionId, COUNT(*) AS N, MAX(Id) AS Latest FROM board.QuestionResearch
       WHERE QuestionId IN ({list}) GROUP BY QuestionId"
                ),
                &borrow_params(&params),
            )
            .await?;
        if rows.is_empty() {
            return Ok(BTreeMap::new());
        }
        let latest: Vec<(String, MsValue)> = rows
            .iter()
            .enumerate()
            .map(|(index, row)| Ok((format!("r{index}"), MsValue::BigInt(row.get_i64("Latest")?))))
            .collect::<Result<_>>()?;
        let latest_list = (0..rows.len())
            .map(|index| format!("@r{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let options = self
            .sql
            .query_rows(
                &format!(
                    "SELECT ResearchId, COUNT(*) AS N FROM board.QuestionOptions WHERE ResearchId IN ({latest_list}) GROUP BY ResearchId"
                ),
                &borrow_params(&latest),
            )
            .await?;
        let counts: BTreeMap<i64, i32> = options
            .iter()
            .map(|row| Ok((row.get_i64("ResearchId")?, row.get_i32("N")?)))
            .collect::<Result<_>>()?;
        rows.iter()
            .map(|row| {
                let latest = row.get_i64("Latest")?;
                Ok((
                    row.get_str("QuestionId")?,
                    BoardQuestionResearchSummary {
                        count: row.get_i32("N")?,
                        option_count: counts.get(&latest).copied().unwrap_or(0),
                    },
                ))
            })
            .collect()
    }
}
