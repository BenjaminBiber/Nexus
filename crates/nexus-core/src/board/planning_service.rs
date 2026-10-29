//! Planungslauf: Plan, Handbuch und Diagramm schreiben und das nächste Ticket mit
//! Planungsbedarf holen. Portiert aus `packages/core/src/board/PlanningService.ts`.

use std::sync::Arc;
use std::sync::LazyLock;

use regex::Regex;

use crate::board::basics::{
    BoardActor, TicketStatus, MAX_PERSON_NAME_LENGTH, PLANNING_CLAIM_TTL_MS,
};
use crate::board::database::{BoardDatabase, TicketFilter};
use crate::board::limits::BoardLimits;
use crate::board::planning_candidate::PlanningCandidate;
use crate::board::ticket_service::{self, TicketView};
use crate::board::types::Ticket;
use crate::clock::{self, Clock};
use crate::error::{HarnessError, Result};
use crate::tx;

/// Ein umschließender ```mermaid-Block, den [`PlanningService`] vom Diagramm abzieht
static FENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^\s*```(?:mermaid)?\s*\n(.*?)\n\s*```\s*$").expect("festes Muster")
});

/// Die Texte eines Planungslaufs; ein nicht gesetztes Feld bleibt, wie es ist
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanInput {
    pub plan: Option<String>,
    pub handbook: Option<String>,
    pub diagram: Option<String>,
    /// Abnahme des Tickets; fehlt sie, bleibt die vorhandene
    pub acceptance: Option<String>,
    pub status: Option<TicketStatus>,
}

/// Plan, Handbuch und Diagramm schreiben, Planungsbedarf und Claim verwalten.
#[derive(Debug, Clone)]
pub struct PlanningService<D: BoardDatabase> {
    db: Arc<D>,
    clock: Clock,
}

impl<D: BoardDatabase> PlanningService<D> {
    /// Dienst auf dieser Datenbank mit dieser Uhr
    pub fn new(db: Arc<D>, clock: Clock) -> Self {
        Self { db, clock }
    }

    /// Schreibt die Texte, setzt den Status (Standard: Geplant), löscht Planungsbedarf und
    /// Claim. Ohne Sperren für Abhängigkeiten und Abnahme (wie im alten Board).
    pub async fn plan(
        &self,
        id: &str,
        input: &PlanInput,
        actor: &BoardActor,
    ) -> Result<TicketView> {
        if input.plan.is_none() && input.handbook.is_none() && input.diagram.is_none() {
            return Err(HarnessError::new(
                "plan-empty",
                "Mindestens Plan, Handbuch oder Diagramm angeben.",
            )
            .into());
        }
        let id = id.to_string();
        let input = input.clone();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, input, actor, clock], |u| {
            let before = ticket_service::load(u, &id, None, true).await?;
            let status = input.status.unwrap_or(TicketStatus::Planned);
            let mut after = Ticket {
                plan: input.plan.clone().unwrap_or_else(|| before.plan.clone()),
                handbook: input
                    .handbook
                    .clone()
                    .unwrap_or_else(|| before.handbook.clone()),
                diagram: match input.diagram.as_deref() {
                    Some(diagram) => strip_fence(diagram),
                    None => before.diagram.clone(),
                },
                acceptance: match input.acceptance.as_deref() {
                    Some(acceptance) if !acceptance.trim().is_empty() => acceptance.to_string(),
                    _ => before.acceptance.clone(),
                },
                status,
                needs_planning: false,
                planning_claim_by: None,
                planning_claim_at: None,
                planned_at: Some(clock.iso()),
                planned_by: Some(actor.name.clone()),
                ..before.clone()
            };
            if status != before.status {
                after.sort_order = ticket_service::next_order(u, before.board_id, status).await?;
            }
            let note = if status != before.status {
                "geplant"
            } else {
                "Plan aktualisiert"
            };
            let saved =
                ticket_service::save(u, &before, &after, &actor, Some(note), &clock).await?;
            ticket_service::view(u, saved).await
        })
        .await
    }

    /// Nächstes Ticket mit Planungsbedarf (Anzeige-Reihenfolge); mit `claim_agent` wird es
    /// 15 Minuten beansprucht. Beim Claim wird jeder Kandidat gesperrt und erneut geprüft;
    /// hat ihn inzwischen ein anderer beansprucht, kommt der nächste dran. `None`, wenn
    /// keiner übrig bleibt.
    pub async fn next(
        &self,
        board_ids: &[i32],
        claim_agent: Option<&str>,
        actor: &BoardActor,
    ) -> Result<Option<TicketView>> {
        let agent = claim_agent.map(|agent| agent.trim().to_string());
        if agent.as_deref() == Some("") {
            return Err(HarnessError::new(
                "agent-missing",
                "Für den Claim fehlt der Name des Agenten (--claim).",
            )
            .into());
        }
        let board_ids = board_ids.to_vec();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [board_ids, agent, actor, clock], |u| {
            let now = clock.now().timestamp_millis();
            if let Some(agent) = agent.as_deref() {
                BoardLimits::text(agent, MAX_PERSON_NAME_LENGTH, "Der Name des Agenten")?;
            }
            // Innerhalb der Transaktion nur Methoden mit „u“ nutzen (`list()` würde eine
            // zweite Transaktion öffnen)
            let tickets = u
                .tickets()
                .list(&TicketFilter {
                    board_ids: Some(board_ids.clone()),
                    ..TicketFilter::default()
                })
                .await?;
            let mut views = ticket_service::views(u, tickets).await?;
            ticket_service::sort_views(&mut views);
            let candidates: Vec<TicketView> = views
                .into_iter()
                .filter(|view| {
                    PlanningCandidate::matches(view, now, view.waiting_on_questions.len())
                })
                .collect();
            let Some(agent) = agent.as_deref() else {
                return Ok(candidates.into_iter().next());
            };
            for candidate in candidates {
                // Gesperrt neu lesen: gelöscht oder inzwischen beansprucht → nächster Kandidat
                let Some(before) = u.tickets().by_id(&candidate.ticket.id, true).await? else {
                    continue;
                };
                if !PlanningCandidate::matches(&before, now, candidate.waiting_on_questions.len()) {
                    continue;
                }
                let after = Ticket {
                    planning_claim_by: Some(agent.to_string()),
                    planning_claim_at: Some(clock::iso(clock.now())),
                    ..before.clone()
                };
                let saved = ticket_service::save(u, &before, &after, &actor, None, &clock).await?;
                return Ok(Some(ticket_service::view(u, saved).await?));
            }
            Ok(None)
        })
        .await
    }

    /// Beansprucht genau dieses Ticket 15 Minuten für die Planung („Planen“ im Ticket). Ein
    /// lebender Claim eines anderen Agenten ergibt „plan-claimed“; der eigene wird erneuert.
    pub async fn claim(&self, id: &str, agent: &str, actor: &BoardActor) -> Result<TicketView> {
        let name = agent.trim().to_string();
        if name.is_empty() {
            return Err(HarnessError::new(
                "agent-missing",
                "Für den Claim fehlt der Name des Agenten.",
            )
            .into());
        }
        let id = id.to_string();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, name, actor, clock], |u| {
            BoardLimits::text(&name, MAX_PERSON_NAME_LENGTH, "Der Name des Agenten")?;
            let before = ticket_service::load(u, &id, None, true).await?;
            let now = clock.now().timestamp_millis();
            let alive = before
                .planning_claim_at
                .as_deref()
                .and_then(clock::parse_millis)
                .is_some_and(|at| now - at < PLANNING_CLAIM_TTL_MS);
            if let Some(owner) = before.planning_claim_by.as_deref() {
                if owner != name && alive {
                    return Err(HarnessError::new(
                        "plan-claimed",
                        format!("{} wird gerade von {owner} geplant.", before.id),
                    )
                    .into());
                }
            }
            let after = Ticket {
                planning_claim_by: Some(name.clone()),
                planning_claim_at: Some(clock::iso(clock.now())),
                ..before.clone()
            };
            let saved = ticket_service::save(u, &before, &after, &actor, None, &clock).await?;
            ticket_service::view(u, saved).await
        })
        .await
    }

    /// Gibt den Planungs-Claim frei, wenn er (noch) diesem Agenten gehört; sonst bleibt
    /// alles, wie es ist
    pub async fn release(&self, id: &str, agent: &str, actor: &BoardActor) -> Result<()> {
        let id = id.to_string();
        let agent = agent.to_string();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, agent, actor, clock], |u| {
            let before = ticket_service::load(u, &id, None, true).await?;
            if before.planning_claim_by.as_deref() != Some(agent.as_str()) {
                return Ok(());
            }
            let after = Ticket {
                planning_claim_by: None,
                planning_claim_at: None,
                ..before.clone()
            };
            ticket_service::save(u, &before, &after, &actor, None, &clock).await?;
            Ok(())
        })
        .await
    }
}

/// Entfernt einen umschließenden ```mermaid-Block
fn strip_fence(diagram: &str) -> String {
    FENCE
        .captures(diagram)
        .map_or_else(|| diagram.to_string(), |found| found[1].to_string())
}

#[cfg(test)]
mod tests;
