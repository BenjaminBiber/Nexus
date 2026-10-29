//! Entscheidet, ob ein Ticket für den nächsten Planungslauf in Frage kommt; portiert aus
//! `packages/core/src/board/PlanningCandidate.ts`.

use crate::board::basics::{TicketStatus, PLANNING_CLAIM_TTL_MS};
use crate::board::types::Ticket;

/// Prüft die Bedingungen eines Planungs-Kandidaten.
pub struct PlanningCandidate;

impl PlanningCandidate {
    /// Planungsbedarf, nicht In Planung/Nicht mehr nötig, keine offene Frage zum Ticket und
    /// kein lebender Claim. Der Blockiert-Grund „Rückfrage“ hat keine Sonderrolle mehr:
    /// Rückfragen sind Fragen auf der Board-Datenbank.
    ///
    /// `now` ist der Zeitpunkt in Millisekunden seit der Epoche (JavaScript `Date.getTime()`).
    pub fn matches(ticket: &Ticket, now: i64, open_questions: usize) -> bool {
        ticket.needs_planning
            && ticket.status != TicketStatus::InPlanning
            && ticket.status != TicketStatus::NotNeeded
            && open_questions == 0
            && !Self::claim_alive(ticket, now)
    }

    /// Lebt der Planungs-Claim noch (höchstens 15 Minuten alt)? Ein unlesbarer Zeitpunkt
    /// zählt wie in JavaScript (`Date.parse` ergibt `NaN`) als abgelaufen.
    fn claim_alive(ticket: &Ticket, now: i64) -> bool {
        let Some(at) = ticket.planning_claim_at.as_deref() else {
            return false;
        };
        crate::clock::parse_millis(at).is_some_and(|at| now - at < PLANNING_CLAIM_TTL_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ticket_fixture;

    const NOW: i64 = 1_792_058_400_000;

    /// Ein Ticket mit Planungsbedarf und ohne Claim (das `base` aus TypeScript)
    fn base() -> Ticket {
        Ticket {
            needs_planning: true,
            status: TicketStatus::Backlog,
            blocked_reason: None,
            planning_claim_by: None,
            planning_claim_at: None,
            ..ticket_fixture("ARC-0001", 1, 1, 1)
        }
    }

    /// Ein Zeitpunkt `millis` Millisekunden vor `NOW` als ISO-Text
    fn before(millis: i64) -> Option<String> {
        Some(crate::clock::iso(
            chrono::DateTime::from_timestamp_millis(NOW - millis).expect("gültiger Zeitpunkt"),
        ))
    }

    #[test]
    fn nimmt_tickets_mit_planungsbedarf_ohne_claim() {
        assert!(PlanningCandidate::matches(&base(), NOW, 0));
    }

    #[test]
    fn verwirft_fehlenden_planungsbedarf_in_planung_nicht_mehr_notig_und_offene_fragen() {
        assert!(!PlanningCandidate::matches(
            &Ticket {
                needs_planning: false,
                ..base()
            },
            NOW,
            0
        ));
        assert!(!PlanningCandidate::matches(
            &Ticket {
                status: TicketStatus::InPlanning,
                ..base()
            },
            NOW,
            0
        ));
        assert!(!PlanningCandidate::matches(
            &Ticket {
                status: TicketStatus::NotNeeded,
                ..base()
            },
            NOW,
            0
        ));
        assert!(!PlanningCandidate::matches(&base(), NOW, 1));
        // „Rückfrage“ ist kein Sonderfall mehr
        assert!(PlanningCandidate::matches(
            &Ticket {
                status: TicketStatus::Blocked,
                blocked_reason: Some("Rückfrage".into()),
                ..base()
            },
            NOW,
            0
        ));
        assert!(PlanningCandidate::matches(
            &Ticket {
                status: TicketStatus::Blocked,
                blocked_reason: Some("Warten".into()),
                ..base()
            },
            NOW,
            0
        ));
    }

    #[test]
    fn verwirft_lebende_claims_und_nimmt_abgelaufene() {
        assert!(!PlanningCandidate::matches(
            &Ticket {
                planning_claim_by: Some("x".into()),
                planning_claim_at: before(60_000),
                ..base()
            },
            NOW,
            0
        ));
        assert!(PlanningCandidate::matches(
            &Ticket {
                planning_claim_by: Some("x".into()),
                planning_claim_at: before(PLANNING_CLAIM_TTL_MS),
                ..base()
            },
            NOW,
            0
        ));
    }

    #[test]
    fn behandelt_einen_unlesbaren_zeitpunkt_wie_einen_abgelaufenen_claim() {
        assert!(PlanningCandidate::matches(
            &Ticket {
                planning_claim_by: Some("x".into()),
                planning_claim_at: Some("kaputt".into()),
                ..base()
            },
            NOW,
            0
        ));
    }
}
