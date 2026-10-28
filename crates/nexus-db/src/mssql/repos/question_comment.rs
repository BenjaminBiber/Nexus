//! Kommentare an einer Frage auf SQL Server; portiert aus
//! `packages/db/src/MssqlQuestionCommentRepository.ts`. Nur anhängen.

use std::str::FromStr;

use async_trait::async_trait;
use nexus_core::actors::AuthorKind;
use nexus_core::board::database::{BoardQuestionCommentRepository, NewQuestionComment};
use nexus_core::board::question_types::BoardQuestionComment;
use nexus_core::{Error, Result};

use crate::mssql::repos::first_row;
use crate::mssql::runner::MsValue;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

#[async_trait]
impl BoardQuestionCommentRepository for MssqlUnitOfWork<'_> {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionComment>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT Id, QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt FROM board.QuestionComments WHERE QuestionId = @questionId ORDER BY Id",
                &[("questionId", MsValue::text(question_id))],
            )
            .await?;
        rows.iter()
            .map(|row| {
                let id = row.get_i64("Id")?;
                let kind = row.get_str("AuthorKind")?;
                Ok(BoardQuestionComment {
                    id,
                    question_id: row.get_str("QuestionId")?,
                    author_id: row.get_guid("AuthorId")?,
                    author_name: row.get_str("AuthorName")?,
                    author_kind: AuthorKind::from_str(&kind).map_err(|_| {
                        Error::unexpected_message(format!(
                            "Kommentar {id} an einer Frage: Art „{kind}“ ist unbekannt."
                        ))
                    })?,
                    body: row.get_str("Body")?,
                    created_at: row.get_iso("CreatedAt")?,
                })
            })
            .collect()
    }

    async fn insert(&mut self, comment: &NewQuestionComment) -> Result<BoardQuestionComment> {
        let rows = self
            .sql
            .query_rows(
                "INSERT INTO board.QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt)
         OUTPUT inserted.Id VALUES (@questionId, @authorId, @authorName, @authorKind, @body, @createdAt)",
                &[
                    ("questionId", MsValue::text(&comment.question_id)),
                    ("authorId", MsValue::text(&comment.author_id)),
                    ("authorName", MsValue::text(&comment.author_name)),
                    ("authorKind", MsValue::text(comment.author_kind.as_str())),
                    ("body", MsValue::text(&comment.body)),
                    ("createdAt", MsValue::date(&comment.created_at)?),
                ],
            )
            .await?;
        Ok(BoardQuestionComment {
            id: first_row(&rows, "question_comments.insert")?.get_i64("Id")?,
            question_id: comment.question_id.clone(),
            author_id: comment.author_id.clone(),
            author_name: comment.author_name.clone(),
            author_kind: comment.author_kind,
            body: comment.body.clone(),
            created_at: comment.created_at.clone(),
        })
    }
}
