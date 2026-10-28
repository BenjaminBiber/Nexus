//! Recherchen zu einer Frage in `QuestionResearch` mit ihren Optionen in `QuestionOptions`
//! (nur anhängen); portiert aus `packages/db/src/sqlite/SqliteQuestionResearchRepository.ts`.

use std::collections::BTreeMap;
use std::str::FromStr;

use async_trait::async_trait;

use nexus_core::actors::AuthorKind;
use nexus_core::board::database::{BoardQuestionResearchRepository, NewQuestionResearch};
use nexus_core::board::question_types::{
    BoardQuestionOption, BoardQuestionResearch, BoardQuestionResearchSummary,
};
use nexus_core::{Error, Result};

use super::question::bind;
use crate::sqlite::runner::{bool_of, date, guid, iso_millis, missing_row, Params, SqlValue};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;

/// Eine Zeile aus `QuestionOptions` samt ihrer Recherche
struct OptionRow {
    research_id: i64,
    option: BoardQuestionOption,
}

#[async_trait]
impl BoardQuestionResearchRepository for SqliteUnitOfWork<'_> {
    /// Recherchen in Reihenfolge des Anlegens, Optionen nach Recherche und Nummer
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionResearch>> {
        let params = sqlite_params! { "questionId": question_id };
        let research = self.sql.query_rows(
            "SELECT Id, QuestionId, At, ById, ByName, Kind, Context FROM QuestionResearch WHERE QuestionId = @questionId ORDER BY Id",
            &params,
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )?;
        if research.is_empty() {
            return Ok(Vec::new());
        }
        let options = self.sql.query_rows(
            "SELECT ResearchId, OptionId, Title, Text, Pros, Cons, Recommended FROM QuestionOptions
             WHERE QuestionId = @questionId ORDER BY ResearchId, CAST(substr(OptionId, 2) AS INTEGER)",
            &params,
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )?;
        let options = options
            .into_iter()
            .map(|(research_id, id, title, text, pros, cons, recommended)| {
                Ok(OptionRow {
                    research_id,
                    option: BoardQuestionOption {
                        id,
                        title,
                        text,
                        pros: texts_of(&pros)?,
                        cons: texts_of(&cons)?,
                        recommended: bool_of(recommended),
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?;
        research
            .into_iter()
            .map(|(id, question_id, at, by_id, by_name, kind, context)| {
                Ok(BoardQuestionResearch {
                    options: options
                        .iter()
                        .filter(|row| row.research_id == id)
                        .map(|row| row.option.clone())
                        .collect(),
                    id,
                    question_id,
                    at: iso_millis(&at),
                    by_id: guid(&by_id),
                    by_name,
                    kind: AuthorKind::from_str(&kind)?,
                    context,
                })
            })
            .collect()
    }

    async fn insert(&mut self, research: &NewQuestionResearch) -> Result<BoardQuestionResearch> {
        let ids = self.sql.query_rows(
            "INSERT INTO QuestionResearch (QuestionId, At, ById, ByName, Kind, Context) VALUES (@questionId, @at, @byId, @byName, @kind, @context) RETURNING Id",
            &sqlite_params! {
                "questionId": research.question_id.clone(),
                "at": date(&research.at),
                "byId": research.by_id.to_lowercase(),
                "byName": research.by_name.clone(),
                "kind": research.kind.as_str(),
                "context": research.context.clone(),
            },
            |row| row.get::<_, i64>(0),
        )?;
        let id = ids
            .into_iter()
            .next()
            .ok_or_else(|| missing_row("QuestionResearch.Id"))?;
        for option in &research.options {
            self.sql.execute(
                "INSERT INTO QuestionOptions (QuestionId, ResearchId, OptionId, Title, Text, Pros, Cons, Recommended)
                 VALUES (@questionId, @researchId, @optionId, @title, @text, @pros, @cons, @recommended)",
                &sqlite_params! {
                    "questionId": research.question_id.clone(),
                    "researchId": id,
                    "optionId": option.id.clone(),
                    "title": option.title.clone(),
                    "text": option.text.clone(),
                    "pros": json_of(&option.pros)?,
                    "cons": json_of(&option.cons)?,
                    "recommended": option.recommended,
                },
            )?;
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

    /// Zahl der Recherchen und Optionen der neuesten je Frage; Fragen ohne Recherche fehlen
    async fn summaries(
        &mut self,
        question_ids: &[String],
    ) -> Result<BTreeMap<String, BoardQuestionResearchSummary>> {
        if question_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut params: Params = Vec::new();
        let list = bind(&mut params, "q", question_ids.iter().map(SqlValue::from));
        let rows = self.sql.query_rows(
            &format!(
                "SELECT QuestionId, COUNT(*) AS N, MAX(Id) AS Latest FROM QuestionResearch
                 WHERE QuestionId IN ({list}) GROUP BY QuestionId"
            ),
            &params,
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?;
        if rows.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut latest: Params = Vec::new();
        let list = bind(
            &mut latest,
            "r",
            rows.iter().map(|(_, _, id)| SqlValue::from(*id)),
        );
        let counts = self.sql.query_rows(
            &format!(
                "SELECT ResearchId, COUNT(*) AS N FROM QuestionOptions WHERE ResearchId IN ({list}) GROUP BY ResearchId"
            ),
            &latest,
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?)),
        )?;
        Ok(rows
            .into_iter()
            .map(|(question_id, count, latest)| {
                let option_count = counts
                    .iter()
                    .find(|(id, _)| *id == latest)
                    .map_or(0, |(_, count)| *count);
                (
                    question_id,
                    BoardQuestionResearchSummary {
                        count,
                        option_count,
                    },
                )
            })
            .collect())
    }
}

/// Vor- bzw. Nachteile aus dem JSON-Text der Spalte
fn texts_of(json: &str) -> Result<Vec<String>> {
    serde_json::from_str(json).map_err(|error| {
        Error::unexpected_message(format!("Liste „{json}“ ist kein JSON-Text: {error}"))
    })
}

/// Vor- bzw. Nachteile als JSON-Text für die Spalte
fn json_of(texts: &[String]) -> Result<String> {
    serde_json::to_string(texts).map_err(Error::unexpected)
}
