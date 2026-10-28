//! Zählt Tickets je Spec nach Erledigt, Blockiert, Wartet, In Arbeit und Offen (reine Funktion).
//! Portiert aus `packages/core/src/board/SpecProgress.ts`.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::board::basics::TicketStatus;
use crate::board::dependency_rules::DependencyRules;
use crate::board::types::Ticket;

/// Stand einer Spec: jedes Ticket in genau einer Gruppe; `total` ohne „nicht mehr nötig“
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct SpecProgressCount {
    pub spec: String,
    pub total: i64,
    pub done: i64,
    pub blocked: i64,
    pub waiting: i64,
    pub in_progress: i64,
    pub open: i64,
    pub not_needed: i64,
}

/// Zählt den Fortschritt der Specs.
pub struct SpecProgress;

impl SpecProgress {
    /// Zählung je Spec (nach Name sortiert); `all` löst Voraussetzungen auf (ohne Angabe die
    /// gezählten Tickets)
    pub fn count(tickets: &[Ticket], all: Option<&[Ticket]>) -> Vec<SpecProgressCount> {
        let mut by_id: HashMap<String, Ticket> = all
            .unwrap_or(tickets)
            .iter()
            .map(|ticket| (ticket.id.clone(), ticket.clone()))
            .collect();
        for ticket in tickets {
            by_id.insert(ticket.id.clone(), ticket.clone());
        }
        let mut result: BTreeMap<String, SpecProgressCount> = BTreeMap::new();
        for ticket in tickets {
            let Some(spec) = ticket.spec.as_ref().filter(|spec| !spec.is_empty()) else {
                continue;
            };
            let count = result
                .entry(spec.clone())
                .or_insert_with(|| SpecProgressCount {
                    spec: spec.clone(),
                    ..SpecProgressCount::default()
                });
            if ticket.status == TicketStatus::NotNeeded {
                count.not_needed += 1;
                continue;
            }
            count.total += 1;
            if ticket.status == TicketStatus::Done {
                count.done += 1;
            } else if ticket.status == TicketStatus::Blocked {
                count.blocked += 1;
            } else if DependencyRules::is_waiting(ticket, &by_id) {
                count.waiting += 1;
            } else if ticket.status == TicketStatus::InProgress {
                count.in_progress += 1;
            } else {
                count.open += 1;
            }
        }
        result.into_values().collect()
    }
}
