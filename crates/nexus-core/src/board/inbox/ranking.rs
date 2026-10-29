//! Art und Dringlichkeit der Einträge von „Meine Aufgaben“; portiert aus
//! `packages/core/src/board/inbox/InboxRanking.ts`.

use std::cmp::Ordering;
use std::str::FromStr;

use crate::board::basics::{is_human_interaction, Priority, TicketStatus};
use crate::board::inbox::types::{InboxItem, InboxKind, INBOX_KINDS};
use crate::dispatch::config::{MERGE_REASON_PREFIX, SETUP_REASON_PREFIX};

/// Art und Dringlichkeit der Einträge.
#[derive(Debug, Clone, Copy, Default)]
pub struct InboxRanking;

impl InboxRanking {
    /// Art eines Tickets aus Status und Blockiert-Grund; `None` = wartet nicht auf den
    /// Benutzer. Es kommen nur Arten vor, die aus einem Ticket entstehen (in TypeScript
    /// `InboxTicketKind`): nie `question` oder `note`.
    pub fn classify(status: TicketStatus, blocked_reason: Option<&str>) -> Option<InboxKind> {
        if status == TicketStatus::InPlanning {
            return Some(InboxKind::Planning);
        }
        if status != TicketStatus::Blocked {
            return None;
        }
        let reason = blocked_reason.unwrap_or("").trim();
        if reason.starts_with(MERGE_REASON_PREFIX) {
            Some(InboxKind::Merge)
        } else if reason.starts_with(SETUP_REASON_PREFIX) {
            Some(InboxKind::Setup)
        } else if is_human_interaction(Some(reason)) {
            Some(InboxKind::Accept)
        } else {
            Some(InboxKind::Blocked)
        }
    }

    /// Rang einer Art (1 = am dringendsten)
    pub fn rank(kind: InboxKind) -> i32 {
        INBOX_KINDS
            .iter()
            .position(|known| *known == kind)
            .map_or(0, |index| index as i32 + 1)
    }

    /// Vergleich für die Standardsortierung: Rang, bei Fragen mehr wartende Tickets zuerst,
    /// bei Tickets höhere Priorität, dann der ältere Eintrag, zuletzt der Schlüssel (stabil)
    pub fn compare(a: &InboxItem, b: &InboxItem) -> Ordering {
        a.rank
            .cmp(&b.rank)
            .then_with(|| b.waiting_tickets.cmp(&a.waiting_tickets))
            .then_with(|| priority(a).cmp(&priority(b)))
            .then_with(|| a.since.cmp(&b.since))
            .then_with(|| a.key.cmp(&b.key))
    }

    /// Liefert die Einträge nach Dringlichkeit sortiert
    pub fn sort(items: &mut [InboxItem]) {
        items.sort_by(Self::compare);
    }
}

/// Platz der Priorität in der Sortierung; ohne oder mit unbekannter Priorität gilt „normal“
fn priority(item: &InboxItem) -> u8 {
    item.priority
        .as_deref()
        .and_then(|value| Priority::from_str(value).ok())
        .unwrap_or(Priority::Normal)
        .rank()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINCE: &str = "2026-10-19T10:00:00.000Z";

    /// Ein Eintrag dieser Art
    fn item(key: &str, kind: InboxKind) -> InboxItem {
        InboxItem {
            key: key.to_string(),
            kind,
            title: key.to_string(),
            detail: None,
            board_id: Some(1),
            board_name: Some("B".into()),
            project_key: "AIH".into(),
            ticket_id: None,
            question_id: None,
            note_id: None,
            waiting_tickets: 0,
            priority: None,
            since: SINCE.into(),
            rank: InboxRanking::rank(kind),
        }
    }

    /// Sortiert und liefert die Schlüssel
    fn sorted(mut items: Vec<InboxItem>) -> Vec<String> {
        InboxRanking::sort(&mut items);
        items.into_iter().map(|item| item.key).collect()
    }

    #[test]
    fn ordnet_blockiert_nach_seinem_grund_ein() {
        let fälle = [
            (Some("Merge: Konflikt in a.ts"), InboxKind::Merge),
            (Some("Einrichtung fehlgeschlagen: npm ci"), InboxKind::Setup),
            (
                Some("Human Interaction: Oberfläche ansehen"),
                InboxKind::Accept,
            ),
            (Some("  Human Interaction"), InboxKind::Accept),
            (Some("Zugang zur DB fehlt"), InboxKind::Blocked),
            (None, InboxKind::Blocked),
        ];
        for (reason, kind) in fälle {
            assert_eq!(
                InboxRanking::classify(TicketStatus::Blocked, reason),
                Some(kind),
                "{reason:?}"
            );
        }
    }

    #[test]
    fn nimmt_tickets_in_planung_als_planning_andere_status_gar_nicht() {
        assert_eq!(
            InboxRanking::classify(TicketStatus::InPlanning, None),
            Some(InboxKind::Planning)
        );
        for status in [
            TicketStatus::Backlog,
            TicketStatus::Planned,
            TicketStatus::InProgress,
            TicketStatus::Done,
            TicketStatus::NotNeeded,
        ] {
            assert_eq!(InboxRanking::classify(status, Some("Merge: alt")), None);
        }
    }

    #[test]
    fn erkennt_vorsilben_nur_mit_genauer_schreibung_wie_is_human_interaction() {
        assert_eq!(
            InboxRanking::classify(TicketStatus::Blocked, Some("merge: klein")),
            Some(InboxKind::Blocked)
        );
        assert_eq!(
            InboxRanking::classify(TicketStatus::Blocked, Some("human interaction: klein")),
            Some(InboxKind::Blocked)
        );
    }

    #[test]
    fn sortiert_nach_art_merge_setup_question_accept_blocked_planning_note() {
        let kinds = [
            InboxKind::Note,
            InboxKind::Planning,
            InboxKind::Blocked,
            InboxKind::Accept,
            InboxKind::Question,
            InboxKind::Setup,
            InboxKind::Merge,
        ];
        let mut items: Vec<InboxItem> = kinds
            .iter()
            .map(|kind| item(kind.as_str(), *kind))
            .collect();
        InboxRanking::sort(&mut items);
        let mut erwartet: Vec<&str> = kinds.iter().map(|kind| kind.as_str()).collect();
        erwartet.reverse();
        assert_eq!(
            items
                .iter()
                .map(|item| item.kind.as_str())
                .collect::<Vec<_>>(),
            erwartet
        );
    }

    #[test]
    fn nimmt_bei_gleicher_art_den_alteren_eintrag_zuerst() {
        assert_eq!(
            sorted(vec![
                InboxItem {
                    since: "2026-10-19T12:00:00.000Z".into(),
                    ..item("b", InboxKind::Blocked)
                },
                InboxItem {
                    since: "2026-10-18T12:00:00.000Z".into(),
                    ..item("a", InboxKind::Blocked)
                },
            ]),
            ["a", "b"]
        );
    }

    #[test]
    fn nimmt_bei_fragen_die_mit_mehr_wartenden_tickets_zuerst() {
        assert_eq!(
            sorted(vec![
                InboxItem {
                    waiting_tickets: 1,
                    since: "2026-10-01T00:00:00.000Z".into(),
                    ..item("q1", InboxKind::Question)
                },
                InboxItem {
                    waiting_tickets: 3,
                    ..item("q3", InboxKind::Question)
                },
            ]),
            ["q3", "q1"]
        );
    }

    #[test]
    fn nimmt_bei_tickets_in_planung_die_hohere_prioritat_zuerst() {
        assert_eq!(
            sorted(vec![
                InboxItem {
                    priority: Some("low".into()),
                    since: "2026-10-01T00:00:00.000Z".into(),
                    ..item("low", InboxKind::Planning)
                },
                InboxItem {
                    priority: Some("urgent".into()),
                    ..item("urgent", InboxKind::Planning)
                },
                InboxItem {
                    priority: Some("normal".into()),
                    ..item("normal", InboxKind::Planning)
                },
            ]),
            ["urgent", "normal", "low"]
        );
    }
}
