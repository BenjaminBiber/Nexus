//! Fragen an den Nutzer samt Tags, betroffenen Tickets und Abhängigkeiten; portiert aus
//! `packages/db/src/sqlite/SqliteQuestionRepository.ts`.

use std::borrow::Cow;
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

use crate::sqlite::runner::{date, guid, iso, iso_millis, Params, SqlValue};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;

/// Die Spalten einer Frage in der Reihenfolge, in der [`Row::of`] sie liest
const COLUMNS: &str = "q.Id, q.ProjectId, q.Number, q.Title, q.Description, q.State, q.Occasion, q.OriginKind, q.OriginName, q.OriginSession,
  q.DecisionOutcome, q.DecisionOption, q.DecisionText, q.DecisionReason, q.DecidedById, q.DecidedByName, q.DecidedAt,
  q.CreatedAt, q.CreatedById, q.UpdatedAt, q.Rev";

/// Eine Zeile aus `Questions`, noch mit den Texten der Aufzählungen
struct Row {
    id: String,
    project_id: i32,
    number: i32,
    title: String,
    description: String,
    state: String,
    occasion: Option<String>,
    origin_kind: String,
    origin_name: String,
    origin_session: Option<String>,
    decision_outcome: Option<String>,
    decision_option: Option<String>,
    decision_text: Option<String>,
    decision_reason: Option<String>,
    decided_by_id: Option<String>,
    decided_by_name: Option<String>,
    decided_at: Option<String>,
    created_at: String,
    created_by_id: String,
    updated_at: String,
    rev: i32,
}

impl Row {
    /// Liest die Spalten in der Reihenfolge von [`COLUMNS`]
    fn of(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project_id: row.get(1)?,
            number: row.get(2)?,
            title: row.get(3)?,
            description: row.get(4)?,
            state: row.get(5)?,
            occasion: row.get(6)?,
            origin_kind: row.get(7)?,
            origin_name: row.get(8)?,
            origin_session: row.get(9)?,
            decision_outcome: row.get(10)?,
            decision_option: row.get(11)?,
            decision_text: row.get(12)?,
            decision_reason: row.get(13)?,
            decided_by_id: row.get(14)?,
            decided_by_name: row.get(15)?,
            decided_at: row.get(16)?,
            created_at: row.get(17)?,
            created_by_id: row.get(18)?,
            updated_at: row.get(19)?,
            rev: row.get(20)?,
        })
    }
}

#[async_trait]
impl BoardQuestionRepository for SqliteUnitOfWork<'_> {
    /// `for_update` ist bei SQLite ein No-op (Schreibtransaktionen laufen mit BEGIN IMMEDIATE)
    async fn by_id(&mut self, id: &str, _for_update: bool) -> Result<Option<BoardQuestion>> {
        let found = self.load("WHERE q.Id = @id", &sqlite_params! { "id": id })?;
        Ok(found.into_iter().next())
    }

    /// Leere Listen im Filter ergeben keine Fragen
    async fn list(&mut self, filter: &BoardQuestionFilter) -> Result<Vec<BoardQuestion>> {
        let mut where_parts: Vec<String> = Vec::new();
        let mut params: Params = Vec::new();
        if let Some(project_id) = filter.project_id {
            where_parts.push("q.ProjectId = @projectId".to_string());
            params.push((Cow::Borrowed("projectId"), SqlValue::from(project_id)));
        }
        if let Some(ids) = &filter.ids {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let list = bind(&mut params, "i", ids.iter().map(SqlValue::from));
            where_parts.push(format!("q.Id IN ({list})"));
        }
        if let Some(states) = &filter.states {
            if states.is_empty() {
                return Ok(Vec::new());
            }
            let list = bind(
                &mut params,
                "s",
                states.iter().map(|state| SqlValue::from(state.as_str())),
            );
            where_parts.push(format!("q.State IN ({list})"));
        }
        if let Some(ticket_ids) = &filter.ticket_ids {
            if ticket_ids.is_empty() {
                return Ok(Vec::new());
            }
            let list = bind(&mut params, "t", ticket_ids.iter().map(SqlValue::from));
            where_parts.push(format!(
                "q.Id IN (SELECT QuestionId FROM QuestionTickets WHERE TicketId IN ({list}))"
            ));
        }
        let where_sql = if where_parts.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_parts.join(" AND "))
        };
        self.load(&where_sql, &params)
    }

    async fn insert(&mut self, question: &BoardQuestion) -> Result<()> {
        self.sql.execute(
            "INSERT INTO Questions (Id, ProjectId, Number, Title, Description, State, Occasion, OriginKind, OriginName, OriginSession,
               DecisionOutcome, DecisionOption, DecisionText, DecisionReason, DecidedById, DecidedByName, DecidedAt,
               CreatedAt, CreatedById, UpdatedAt, Rev)
             VALUES (@id, @projectId, @number, @title, @description, @state, @occasion, @originKind, @originName, @originSession,
               @decisionOutcome, @decisionOption, @decisionText, @decisionReason, @decidedById, @decidedByName, @decidedAt,
               @createdAt, @createdById, @updatedAt, @rev)",
            &params_of(question),
        )?;
        self.write_children(question)
    }

    /// Schreibt nur, wenn in der DB noch die Vorgängerversion steht
    async fn update(&mut self, question: &BoardQuestion) -> Result<()> {
        let mut params = params_of(question);
        params.push((
            Cow::Borrowed("previousRev"),
            SqlValue::from(question.rev - 1),
        ));
        let affected = self.sql.execute(
            "UPDATE Questions SET Title = @title, Description = @description, State = @state, Occasion = @occasion,
               DecisionOutcome = @decisionOutcome, DecisionOption = @decisionOption, DecisionText = @decisionText,
               DecisionReason = @decisionReason, DecidedById = @decidedById, DecidedByName = @decidedByName, DecidedAt = @decidedAt,
               UpdatedAt = @updatedAt, Rev = @rev
             WHERE Id = @id AND Rev = @previousRev",
            &params,
        )?;
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
        // Je Aufruf genau eine Anweisung (der MSSQL-Batch ist hier in drei Aufrufe zerlegt)
        let id = sqlite_params! { "id": question.id.clone() };
        self.sql
            .execute("DELETE FROM QuestionTags WHERE QuestionId = @id", &id)?;
        self.sql
            .execute("DELETE FROM QuestionTickets WHERE QuestionId = @id", &id)?;
        self.sql.execute(
            "DELETE FROM QuestionDependencies WHERE QuestionId = @id",
            &id,
        )?;
        self.write_children(question)
    }

    /// Liest die Fragen, die das Ticket betreffen oder davon abhängen (ohne Sperre)
    async fn ids_referencing_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        let rows = self.sql.query_rows(
            "SELECT QuestionId FROM QuestionTickets WHERE TicketId = @ticketId UNION SELECT QuestionId FROM QuestionDependencies WHERE DependsOnId = @ticketId",
            &sqlite_params! { "ticketId": ticket_id },
            |row| row.get::<_, String>(0),
        )?;
        Ok(rows
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }

    /// Entfernt Bezüge und Abhängigkeiten auf das Ticket; die Fragen selbst bleiben unverändert
    /// (Rev erhöht der Dienst)
    async fn remove_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        let params = sqlite_params! { "ticketId": ticket_id };
        let mut ids: BTreeSet<String> = self
            .sql
            .query_rows(
                "DELETE FROM QuestionTickets WHERE TicketId = @ticketId RETURNING QuestionId",
                &params,
                |row| row.get::<_, String>(0),
            )?
            .into_iter()
            .collect();
        ids.extend(self.sql.query_rows(
            "DELETE FROM QuestionDependencies WHERE DependsOnId = @ticketId RETURNING QuestionId",
            &params,
            |row| row.get::<_, String>(0),
        )?);
        Ok(ids.into_iter().collect())
    }
}

impl SqliteUnitOfWork<'_> {
    /// Lädt Fragen zu einer WHERE-Klausel und ergänzt Tags, Tickets und Abhängigkeiten
    fn load(&mut self, where_sql: &str, params: &Params) -> Result<Vec<BoardQuestion>> {
        let rows = self.sql.query_rows(
            &format!(
                "SELECT {COLUMNS} FROM Questions q {where_sql} ORDER BY q.ProjectId, q.Number"
            ),
            params,
            Row::of,
        )?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let ids = format!("SELECT q.Id FROM Questions q {where_sql}");
        let tags = self.sql.query_rows(
            &format!("SELECT QuestionId, Tag FROM QuestionTags WHERE QuestionId IN ({ids}) ORDER BY QuestionId, Position"),
            params,
            pair,
        )?;
        let tickets = self.sql.query_rows(
            &format!(
                "SELECT QuestionId, TicketId FROM QuestionTickets WHERE QuestionId IN ({ids})"
            ),
            params,
            pair,
        )?;
        let deps = self.sql.query_rows(
            &format!("SELECT QuestionId, DependsOnId FROM QuestionDependencies WHERE QuestionId IN ({ids}) ORDER BY QuestionId, Position"),
            params,
            pair,
        )?;
        rows.into_iter()
            .map(|row| {
                // Betroffene Tickets sortiert (in TS nach dem Lesen, nicht über ORDER BY)
                let mut ticket_ids = children(&tickets, &row.id);
                ticket_ids.sort();
                question_of(row, &tags, ticket_ids, &deps)
            })
            .collect()
    }

    /// Schreibt Tags (mit Position), betroffene Tickets und Abhängigkeiten (mit Position)
    fn write_children(&mut self, question: &BoardQuestion) -> Result<()> {
        for (position, tag) in question.tags.iter().enumerate() {
            self.sql.execute(
                "INSERT INTO QuestionTags (QuestionId, Position, Tag) VALUES (@id, @position, @tag)",
                &sqlite_params! { "id": question.id.clone(), "position": position as i64, "tag": tag },
            )?;
        }
        for ticket_id in &question.ticket_ids {
            self.sql.execute(
                "INSERT INTO QuestionTickets (QuestionId, TicketId) VALUES (@id, @ticketId)",
                &sqlite_params! { "id": question.id.clone(), "ticketId": ticket_id },
            )?;
        }
        for (position, dep) in question.depends_on.iter().enumerate() {
            self.sql.execute(
                "INSERT INTO QuestionDependencies (QuestionId, Position, DependsOnId) VALUES (@id, @position, @dep)",
                &sqlite_params! { "id": question.id.clone(), "position": position as i64, "dep": dep },
            )?;
        }
        Ok(())
    }
}

/// Zwei Textspalten einer Kindtabelle: Frage-Id und Wert
fn pair(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, String)> {
    Ok((row.get(0)?, row.get(1)?))
}

/// Die Werte einer Kindtabelle, die zu dieser Frage gehören (in Lesereihenfolge)
fn children(rows: &[(String, String)], question_id: &str) -> Vec<String> {
    rows.iter()
        .filter(|(id, _)| id == question_id)
        .map(|(_, value)| value.clone())
        .collect()
}

/// Setzt eine Frage aus Zeile und Kindtabellen zusammen
fn question_of(
    row: Row,
    tags: &[(String, String)],
    ticket_ids: Vec<String>,
    deps: &[(String, String)],
) -> Result<BoardQuestion> {
    let decision = match &row.decision_outcome {
        Some(outcome) => Some(BoardQuestionDecision {
            outcome: outcome_of(outcome)?,
            option: row.decision_option,
            text: row.decision_text,
            reason: row.decision_reason.unwrap_or_default(),
            by_id: guid(row.decided_by_id.as_deref().unwrap_or_default()),
            by_name: row.decided_by_name.unwrap_or_default(),
            at: iso(row.decided_at).unwrap_or_default(),
        }),
        None => None,
    };
    Ok(BoardQuestion {
        tags: children(tags, &row.id),
        depends_on: children(deps, &row.id),
        ticket_ids,
        state: QuestionState::from_str(&row.state)?,
        origin_kind: AuthorKind::from_str(&row.origin_kind)?,
        decision,
        id: row.id,
        project_id: row.project_id,
        number: row.number,
        title: row.title,
        description: row.description,
        occasion: row.occasion,
        origin_name: row.origin_name,
        origin_session: row.origin_session,
        created_at: iso_millis(&row.created_at),
        created_by_id: guid(&row.created_by_id),
        updated_at: iso_millis(&row.updated_at),
        rev: row.rev,
    })
}

/// Ausgang einer Entscheidung aus dem Spaltentext (die Spalte hat eine CHECK-Bedingung)
fn outcome_of(text: &str) -> Result<BoardQuestionDecisionOutcome> {
    BoardQuestionDecisionOutcome::ALL
        .into_iter()
        .find(|outcome| outcome.as_str() == text)
        .ok_or_else(|| {
            Error::unexpected_message(format!(
                "Ausgang „{text}“ einer Entscheidung ist unbekannt."
            ))
        })
}

/// Bindet eine Liste als `@präfix0`, `@präfix1`, … und liefert die Platzhalter
pub(crate) fn bind(
    params: &mut Params,
    prefix: &str,
    values: impl IntoIterator<Item = SqlValue>,
) -> String {
    let mut placeholders = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        params.push((Cow::Owned(format!("{prefix}{index}")), value));
        placeholders.push(format!("@{prefix}{index}"));
    }
    placeholders.join(", ")
}

/// Parameter einer Frage für INSERT/UPDATE (GUIDs klein, Zeitpunkte als ISO-Text)
fn params_of(question: &BoardQuestion) -> Params {
    let decision = question.decision.as_ref();
    sqlite_params! {
        "id": question.id.clone(),
        "projectId": question.project_id,
        "number": question.number,
        "title": question.title.clone(),
        "description": question.description.clone(),
        "state": question.state.as_str(),
        "occasion": question.occasion.clone(),
        "originKind": question.origin_kind.as_str(),
        "originName": question.origin_name.clone(),
        "originSession": question.origin_session.clone(),
        "decisionOutcome": decision.map(|d| d.outcome.as_str()),
        "decisionOption": decision.and_then(|d| d.option.clone()),
        "decisionText": decision.and_then(|d| d.text.clone()),
        "decisionReason": decision.map(|d| d.reason.clone()),
        "decidedById": decision.map(|d| d.by_id.to_lowercase()),
        "decidedByName": decision.map(|d| d.by_name.clone()),
        "decidedAt": decision.map(|d| date(&d.at)),
        "createdAt": date(&question.created_at),
        "createdById": question.created_by_id.to_lowercase(),
        "updatedAt": date(&question.updated_at),
        "rev": question.rev,
    }
}
