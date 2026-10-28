//! Reihenfolge der Anzeige: Spalte, dann Priorität (dringend zuerst), dann Ticketnummer.
//! Portiert aus `packages/core/src/board/TicketOrdering.ts`.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::board::basics::{TicketStatus, WAITING_LABEL};
use crate::board::types::Ticket;

/// Spalte des Boards: ein Status oder die abgeleitete Spalte „Wartet“
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "board.ts")]
pub enum BoardColumn {
    Backlog,
    Waiting,
    InPlanning,
    Planned,
    InProgress,
    Blocked,
    Done,
    NotNeeded,
}

/// Alle Spalten in der Reihenfolge der Anzeige (`COLUMN_ORDER`)
pub const COLUMN_ORDER: [BoardColumn; 8] = [
    BoardColumn::Backlog,
    BoardColumn::Waiting,
    BoardColumn::InPlanning,
    BoardColumn::Planned,
    BoardColumn::InProgress,
    BoardColumn::Blocked,
    BoardColumn::Done,
    BoardColumn::NotNeeded,
];

impl BoardColumn {
    /// Schreibweise in JSON und in Befehlen
    pub fn as_str(&self) -> &'static str {
        match self {
            BoardColumn::Backlog => "backlog",
            BoardColumn::Waiting => "waiting",
            BoardColumn::InPlanning => "in_planning",
            BoardColumn::Planned => "planned",
            BoardColumn::InProgress => "in_progress",
            BoardColumn::Blocked => "blocked",
            BoardColumn::Done => "done",
            BoardColumn::NotNeeded => "not_needed",
        }
    }

    /// Spaltentitel der Anzeige; „Wartet“ ist kein Status und hat seinen eigenen Titel
    pub fn label(&self) -> &'static str {
        match self {
            BoardColumn::Waiting => WAITING_LABEL,
            other => other
                .status()
                .expect("nur „Wartet“ ist kein Status")
                .label(),
        }
    }

    /// Der Status zur Spalte; „Wartet“ hat keinen
    pub fn status(&self) -> Option<TicketStatus> {
        match self {
            BoardColumn::Backlog => Some(TicketStatus::Backlog),
            BoardColumn::Waiting => None,
            BoardColumn::InPlanning => Some(TicketStatus::InPlanning),
            BoardColumn::Planned => Some(TicketStatus::Planned),
            BoardColumn::InProgress => Some(TicketStatus::InProgress),
            BoardColumn::Blocked => Some(TicketStatus::Blocked),
            BoardColumn::Done => Some(TicketStatus::Done),
            BoardColumn::NotNeeded => Some(TicketStatus::NotNeeded),
        }
    }

    /// Platz in der Reihenfolge der Anzeige (`COLUMN_ORDER.indexOf`)
    pub fn rank(&self) -> usize {
        COLUMN_ORDER
            .iter()
            .position(|column| column == self)
            .expect("jede Spalte steht in COLUMN_ORDER")
    }
}

impl From<TicketStatus> for BoardColumn {
    fn from(status: TicketStatus) -> Self {
        match status {
            TicketStatus::Backlog => BoardColumn::Backlog,
            TicketStatus::InPlanning => BoardColumn::InPlanning,
            TicketStatus::Planned => BoardColumn::Planned,
            TicketStatus::InProgress => BoardColumn::InProgress,
            TicketStatus::Blocked => BoardColumn::Blocked,
            TicketStatus::Done => BoardColumn::Done,
            TicketStatus::NotNeeded => BoardColumn::NotNeeded,
        }
    }
}

impl fmt::Display for BoardColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Sortiert Tickets für die Anzeige.
pub struct TicketOrdering;

impl TicketOrdering {
    /// Spalte eines Tickets; wartende Tickets stehen in „Wartet“
    pub fn column(ticket: &Ticket, waiting: bool) -> BoardColumn {
        if waiting {
            BoardColumn::Waiting
        } else {
            ticket.status.into()
        }
    }

    /// Sortiert an Ort und Stelle: Spalte, Priorität, Nummer, ID
    pub fn sort(tickets: &mut [Ticket], waiting_ids: &HashSet<String>) {
        tickets.sort_by(|a, b| Self::key(a, waiting_ids).cmp(&Self::key(b, waiting_ids)));
    }

    /// Sortiert Anzeigefassungen, die ein Ticket mitbringen (etwa `TicketView`)
    pub fn sort_by<T>(
        items: &mut [T],
        ticket: impl Fn(&T) -> &Ticket,
        waiting_ids: &HashSet<String>,
    ) {
        items.sort_by(|a, b| Self::key(ticket(a), waiting_ids).cmp(&Self::key(ticket(b), waiting_ids)));
    }

    /// Schlüssel der Sortierung; die ID entscheidet zuletzt, damit die Reihenfolge eindeutig
    /// bleibt (IDs sind ASCII, dort ist die Byte-Folge dieselbe wie `localeCompare`)
    fn key<'a>(ticket: &'a Ticket, waiting_ids: &HashSet<String>) -> (usize, u8, i32, &'a str) {
        (
            Self::column(ticket, waiting_ids.contains(&ticket.id)).rank(),
            ticket.priority.rank(),
            ticket.number,
            &ticket.id,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::basics::Priority;
    use crate::testing::ticket_fixture;

    /// Ticket mit Nummer, Status und Priorität, sonst wie die Vorlage
    fn t(n: i32, status: TicketStatus, priority: Priority) -> Ticket {
        Ticket {
            status,
            priority,
            ..ticket_fixture(&format!("ARC-{n:04}"), 1, 1, n)
        }
    }

    #[test]
    fn sortiert_nach_spalte_prioritat_und_nummer_wartet_steht_nach_dem_backlog() {
        let mut list = vec![
            t(5, TicketStatus::Done, Priority::Normal),
            t(3, TicketStatus::Planned, Priority::Normal),
            t(2, TicketStatus::Backlog, Priority::Low),
            t(4, TicketStatus::Backlog, Priority::Urgent),
            t(1, TicketStatus::Backlog, Priority::Normal),
            t(6, TicketStatus::Backlog, Priority::Normal),
        ];

        TicketOrdering::sort(&mut list, &HashSet::from(["ARC-0006".to_string()]));

        assert_eq!(
            list.iter().map(|x| x.number).collect::<Vec<_>>(),
            [4, 1, 2, 6, 3, 5]
        );
    }

    #[test]
    fn ordnet_wartende_tickets_der_spalte_wartet_zu() {
        let ticket = t(1, TicketStatus::Planned, Priority::Normal);
        assert_eq!(
            TicketOrdering::column(&ticket, true),
            BoardColumn::Waiting
        );
        assert_eq!(
            TicketOrdering::column(&ticket, false),
            BoardColumn::Planned
        );
    }

    #[test]
    fn spalten_behalten_reihenfolge_schreibweise_und_titel() {
        assert_eq!(
            COLUMN_ORDER.map(|column| column.as_str()),
            [
                "backlog",
                "waiting",
                "in_planning",
                "planned",
                "in_progress",
                "blocked",
                "done",
                "not_needed"
            ]
        );
        assert_eq!(COLUMN_ORDER.map(|column| column.label())[1], "Wartet");
        assert_eq!(BoardColumn::NotNeeded.label(), "Nicht mehr nötig");
        assert_eq!(BoardColumn::Waiting.status(), None);
        assert_eq!(
            BoardColumn::Done.status(),
            Some(TicketStatus::Done)
        );
        assert_eq!(
            serde_json::to_string(&BoardColumn::InPlanning).unwrap(),
            r#""in_planning""#
        );
    }
}
