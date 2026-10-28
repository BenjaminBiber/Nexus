//! Tickets anlegen, lesen, ändern, verschieben, löschen, übernehmen und freigeben (Regeln des
//! alten Boards). Portiert aus `packages/core/src/board/TicketService.ts`.
//!
//! # Aufteilung
//!
//! [`TicketService`] hält Datenbank und Uhr und öffnet je Aktion eine Transaktion – wie die
//! Klasse in TypeScript. Was dort als öffentliche Methode *innerhalb* einer fremden Transaktion
//! aufgerufen wird (`load`, `save`, `view`, `views`, `createIn` …), steht hier als freie Funktion
//! des Moduls: der Dienst ist über die Datenbank generisch, die Helfer sind es nicht, und
//! `ticket_service::load(u, …)` erspart allen Aufrufern (Kommentare, Board-Ansicht, Notizen) das
//! Nennen des Datenbanktyps.
//!
//! # Prioritäten
//!
//! `requirePriority` aus TypeScript fehlt: in Rust ist die Priorität ein Enum, die Prüfung mit
//! dem Fehlercode `priority-invalid` macht `Priority::from_str` am Rand (CLI, HTTP).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use crate::board::basics::{
    is_human_interaction, BoardActor, Priority, TicketStatus, HUMAN_INTERACTION_PREFIX,
    MAX_BLOCKED_REASON_LENGTH, MAX_BRANCH_LENGTH, MAX_PERSON_NAME_LENGTH, MAX_SESSION_ID_LENGTH,
    MAX_SPEC_LENGTH, MAX_SPEC_TASK_LENGTH, MAX_STEP_TEXT_LENGTH, MAX_TITLE_LENGTH,
    MAX_WORKTREE_LENGTH,
};
use crate::board::board_service::BoardService;
use crate::board::database::{BoardDatabase, BoardUnitOfWork, TicketFilter};
use crate::board::dependency_cleanup::DependencyCleanup;
use crate::board::dependency_graph::DependencyGraph;
use crate::board::dependency_rules::{DependencyGraphMap, DependencyRules};
use crate::board::labels::BoardLabels;
use crate::board::limits::BoardLimits;
use crate::board::question_blocking::BoardQuestionBlocking;
use crate::board::question_ticket_cleanup::{DeletedTicket, QuestionTicketCleanup};
use crate::board::question_types::BoardOpenQuestion;
use crate::board::ticket_history_diff::TicketHistoryDiff;
use crate::board::ticket_ids::TicketIds;
use crate::board::ticket_ordering::{BoardColumn, TicketOrdering};
use crate::board::ticket_step_edits::{
    StepChange, StepEdit, StepGuard, StepPosition, TicketStepEdits,
};
use crate::board::types::{Comment, Deletion, HistoryEntry, Ticket, TicketStep};
use crate::clock::Clock;
use crate::error::{Error, HarnessError, Result};
use crate::tx;

// ---------------------------------------------------------------------------------------------
// Eingabe- und Ausgabeformen
// ---------------------------------------------------------------------------------------------

/// Inhalt eines neuen Tickets; weggelassene Felder bekommen den Standardwert
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewTicketInput {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TicketStatus>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
    pub depends_on: Option<Vec<String>>,
    pub acceptance: Option<String>,
    pub needs_planning: Option<bool>,
    pub spec: Option<String>,
    pub spec_task: Option<String>,
    pub target_branch: Option<String>,
    pub touches: Option<Vec<String>>,
    pub consumes: Option<Vec<String>>,
    pub produces: Option<Vec<String>>,
    pub steps: Option<Vec<TicketStep>>,
}

impl NewTicketInput {
    /// Ein Ticket, von dem nur der Titel feststeht
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
}

/// Änderung an einem Ticket. `None` lässt das Feld unverändert; bei Feldern, die auch leer sein
/// dürfen, bedeutet `Some(None)` „leeren“ (das `null` aus TypeScript).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TicketPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub plan: Option<String>,
    pub handbook: Option<String>,
    pub diagram: Option<String>,
    pub status: Option<TicketStatus>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
    pub assignee: Option<Option<String>>,
    pub needs_planning: Option<bool>,
    pub blocked_reason: Option<Option<String>>,
    pub acceptance: Option<String>,
    pub depends_on: Option<Vec<String>>,
    pub depends_on_add: Option<Vec<String>>,
    pub depends_on_remove: Option<Vec<String>>,
    pub board_id: Option<i32>,
    pub spec: Option<Option<String>>,
    pub spec_task: Option<Option<String>>,
    pub branch: Option<Option<String>>,
    pub worktree: Option<Option<String>>,
    pub target_branch: Option<Option<String>>,
    pub touches: Option<Vec<String>>,
    pub consumes: Option<Vec<String>>,
    pub produces: Option<Vec<String>>,
    pub steps: Option<Vec<TicketStep>>,
}

impl TicketPatch {
    /// Nennt die Änderung kein einziges Feld? (`Object.values(patch).every(…)` in TypeScript)
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// `force`: Abhängigkeits- und not_needed-Sperre übergehen; `allow_duplicate`: beim Anlegen ein
/// offenes Ticket mit gleichem Titel zulassen; `by_user`: Aktion eines Menschen in der Oberfläche
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteOptions {
    pub rev: Option<i32>,
    pub force: bool,
    pub allow_duplicate: bool,
    pub by_user: bool,
}

/// Wie [`WriteOptions`], dazu die Abnahme, die beim Verschieben nach Erledigt gleich mitkommt
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MoveOptions {
    pub rev: Option<i32>,
    pub force: bool,
    pub by_user: bool,
    pub acceptance: Option<String>,
}

impl MoveOptions {
    /// Die Sperr-Optionen daraus
    fn write(&self) -> WriteOptions {
        WriteOptions {
            rev: self.rev,
            force: self.force,
            allow_duplicate: false,
            by_user: self.by_user,
        }
    }
}

/// Wie ein Ticket übernommen wird
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaimOptions {
    pub status: Option<TicketStatus>,
    pub force: bool,
    pub session_id: Option<String>,
    pub rev: Option<i32>,
}

/// Wohin ein Schritt kommt und was dabei unverändert sein muss
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AddStepOptions {
    pub detail: Option<String>,
    pub at: Option<i32>,
    pub rev: Option<i32>,
    pub expect_text: Option<String>,
}

/// Ein Ticket, wie die Oberfläche es zeigt: mit Spalte, Voraussetzungen, Abhängigen,
/// offenen Fragen, Schritten und Sessions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct TicketView {
    #[serde(flatten)]
    #[ts(flatten)]
    pub ticket: Ticket,
    pub column: BoardColumn,
    pub waiting: bool,
    pub depends_open: Vec<String>,
    /// Erledigte (oder nicht mehr nötige) Voraussetzungen mit noch gesetztem Branch:
    /// noch nicht gemergt
    pub depends_unmerged: Vec<String>,
    pub dependents: Vec<String>,
    /// Abhängige, deren letzte offene Voraussetzung dieses Ticket ist (werden mit seinem
    /// Erledigt bereit)
    pub unlocks: Vec<String>,
    /// Offene Fragen, die dieses Ticket betreffen (sperren In Arbeit, Erledigt, claim und
    /// next-planning)
    pub waiting_on_questions: Vec<BoardOpenQuestion>,
    /// Erledigte und alle Arbeitsschritte
    pub steps_done: i32,
    pub steps_total: i32,
    /// Verknüpfte Claude-Sessions in Reihenfolge der Verknüpfung
    pub sessions: Vec<String>,
}

impl std::ops::Deref for TicketView {
    type Target = Ticket;

    fn deref(&self) -> &Ticket {
        &self.ticket
    }
}

/// Ein Ticket mit Kommentaren, Verlauf und Schritten
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct TicketDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub view: TicketView,
    pub comments: Vec<Comment>,
    pub history: Vec<HistoryEntry>,
    pub steps: Vec<TicketStep>,
}

impl std::ops::Deref for TicketDetail {
    type Target = TicketView;

    fn deref(&self) -> &TicketView {
        &self.view
    }
}

/// Kurzangaben eines Tickets für Sessions, Favoriten und Meldungen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct TicketSummary {
    pub id: String,
    pub title: String,
    pub status: TicketStatus,
    pub board_id: i32,
}

/// Auswahl beim Auflisten der Tickets
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TicketListFilter {
    pub board_ids: Vec<i32>,
    pub columns: Option<Vec<BoardColumn>>,
    pub needs_planning: Option<bool>,
    pub assignee: Option<String>,
    pub label: Option<String>,
    pub q: Option<String>,
    /// Lange Texte nur als Kennzeichen ([`TicketFilter::without_texts`]); mit `q` wird trotzdem
    /// voll geladen (Suche in der Beschreibung)
    pub without_texts: bool,
}

// ---------------------------------------------------------------------------------------------
// Der Dienst
// ---------------------------------------------------------------------------------------------

/// Tickets eines Boards – anlegen, lesen, ändern, verschieben, löschen.
#[derive(Debug, Clone)]
pub struct TicketService<D: BoardDatabase> {
    db: Arc<D>,
    clock: Clock,
}

impl<D: BoardDatabase> TicketService<D> {
    /// Dienst auf dieser Datenbank mit dieser Uhr
    pub fn new(db: Arc<D>, clock: Clock) -> Self {
        Self { db, clock }
    }

    /// Legt ein Ticket auf dem Board an; ein offenes Ticket mit gleichem Titel auf dem Board
    /// lehnt es ab (`allow_duplicate` übergeht das)
    pub async fn create(
        &self,
        board_id: i32,
        input: &NewTicketInput,
        actor: &BoardActor,
        options: &WriteOptions,
    ) -> Result<TicketView> {
        let input = input.clone();
        let actor = actor.clone();
        let options = *options;
        let clock = self.clock.clone();
        tx!(self.db, [input, actor, clock], |u| {
            if !options.allow_duplicate {
                reject_duplicate(u, board_id, &input.title).await?;
            }
            let ticket = create_in(u, board_id, &input, &actor, &options, &clock).await?;
            view(u, ticket).await
        })
        .await
    }

    /// Ein Ticket mit Kommentaren, Verlauf und Schritten
    pub async fn get(&self, id: &str) -> Result<TicketDetail> {
        let id = id.to_string();
        tx!(self.db, [id], |u| {
            let ticket = load(u, &id, None, false).await?;
            detail(u, &ticket.id).await
        })
        .await
    }

    /// Tickets der Boards, gefiltert und in Anzeige-Reihenfolge
    pub async fn list(&self, filter: &TicketListFilter) -> Result<Vec<TicketView>> {
        let filter = filter.clone();
        tx!(self.db, [filter], |u| {
            let q = filter.q.as_ref().map(|text| text.to_lowercase());
            let tickets = u
                .tickets()
                .list(&TicketFilter {
                    board_ids: Some(filter.board_ids.clone()),
                    ids: None,
                    without_texts: filter.without_texts && q.is_none(),
                })
                .await?;
            let mut result: Vec<TicketView> = views(u, tickets)
                .await?
                .into_iter()
                .filter(|t| {
                    filter
                        .columns
                        .as_ref()
                        .is_none_or(|columns| columns.contains(&t.column))
                        && filter
                            .needs_planning
                            .is_none_or(|wanted| t.ticket.needs_planning == wanted)
                        && filter
                            .assignee
                            .as_ref()
                            .is_none_or(|wanted| t.ticket.assignee.as_ref() == Some(wanted))
                        && filter
                            .label
                            .as_ref()
                            .is_none_or(|wanted| t.ticket.labels.contains(wanted))
                        && q.as_ref().is_none_or(|q| {
                            format!(
                                "{} {} {}",
                                t.ticket.id, t.ticket.title, t.ticket.description
                            )
                            .to_lowercase()
                            .contains(q)
                        })
                })
                .collect();
            sort_views(&mut result);
            Ok(result)
        })
        .await
    }

    /// Ändert Felder; Statuswechsel unterliegen denselben Sperren wie `move_to`. Schritte
    /// werden als Ganzes ersetzt.
    pub async fn update(
        &self,
        id: &str,
        patch: &TicketPatch,
        actor: &BoardActor,
        options: &WriteOptions,
    ) -> Result<TicketView> {
        if patch.is_empty() {
            return Err(HarnessError::new("nothing-changed", "Keine Änderung angegeben.").into());
        }
        let id = id.to_string();
        let patch = patch.clone();
        let actor = actor.clone();
        let options = *options;
        let clock = self.clock.clone();
        tx!(self.db, [id, patch, actor, clock], |u| {
            let before = load(u, &id, options.rev, true).await?;
            let after = apply(u, &before, &patch).await?;
            let steps = match &patch.steps {
                Some(steps) => Some(clean_steps(steps)?),
                None => None,
            };
            check_status(u, &before, &after, &options).await?;
            let saved = save(u, &before, &after, &actor, None, &clock).await?;
            if let Some(steps) = steps {
                write_steps(u, &saved, &steps, &actor).await?;
            }
            view(u, saved).await
        })
        .await
    }

    /// Verschiebt in eine Spalte; außerhalb von Blockiert wird der Grund gelöscht
    /// (`move` in TypeScript)
    pub async fn move_to(
        &self,
        id: &str,
        status: TicketStatus,
        actor: &BoardActor,
        options: &MoveOptions,
    ) -> Result<TicketView> {
        let id = id.to_string();
        let actor = actor.clone();
        let options = options.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, actor, options, clock], |u| {
            let before = load(u, &id, options.rev, true).await?;
            let mut after = Ticket {
                status,
                acceptance: options
                    .acceptance
                    .clone()
                    .unwrap_or_else(|| before.acceptance.clone()),
                blocked_reason: if status == TicketStatus::Blocked {
                    before.blocked_reason.clone()
                } else {
                    None
                },
                ..before.clone()
            };
            if status != before.status {
                after.sort_order = next_order(u, before.board_id, status).await?;
            }
            if TicketHistoryDiff::diff(&before, &after, &actor, "", None).is_empty() {
                return view(u, before).await;
            }
            check_status(u, &before, &after, &options.write()).await?;
            let saved = save(u, &before, &after, &actor, Some("verschoben"), &clock).await?;
            view(u, saved).await
        })
        .await
    }

    /// Abnahme durch einen Menschen: nur Blockiert mit Grund „Human Interaction: …“; setzt
    /// Erledigt (Abnahme-Text Pflicht wie bei done), leert den Grund und vermerkt „abgenommen“
    /// im Verlauf
    pub async fn accept(
        &self,
        id: &str,
        actor: &BoardActor,
        options: &WriteOptions,
    ) -> Result<TicketView> {
        let id = id.to_string();
        let actor = actor.clone();
        let options = *options;
        let clock = self.clock.clone();
        tx!(self.db, [id, actor, clock], |u| {
            let before = load(u, &id, options.rev, true).await?;
            if before.status != TicketStatus::Blocked
                || !is_human_interaction(before.blocked_reason.as_deref())
            {
                return Err(HarnessError::new(
                    "accept-not-possible",
                    format!(
                        "{} wartet auf keine Abnahme (nur Blockiert mit Grund „{HUMAN_INTERACTION_PREFIX} …“).",
                        before.id
                    ),
                )
                .into());
            }
            let after = Ticket {
                status: TicketStatus::Done,
                blocked_reason: None,
                sort_order: next_order(u, before.board_id, TicketStatus::Done).await?,
                ..before.clone()
            };
            check_status(u, &before, &after, &options).await?;
            let saved = save(u, &before, &after, &actor, Some("abgenommen"), &clock).await?;
            view(u, saved).await
        })
        .await
    }

    /// Holt Tickets von anderen Boards desselben Projekts auf das Ziel-Board (eine Transaktion,
    /// alles oder nichts). Jedes Ticket kommt ans Ende seiner Spalte; der Verlauf nennt das
    /// Quell-Board.
    pub async fn move_to_board(
        &self,
        ids: &[String],
        target_board_id: i32,
        actor: &BoardActor,
    ) -> Result<Vec<TicketView>> {
        if ids.is_empty() {
            return Err(HarnessError::new("nothing-changed", "Keine Tickets angegeben.").into());
        }
        let ids = ids.to_vec();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [ids, actor, clock], |u| {
            BoardLimits::actor(&actor)?;
            let target = BoardService::<D>::require_in(u, target_board_id).await?;
            let mut wanted: Vec<String> = Vec::new();
            for id in &ids {
                let id = TicketIds::canonical(id)?;
                if !wanted.contains(&id) {
                    wanted.push(id);
                }
            }
            let mut tickets: Vec<Ticket> = Vec::new();
            for id in &wanted {
                tickets.push(load(u, id, None, true).await?);
            }
            for ticket in &tickets {
                if ticket.board_id == target.id {
                    return Err(HarnessError::new(
                        "ticket-already-on-board",
                        format!("{} liegt schon auf Board {}.", ticket.id, target.name),
                    )
                    .into());
                }
                if ticket.project_id != target.project_id {
                    return Err(HarnessError::new(
                        "board-other-project",
                        format!("{} gehört zu einem anderen Projekt. Tickets wechseln nur zwischen Boards desselben Projekts.", ticket.id),
                    )
                    .into());
                }
            }
            let mut source_names: HashMap<i32, String> = HashMap::new();
            let mut moved: Vec<Ticket> = Vec::new();
            for before in &tickets {
                let source_name = match source_names.get(&before.board_id) {
                    Some(name) => name.clone(),
                    None => {
                        let source = BoardService::<D>::require_in(u, before.board_id).await?;
                        source_names.insert(before.board_id, source.name.clone());
                        source.name
                    }
                };
                let after = Ticket {
                    board_id: target.id,
                    sort_order: next_order(u, target.id, before.status).await?,
                    ..before.clone()
                };
                let note = format!("von Board {source_name} geholt");
                moved.push(save(u, before, &after, &actor, Some(&note), &clock).await?);
            }
            views(u, moved).await
        })
        .await
    }

    /// Löscht endgültig, entfernt Verweise anderer Tickets und protokolliert die Löschung
    pub async fn delete(&self, id: &str, actor: &BoardActor) -> Result<()> {
        let id = id.to_string();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, actor, clock], |u| {
            BoardLimits::actor(&actor)?;
            let ticket = load(u, &id, None, true).await?;
            let at = clock.iso();
            DependencyCleanup::remove(u, &ticket.id, &actor, &at, &HashSet::new()).await?;
            QuestionTicketCleanup::remove(
                u,
                &DeletedTicket {
                    id: ticket.id.clone(),
                    title: ticket.title.clone(),
                },
                &actor,
                &at,
            )
            .await?;
            u.tickets().delete(&ticket.id).await?;
            u.deletions()
                .insert(&Deletion {
                    at: at.clone(),
                    by_id: actor.user_id.clone(),
                    by_name: actor.name.clone(),
                    ticket_id: ticket.id.clone(),
                    ticket_title: ticket.title.clone(),
                    from_status: Some(ticket.status),
                })
                .await
        })
        .await
    }

    /// Übernimmt ein Ticket für einen Agenten (Standard: In Arbeit); bereits zugewiesen (auch an
    /// denselben Namen) oder mit offener Voraussetzung nur mit `force`. Die Abnahme wird hier
    /// nicht geprüft. Mit `rev` nur, wenn das Ticket seitdem unverändert ist (sonst „conflict“).
    pub async fn claim(
        &self,
        id: &str,
        agent: &str,
        actor: &BoardActor,
        options: &ClaimOptions,
    ) -> Result<TicketView> {
        let name = agent.trim().to_string();
        if name.is_empty() {
            return Err(HarnessError::new(
                "agent-missing",
                "Für die Übernahme fehlt der Name des Agenten (--agent).",
            )
            .into());
        }
        let id = id.to_string();
        let actor = actor.clone();
        let options = options.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, name, actor, options, clock], |u| {
            BoardLimits::text(&name, MAX_PERSON_NAME_LENGTH, "Der Name des Agenten")?;
            let before = load(u, &id, options.rev, true).await?;
            // Auch bei gleichem Namen: mehrere Sitzungen heißen oft gleich, nur so bleibt die
            // Übernahme exklusiv
            if let Some(assignee) = &before.assignee {
                if !options.force {
                    return Err(HarnessError::new(
                        "conflict",
                        format!("{} ist bereits {assignee} zugewiesen. Ist es deins, einfach weitermachen; sonst nächstes freies Ticket nehmen oder nachfragen.", before.id),
                    )
                    .into());
                }
            }
            let status = options.status.unwrap_or(TicketStatus::InProgress);
            if status == TicketStatus::NotNeeded && !options.force {
                return Err(HarnessError::new(
                    "not-needed-user",
                    "„Nicht mehr nötig“ entscheidet der Nutzer.",
                )
                .into());
            }
            if !options.force {
                require_no_open_dependency(u, &before).await?;
            }
            let mut after = Ticket {
                assignee: Some(name.clone()),
                status,
                ..before.clone()
            };
            if status != before.status {
                after.sort_order = next_order(u, before.board_id, status).await?;
            }
            let saved = save(u, &before, &after, &actor, Some("übernommen"), &clock).await?;
            if let Some(session_id) = &options.session_id {
                add_session(u, &saved.id, session_id, &clock).await?;
            }
            view(u, saved).await
        })
        .await
    }

    /// Gibt ein Ticket frei; der Status ändert sich nur, wenn er angegeben ist (ohne Sperren)
    pub async fn release(
        &self,
        id: &str,
        actor: &BoardActor,
        status: Option<TicketStatus>,
    ) -> Result<TicketView> {
        let id = id.to_string();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, actor, clock], |u| {
            let before = load(u, &id, None, true).await?;
            let mut after = Ticket {
                assignee: None,
                status: status.unwrap_or(before.status),
                ..before.clone()
            };
            if after.status != before.status {
                after.sort_order = next_order(u, before.board_id, after.status).await?;
            }
            let saved = save(u, &before, &after, &actor, Some("freigegeben"), &clock).await?;
            view(u, saved).await
        })
        .await
    }

    /// Hakt einen Schritt ab (Nummer ab 1) oder nimmt den Haken zurück; erhöht die Version wie
    /// ein Kommentar. Mit `guard.rev` bzw. `guard.expect_text` nur, wenn Ticket bzw. Schritt
    /// seitdem unverändert sind (sonst „conflict“).
    pub async fn set_step(
        &self,
        id: &str,
        number: i32,
        done: bool,
        actor: &BoardActor,
        guard: &StepGuard,
    ) -> Result<TicketDetail> {
        let id = id.to_string();
        let actor = actor.clone();
        let guard = guard.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, actor, guard, clock], |u| {
            BoardLimits::actor(&actor)?;
            let before = load(u, &id, guard.rev, true).await?;
            let steps = u.steps().list(&before.id).await?;
            TicketStepEdits::expect(&before.id, &steps, number, guard.expect_text.as_deref())?;
            let step = steps[TicketStepEdits::index(&before.id, &steps, number)?].clone();
            if step.done != done {
                let at = clock.iso();
                u.steps().set_done(&before.id, number - 1, done).await?;
                u.tickets()
                    .update(&Ticket {
                        rev: before.rev + 1,
                        updated_at: at.clone(),
                        ..before.clone()
                    })
                    .await?;
                u.history()
                    .insert(&[HistoryEntry {
                        ticket_id: before.id.clone(),
                        at,
                        by_id: actor.user_id.clone(),
                        by_name: actor.name.clone(),
                        field: "step".into(),
                        from: Some(format!("{number}: {}", flag(step.done))),
                        to: Some(format!("{number}: {}", flag(done))),
                        note: Some(crate::text::utf16_prefix(&step.text, 200).to_string()),
                    }])
                    .await?;
            }
            detail(u, &before.id).await
        })
        .await
    }

    /// Hängt einen Schritt an (oder fügt ihn an Position `at` ab 1 ein); erhöht die Version und
    /// schreibt den Verlauf. `expect_text` prüft den Schritt, der bisher an Position `at` steht
    /// (nur mit `at`); `rev` die Ticket-Version.
    pub async fn add_step(
        &self,
        id: &str,
        text: &str,
        actor: &BoardActor,
        options: &AddStepOptions,
    ) -> Result<TicketDetail> {
        let text = text.to_string();
        let options = options.clone();
        self.change_steps(
            id,
            actor,
            &StepGuard {
                rev: options.rev,
                expect_text: None,
            },
            move |ticket_id, steps| {
                let position = StepPosition {
                    detail: options.detail.clone(),
                    at: options.at,
                };
                if let Some(at) = position.at {
                    TicketStepEdits::expect(ticket_id, steps, at, options.expect_text.as_deref())?;
                }
                TicketStepEdits::add(ticket_id, steps, &text, &position)
            },
        )
        .await
    }

    /// Ändert Text und/oder Anleitung eines Schritts (Nummer ab 1); der Haken bleibt
    pub async fn edit_step(
        &self,
        id: &str,
        number: i32,
        change: &StepChange,
        actor: &BoardActor,
        guard: &StepGuard,
    ) -> Result<TicketDetail> {
        let change = change.clone();
        let expect = guard.expect_text.clone();
        self.change_steps(id, actor, guard, move |ticket_id, steps| {
            TicketStepEdits::expect(ticket_id, steps, number, expect.as_deref())?;
            TicketStepEdits::edit(ticket_id, steps, number, &change)
        })
        .await
    }

    /// Löscht einen Schritt (Nummer ab 1); die folgenden rücken nach
    pub async fn remove_step(
        &self,
        id: &str,
        number: i32,
        actor: &BoardActor,
        guard: &StepGuard,
    ) -> Result<TicketDetail> {
        let expect = guard.expect_text.clone();
        self.change_steps(id, actor, guard, move |ticket_id, steps| {
            TicketStepEdits::expect(ticket_id, steps, number, expect.as_deref())?;
            TicketStepEdits::remove(ticket_id, steps, number)
        })
        .await
    }

    /// Verschiebt einen Schritt von Nummer `from` auf Nummer `to`; `expect_text` prüft den
    /// Schritt an Nummer `from`
    pub async fn move_step(
        &self,
        id: &str,
        from: i32,
        to: i32,
        actor: &BoardActor,
        guard: &StepGuard,
    ) -> Result<TicketDetail> {
        let expect = guard.expect_text.clone();
        self.change_steps(id, actor, guard, move |ticket_id, steps| {
            TicketStepEdits::expect(ticket_id, steps, from, expect.as_deref())?;
            TicketStepEdits::move_step(ticket_id, steps, from, to)
        })
        .await
    }

    /// Verknüpft eine Claude-Session mit dem Ticket (ohne neue Version); `false`, wenn sie schon
    /// verknüpft war
    pub async fn link_session(&self, id: &str, session_id: &str) -> Result<bool> {
        let id = id.to_string();
        let session_id = session_id.to_string();
        let clock = self.clock.clone();
        tx!(self.db, [id, session_id, clock], |u| {
            let ticket = load(u, &id, None, false).await?;
            add_session(u, &ticket.id, &session_id, &clock).await
        })
        .await
    }

    /// Prüft, ob das Ticket nach In Arbeit dürfte (für `harness board start`, bevor ein Worktree
    /// entsteht)
    pub async fn check_start(&self, id: &str, force: bool) -> Result<TicketView> {
        let id = id.to_string();
        tx!(self.db, [id], |u| {
            let ticket = load(u, &id, None, false).await?;
            if ticket.status != TicketStatus::InProgress {
                let wanted = Ticket {
                    status: TicketStatus::InProgress,
                    ..ticket.clone()
                };
                guard_status(
                    u,
                    &wanted,
                    &WriteOptions {
                        force,
                        ..WriteOptions::default()
                    },
                )
                .await?;
            }
            view(u, ticket).await
        })
        .await
    }

    /// Kurzangaben zu Tickets in der Reihenfolge der Anfrage; unbekannte und ungültige IDs fehlen
    pub async fn summaries(&self, ids: &[String]) -> Result<Vec<TicketSummary>> {
        let wanted: Vec<String> = ids
            .iter()
            .filter_map(|id| TicketIds::canonical(id).ok())
            .collect();
        if wanted.is_empty() {
            return Ok(Vec::new());
        }
        tx!(self.db, [wanted], |u| {
            let mut unique: Vec<String> = Vec::new();
            for id in &wanted {
                if !unique.contains(id) {
                    unique.push(id.clone());
                }
            }
            let found: HashMap<String, Ticket> = u
                .tickets()
                .list(&TicketFilter {
                    ids: Some(unique),
                    ..TicketFilter::default()
                })
                .await?
                .into_iter()
                .map(|ticket| (ticket.id.clone(), ticket))
                .collect();
            Ok(wanted
                .iter()
                .filter_map(|id| found.get(id))
                .map(|ticket| TicketSummary {
                    id: ticket.id.clone(),
                    title: ticket.title.clone(),
                    status: ticket.status,
                    board_id: ticket.board_id,
                })
                .collect())
        })
        .await
    }

    /// Tickets, mit denen eine Session verknüpft ist (sortiert)
    pub async fn tickets_of_session(&self, session_id: &str) -> Result<Vec<String>> {
        let session_id = session_id.to_string();
        tx!(self.db, [session_id], |u| {
            u.ticket_sessions().tickets_of(&session_id).await
        })
        .await
    }

    /// Eine Schritt-Änderung in einer Transaktion: Liste lesen, ändern, ersetzen; bei Änderung
    /// neue Version und Verlauf. Mit `guard.rev` nur, wenn das Ticket seitdem unverändert ist
    /// (sonst „conflict“).
    async fn change_steps(
        &self,
        id: &str,
        actor: &BoardActor,
        guard: &StepGuard,
        edit: impl Fn(&str, &[TicketStep]) -> Result<StepEdit> + Send + Sync + 'static,
    ) -> Result<TicketDetail> {
        let id = id.to_string();
        let actor = actor.clone();
        let guard = guard.clone();
        let clock = self.clock.clone();
        let edit = Arc::new(edit);
        tx!(self.db, [id, actor, guard, clock, edit], |u| {
            BoardLimits::actor(&actor)?;
            let before = load(u, &id, guard.rev, true).await?;
            let current = u.steps().list(&before.id).await?;
            let result = edit(&before.id, &current)?;
            if result.steps != current {
                let at = clock.iso();
                u.steps().replace(&before.id, &result.steps).await?;
                u.tickets()
                    .update(&Ticket {
                        rev: before.rev + 1,
                        updated_at: at.clone(),
                        ..before.clone()
                    })
                    .await?;
                u.history()
                    .insert(&[HistoryEntry {
                        ticket_id: before.id.clone(),
                        at,
                        by_id: actor.user_id.clone(),
                        by_name: actor.name.clone(),
                        field: "step".into(),
                        from: result.from.clone(),
                        to: result.to.clone(),
                        note: Some(result.note.clone()),
                    }])
                    .await?;
            }
            detail(u, &before.id).await
        })
        .await
    }
}

// ---------------------------------------------------------------------------------------------
// Helfer innerhalb einer laufenden Transaktion
// ---------------------------------------------------------------------------------------------

/// Liest ein Ticket (Schreibweise egal) und prüft die erwartete Version
pub async fn load(
    u: &mut dyn BoardUnitOfWork,
    id: &str,
    rev: Option<i32>,
    for_update: bool,
) -> Result<Ticket> {
    let canonical = TicketIds::canonical(id)?;
    let ticket = u
        .tickets()
        .by_id(&canonical, for_update)
        .await?
        .ok_or_else(|| {
            HarnessError::new(
                "ticket-not-found",
                format!("Ticket {canonical} gibt es nicht."),
            )
        })?;
    if let Some(rev) = rev {
        if rev != ticket.rev {
            return Err(HarnessError::new(
                "conflict",
                format!(
                    "Ticket {canonical} wurde inzwischen geändert (Version {}, erwartet {rev}). Bitte neu lesen.",
                    ticket.rev
                ),
            )
            .into());
        }
    }
    Ok(ticket)
}

/// Schreibt die neue Fassung mit nächster Version und Zeitstempel und protokolliert die
/// Änderungen
pub async fn save(
    u: &mut dyn BoardUnitOfWork,
    before: &Ticket,
    after: &Ticket,
    actor: &BoardActor,
    note: Option<&str>,
    clock: &Clock,
) -> Result<Ticket> {
    BoardLimits::actor(actor)?;
    let at = clock.iso();
    let next = Ticket {
        rev: before.rev + 1,
        updated_at: at.clone(),
        ..after.clone()
    };
    u.tickets().update(&next).await?;
    let entries = TicketHistoryDiff::diff(before, &next, actor, &at, note);
    if !entries.is_empty() {
        u.history().insert(&entries).await?;
    }
    Ok(next)
}

/// Anzeigefassung eines Tickets (Spalte, wartet, offene Voraussetzungen, Abhängige)
pub async fn view(u: &mut dyn BoardUnitOfWork, ticket: Ticket) -> Result<TicketView> {
    Ok(views(u, vec![ticket]).await?.remove(0))
}

/// Anzeigefassungen; Voraussetzungen werden über alle Boards aufgelöst, offene Fragen,
/// Schritte und Sessions je Ticket ergänzt
pub async fn views(u: &mut dyn BoardUnitOfWork, tickets: Vec<Ticket>) -> Result<Vec<TicketView>> {
    let all = u
        .tickets()
        .list(&TicketFilter {
            without_texts: true,
            ..TicketFilter::default()
        })
        .await?;
    let mut by_id: HashMap<String, Ticket> = all
        .iter()
        .map(|ticket| (ticket.id.clone(), ticket.clone()))
        .collect();
    for ticket in &tickets {
        by_id.insert(ticket.id.clone(), ticket.clone());
    }
    let ids: Vec<String> = tickets.iter().map(|ticket| ticket.id.clone()).collect();
    let questions = BoardQuestionBlocking::open_by_ticket(u, &ids).await?;
    let progress = if ids.is_empty() {
        Default::default()
    } else {
        u.steps().progress(&ids).await?
    };
    let sessions = if ids.is_empty() {
        Default::default()
    } else {
        u.ticket_sessions().by_tickets(&ids).await?
    };
    let known: Vec<Ticket> = by_id.values().cloned().collect();

    Ok(tickets
        .into_iter()
        .map(|ticket| {
            let waiting = DependencyRules::is_waiting(&ticket, &by_id);
            let mut dependents: Vec<String> = all
                .iter()
                .filter(|other| other.depends_on.contains(&ticket.id))
                .map(|other| other.id.clone())
                .collect();
            dependents.sort();
            TicketView {
                column: TicketOrdering::column(&ticket, waiting),
                waiting,
                depends_open: DependencyRules::open_dependencies(&ticket, &by_id),
                depends_unmerged: DependencyRules::unmerged_dependencies(&ticket, &by_id),
                dependents,
                unlocks: DependencyGraph::unlocked_by(&ticket.id, &known, &by_id),
                waiting_on_questions: questions.get(&ticket.id).cloned().unwrap_or_default(),
                steps_done: progress.get(&ticket.id).map_or(0, |steps| steps.done),
                steps_total: progress.get(&ticket.id).map_or(0, |steps| steps.total),
                sessions: sessions.get(&ticket.id).cloned().unwrap_or_default(),
                ticket,
            }
        })
        .collect())
}

/// Sortiert Anzeigefassungen in Anzeige-Reihenfolge
pub fn sort_views(views: &mut [TicketView]) {
    let waiting: HashSet<String> = views
        .iter()
        .filter(|view| view.waiting)
        .map(|view| view.ticket.id.clone())
        .collect();
    TicketOrdering::sort_by(views, |view| &view.ticket, &waiting);
}

/// Detailfassung: Anzeige plus Kommentare, Verlauf und Schritte
pub async fn detail(u: &mut dyn BoardUnitOfWork, id: &str) -> Result<TicketDetail> {
    let ticket = u
        .tickets()
        .by_id(id, false)
        .await?
        .ok_or_else(|| Error::unexpected_message(format!("Ticket {id} fehlt")))?;
    Ok(TicketDetail {
        view: view(u, ticket).await?,
        comments: u.comments().list(id).await?,
        history: u.history().list(id).await?,
        steps: u.steps().list(id).await?,
    })
}

/// Legt ein Ticket innerhalb einer laufenden Transaktion an (auch für Notizen)
pub async fn create_in(
    u: &mut dyn BoardUnitOfWork,
    board_id: i32,
    input: &NewTicketInput,
    actor: &BoardActor,
    options: &WriteOptions,
    clock: &Clock,
) -> Result<Ticket> {
    BoardLimits::actor(actor)?;
    let board = u.boards().by_id(board_id).await?.ok_or_else(|| {
        HarnessError::new(
            "board-not-found",
            format!("Board {board_id} gibt es nicht."),
        )
    })?;
    let project =
        u.projects().by_id(board.project_id).await?.ok_or_else(|| {
            Error::unexpected_message(format!("Projekt {} fehlt", board.project_id))
        })?;
    let title = require_title(&input.title)?;
    let priority = input.priority.unwrap_or(Priority::Normal);
    let status = input.status.unwrap_or(TicketStatus::Backlog);
    let depends_on = DependencyRules::normalize(input.depends_on.as_deref().unwrap_or_default())?;
    let steps = clean_steps(input.steps.as_deref().unwrap_or_default())?;
    let number = u.projects().take_ticket_number(project.id).await?;
    let id = TicketIds::format(&project.key, number as u32);
    if !depends_on.is_empty() {
        DependencyRules::validate(&id, &depends_on, &graph(u).await?)?;
    }
    let at = clock.iso();
    let ticket = Ticket {
        id: id.clone(),
        project_id: project.id,
        board_id,
        number,
        title,
        description: input.description.clone().unwrap_or_default(),
        status,
        priority,
        labels: BoardLabels::normalize(input.labels.as_deref().unwrap_or_default())?,
        depends_on,
        sort_order: next_order(u, board_id, status).await?,
        assignee: None,
        needs_planning: input.needs_planning.unwrap_or(true),
        acceptance: input.acceptance.clone().unwrap_or_default(),
        plan: String::new(),
        handbook: String::new(),
        diagram: String::new(),
        blocked_reason: None,
        spec: optional(input.spec.as_deref(), MAX_SPEC_LENGTH, "Die Spec")?,
        spec_task: optional(
            input.spec_task.as_deref(),
            MAX_SPEC_TASK_LENGTH,
            "Die Spec-Aufgabe",
        )?,
        branch: None,
        worktree: None,
        target_branch: crate::git::TargetBranchRule::new()
            .normalize(input.target_branch.as_deref())?,
        touches: clean_list(input.touches.as_deref().unwrap_or_default()),
        consumes: clean_list(input.consumes.as_deref().unwrap_or_default()),
        produces: clean_list(input.produces.as_deref().unwrap_or_default()),
        planned_at: None,
        planned_by: None,
        planning_claim_by: None,
        planning_claim_at: None,
        created_at: at.clone(),
        created_by_id: actor.user_id.clone(),
        created_by_name: actor.name.clone(),
        updated_at: at.clone(),
        rev: 1,
    };
    guard_status(u, &ticket, options).await?;
    u.tickets().insert(&ticket).await?;
    u.history()
        .insert(&[HistoryEntry {
            ticket_id: id,
            at,
            by_id: actor.user_id.clone(),
            by_name: actor.name.clone(),
            field: "status".into(),
            from: None,
            to: Some(status.as_str().to_string()),
            note: Some("angelegt".into()),
        }])
        .await?;
    if !steps.is_empty() {
        u.steps().replace(&ticket.id, &steps).await?;
    }
    Ok(ticket)
}

/// Sperren beim Statuswechsel: `not_needed` nur durch den Nutzer, `done` nur mit Abnahme,
/// `in_progress`/`done` nur ohne offene Voraussetzung (`force` übergeht die erste und die
/// letzte Sperre)
pub async fn check_status(
    u: &mut dyn BoardUnitOfWork,
    before: &Ticket,
    after: &Ticket,
    options: &WriteOptions,
) -> Result<()> {
    if before.status == after.status {
        return Ok(());
    }
    guard_status(u, after, options).await
}

/// Die Sperren für den Status eines Tickets (beim Wechsel und beim Anlegen)
pub async fn guard_status(
    u: &mut dyn BoardUnitOfWork,
    after: &Ticket,
    options: &WriteOptions,
) -> Result<()> {
    if after.status == TicketStatus::NotNeeded && !options.by_user && !options.force {
        return Err(HarnessError::new(
            "not-needed-user",
            "„Nicht mehr nötig“ entscheidet der Nutzer. Hältst du das Ticket für überflüssig, schreib einen Kommentar; --force nur auf ausdrückliche Anweisung.",
        )
        .into());
    }
    if after.status == TicketStatus::Done && after.acceptance.trim().is_empty() {
        return Err(HarnessError::new(
            "acceptance-missing",
            format!(
                "{} hat keine Abnahme. Mit --acceptance angeben, woran man sieht, dass es fertig ist.",
                after.id
            ),
        )
        .into());
    }
    if matches!(after.status, TicketStatus::InProgress | TicketStatus::Done) && !options.force {
        require_no_open_dependency(u, after).await?;
    }
    Ok(())
}

/// Wirft, wenn eine Voraussetzung oder eine Frage zum Ticket offen ist
pub async fn require_no_open_dependency(
    u: &mut dyn BoardUnitOfWork,
    ticket: &Ticket,
) -> Result<()> {
    let by_id: HashMap<String, Ticket> = u
        .tickets()
        .list(&TicketFilter {
            without_texts: true,
            ..TicketFilter::default()
        })
        .await?
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();
    let open = DependencyRules::open_dependencies(ticket, &by_id);
    if !open.is_empty() {
        return Err(HarnessError::new(
            "dependency-open",
            format!(
                "{} wartet auf {}. Erst die Voraussetzung erledigen.",
                ticket.id,
                open.join(", ")
            ),
        )
        .into());
    }
    let questions = BoardQuestionBlocking::open_by_ticket(u, std::slice::from_ref(&ticket.id))
        .await?
        .remove(&ticket.id)
        .unwrap_or_default();
    if !questions.is_empty() {
        return Err(HarnessError::new(
            "question-open",
            format!(
                "{} wartet auf Frage {}. Erst die Frage entscheiden lassen.",
                ticket.id,
                questions
                    .iter()
                    .map(|question| question.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .into());
    }
    Ok(())
}

/// Nächste SortOrder am Ende einer Spalte
pub async fn next_order(
    u: &mut dyn BoardUnitOfWork,
    board_id: i32,
    status: TicketStatus,
) -> Result<i32> {
    Ok(u.tickets()
        .max_sort_order(board_id, status)
        .await?
        .map_or(0, |max| max + 1))
}

/// Wirft „ticket-duplicate“, wenn auf dem Board ein offenes Ticket (nicht Erledigt, nicht
/// „Nicht mehr nötig“) denselben Titel trägt (getrimmt, ohne Rücksicht auf
/// Groß-/Kleinschreibung)
async fn reject_duplicate(u: &mut dyn BoardUnitOfWork, board_id: i32, title: &str) -> Result<()> {
    let wanted = title.trim().to_lowercase();
    if wanted.is_empty() {
        return Ok(());
    }
    let same = u
        .tickets()
        .list(&TicketFilter {
            board_ids: Some(vec![board_id]),
            ..TicketFilter::default()
        })
        .await?
        .into_iter()
        .find(|t| {
            !matches!(t.status, TicketStatus::Done | TicketStatus::NotNeeded)
                && t.title.trim().to_lowercase() == wanted
        });
    match same {
        Some(same) => Err(HarnessError::new(
            "ticket-duplicate",
            format!(
                "Ein offenes Ticket mit diesem Titel gibt es schon: {}.",
                same.id
            ),
        )
        .with_details(json!({ "existingId": same.id }))
        .into()),
        None => Ok(()),
    }
}

/// Wendet eine Änderung an und prüft die betroffenen Felder
async fn apply(
    u: &mut dyn BoardUnitOfWork,
    before: &Ticket,
    patch: &TicketPatch,
) -> Result<Ticket> {
    let mut after = before.clone();
    if let Some(title) = &patch.title {
        after.title = require_title(title)?;
    }
    if let Some(description) = &patch.description {
        after.description = description.clone();
    }
    if let Some(plan) = &patch.plan {
        after.plan = plan.clone();
    }
    if let Some(handbook) = &patch.handbook {
        after.handbook = handbook.clone();
    }
    if let Some(diagram) = &patch.diagram {
        after.diagram = diagram.clone();
    }
    if let Some(status) = patch.status {
        after.status = status;
    }
    if let Some(priority) = patch.priority {
        after.priority = priority;
    }
    if let Some(labels) = &patch.labels {
        after.labels = BoardLabels::normalize(labels)?;
    }
    if let Some(assignee) = &patch.assignee {
        after.assignee = optional(
            assignee.as_deref(),
            MAX_PERSON_NAME_LENGTH,
            "Der Name des Zuständigen",
        )?;
    }
    if let Some(needs_planning) = patch.needs_planning {
        after.needs_planning = needs_planning;
    }
    if before.status == TicketStatus::Blocked
        && after.status != TicketStatus::Blocked
        && patch.blocked_reason.is_none()
    {
        after.blocked_reason = None;
    }
    if let Some(reason) = &patch.blocked_reason {
        after.blocked_reason = optional(
            reason.as_deref(),
            MAX_BLOCKED_REASON_LENGTH,
            "Der Grund für Blockiert",
        )?;
    }
    if let Some(acceptance) = &patch.acceptance {
        after.acceptance = acceptance.clone();
    }
    if let Some(spec) = &patch.spec {
        after.spec = optional(spec.as_deref(), MAX_SPEC_LENGTH, "Die Spec")?;
    }
    if let Some(spec_task) = &patch.spec_task {
        after.spec_task = optional(
            spec_task.as_deref(),
            MAX_SPEC_TASK_LENGTH,
            "Die Spec-Aufgabe",
        )?;
    }
    if let Some(branch) = &patch.branch {
        after.branch = optional(branch.as_deref(), MAX_BRANCH_LENGTH, "Der Branch")?;
    }
    if let Some(worktree) = &patch.worktree {
        after.worktree = optional(
            worktree.as_deref(),
            MAX_WORKTREE_LENGTH,
            "Der Worktree-Pfad",
        )?;
    }
    if let Some(target_branch) = &patch.target_branch {
        after.target_branch =
            crate::git::TargetBranchRule::new().normalize(target_branch.as_deref())?;
        // Gesperrt wird nach dem Stand vor der Änderung (before.branch); ein Patch, der Branch
        // und Zielbranch zugleich setzt oder leert, darf also beides ändern, solange vorher kein
        // Branch gesetzt war
        if after.target_branch != before.target_branch {
            if let Some(branch) = &before.branch {
                return Err(HarnessError::new(
                    "target-branch-locked",
                    format!("{} läuft schon auf Branch {branch}; der Zielbranch lässt sich nur vor dem Start ändern.", before.id),
                )
                .into());
            }
        }
    }
    if let Some(touches) = &patch.touches {
        after.touches = clean_list(touches);
    }
    if let Some(consumes) = &patch.consumes {
        after.consumes = clean_list(consumes);
    }
    if let Some(produces) = &patch.produces {
        after.produces = clean_list(produces);
    }
    if patch.depends_on.is_some()
        || patch.depends_on_add.is_some()
        || patch.depends_on_remove.is_some()
    {
        let remove: HashSet<String> =
            DependencyRules::normalize(patch.depends_on_remove.as_deref().unwrap_or_default())?
                .into_iter()
                .collect();
        let base: Vec<String> = match &patch.depends_on {
            Some(list) => list.clone(),
            None => before
                .depends_on
                .iter()
                .cloned()
                .chain(patch.depends_on_add.clone().unwrap_or_default())
                .collect(),
        };
        after.depends_on = DependencyRules::normalize(&base)?
            .into_iter()
            .filter(|dep| !remove.contains(dep))
            .collect();
        if after.depends_on != before.depends_on {
            DependencyRules::validate(&before.id, &after.depends_on, &graph(u).await?)?;
        }
    }
    if let Some(board_id) = patch.board_id {
        if board_id != before.board_id {
            let target = u.boards().by_id(board_id).await?.ok_or_else(|| {
                HarnessError::new(
                    "board-not-found",
                    format!("Board {board_id} gibt es nicht."),
                )
            })?;
            if target.project_id != before.project_id {
                return Err(HarnessError::new(
                    "board-other-project",
                    format!("Board {} gehört zu einem anderen Projekt. Tickets wechseln nur zwischen Boards desselben Projekts.", target.id),
                )
                .into());
            }
            after.board_id = target.id;
        }
    }
    if after.status != before.status || after.board_id != before.board_id {
        after.sort_order = next_order(u, after.board_id, after.status).await?;
    }
    Ok(after)
}

/// Alle Tickets mit ihren Voraussetzungen (für die Kreisprüfung)
async fn graph(u: &mut dyn BoardUnitOfWork) -> Result<DependencyGraphMap> {
    Ok(u.tickets()
        .list(&TicketFilter {
            without_texts: true,
            ..TicketFilter::default()
        })
        .await?
        .into_iter()
        .map(|ticket| (ticket.id, ticket.depends_on))
        .collect())
}

/// Ersetzt die Schritte nur bei Änderung und vermerkt die neue Zahl im Verlauf
async fn write_steps(
    u: &mut dyn BoardUnitOfWork,
    ticket: &Ticket,
    steps: &[TicketStep],
    actor: &BoardActor,
) -> Result<()> {
    let current = u.steps().list(&ticket.id).await?;
    if current == steps {
        return Ok(());
    }
    u.steps().replace(&ticket.id, steps).await?;
    u.history()
        .insert(&[HistoryEntry {
            ticket_id: ticket.id.clone(),
            at: ticket.updated_at.clone(),
            by_id: actor.user_id.clone(),
            by_name: actor.name.clone(),
            field: "steps".into(),
            from: Some(format!("{} Schritte", current.len())),
            to: Some(format!("{} Schritte", steps.len())),
            note: None,
        }])
        .await?;
    Ok(())
}

/// Verknüpft eine Session innerhalb der Transaktion; leere oder zu lange Kennungen sind nicht
/// erlaubt
async fn add_session(
    u: &mut dyn BoardUnitOfWork,
    ticket_id: &str,
    session_id: &str,
    clock: &Clock,
) -> Result<bool> {
    let value = session_id.trim();
    if value.is_empty() {
        return Err(HarnessError::new("session-invalid", "Die Kennung der Session fehlt.").into());
    }
    BoardLimits::text(value, MAX_SESSION_ID_LENGTH, "Die Kennung der Session")?;
    u.ticket_sessions()
        .add(ticket_id, value, &clock.iso())
        .await
}

/// Titel ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
fn require_title(title: &str) -> Result<String> {
    let value = title.trim();
    if value.is_empty() {
        return Err(HarnessError::new("title-empty", "Der Titel darf nicht leer sein.").into());
    }
    BoardLimits::text(value, MAX_TITLE_LENGTH, "Der Titel")?;
    Ok(value.to_string())
}

/// Schritte ohne Leerraum am Rand; leerer Text oder zu lang ist nicht erlaubt, ein leeres Detail
/// entfällt
fn clean_steps(steps: &[TicketStep]) -> Result<Vec<TicketStep>> {
    steps
        .iter()
        .map(|step| {
            let text = step.text.trim();
            if text.is_empty() {
                return Err(
                    HarnessError::new("step-empty", "Ein Schritt braucht einen Text.").into(),
                );
            }
            BoardLimits::text(text, MAX_STEP_TEXT_LENGTH, "Der Text eines Schritts")?;
            Ok(TicketStep {
                text: text.to_string(),
                done: step.done,
                detail: step
                    .detail
                    .clone()
                    .filter(|detail| !detail.trim().is_empty()),
            })
        })
        .collect()
}

/// Liste ohne Leerraum am Rand und ohne leere Einträge
fn clean_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Optionaler Text: leer wird `None`, die Länge wird geprüft
fn optional(value: Option<&str>, max: usize, what: &str) -> Result<Option<String>> {
    let value = value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string);
    BoardLimits::text_opt(value.as_deref(), max, what)?;
    Ok(value)
}

/// „erledigt“ oder „offen“ für den Verlauf eines Schritts
fn flag(done: bool) -> &'static str {
    if done {
        "erledigt"
    } else {
        "offen"
    }
}
