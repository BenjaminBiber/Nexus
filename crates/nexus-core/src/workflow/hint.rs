//! Baut den Hinweis auf den Ablauf für den Agenten, damit er dem Nutzer den nächsten Schritt
//! vorschlagen kann. Portiert aus `packages/core/src/workflow/WorkflowHint.ts`.

use crate::board::basics::TicketStatus;

use super::phases::{workflow_phases, WorkflowPhase};

/// Befehl, der das nächste bereite Ticket zeigt
const NEXT_READY: &str = "`harness board list --status geplant --md`";

/// Befehl, der das nächste Ticket mit Planungsbedarf holt. Er steht hier und nicht in
/// `workflow_phases`, weil dort nur Befehle für Claude Code (Slash-Befehle) stehen.
const NEXT_PLANNING: &str = "`harness --json board next-planning --claim claude`";

/// Befehl, der Plan, Handbuch und Diagramm schreibt (setzt Geplant)
fn plan_json(id: &str) -> String {
    format!("`harness board plan-json {id} --file payload.json --by claude`")
}

/// Ausnahme vom Ablauf für schnelle Aufgaben; gilt nur nach ausdrücklicher Bestätigung
/// des Nutzers
pub const SIDE_TASKS: &str = "Schnelle Aufgaben und Nebenaufgaben dürfen ohne den Ablauf (ohne Spec, Ticket und Worktree) erledigt werden, aber nur, wenn der Nutzer das ausdrücklich bestätigt: Wirkt eine Anfrage wie eine schnelle Aufgabe und hat der Nutzer nicht schon selbst „ohne Workflow“ gesagt, frage zuerst „Ohne Workflow direkt umsetzen?“ und arbeite erst nach einem klaren Ja so. Ohne diese Bestätigung folgst du dem Ablauf.";

/// Ein Ticket, soweit der Hinweis es braucht: Kennung und Status
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintTicket {
    pub id: String,
    pub status: TicketStatus,
}

/// Der Hinweis auf den Ablauf über einem (womöglich eigenen) Workflow
#[derive(Debug, Clone)]
pub struct WorkflowHint {
    phases: Vec<WorkflowPhase>,
}

impl Default for WorkflowHint {
    fn default() -> Self {
        Self::new(workflow_phases())
    }
}

impl WorkflowHint {
    /// Hinweis über diesen Phasen (ohne Angabe der Standard: [`WorkflowHint::default`])
    pub fn new(phases: Vec<WorkflowPhase>) -> Self {
        Self { phases }
    }

    /// Ablauf plus Vorschlag zum aktuellen Stand der Tickets
    pub fn build(&self, tickets: &[HintTicket]) -> String {
        [
            self.overview(),
            "Schlage nach einem erledigten Schritt den nächsten vor. Halte nur an, wenn eine Entscheidung des Nutzers nötig ist (unklare Anforderung, rote Tests, Merge, Push); sonst mache mit dem nächsten bereiten Ticket derselben Spec bzw. desselben Plans weiter, nie mit dem einer anderen.".to_string(),
            SIDE_TASKS.to_string(),
            self.state(tickets),
        ]
        .join("\n")
    }

    /// Eine Zeile mit allen Phasen und ihren Befehlen; eine Phase ohne Befehl steht nur mit Titel
    fn overview(&self) -> String {
        let parts: Vec<String> = self
            .phases
            .iter()
            .map(|phase| {
                let commands = commands(phase);
                if commands.is_empty() {
                    phase.title.clone()
                } else {
                    format!("{}: {}", phase.title, commands.join(" → "))
                }
            })
            .collect();
        format!(
            "Ablauf im Harness: {}. Aus einem Superpowers-Plan legst du auf Wunsch je Task ein Ticket an und schreibst Plan, Handbuch und Diagramm mit {} (setzt Geplant; Skill board, „Tickets aus einem Superpowers-Plan“). Tickets mit Planungsbedarf (etwa aus der Ticket-Auswahl der Docs-Seite) werden vor dem Umsetzen geplant: {} holt das nächste, {} schreibt Plan, Handbuch und Diagramm. Abgeschlossen wird mit `harness board done`, gemergt wird von Hand.",
            parts.join("; "),
            plan_json("<ID>"),
            NEXT_PLANNING,
            plan_json("<ID>")
        )
    }

    /// Erster Befehl einer Phase, etwa der zum Planen oder Umsetzen
    fn first(&self, id: &str) -> String {
        self.phases
            .iter()
            .find(|phase| phase.id == id)
            .and_then(|phase| phase.steps.iter().find_map(|step| step.command.clone()))
            .unwrap_or_default()
    }

    /// Verweis auf die Phase für den Fall ohne Befehl; fehlt die Phase, entfällt er
    fn see(&self, id: &str) -> String {
        self.phases
            .iter()
            .find(|phase| phase.id == id)
            .map(|phase| format!(" (siehe Phase „{}“)", phase.title))
            .unwrap_or_default()
    }

    /// „eine neue Funktion mit <Befehl> planen“; ohne Befehl mit Verweis auf die Phase
    fn planning(&self) -> String {
        let command = self.first("planen");
        if command.is_empty() {
            format!("eine neue Funktion planen{}", self.see("planen"))
        } else {
            format!("eine neue Funktion mit {command} planen")
        }
    }

    /// „mit <Befehl> [<Ticket>] umsetzen“; ohne Befehl „<Ticket> umsetzen“ mit Verweis auf
    /// die Phase
    fn implementing(&self, ticket: Option<&str>) -> String {
        let command = self.first("umsetzen");
        if !command.is_empty() {
            return match ticket {
                Some(ticket) => format!("mit {command} {ticket} umsetzen"),
                None => format!("mit {command} umsetzen"),
            };
        }
        format!(
            "{} umsetzen{}",
            ticket.unwrap_or("das Ticket"),
            self.see("umsetzen")
        )
    }

    /// Ticket im Backlog oder In Planung: erst planen, dann umsetzen. In Planung gehört dem
    /// Nutzer (next-planning überspringt es)
    fn planning_ticket(&self, id: &str, with_user: bool) -> String {
        let write = format!(
            "Plan, Handbuch und Diagramm mit {} schreiben (setzt Geplant)",
            plan_json(id)
        );
        let implement = self.implementing(Some(id));
        if with_user {
            format!("den Plan mit dem Nutzer abschließen (In Planung gehört ihm), dann {write}; erst danach {implement}")
        } else {
            format!("vor dem Umsetzen planen: {write}; erst danach {implement}")
        }
    }

    /// Vorschlag passend zum Stand des ersten offenen Tickets
    fn state(&self, tickets: &[HintTicket]) -> String {
        if tickets.is_empty() {
            return format!(
                "Stand: keinem Ticket zugeordnet. Vorschlag: {} oder mit {NEXT_READY} das nächste bereite Ticket suchen.",
                self.planning()
            );
        }
        let open = tickets.iter().find(|ticket| {
            ticket.status != TicketStatus::Done && ticket.status != TicketStatus::NotNeeded
        });
        let Some(open) = open else {
            return format!(
                "Stand: {} ist erledigt. Vorschlag: mit {NEXT_READY} das nächste bereite Ticket derselben Spec bzw. desselben Plans suchen und {}.",
                tickets[0].id,
                self.implementing(None)
            );
        };
        let next = match open.status {
            TicketStatus::InProgress => {
                "nach der Umsetzung Review und Abschluss mit `harness board done` vorschlagen"
                    .to_string()
            }
            TicketStatus::Blocked => {
                "den Grund für Blockiert klären, bevor weitergearbeitet wird".to_string()
            }
            TicketStatus::Backlog | TicketStatus::InPlanning => {
                self.planning_ticket(&open.id, open.status == TicketStatus::InPlanning)
            }
            _ => self.implementing(Some(&open.id)),
        };
        format!(
            "Stand: {} ist {}. Vorschlag: {next}.",
            open.id,
            open.status.label()
        )
    }
}

/// Befehle einer Phase in Reihenfolge; Schritte ohne Befehl entfallen
fn commands(phase: &WorkflowPhase) -> Vec<String> {
    phase
        .steps
        .iter()
        .filter_map(|step| step.command.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Ticket mit diesem Status
    fn ticket(id: &str, status: TicketStatus) -> HintTicket {
        HintTicket {
            id: id.to_string(),
            status,
        }
    }

    #[test]
    fn nennt_alle_phasen_mit_ihren_befehlen() {
        let hint = WorkflowHint::default().build(&[]);
        assert!(hint.starts_with("Ablauf im Harness: Neue Funktion planen: /superpowers:brainstorming → /superpowers:writing-plans; Ticket umsetzen: /ai-harness:harness-implement."));
        assert!(hint.contains("Stand: keinem Ticket zugeordnet. Vorschlag: eine neue Funktion mit /superpowers:brainstorming planen oder mit `harness board list --status geplant --md` das nächste bereite Ticket suchen."));
        assert!(hint.contains("Schnelle Aufgaben und Nebenaufgaben"));
    }

    #[test]
    fn schlagt_je_nach_stand_den_nachsten_schritt_vor() {
        let hint = WorkflowHint::default();
        assert!(hint
            .build(&[ticket("ARC-0001", TicketStatus::Backlog)])
            .contains("Stand: ARC-0001 ist Backlog. Vorschlag: vor dem Umsetzen planen: Plan, Handbuch und Diagramm mit `harness board plan-json ARC-0001 --file payload.json --by claude` schreiben (setzt Geplant); erst danach mit /ai-harness:harness-implement ARC-0001 umsetzen."));
        assert!(hint
            .build(&[ticket("ARC-0001", TicketStatus::InPlanning)])
            .contains("Stand: ARC-0001 ist In Planung. Vorschlag: den Plan mit dem Nutzer abschließen (In Planung gehört ihm), dann Plan, Handbuch und Diagramm mit"));
        assert!(hint
            .build(&[ticket("ARC-0001", TicketStatus::Planned)])
            .contains("Vorschlag: mit /ai-harness:harness-implement ARC-0001 umsetzen."));
        assert!(hint
            .build(&[ticket("ARC-0001", TicketStatus::InProgress)])
            .contains("Stand: ARC-0001 ist In Arbeit. Vorschlag: nach der Umsetzung Review und Abschluss mit `harness board done` vorschlagen."));
        assert!(hint
            .build(&[ticket("ARC-0001", TicketStatus::Blocked)])
            .contains("Vorschlag: den Grund für Blockiert klären, bevor weitergearbeitet wird."));
        assert!(hint
            .build(&[ticket("ARC-0001", TicketStatus::Done)])
            .contains("Stand: ARC-0001 ist erledigt."));
    }

    #[test]
    fn formuliert_den_vorschlag_ohne_befehl_wenn_die_phasen_keinen_schritt_mit_befehl_haben() {
        let phases: Vec<WorkflowPhase> = workflow_phases()
            .into_iter()
            .map(|phase| WorkflowPhase {
                steps: Vec::new(),
                ..phase
            })
            .collect();
        let hint = WorkflowHint::new(phases);
        let none = hint.build(&[]);
        let open = hint.build(&[ticket("ARC-0002", TicketStatus::Planned)]);
        let done = hint.build(&[ticket("ARC-0002", TicketStatus::Done)]);
        assert!(none.contains("eine neue Funktion planen (siehe Phase „Neue Funktion planen“)"));
        assert!(open.contains("ARC-0002 umsetzen (siehe Phase „Ticket umsetzen“)"));
        assert!(done.contains("und das Ticket umsetzen (siehe Phase „Ticket umsetzen“)"));
        for text in [&none, &open, &done] {
            // Kein leerer Befehl: weder doppelte Leerzeichen noch ein Doppelpunkt ohne Befehle
            assert!(!text.contains("  "), "{text}");
            assert!(!text.contains(": ;") && !text.contains(": ."), "{text}");
            assert!(!text.contains("mit  "), "{text}");
        }
    }
}
