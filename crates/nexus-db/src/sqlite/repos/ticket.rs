//! Tickets in der Tabelle `Tickets` samt Labels (`TicketLabels`) und Voraussetzungen
//! (`TicketDependencies`); portiert aus `packages/db/src/sqlite/SqliteTicketRepository.ts`.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use async_trait::async_trait;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::database::{TicketFilter, TicketRepository};
use nexus_core::board::types::Ticket;
use nexus_core::{HarnessError, Result};

use crate::sqlite::runner::{bool_of, date, guid, iso, Params, SqlValue};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;
use crate::RowGroups;

/// Alle Spalten eines Tickets, mit dem Alias `t` der FROM-Klausel
const COLUMNS: &str = "t.Id, t.ProjectId, t.BoardId, t.Number, t.Title, t.Description, t.Status, t.Priority, t.SortOrder, t.Assignee,
  t.NeedsPlanning, t.Acceptance, t.\"Plan\", t.Handbook, t.Diagram, t.BlockedReason, t.PlannedAt, t.PlannedBy,
  t.PlanningClaimBy, t.PlanningClaimAt, t.CreatedAt, t.CreatedById, t.CreatedByName, t.UpdatedAt, t.Rev,
  t.Spec, t.SpecTask, t.Branch, t.Worktree, t.TargetBranch, t.Touches, t.Consumes, t.Produces";

/// Die langen Texte, die `without_texts` nur noch als Kennzeichen liefert
const LONG_TEXTS: [&str; 5] = [
    "Description",
    "Acceptance",
    "\"Plan\"",
    "Handbook",
    "Diagram",
];

/// Wie [`COLUMNS`], aber die langen Texte nur als Kennzeichen (`filter.without_texts`):
/// leer bleibt leer, sonst `'x'`
static LIGHT_COLUMNS: LazyLock<String> = LazyLock::new(|| {
    LONG_TEXTS
        .iter()
        .fold(COLUMNS.to_string(), |columns, name| {
            columns.replace(
                &format!("t.{name}"),
                &format!("CASE WHEN length(trim(t.{name})) > 0 THEN 'x' ELSE '' END AS {name}"),
            )
        })
});

#[async_trait]
impl TicketRepository for SqliteUnitOfWork<'_> {
    /// `for_update` ist hier ein No-op: `BEGIN IMMEDIATE` sperrt die Datei schon bis zum Ende
    /// der Transaktion
    async fn by_id(&mut self, id: &str, _for_update: bool) -> Result<Option<Ticket>> {
        let found = self.load(
            "FROM Tickets t WHERE t.Id = @id",
            &sqlite_params! { "id": id },
            false,
        )?;
        Ok(found.into_iter().next())
    }

    /// Sortiert nach Projekt und Nummer, damit die Reihenfolge stabil ist
    async fn list(&mut self, filter: &TicketFilter) -> Result<Vec<Ticket>> {
        let mut where_parts: Vec<String> = Vec::new();
        let mut params: Params = Vec::new();
        if let Some(board_ids) = &filter.board_ids {
            if board_ids.is_empty() {
                return Ok(Vec::new());
            }
            where_parts.push(format!(
                "t.BoardId IN ({})",
                named(&mut params, "b", board_ids)
            ));
        }
        if let Some(ids) = &filter.ids {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            where_parts.push(format!("t.Id IN ({})", named(&mut params, "i", ids)));
        }
        let clause = if where_parts.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_parts.join(" AND "))
        };
        self.load(
            &format!("FROM Tickets t {clause}"),
            &params,
            filter.without_texts,
        )
    }

    async fn insert(&mut self, ticket: &Ticket) -> Result<()> {
        self.sql.execute(
            "INSERT INTO Tickets (Id, ProjectId, BoardId, Number, Title, Description, Status, Priority, SortOrder, Assignee,
         NeedsPlanning, Acceptance, \"Plan\", Handbook, Diagram, BlockedReason, PlannedAt, PlannedBy, PlanningClaimBy,
         PlanningClaimAt, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev,
         Spec, SpecTask, Branch, Worktree, TargetBranch, Touches, Consumes, Produces)
       VALUES (@id, @projectId, @boardId, @number, @title, @description, @status, @priority, @sortOrder, @assignee,
         @needsPlanning, @acceptance, @plan, @handbook, @diagram, @blockedReason, @plannedAt, @plannedBy, @planningClaimBy,
         @planningClaimAt, @createdAt, @createdById, @createdByName, @updatedAt, @rev,
         @spec, @specTask, @branch, @worktree, @targetBranch, @touches, @consumes, @produces)",
            &ticket_params(ticket),
        )?;
        self.write_children(ticket)
    }

    /// Schreibt nur, wenn in der DB noch die Vorgängerversion steht; nicht änderbare Spalten
    /// stehen nicht im UPDATE
    async fn update(&mut self, ticket: &Ticket) -> Result<()> {
        let mut params = ticket_params(ticket);
        params.push((
            "previousRev".into(),
            SqlValue::Int(i64::from(ticket.rev - 1)),
        ));
        let affected = self.sql.execute(
            "UPDATE Tickets SET BoardId = @boardId, Title = @title, Description = @description, Status = @status,
         Priority = @priority, SortOrder = @sortOrder, Assignee = @assignee, NeedsPlanning = @needsPlanning,
         Acceptance = @acceptance, \"Plan\" = @plan, Handbook = @handbook, Diagram = @diagram, BlockedReason = @blockedReason,
         PlannedAt = @plannedAt, PlannedBy = @plannedBy, PlanningClaimBy = @planningClaimBy, PlanningClaimAt = @planningClaimAt,
         Spec = @spec, SpecTask = @specTask, Branch = @branch, Worktree = @worktree, TargetBranch = @targetBranch, Touches = @touches, Consumes = @consumes, Produces = @produces,
         UpdatedAt = @updatedAt, Rev = @rev
       WHERE Id = @id AND Rev = @previousRev",
            &params,
        )?;
        if affected == 0 {
            return Err(HarnessError::new(
                "conflict",
                format!(
                    "Ticket {} wurde inzwischen geändert. Bitte neu lesen.",
                    ticket.id
                ),
            )
            .into());
        }
        // Kindzeilen neu schreiben; je Aufruf nur eine Anweisung
        self.sql.execute(
            "DELETE FROM TicketLabels WHERE TicketId = @id",
            &sqlite_params! { "id": ticket.id.clone() },
        )?;
        self.sql.execute(
            "DELETE FROM TicketDependencies WHERE TicketId = @id",
            &sqlite_params! { "id": ticket.id.clone() },
        )?;
        self.write_children(ticket)
    }

    /// Erst die Verweise anderer Tickets (`DependsOnId` hat keine Kaskade), dann das Ticket
    /// selbst; Labels, eigene Voraussetzungen, Kommentare und Verlauf fallen per Kaskade mit
    async fn delete(&mut self, id: &str) -> Result<()> {
        self.sql.execute(
            "DELETE FROM TicketDependencies WHERE DependsOnId = @id",
            &sqlite_params! { "id": id },
        )?;
        self.sql.execute(
            "DELETE FROM Tickets WHERE Id = @id",
            &sqlite_params! { "id": id },
        )?;
        Ok(())
    }

    /// Sortiert in Rust, damit die Reihenfolge nicht von der Collation abhängt
    async fn dependents(&mut self, id: &str) -> Result<Vec<String>> {
        let mut ids = self.sql.query_rows(
            "SELECT TicketId FROM TicketDependencies WHERE DependsOnId = @id",
            &sqlite_params! { "id": id },
            |row| row.get::<_, String>("TicketId"),
        )?;
        ids.sort();
        Ok(ids)
    }

    async fn remove_dependency_on(&mut self, id: &str) -> Result<Vec<String>> {
        let mut ids = self.sql.query_rows(
            "DELETE FROM TicketDependencies WHERE DependsOnId = @id RETURNING TicketId",
            &sqlite_params! { "id": id },
            |row| row.get::<_, String>("TicketId"),
        )?;
        ids.sort();
        Ok(ids)
    }

    async fn max_sort_order(&mut self, board_id: i32, status: TicketStatus) -> Result<Option<i32>> {
        let rows = self.sql.query_rows(
            "SELECT MAX(SortOrder) AS M FROM Tickets WHERE BoardId = @boardId AND Status = @status",
            &sqlite_params! { "boardId": board_id, "status": status.as_str() },
            |row| row.get::<_, Option<i32>>("M"),
        )?;
        Ok(rows.into_iter().next().flatten())
    }

    async fn open_count_by_board(&mut self, board_ids: &[i32]) -> Result<BTreeMap<i32, i64>> {
        if board_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut params: Params = Vec::new();
        let list = named(&mut params, "b", board_ids);
        let sql = format!(
            "SELECT BoardId, COUNT(*) AS N FROM Tickets
       WHERE BoardId IN ({list}) AND Status NOT IN ('done', 'not_needed') GROUP BY BoardId"
        );
        let rows = self.sql.query_rows(&sql, &params, |row| {
            Ok((row.get::<_, i32>("BoardId")?, row.get::<_, i64>("N")?))
        })?;
        Ok(rows.into_iter().collect())
    }
}

impl SqliteUnitOfWork<'_> {
    /// Lädt Tickets zu einer FROM/WHERE-Klausel und ergänzt Labels und Voraussetzungen
    fn load(&mut self, from: &str, params: &Params, without_texts: bool) -> Result<Vec<Ticket>> {
        let columns: &str = if without_texts {
            LIGHT_COLUMNS.as_str()
        } else {
            COLUMNS
        };
        let rows = self.sql.query_rows(
            &format!("SELECT {columns} {from} ORDER BY t.ProjectId, t.Number"),
            params,
            map_row,
        )?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let ids = format!("SELECT t.Id {from}");
        let labels = self.sql.query_rows(
            &format!(
                "SELECT TicketId, Label FROM TicketLabels WHERE TicketId IN ({ids}) ORDER BY TicketId, Position"
            ),
            params,
            |row| Ok((row.get::<_, String>("TicketId")?, row.get::<_, String>("Label")?)),
        )?;
        let deps = self.sql.query_rows(
            &format!(
                "SELECT TicketId, DependsOnId FROM TicketDependencies WHERE TicketId IN ({ids}) ORDER BY TicketId, Position"
            ),
            params,
            |row| Ok((row.get::<_, String>("TicketId")?, row.get::<_, String>("DependsOnId")?)),
        )?;
        let mut labels_of = RowGroups::by(&labels, |r| r.0.clone(), |r| r.1.clone());
        let mut deps_of = RowGroups::by(&deps, |r| r.0.clone(), |r| r.1.clone());
        Ok(rows
            .into_iter()
            .map(|mut ticket| {
                ticket.labels = labels_of.remove(&ticket.id).unwrap_or_default();
                ticket.depends_on = deps_of.remove(&ticket.id).unwrap_or_default();
                ticket
            })
            .collect())
    }

    /// Schreibt Labels und Voraussetzungen mit ihrer Position
    fn write_children(&mut self, ticket: &Ticket) -> Result<()> {
        for (position, label) in ticket.labels.iter().enumerate() {
            self.sql.execute(
                "INSERT INTO TicketLabels (TicketId, Position, Label) VALUES (@id, @position, @label)",
                &sqlite_params! {
                    "id": ticket.id.clone(),
                    "position": i64::try_from(position).unwrap_or_default(),
                    "label": label.clone(),
                },
            )?;
        }
        for (position, dep) in ticket.depends_on.iter().enumerate() {
            self.sql.execute(
                "INSERT INTO TicketDependencies (TicketId, Position, DependsOnId) VALUES (@id, @position, @dep)",
                &sqlite_params! {
                    "id": ticket.id.clone(),
                    "position": i64::try_from(position).unwrap_or_default(),
                    "dep": dep.clone(),
                },
            )?;
        }
        Ok(())
    }
}

/// Parameter `@<prefix>0`, `@<prefix>1` … für eine IN-Liste; liefert die Liste für das SQL
fn named<T: Clone + Into<SqlValue>>(params: &mut Params, prefix: &str, values: &[T]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            params.push((format!("{prefix}{index}").into(), value.clone().into()));
            format!("@{prefix}{index}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Eine Zeile aus `Tickets` als Fachobjekt; Labels und Voraussetzungen ergänzt [`SqliteUnitOfWork::load`]
fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Ticket> {
    Ok(Ticket {
        id: row.get("Id")?,
        project_id: row.get("ProjectId")?,
        board_id: row.get("BoardId")?,
        number: row.get("Number")?,
        title: row.get("Title")?,
        description: row.get("Description")?,
        status: parsed(row, "Status")?,
        priority: parsed(row, "Priority")?,
        labels: Vec::new(),
        depends_on: Vec::new(),
        sort_order: row.get("SortOrder")?,
        assignee: row.get("Assignee")?,
        needs_planning: bool_of(row.get::<_, i64>("NeedsPlanning")?),
        acceptance: row.get("Acceptance")?,
        plan: row.get("Plan")?,
        handbook: row.get("Handbook")?,
        diagram: row.get("Diagram")?,
        spec: row.get("Spec")?,
        spec_task: row.get("SpecTask")?,
        branch: row.get("Branch")?,
        worktree: row.get("Worktree")?,
        target_branch: row.get("TargetBranch")?,
        touches: json_list(row.get("Touches")?),
        consumes: json_list(row.get("Consumes")?),
        produces: json_list(row.get("Produces")?),
        blocked_reason: row.get("BlockedReason")?,
        planned_at: iso(row.get("PlannedAt")?),
        planned_by: row.get("PlannedBy")?,
        planning_claim_by: row.get("PlanningClaimBy")?,
        planning_claim_at: iso(row.get("PlanningClaimAt")?),
        created_at: iso(row.get("CreatedAt")?).unwrap_or_default(),
        created_by_id: guid(&row.get::<_, String>("CreatedById")?),
        created_by_name: row.get("CreatedByName")?,
        updated_at: iso(row.get("UpdatedAt")?).unwrap_or_default(),
        rev: row.get("Rev")?,
    })
}

/// Eine Spalte als Aufzählung; eine unbekannte Schreibweise ist ein Lesefehler
fn parsed<T: std::str::FromStr>(row: &rusqlite::Row<'_>, column: &str) -> rusqlite::Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let text: String = row.get(column)?;
    text.parse::<T>().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

/// JSON-Liste aus der DB; Unlesbares ergibt eine leere Liste
fn json_list(json: Option<String>) -> Vec<String> {
    let text = json.unwrap_or_else(|| "[]".to_string());
    let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(&text) else {
        return Vec::new();
    };
    values
        .into_iter()
        .filter_map(|value| match value {
            serde_json::Value::String(text) => Some(text),
            _ => None,
        })
        .collect()
}

/// Ein Zeitpunkt als Parameter; `None` bleibt NULL
fn opt_date(value: Option<&String>) -> SqlValue {
    value.map_or(SqlValue::Null, |text| date(text))
}

/// Parameter eines Tickets für INSERT und UPDATE
fn ticket_params(t: &Ticket) -> Params {
    sqlite_params! {
        "id": t.id.clone(),
        "projectId": t.project_id,
        "boardId": t.board_id,
        "number": t.number,
        "title": t.title.clone(),
        "description": t.description.clone(),
        "status": t.status.as_str(),
        "priority": t.priority.as_str(),
        "sortOrder": t.sort_order,
        "assignee": t.assignee.clone(),
        "needsPlanning": t.needs_planning,
        "acceptance": t.acceptance.clone(),
        "plan": t.plan.clone(),
        "handbook": t.handbook.clone(),
        "diagram": t.diagram.clone(),
        "spec": t.spec.clone(),
        "specTask": t.spec_task.clone(),
        "branch": t.branch.clone(),
        "worktree": t.worktree.clone(),
        "targetBranch": t.target_branch.clone(),
        "touches": json_text(&t.touches),
        "consumes": json_text(&t.consumes),
        "produces": json_text(&t.produces),
        "blockedReason": t.blocked_reason.clone(),
        "plannedAt": opt_date(t.planned_at.as_ref()),
        "plannedBy": t.planned_by.clone(),
        "planningClaimBy": t.planning_claim_by.clone(),
        "planningClaimAt": opt_date(t.planning_claim_at.as_ref()),
        "createdAt": date(&t.created_at),
        "createdById": t.created_by_id.to_lowercase(),
        "createdByName": t.created_by_name.clone(),
        "updatedAt": date(&t.updated_at),
        "rev": t.rev,
    }
}

/// Eine Liste als JSON-Text, wie `JSON.stringify` sie schreibt
fn json_text(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}
