//! Der lesende Teil von `packages/core/src/board/TicketService.ts`: Anzeigefassung eines
//! Tickets, Laden mit Versionsprüfung und die Anzeige-Reihenfolge.
//!
//! Die schreibenden Methoden (anlegen, ändern, verschieben, übernehmen, löschen) bringt Teil 2
//! des Tickets P3-2. Sie kommen in dieselbe Datei; [`TicketService`] bekommt dann Datenbank und
//! Uhr als Felder, die hier gezeigten Hilfen bleiben zugehörige Funktionen.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::board::database::{BoardUnitOfWork, TicketFilter};
use crate::board::dependency_graph::DependencyGraph;
use crate::board::dependency_rules::DependencyRules;
use crate::board::question_blocking::BoardQuestionBlocking;
use crate::board::question_types::BoardOpenQuestion;
use crate::board::ticket_ids::TicketIds;
use crate::board::ticket_ordering::{BoardColumn, TicketOrdering};
use crate::board::types::Ticket;
use crate::error::{HarnessError, Result};

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

/// Tickets lesen und für die Anzeige aufbereiten.
pub struct TicketService;

impl TicketService {
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

    /// Anzeigefassung eines Tickets (Spalte, wartet, offene Voraussetzungen, Abhängige)
    pub async fn view(u: &mut dyn BoardUnitOfWork, ticket: Ticket) -> Result<TicketView> {
        Ok(Self::views(u, vec![ticket]).await?.remove(0))
    }

    /// Anzeigefassungen; Voraussetzungen werden über alle Boards aufgelöst, offene Fragen,
    /// Schritte und Sessions je Ticket ergänzt
    pub async fn views(
        u: &mut dyn BoardUnitOfWork,
        tickets: Vec<Ticket>,
    ) -> Result<Vec<TicketView>> {
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
}
