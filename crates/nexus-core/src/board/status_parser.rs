//! Übersetzt eine Spaltenangabe aus CLI oder Oberfläche in einen Status.

use crate::board::basics::{status_invalid, TicketStatus};
use crate::error::{HarnessError, Result};

/// Deutsche und englische Schreibweisen der Spalten, wie im alten Board
fn alias(dashed: &str) -> Option<TicketStatus> {
    Some(match dashed {
        "backlog" => TicketStatus::Backlog,
        "in-planung" | "planung" | "verfeinern" => TicketStatus::InPlanning,
        "geplant" | "plan" => TicketStatus::Planned,
        "in-arbeit" | "wip" | "arbeit" => TicketStatus::InProgress,
        "blockiert" => TicketStatus::Blocked,
        "erledigt" | "fertig" => TicketStatus::Done,
        "nicht-mehr-noetig" | "nicht-mehr-nötig" | "nicht-noetig" | "unnoetig" | "hinfaellig" => {
            TicketStatus::NotNeeded
        }
        _ => return None,
    })
}

/// Liest Spaltenangaben.
pub struct StatusParser;

impl StatusParser {
    /// Liefert den Status; „wartet“ und Unbekanntes werden mit deutscher Meldung abgelehnt
    pub fn parse(input: &str) -> Result<TicketStatus> {
        // wie JavaScript `trim().toLowerCase().replace(/\s+/g, '-')`
        let dashed = input
            .trim()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-");
        if dashed == "wartet" || dashed == "waiting" {
            return Err(HarnessError::new(
                "status-invalid",
                "„Wartet“ ist abgeleitet aus offenen Voraussetzungen und kein Status.",
            )
            .into());
        }
        let underscored = dashed.replace('-', "_");
        let direct = TicketStatus::ALL
            .into_iter()
            .find(|status| status.as_str() == underscored);
        direct
            .or_else(|| alias(&dashed))
            .ok_or_else(|| status_invalid(input).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ubersetzt_die_schreibweisen_der_spalten() {
        let faelle = [
            ("backlog", TicketStatus::Backlog),
            ("In Planung", TicketStatus::InPlanning),
            ("verfeinern", TicketStatus::InPlanning),
            ("geplant", TicketStatus::Planned),
            ("in-arbeit", TicketStatus::InProgress),
            ("WIP", TicketStatus::InProgress),
            ("blockiert", TicketStatus::Blocked),
            ("erledigt", TicketStatus::Done),
            ("fertig", TicketStatus::Done),
            ("nicht mehr nötig", TicketStatus::NotNeeded),
            ("hinfaellig", TicketStatus::NotNeeded),
            ("in_progress", TicketStatus::InProgress),
        ];
        for (input, expected) in faelle {
            assert_eq!(
                StatusParser::parse(input).unwrap(),
                expected,
                "„{input}“ übersetzt falsch"
            );
        }
    }

    #[test]
    fn lehnt_wartet_mit_hinweis_ab() {
        for input in ["wartet", "Wartet", "waiting"] {
            let error = StatusParser::parse(input).unwrap_err();
            assert_eq!(error.code(), Some("status-invalid"));
            assert_eq!(
                error.to_string(),
                "„Wartet“ ist abgeleitet aus offenen Voraussetzungen und kein Status."
            );
        }
    }

    #[test]
    fn lehnt_unbekanntes_mit_code_status_invalid_ab() {
        let error = StatusParser::parse("irgendwas").unwrap_err();
        assert_eq!(error.code(), Some("status-invalid"));
        assert_eq!(
            error.to_string(),
            "Status „irgendwas“ ist ungültig. Erlaubt: backlog, in_planning, planned, in_progress, blocked, done, not_needed."
        );
    }
}
