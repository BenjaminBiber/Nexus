//! Kommentare an einer Frage in `QuestionComments` (nur anhängen); portiert aus
//! `packages/db/src/sqlite/SqliteQuestionCommentRepository.ts`.

use std::str::FromStr;

use async_trait::async_trait;

use nexus_core::actors::AuthorKind;
use nexus_core::board::database::{BoardQuestionCommentRepository, NewQuestionComment};
use nexus_core::board::question_types::BoardQuestionComment;
use nexus_core::Result;

use crate::sqlite::runner::{date, guid, iso_millis, missing_row};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;

#[async_trait]
impl BoardQuestionCommentRepository for SqliteUnitOfWork<'_> {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionComment>> {
        let rows = self.sql.query_rows(
            "SELECT Id, QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt FROM QuestionComments WHERE QuestionId = @questionId ORDER BY Id",
            &sqlite_params! { "questionId": question_id },
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
        rows.into_iter()
            .map(
                |(id, question_id, author_id, author_name, author_kind, body, created_at)| {
                    Ok(BoardQuestionComment {
                        id,
                        question_id,
                        author_id: guid(&author_id),
                        author_name,
                        author_kind: AuthorKind::from_str(&author_kind)?,
                        body,
                        created_at: iso_millis(&created_at),
                    })
                },
            )
            .collect()
    }

    async fn insert(&mut self, comment: &NewQuestionComment) -> Result<BoardQuestionComment> {
        let ids = self.sql.query_rows(
            "INSERT INTO QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt)
             VALUES (@questionId, @authorId, @authorName, @authorKind, @body, @createdAt) RETURNING Id",
            &sqlite_params! {
                "questionId": comment.question_id.clone(),
                "authorId": comment.author_id.to_lowercase(),
                "authorName": comment.author_name.clone(),
                "authorKind": comment.author_kind.as_str(),
                "body": comment.body.clone(),
                "createdAt": date(&comment.created_at),
            },
            |row| row.get::<_, i64>(0),
        )?;
        let id = ids
            .into_iter()
            .next()
            .ok_or_else(|| missing_row("QuestionComments.Id"))?;
        Ok(BoardQuestionComment {
            id,
            question_id: comment.question_id.clone(),
            author_id: comment.author_id.clone(),
            author_name: comment.author_name.clone(),
            author_kind: comment.author_kind,
            body: comment.body.clone(),
            created_at: comment.created_at.clone(),
        })
    }
}
