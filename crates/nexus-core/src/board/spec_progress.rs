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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ticket_fixture;

    /// Minimales Ticket mit Spec
    fn ticket(id: &str, spec: Option<&str>, status: TicketStatus, deps: &[&str]) -> Ticket {
        Ticket {
            title: id.to_string(),
            status,
            depends_on: deps.iter().map(|id| id.to_string()).collect(),
            needs_planning: false,
            spec: spec.map(ToString::to_string),
            created_at: String::new(),
            updated_at: String::new(),
            ..ticket_fixture(id, 1, 1, 1)
        }
    }

    #[test]
    fn ordnet_jedes_ticket_genau_einer_gruppe_zu_nicht_mehr_notig_zahlt_nicht_zur_summe() {
        let tickets = [
            ticket("A-1", Some("002-b"), TicketStatus::Done, &[]),
            ticket("A-2", Some("001-a"), TicketStatus::Done, &[]),
            ticket("A-3", Some("001-a"), TicketStatus::Blocked, &[]),
            ticket("A-4", Some("001-a"), TicketStatus::Planned, &["A-5"]),
            ticket("A-5", Some("001-a"), TicketStatus::Backlog, &[]),
            ticket("A-6", Some("001-a"), TicketStatus::InProgress, &["A-5"]),
            ticket("A-7", Some("001-a"), TicketStatus::NotNeeded, &[]),
            ticket("A-8", None, TicketStatus::Planned, &[]),
        ];
        assert_eq!(
            SpecProgress::count(&tickets, None),
            [
                SpecProgressCount {
                    spec: "001-a".into(),
                    total: 5,
                    done: 1,
                    blocked: 1,
                    waiting: 1,
                    in_progress: 1,
                    open: 1,
                    not_needed: 1,
                },
                SpecProgressCount {
                    spec: "002-b".into(),
                    total: 1,
                    done: 1,
                    blocked: 0,
                    waiting: 0,
                    in_progress: 0,
                    open: 0,
                    not_needed: 0,
                },
            ]
        );
    }

    #[test]
    fn lost_voraussetzungen_uber_alle_tickets_auf_auch_ausserhalb_der_gezahlten() {
        let counted = [ticket(
            "A-1",
            Some("001-a"),
            TicketStatus::Planned,
            &["B-1"],
        )];
        let all = [
            counted[0].clone(),
            ticket("B-1", None, TicketStatus::InProgress, &[]),
        ];
        let count = SpecProgress::count(&counted, Some(&all));
        assert_eq!(count[0].waiting, 1);
        assert_eq!(count[0].open, 0);
    }
}
