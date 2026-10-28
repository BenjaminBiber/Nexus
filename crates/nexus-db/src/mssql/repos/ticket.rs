//! Tickets auf SQL Server samt Labels (`board.TicketLabels`) und Voraussetzungen
//! (`board.TicketDependencies`); portiert aus `packages/db/src/MssqlTicketRepository.ts`.
//!
//! Dieses Repository braucht das Feature-Gate: ohne die Erweiterung `target-branch` liest und
//! schreibt es Tickets ohne die Spalte `TargetBranch`, und nur ein gesetzter Zielbranch
//! verlangt das Skript (siehe `MssqlFeatureGate.test.ts`).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use async_trait::async_trait;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::database::{TicketFilter, TicketRepository};
use nexus_core::board::types::Ticket;
use nexus_core::{Error, HarnessError, Result};

use crate::mssql::runner::{MsRow, MsValue};
use crate::mssql::unit_of_work::MssqlUnitOfWork;
use crate::{RowGroups, SqlChunks};

/// Alle Spalten eines Tickets, mit dem Alias `t` der FROM-Klausel
const COLUMNS: &str = "t.Id, t.ProjectId, t.BoardId, t.Number, t.Title, t.Description, t.Status, t.Priority, t.SortOrder, t.Assignee,
  t.NeedsPlanning, t.Acceptance, t.[Plan], t.Handbook, t.Diagram, t.BlockedReason, t.PlannedAt, t.PlannedBy,
  t.PlanningClaimBy, t.PlanningClaimAt, t.CreatedAt, t.CreatedById, t.CreatedByName, t.UpdatedAt, t.Rev,
  t.Spec, t.SpecTask, t.Branch, t.Worktree, t.Touches, t.Consumes, t.Produces";

/// Die langen Texte, die `without_texts` nur noch als Kennzeichen liefert
const LONG_TEXTS: [&str; 5] = ["Description", "Acceptance", "[Plan]", "Handbook", "Diagram"];

/// Der Sperr-Hinweis, den `for_update` an die FROM-Klausel hängt
const LOCK_HINT: &str = "WITH (UPDLOCK, ROWLOCK)";

/// Wie [`COLUMNS`], aber die langen Texte nur als Kennzeichen (`filter.without_texts`):
/// leer bleibt leer, sonst `'x'`
static LIGHT_COLUMNS: LazyLock<String> = LazyLock::new(|| {
    LONG_TEXTS
        .iter()
        .fold(COLUMNS.to_string(), |columns, name| {
            columns.replace(
                &format!("t.{name}"),
                &format!("CASE WHEN LEN(LTRIM(t.{name})) > 0 THEN N'x' ELSE N'' END AS {name}"),
            )
        })
});

#[async_trait]
impl TicketRepository for MssqlUnitOfWork<'_> {
    /// `for_update` sperrt die Zeile bis zum Ende der Transaktion
    async fn by_id(&mut self, id: &str, for_update: bool) -> Result<Option<Ticket>> {
        let hint = if for_update { LOCK_HINT } else { "" };
        let found = self
            .load(
                &format!("FROM board.Tickets t {hint} WHERE t.Id = @id"),
                &[("id", MsValue::text(id))],
                false,
            )
            .await?;
        Ok(found.into_iter().next())
    }

    /// Sortiert nach Projekt und Nummer, damit die Reihenfolge stabil ist
    async fn list(&mut self, filter: &TicketFilter) -> Result<Vec<Ticket>> {
        let mut where_parts: Vec<String> = Vec::new();
        let mut params: Vec<(String, MsValue)> = Vec::new();
        if let Some(board_ids) = &filter.board_ids {
            if board_ids.is_empty() {
                return Ok(Vec::new());
            }
            let list = named(&mut params, "b", board_ids.iter().map(|b| MsValue::Int(*b)));
            where_parts.push(format!("t.BoardId IN ({list})"));
        }
        if let Some(ids) = &filter.ids {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let list = named(
                &mut params,
                "i",
                ids.iter().map(|id| MsValue::text(id.as_str())),
            );
            where_parts.push(format!("t.Id IN ({list})"));
        }
        let clause = if where_parts.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_parts.join(" AND "))
        };
        let borrowed: Vec<(&str, MsValue)> = params
            .iter()
            .map(|(name, value)| (name.as_str(), value.clone()))
            .collect();
        self.load(
            &format!("FROM board.Tickets t {clause}"),
            &borrowed,
            filter.without_texts,
        )
        .await
    }

    async fn insert(&mut self, ticket: &Ticket) -> Result<()> {
        let target = self.has_target(Some(ticket))?;
        let spalte = if target { "TargetBranch, " } else { "" };
        let wert = if target { "@targetBranch, " } else { "" };
        self.sql
            .execute(
                &format!(
                    "INSERT INTO board.Tickets (Id, ProjectId, BoardId, Number, Title, Description, Status, Priority, SortOrder, Assignee,
         NeedsPlanning, Acceptance, [Plan], Handbook, Diagram, BlockedReason, PlannedAt, PlannedBy, PlanningClaimBy,
         PlanningClaimAt, CreatedAt, CreatedById, CreatedByName, UpdatedAt, Rev,
         Spec, SpecTask, Branch, Worktree, {spalte}Touches, Consumes, Produces)
       VALUES (@id, @projectId, @boardId, @number, @title, @description, @status, @priority, @sortOrder, @assignee,
         @needsPlanning, @acceptance, @plan, @handbook, @diagram, @blockedReason, @plannedAt, @plannedBy, @planningClaimBy,
         @planningClaimAt, @createdAt, @createdById, @createdByName, @updatedAt, @rev,
         @spec, @specTask, @branch, @worktree, {wert}@touches, @consumes, @produces)"
                ),
                &ticket_params(ticket)?,
            )
            .await?;
        self.write_children(ticket).await
    }

    /// Schreibt nur, wenn in der DB noch die Vorgängerversion steht
    async fn update(&mut self, ticket: &Ticket) -> Result<()> {
        let target = self.has_target(Some(ticket))?;
        let spalte = if target {
            "TargetBranch = @targetBranch, "
        } else {
            ""
        };
        let mut params = ticket_params(ticket)?;
        params.push(("previousRev", MsValue::Int(ticket.rev - 1)));
        let affected = self
            .sql
            .execute(
                &format!(
                    "UPDATE board.Tickets SET BoardId = @boardId, Title = @title, Description = @description, Status = @status,
         Priority = @priority, SortOrder = @sortOrder, Assignee = @assignee, NeedsPlanning = @needsPlanning,
         Acceptance = @acceptance, [Plan] = @plan, Handbook = @handbook, Diagram = @diagram, BlockedReason = @blockedReason,
         PlannedAt = @plannedAt, PlannedBy = @plannedBy, PlanningClaimBy = @planningClaimBy, PlanningClaimAt = @planningClaimAt,
         Spec = @spec, SpecTask = @specTask, Branch = @branch, Worktree = @worktree, {spalte}Touches = @touches, Consumes = @consumes, Produces = @produces,
         UpdatedAt = @updatedAt, Rev = @rev
       WHERE Id = @id AND Rev = @previousRev"
                ),
                &params,
            )
            .await?;
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
        self.sql
            .execute(
                "DELETE FROM board.TicketLabels WHERE TicketId = @id; DELETE FROM board.TicketDependencies WHERE TicketId = @id;",
                &[("id", MsValue::text(ticket.id.as_str()))],
            )
            .await?;
        self.write_children(ticket).await
    }

    /// Labels, eigene Voraussetzungen, Kommentare und Verlauf fallen per Kaskade mit
    async fn delete(&mut self, id: &str) -> Result<()> {
        self.sql
            .execute(
                "DELETE FROM board.TicketDependencies WHERE DependsOnId = @id; DELETE FROM board.Tickets WHERE Id = @id;",
                &[("id", MsValue::text(id))],
            )
            .await?;
        Ok(())
    }

    /// Sortiert in Rust, damit die Reihenfolge nicht von der DB-Collation abhängt
    async fn dependents(&mut self, id: &str) -> Result<Vec<String>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT TicketId FROM board.TicketDependencies WHERE DependsOnId = @id",
                &[("id", MsValue::text(id))],
            )
            .await?;
        sorted_ids(&rows)
    }

    async fn remove_dependency_on(&mut self, id: &str) -> Result<Vec<String>> {
        let rows = self
            .sql
            .query_rows(
                "DELETE FROM board.TicketDependencies OUTPUT deleted.TicketId WHERE DependsOnId = @id",
                &[("id", MsValue::text(id))],
            )
            .await?;
        sorted_ids(&rows)
    }

    async fn max_sort_order(&mut self, board_id: i32, status: TicketStatus) -> Result<Option<i32>> {
        let rows = self
            .sql
            .query_rows(
                "SELECT MAX(SortOrder) AS M FROM board.Tickets WHERE BoardId = @boardId AND Status = @status",
                &[
                    ("boardId", MsValue::Int(board_id)),
                    ("status", MsValue::text(status.as_str())),
                ],
            )
            .await?;
        match rows.first() {
            Some(row) => row.get_opt_i32("M"),
            None => Ok(None),
        }
    }

    async fn open_count_by_board(&mut self, board_ids: &[i32]) -> Result<BTreeMap<i32, i64>> {
        if board_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut counts = BTreeMap::new();
        for chunk in SqlChunks::of(board_ids) {
            let (list, chunk_params) = SqlChunks::params("b", chunk);
            let params: Vec<(String, MsValue)> = chunk_params
                .into_iter()
                .map(|(name, value)| (name, MsValue::Int(value)))
                .collect();
            let borrowed: Vec<(&str, MsValue)> = params
                .iter()
                .map(|(name, value)| (name.as_str(), value.clone()))
                .collect();
            let rows = self
                .sql
                .query_rows(
                    &format!(
                        "SELECT BoardId, COUNT(*) AS N FROM board.Tickets
       WHERE BoardId IN ({list}) AND Status NOT IN ('done', 'not_needed') GROUP BY BoardId"
                    ),
                    &borrowed,
                )
                .await?;
            for row in &rows {
                counts.insert(row.get_i32("BoardId")?, row.get_i64("N")?);
            }
        }
        Ok(counts)
    }
}

impl MssqlUnitOfWork<'_> {
    /// Ob die Spalte `TargetBranch` da ist; ein gesetzter Zielbranch verlangt sie
    fn has_target(&self, ticket: Option<&Ticket>) -> Result<bool> {
        if self.gate.has("target-branch") {
            return Ok(true);
        }
        if ticket.is_some_and(|t| t.target_branch.as_deref().is_some_and(|b| !b.is_empty())) {
            self.gate.require(&["target-branch"])?;
        }
        Ok(false)
    }

    /// Lädt Tickets zu einer FROM/WHERE-Klausel und ergänzt Labels und Voraussetzungen
    async fn load(
        &mut self,
        from: &str,
        params: &[(&str, MsValue)],
        without_texts: bool,
    ) -> Result<Vec<Ticket>> {
        let columns: &str = if without_texts {
            LIGHT_COLUMNS.as_str()
        } else {
            COLUMNS
        };
        let target = if self.has_target(None)? {
            "t.TargetBranch"
        } else {
            "CAST(NULL AS nvarchar(400)) AS TargetBranch"
        };
        let rows = self
            .sql
            .query_rows(
                &format!("SELECT {columns}, {target} {from} ORDER BY t.ProjectId, t.Number"),
                params,
            )
            .await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        // Der Sperr-Hinweis gehört nicht in die Unterabfrage – genauso macht es TypeScript
        let ids = format!("SELECT t.Id {}", from.replace(LOCK_HINT, ""));
        let labels = self
            .sql
            .query_rows(
                &format!(
                    "SELECT TicketId, Label FROM board.TicketLabels WHERE TicketId IN ({ids}) ORDER BY TicketId, Position"
                ),
                params,
            )
            .await?;
        let deps = self
            .sql
            .query_rows(
                &format!(
                    "SELECT TicketId, DependsOnId FROM board.TicketDependencies WHERE TicketId IN ({ids}) ORDER BY TicketId, Position"
                ),
                params,
            )
            .await?;
        let labels = paare(&labels, "Label")?;
        let deps = paare(&deps, "DependsOnId")?;
        let mut labels_of = RowGroups::by(&labels, |r| r.0.clone(), |r| r.1.clone());
        let mut deps_of = RowGroups::by(&deps, |r| r.0.clone(), |r| r.1.clone());
        rows.iter()
            .map(|row| {
                let mut ticket = map_row(row)?;
                ticket.labels = labels_of.remove(&ticket.id).unwrap_or_default();
                ticket.depends_on = deps_of.remove(&ticket.id).unwrap_or_default();
                Ok(ticket)
            })
            .collect()
    }

    /// Schreibt Labels und Voraussetzungen mit ihrer Position
    async fn write_children(&mut self, ticket: &Ticket) -> Result<()> {
        for (position, label) in ticket.labels.iter().enumerate() {
            self.sql
                .execute(
                    "INSERT INTO board.TicketLabels (TicketId, Position, Label) VALUES (@id, @position, @label)",
                    &[
                        ("id", MsValue::text(ticket.id.as_str())),
                        ("position", MsValue::Int(i32::try_from(position).unwrap_or_default())),
                        ("label", MsValue::text(label.as_str())),
                    ],
                )
                .await?;
        }
        for (position, dep) in ticket.depends_on.iter().enumerate() {
            self.sql
                .execute(
                    "INSERT INTO board.TicketDependencies (TicketId, Position, DependsOnId) VALUES (@id, @position, @dep)",
                    &[
                        ("id", MsValue::text(ticket.id.as_str())),
                        ("position", MsValue::Int(i32::try_from(position).unwrap_or_default())),
                        ("dep", MsValue::text(dep.as_str())),
                    ],
                )
                .await?;
        }
        Ok(())
    }
}

/// Parameter `@<prefix>0`, `@<prefix>1` … für eine IN-Liste; liefert die Liste für das SQL
fn named(
    params: &mut Vec<(String, MsValue)>,
    prefix: &str,
    values: impl IntoIterator<Item = MsValue>,
) -> String {
    let start = params.len();
    for (index, value) in values.into_iter().enumerate() {
        params.push((format!("{prefix}{index}"), value));
    }
    params[start..]
        .iter()
        .map(|(name, _)| format!("@{name}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Die Ticket-IDs einer Ergebnismenge, in Rust sortiert
fn sorted_ids(rows: &[MsRow]) -> Result<Vec<String>> {
    let mut ids = rows
        .iter()
        .map(|row| row.get_str("TicketId"))
        .collect::<Result<Vec<_>>>()?;
    ids.sort();
    Ok(ids)
}

/// Ticket-ID und zweite Spalte je Zeile, als Paare für [`RowGroups`]
fn paare(rows: &[MsRow], column: &str) -> Result<Vec<(String, String)>> {
    rows.iter()
        .map(|row| Ok((row.get_str("TicketId")?, row.get_str(column)?)))
        .collect()
}

/// Eine Zeile aus `board.Tickets` als Fachobjekt; Labels und Voraussetzungen ergänzt
/// [`MssqlUnitOfWork::load`]
fn map_row(row: &MsRow) -> Result<Ticket> {
    Ok(Ticket {
        id: row.get_str("Id")?,
        project_id: row.get_i32("ProjectId")?,
        board_id: row.get_i32("BoardId")?,
        number: row.get_i32("Number")?,
        title: row.get_str("Title")?,
        description: row.get_str("Description")?,
        status: parsed(row, "Status")?,
        priority: parsed(row, "Priority")?,
        labels: Vec::new(),
        depends_on: Vec::new(),
        sort_order: row.get_i32("SortOrder")?,
        assignee: row.get_opt_str("Assignee")?,
        needs_planning: row.get_bool("NeedsPlanning")?,
        acceptance: row.get_str("Acceptance")?,
        plan: row.get_str("Plan")?,
        handbook: row.get_str("Handbook")?,
        diagram: row.get_str("Diagram")?,
        spec: row.get_opt_str("Spec")?,
        spec_task: row.get_opt_str("SpecTask")?,
        branch: row.get_opt_str("Branch")?,
        worktree: row.get_opt_str("Worktree")?,
        target_branch: row.get_opt_str("TargetBranch")?,
        touches: json_list(row.get_opt_str("Touches")?),
        consumes: json_list(row.get_opt_str("Consumes")?),
        produces: json_list(row.get_opt_str("Produces")?),
        blocked_reason: row.get_opt_str("BlockedReason")?,
        planned_at: row.get_opt_iso("PlannedAt")?,
        planned_by: row.get_opt_str("PlannedBy")?,
        planning_claim_by: row.get_opt_str("PlanningClaimBy")?,
        planning_claim_at: row.get_opt_iso("PlanningClaimAt")?,
        created_at: row.get_iso("CreatedAt")?,
        created_by_id: row.get_guid("CreatedById")?,
        created_by_name: row.get_str("CreatedByName")?,
        updated_at: row.get_iso("UpdatedAt")?,
        rev: row.get_i32("Rev")?,
    })
}

/// Eine Spalte als Aufzählung; eine unbekannte Schreibweise ist ein Lesefehler
fn parsed<T: std::str::FromStr>(row: &MsRow, column: &str) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    let text = row.get_str(column)?;
    text.parse::<T>().map_err(|error| {
        Error::unexpected_message(format!("Die Spalte {column} ist unlesbar: {error}"))
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

/// Eine Liste als JSON-Text, wie `JSON.stringify` sie schreibt
fn json_text(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

/// Parameter eines Tickets für INSERT und UPDATE
fn ticket_params(t: &Ticket) -> Result<Vec<(&'static str, MsValue)>> {
    Ok(vec![
        ("id", MsValue::text(t.id.as_str())),
        ("projectId", MsValue::Int(t.project_id)),
        ("boardId", MsValue::Int(t.board_id)),
        ("number", MsValue::Int(t.number)),
        ("title", MsValue::text(t.title.as_str())),
        ("description", MsValue::text(t.description.as_str())),
        ("status", MsValue::text(t.status.as_str())),
        ("priority", MsValue::text(t.priority.as_str())),
        ("sortOrder", MsValue::Int(t.sort_order)),
        ("assignee", MsValue::text_opt(t.assignee.as_deref())),
        ("needsPlanning", MsValue::Bit(t.needs_planning)),
        ("acceptance", MsValue::text(t.acceptance.as_str())),
        ("plan", MsValue::text(t.plan.as_str())),
        ("handbook", MsValue::text(t.handbook.as_str())),
        ("diagram", MsValue::text(t.diagram.as_str())),
        ("spec", MsValue::text_opt(t.spec.as_deref())),
        ("specTask", MsValue::text_opt(t.spec_task.as_deref())),
        ("branch", MsValue::text_opt(t.branch.as_deref())),
        ("worktree", MsValue::text_opt(t.worktree.as_deref())),
        (
            "targetBranch",
            MsValue::text_opt(t.target_branch.as_deref()),
        ),
        ("touches", MsValue::text(json_text(&t.touches))),
        ("consumes", MsValue::text(json_text(&t.consumes))),
        ("produces", MsValue::text(json_text(&t.produces))),
        (
            "blockedReason",
            MsValue::text_opt(t.blocked_reason.as_deref()),
        ),
        ("plannedAt", MsValue::date_opt(t.planned_at.as_deref())?),
        ("plannedBy", MsValue::text_opt(t.planned_by.as_deref())),
        (
            "planningClaimBy",
            MsValue::text_opt(t.planning_claim_by.as_deref()),
        ),
        (
            "planningClaimAt",
            MsValue::date_opt(t.planning_claim_at.as_deref())?,
        ),
        ("createdAt", MsValue::date(&t.created_at)?),
        ("createdById", MsValue::text(t.created_by_id.as_str())),
        ("createdByName", MsValue::text(t.created_by_name.as_str())),
        ("updatedAt", MsValue::date(&t.updated_at)?),
        ("rev", MsValue::Int(t.rev)),
    ])
}
