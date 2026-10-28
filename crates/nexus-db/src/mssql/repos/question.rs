//! Fragen an den Nutzer auf SQL Server; portiert aus
//! `packages/db/src/MssqlQuestionRepository.ts`.
//!
//! Die Frage steht in `board.Questions`, ihre Tags, betroffenen Tickets und Abhängigkeiten in
//! drei Kindtabellen. Gelesen wird in vier Abfragen: erst die Fragen, dann die Kinder über
//! dieselbe FROM/WHERE-Klausel als Unterabfrage – ohne deren Sperr-Hinweis, der nur für die
//! Zeile selbst gilt.

use std::collections::BTreeSet;
use std::str::FromStr;

use async_trait::async_trait;
use nexus_core::actors::AuthorKind;
use nexus_core::board::database::{BoardQuestionFilter, BoardQuestionRepository};
use nexus_core::board::question_types::{
    BoardQuestion, BoardQuestionDecision, BoardQuestionDecisionOutcome,
};
use nexus_core::questions::QuestionState;
use nexus_core::{Error, HarnessError, Result};

use crate::mssql::repos::borrow_params;
use crate::mssql::runner::{MsRow, MsValue};
use crate::mssql::unit_of_work::MssqlUnitOfWork;

const COLUMNS: &str = "q.Id, q.ProjectId, q.Number, q.Title, q.Description, q.State, q.Occasion, q.OriginKind, q.OriginName, q.OriginSession,
  q.DecisionOutcome, q.DecisionOption, q.DecisionText, q.DecisionReason, q.DecidedById, q.DecidedByName, q.DecidedAt,
  q.CreatedAt, q.CreatedById, q.UpdatedAt, q.Rev";

/// Der Sperr-Hinweis, den `by_id(for_update)` an die FROM-Klausel hängt
const LOCK_HINT: &str = "WITH (UPDLOCK, ROWLOCK)";

/// Ausgang einer Entscheidung aus der Spalte `DecisionOutcome`
fn outcome_from(text: &str, id: &str) -> Result<BoardQuestionDecisionOutcome> {
    BoardQuestionDecisionOutcome::ALL
        .into_iter()
        .find(|outcome| outcome.as_str() == text)
        .ok_or_else(|| {
            Error::unexpected_message(format!(
                "Frage {id}: Ausgang „{text}“ der Entscheidung ist unbekannt."
            ))
        })
}

/// Die Parameter einer Frage für INSERT und UPDATE
fn params_of(question: &BoardQuestion) -> Result<Vec<(String, MsValue)>> {
    let decision = question.decision.as_ref();
    Ok(vec![
        ("id".to_string(), MsValue::text(&question.id)),
        ("projectId".to_string(), MsValue::Int(question.project_id)),
        ("number".to_string(), MsValue::Int(question.number)),
        ("title".to_string(), MsValue::text(&question.title)),
        (
            "description".to_string(),
            MsValue::text(&question.description),
        ),
        ("state".to_string(), MsValue::text(question.state.as_str())),
        (
            "occasion".to_string(),
            MsValue::text_opt(question.occasion.as_deref()),
        ),
        (
            "originKind".to_string(),
            MsValue::text(question.origin_kind.as_str()),
        ),
        (
            "originName".to_string(),
            MsValue::text(&question.origin_name),
        ),
        (
            "originSession".to_string(),
            MsValue::text_opt(question.origin_session.as_deref()),
        ),
        (
            "decisionOutcome".to_string(),
            MsValue::text_opt(decision.map(|d| d.outcome.as_str())),
        ),
        (
            "decisionOption".to_string(),
            MsValue::text_opt(decision.and_then(|d| d.option.as_deref())),
        ),
        (
            "decisionText".to_string(),
            MsValue::text_opt(decision.and_then(|d| d.text.as_deref())),
        ),
        (
            "decisionReason".to_string(),
            MsValue::text_opt(decision.map(|d| d.reason.as_str())),
        ),
        (
            "decidedById".to_string(),
            MsValue::text_opt(decision.map(|d| d.by_id.as_str())),
        ),
        (
            "decidedByName".to_string(),
            MsValue::text_opt(decision.map(|d| d.by_name.as_str())),
        ),
        (
            "decidedAt".to_string(),
            MsValue::date_opt(decision.map(|d| d.at.as_str()))?,
        ),
        (
            "createdAt".to_string(),
            MsValue::date(&question.created_at)?,
        ),
        (
            "createdById".to_string(),
            MsValue::text(&question.created_by_id),
        ),
        (
            "updatedAt".to_string(),
            MsValue::date(&question.updated_at)?,
        ),
        ("rev".to_string(), MsValue::Int(question.rev)),
    ])
}

/// Bindet eine Liste als `@präfix0`, `@präfix1`, … und liefert die Platzhalter
fn bind(params: &mut Vec<(String, MsValue)>, prefix: &str, values: &[String]) -> String {
    for (index, value) in values.iter().enumerate() {
        params.push((format!("{prefix}{index}"), MsValue::text(value.as_str())));
    }
    (0..values.len())
        .map(|index| format!("@{prefix}{index}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl MssqlUnitOfWork<'_> {
    /// Lädt Fragen zu einer FROM/WHERE-Klausel und ergänzt Tags, Tickets und Abhängigkeiten
    async fn load_questions(
        &mut self,
        from: &str,
        params: &[(String, MsValue)],
    ) -> Result<Vec<BoardQuestion>> {
        let sql = format!("SELECT {COLUMNS} {from} ORDER BY q.ProjectId, q.Number");
        let rows = self.sql.query_rows(&sql, &borrow_params(params)).await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        // Der Sperr-Hinweis gilt nur der gelesenen Zeile; in der Unterabfrage hat er nichts
        // verloren (so macht es auch TypeScript)
        let ids = format!("SELECT q.Id {}", from.replace(LOCK_HINT, ""));
        let tags = self
            .sql
            .query_rows(
                &format!("SELECT QuestionId, Tag FROM board.QuestionTags WHERE QuestionId IN ({ids}) ORDER BY QuestionId, Position"),
                &borrow_params(params),
            )
            .await?;
        let tickets = self
            .sql
            .query_rows(
                &format!("SELECT QuestionId, TicketId FROM board.QuestionTickets WHERE QuestionId IN ({ids})"),
                &borrow_params(params),
            )
            .await?;
        let deps = self
            .sql
            .query_rows(
                &format!("SELECT QuestionId, DependsOnId FROM board.QuestionDependencies WHERE QuestionId IN ({ids}) ORDER BY QuestionId, Position"),
                &borrow_params(params),
            )
            .await?;

        /// Werte einer Kindtabelle zu dieser Frage, in der Reihenfolge der Abfrage
        fn children(rows: &[MsRow], question_id: &str, column: &str) -> Result<Vec<String>> {
            rows.iter()
                .filter(|row| row.get_str("QuestionId").is_ok_and(|id| id == question_id))
                .map(|row| row.get_str(column))
                .collect()
        }

        rows.iter()
            .map(|row| {
                let id = row.get_str("Id")?;
                let state = row.get_str("State")?;
                let origin_kind = row.get_str("OriginKind")?;
                let mut ticket_ids = children(&tickets, &id, "TicketId")?;
                ticket_ids.sort();
                let decision = match row.get_opt_str("DecisionOutcome")? {
                    None => None,
                    Some(outcome) => Some(BoardQuestionDecision {
                        outcome: outcome_from(&outcome, &id)?,
                        option: row.get_opt_str("DecisionOption")?,
                        text: row.get_opt_str("DecisionText")?,
                        reason: row.get_opt_str("DecisionReason")?.unwrap_or_default(),
                        by_id: row.get_opt_guid("DecidedById")?.unwrap_or_default(),
                        by_name: row.get_opt_str("DecidedByName")?.unwrap_or_default(),
                        at: row.get_iso("DecidedAt")?,
                    }),
                };
                Ok(BoardQuestion {
                    project_id: row.get_i32("ProjectId")?,
                    number: row.get_i32("Number")?,
                    title: row.get_str("Title")?,
                    description: row.get_str("Description")?,
                    state: QuestionState::from_str(&state).map_err(|_| {
                        Error::unexpected_message(format!(
                            "Frage {id}: Zustand „{state}“ ist unbekannt."
                        ))
                    })?,
                    occasion: row.get_opt_str("Occasion")?,
                    origin_kind: AuthorKind::from_str(&origin_kind).map_err(|_| {
                        Error::unexpected_message(format!(
                            "Frage {id}: Herkunft „{origin_kind}“ ist unbekannt."
                        ))
                    })?,
                    origin_name: row.get_str("OriginName")?,
                    origin_session: row.get_opt_str("OriginSession")?,
                    tags: children(&tags, &id, "Tag")?,
                    ticket_ids,
                    depends_on: children(&deps, &id, "DependsOnId")?,
                    decision,
                    created_at: row.get_iso("CreatedAt")?,
                    created_by_id: row.get_guid("CreatedById")?,
                    updated_at: row.get_iso("UpdatedAt")?,
                    rev: row.get_i32("Rev")?,
                    id,
                })
            })
            .collect()
    }

    /// Schreibt Tags (mit Position), betroffene Tickets und Abhängigkeiten (mit Position)
    async fn write_question_children(&mut self, question: &BoardQuestion) -> Result<()> {
        for (position, tag) in question.tags.iter().enumerate() {
            self.sql
                .execute(
                    "INSERT INTO board.QuestionTags (QuestionId, Position, Tag) VALUES (@id, @position, @tag)",
                    &[
                        ("id", MsValue::text(&question.id)),
                        ("position", MsValue::Int(position as i32)),
                        ("tag", MsValue::text(tag.as_str())),
                    ],
                )
                .await?;
        }
        for ticket_id in &question.ticket_ids {
            self.sql
                .execute(
                    "INSERT INTO board.QuestionTickets (QuestionId, TicketId) VALUES (@id, @ticketId)",
                    &[
                        ("id", MsValue::text(&question.id)),
                        ("ticketId", MsValue::text(ticket_id.as_str())),
                    ],
                )
                .await?;
        }
        for (position, dep) in question.depends_on.iter().enumerate() {
            self.sql
                .execute(
                    "INSERT INTO board.QuestionDependencies (QuestionId, Position, DependsOnId) VALUES (@id, @position, @dep)",
                    &[
                        ("id", MsValue::text(&question.id)),
                        ("position", MsValue::Int(position as i32)),
                        ("dep", MsValue::text(dep.as_str())),
                    ],
                )
                .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl BoardQuestionRepository for MssqlUnitOfWork<'_> {
    /// `for_update` sperrt die Zeile bis zum Ende der Transaktion
    async fn by_id(&mut self, id: &str, for_update: bool) -> Result<Option<BoardQuestion>> {
        let hint = if for_update { LOCK_HINT } else { "" };
        let from = format!("FROM board.Questions q {hint} WHERE q.Id = @id");
        let params = vec![("id".to_string(), MsValue::text(id))];
        Ok(self
            .load_questions(&from, &params)
            .await?
            .into_iter()
            .next())
    }

    /// Leere Listen im Filter ergeben keine Fragen
    async fn list(&mut self, filter: &BoardQuestionFilter) -> Result<Vec<BoardQuestion>> {
        let mut where_parts: Vec<String> = Vec::new();
        let mut params: Vec<(String, MsValue)> = Vec::new();
        if let Some(project_id) = filter.project_id {
            where_parts.push("q.ProjectId = @projectId".to_string());
            params.push(("projectId".to_string(), MsValue::Int(project_id)));
        }
        if let Some(ids) = &filter.ids {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let list = bind(&mut params, "i", ids);
            where_parts.push(format!("q.Id IN ({list})"));
        }
        if let Some(states) = &filter.states {
            if states.is_empty() {
                return Ok(Vec::new());
            }
            let texts: Vec<String> = states
                .iter()
                .map(|state| state.as_str().to_string())
                .collect();
            let list = bind(&mut params, "s", &texts);
            where_parts.push(format!("q.State IN ({list})"));
        }
        if let Some(ticket_ids) = &filter.ticket_ids {
            if ticket_ids.is_empty() {
                return Ok(Vec::new());
            }
            let list = bind(&mut params, "t", ticket_ids);
            where_parts.push(format!(
                "q.Id IN (SELECT QuestionId FROM board.QuestionTickets WHERE TicketId IN ({list}))"
            ));
        }
        let where_clause = if where_parts.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_parts.join(" AND "))
        };
        let from = format!("FROM board.Questions q {where_clause}");
        self.load_questions(&from, &params).await
    }

    async fn insert(&mut self, question: &BoardQuestion) -> Result<()> {
        let params = params_of(question)?;
        self.sql
            .execute(
                "INSERT INTO board.Questions (Id, ProjectId, Number, Title, Description, State, Occasion, OriginKind, OriginName, OriginSession,
         DecisionOutcome, DecisionOption, DecisionText, DecisionReason, DecidedById, DecidedByName, DecidedAt,
         CreatedAt, CreatedById, UpdatedAt, Rev)
       VALUES (@id, @projectId, @number, @title, @description, @state, @occasion, @originKind, @originName, @originSession,
         @decisionOutcome, @decisionOption, @decisionText, @decisionReason, @decidedById, @decidedByName, @decidedAt,
         @createdAt, @createdById, @updatedAt, @rev)",
                &borrow_params(&params),
            )
            .await?;
        self.write_question_children(question).await
    }

    /// Schreibt nur, wenn in der DB noch die Vorgängerversion steht
    async fn update(&mut self, question: &BoardQuestion) -> Result<()> {
        let mut params = params_of(question)?;
        params.push(("previousRev".to_string(), MsValue::Int(question.rev - 1)));
        let affected = self
            .sql
            .execute(
                "UPDATE board.Questions SET Title = @title, Description = @description, State = @state, Occasion = @occasion,
         DecisionOutcome = @decisionOutcome, DecisionOption = @decisionOption, DecisionText = @decisionText,
         DecisionReason = @decisionReason, DecidedById = @decidedById, DecidedByName = @decidedByName, DecidedAt = @decidedAt,
         UpdatedAt = @updatedAt, Rev = @rev
       WHERE Id = @id AND Rev = @previousRev",
                &borrow_params(&params),
            )
            .await?;
        if affected == 0 {
            return Err(HarnessError::new(
                "conflict",
                format!(
                    "Frage {} wurde inzwischen geändert. Bitte neu lesen.",
                    question.id
                ),
            )
            .into());
        }
        self.sql
            .execute(
                "DELETE FROM board.QuestionTags WHERE QuestionId = @id; DELETE FROM board.QuestionTickets WHERE QuestionId = @id; DELETE FROM board.QuestionDependencies WHERE QuestionId = @id;",
                &[("id", MsValue::text(&question.id))],
            )
            .await?;
        self.write_question_children(question).await
    }

    /// Liest die Fragen, die das Ticket betreffen oder davon abhängen (ohne Sperre)
    async fn ids_referencing_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT QuestionId FROM board.QuestionTickets WHERE TicketId = @ticketId UNION SELECT QuestionId FROM board.QuestionDependencies WHERE DependsOnId = @ticketId",
                &[("ticketId", MsValue::text(ticket_id))],
            )
            .await?;
        let ids: BTreeSet<String> = rows
            .iter()
            .map(|row| row.get_str("QuestionId"))
            .collect::<Result<_>>()?;
        Ok(ids.into_iter().collect())
    }

    /// Entfernt Bezüge und Abhängigkeiten auf das Ticket; die Fragen selbst bleiben unverändert
    /// (Rev erhöht der Dienst)
    async fn remove_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        let tickets = self
            .sql
            .query_rows(
                "DELETE FROM board.QuestionTickets OUTPUT deleted.QuestionId WHERE TicketId = @ticketId",
                &[("ticketId", MsValue::text(ticket_id))],
            )
            .await?;
        let deps = self
            .sql
            .query_rows(
                "DELETE FROM board.QuestionDependencies OUTPUT deleted.QuestionId WHERE DependsOnId = @ticketId",
                &[("ticketId", MsValue::text(ticket_id))],
            )
            .await?;
        let ids: BTreeSet<String> = tickets
            .iter()
            .chain(deps.iter())
            .map(|row| row.get_str("QuestionId"))
            .collect::<Result<_>>()?;
        Ok(ids.into_iter().collect())
    }
}
