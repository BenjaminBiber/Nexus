//! Erzeugt Verlaufseinträge für alle geänderten Felder eines Tickets.
//! Portiert aus `packages/core/src/board/TicketHistoryDiff.ts`.
//!
//! Die Feldnamen bleiben die aus TypeScript (`needsPlanning`, `boardId`, `dependsOn` …),
//! weil sie so in der Spalte `board.TicketHistory.Field` stehen und die Oberfläche sie
//! danach beschriftet.

use crate::board::basics::BoardActor;
use crate::board::types::{HistoryEntry, Ticket};
use crate::text::utf16_len;

/// Vergleicht zwei Fassungen eines Tickets.
pub struct TicketHistoryDiff;

impl TicketHistoryDiff {
    /// Vergleicht Vorher und Nachher; SortOrder, Rev und Zeitstempel werden nie protokolliert
    pub fn diff(
        before: &Ticket,
        after: &Ticket,
        actor: &BoardActor,
        at: &str,
        note: Option<&str>,
    ) -> Vec<HistoryEntry> {
        let mut entries = Vec::new();
        let mut add = |field: &str, from: Option<String>, to: Option<String>| {
            entries.push(HistoryEntry {
                ticket_id: after.id.clone(),
                at: at.to_string(),
                by_id: actor.user_id.clone(),
                by_name: actor.name.clone(),
                field: field.to_string(),
                from,
                to,
                note: note.map(str::to_string),
            });
        };

        // Einzelwerte in der Reihenfolge, in der sie im Verlauf erscheinen (`SCALARS`)
        for (field, from, to) in [
            (
                "title",
                text(before.title.as_str()),
                text(after.title.as_str()),
            ),
            (
                "status",
                text(before.status.as_str()),
                text(after.status.as_str()),
            ),
            (
                "priority",
                text(before.priority.as_str()),
                text(after.priority.as_str()),
            ),
            ("assignee", before.assignee.clone(), after.assignee.clone()),
            (
                "needsPlanning",
                text(flag(before.needs_planning)),
                text(flag(after.needs_planning)),
            ),
            (
                "acceptance",
                text(before.acceptance.as_str()),
                text(after.acceptance.as_str()),
            ),
            (
                "blockedReason",
                before.blocked_reason.clone(),
                after.blocked_reason.clone(),
            ),
            (
                "boardId",
                text(before.board_id.to_string()),
                text(after.board_id.to_string()),
            ),
            (
                "plannedBy",
                before.planned_by.clone(),
                after.planned_by.clone(),
            ),
            (
                "planningClaimBy",
                before.planning_claim_by.clone(),
                after.planning_claim_by.clone(),
            ),
            ("spec", before.spec.clone(), after.spec.clone()),
            (
                "specTask",
                before.spec_task.clone(),
                after.spec_task.clone(),
            ),
            ("branch", before.branch.clone(), after.branch.clone()),
            ("worktree", before.worktree.clone(), after.worktree.clone()),
            (
                "targetBranch",
                before.target_branch.clone(),
                after.target_branch.clone(),
            ),
        ] {
            if from != to {
                add(field, from, to);
            }
        }

        // Listen als JSON, wortgleich mit `JSON.stringify` (`LISTS`)
        for (field, from, to) in [
            ("labels", &before.labels, &after.labels),
            ("dependsOn", &before.depends_on, &after.depends_on),
            ("touches", &before.touches, &after.touches),
            ("consumes", &before.consumes, &after.consumes),
            ("produces", &before.produces, &after.produces),
        ] {
            if from != to {
                add(field, text(json(from)), text(json(to)));
            }
        }

        // Lange Texte nur mit Zeichenzahl (`LONG_TEXT`)
        for (field, from, to) in [
            ("description", &before.description, &after.description),
            ("plan", &before.plan, &after.plan),
            ("handbook", &before.handbook, &after.handbook),
            ("diagram", &before.diagram, &after.diagram),
        ] {
            if from != to {
                add(field, text(characters(from)), text(characters(to)));
            }
        }

        entries
    }
}

/// Ein immer vorhandener Wert als Eintrag
fn text(value: impl Into<String>) -> Option<String> {
    Some(value.into())
}

/// Wahrheitswert wie `String(value)` in JavaScript
fn flag(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

/// Liste wie `JSON.stringify(list)`
fn json(list: &[String]) -> String {
    serde_json::to_string(list).expect("Textlisten lassen sich immer als JSON schreiben")
}

/// Länge eines langen Texts, gezählt wie in JavaScript
fn characters(value: &str) -> String {
    format!("{} Zeichen", utf16_len(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::basics::TicketStatus;
    use crate::testing::{ticket_fixture, USER_ID};

    const AT: &str = "2026-10-13T11:00:00.000Z";

    /// Der Handelnde der Tests
    fn actor() -> BoardActor {
        BoardActor {
            user_id: USER_ID.into(),
            name: "claude".into(),
        }
    }

    /// Das Ticket vor der Änderung
    fn before() -> Ticket {
        ticket_fixture("ARC-0001", 1, 1, 1)
    }

    #[test]
    fn schreibt_nur_geanderte_felder() {
        let before = before();
        let after = Ticket {
            sort_order: 9,
            rev: 2,
            ..before.clone()
        };
        assert_eq!(
            TicketHistoryDiff::diff(&before, &after, &actor(), AT, None),
            []
        );
    }

    #[test]
    fn schreibt_status_als_einzelwert_mit_vermerk() {
        let before = before();
        let after = Ticket {
            status: TicketStatus::Planned,
            ..before.clone()
        };

        assert_eq!(
            TicketHistoryDiff::diff(&before, &after, &actor(), AT, Some("verschoben")),
            [HistoryEntry {
                ticket_id: "ARC-0001".into(),
                at: AT.into(),
                by_id: USER_ID.into(),
                by_name: "claude".into(),
                field: "status".into(),
                from: Some("backlog".into()),
                to: Some("planned".into()),
                note: Some("verschoben".into()),
            }]
        );
    }

    #[test]
    fn schreibt_lange_texte_nur_mit_zeichenzahl() {
        let before = before();
        let after = Ticket {
            plan: "x".repeat(120),
            ..before.clone()
        };
        let entries = TicketHistoryDiff::diff(&before, &after, &actor(), AT, None);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].field, "plan");
        assert_eq!(entries[0].from.as_deref(), Some("0 Zeichen"));
        assert_eq!(entries[0].to.as_deref(), Some("120 Zeichen"));
    }

    #[test]
    fn schreibt_listen_als_json() {
        let before = before();
        let after = Ticket {
            labels: vec!["ui".into()],
            ..before.clone()
        };
        let entries = TicketHistoryDiff::diff(&before, &after, &actor(), AT, None);

        assert_eq!(entries[0].field, "labels");
        assert_eq!(entries[0].from.as_deref(), Some("[]"));
        assert_eq!(entries[0].to.as_deref(), Some(r#"["ui"]"#));
    }

    #[test]
    fn schreibt_wahrheitswerte_und_leere_werte_lesbar() {
        let before = before();
        let after = Ticket {
            needs_planning: false,
            assignee: Some("claude".into()),
            ..before.clone()
        };
        let entries = TicketHistoryDiff::diff(&before, &after, &actor(), AT, None);

        // Reihenfolge wie in `SCALARS`: assignee steht vor needsPlanning
        assert_eq!(
            entries
                .iter()
                .map(|e| (e.field.as_str(), e.from.as_deref(), e.to.as_deref()))
                .collect::<Vec<_>>(),
            [
                ("assignee", None, Some("claude")),
                ("needsPlanning", Some("true"), Some("false")),
            ]
        );
    }

    #[test]
    fn schreibt_den_planungs_claim_als_einzelwert() {
        let before = before();
        let after = Ticket {
            planning_claim_by: Some("junior-x".into()),
            planning_claim_at: Some(AT.into()),
            ..before.clone()
        };
        let entries = TicketHistoryDiff::diff(&before, &after, &actor(), AT, None);

        // planningClaimAt steht nicht im Verlauf, nur der Name
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].field, "planningClaimBy");
        assert_eq!(entries[0].from, None);
        assert_eq!(entries[0].to.as_deref(), Some("junior-x"));
    }
}
