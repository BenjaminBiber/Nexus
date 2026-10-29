//! Gemeinsame Quelle des Ablaufs, portiert aus `packages/core/src/workflow/WorkflowPhases.ts`.
//! Die Reihenfolge der Felder ist die des TypeScript-Literals: `BaseSnapshot::format_json`
//! muss für diese Daten denselben Text liefern wie `JSON.stringify(WORKFLOW_PHASES, null, 2)`,
//! weil TS und Rust dieselbe Basis in `custom/.base/workflow.json` vergleichen.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Gleichwertige Alternative eines Schritts; die Oberfläche zeigt sie als Umschalter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "workflow.ts")]
pub struct WorkflowOption {
    pub label: String,
    pub command: String,
    pub when: String,
}

/// Ein Schritt einer Phase: Befehl oder Ort, dazu der Hinweis, wann er gilt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "workflow.ts")]
pub struct WorkflowStep {
    pub step: String,
    /// Befehl der ersten (Standard-)Wahl, wenn `options` gesetzt ist; fehlt, wenn der
    /// Schritt keinen Befehl hat
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub command: Option<String>,
    /// Statt eines Befehls: Ort in der Oberfläche oder ein Satz, den man Claude sagt
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub place: Option<String>,
    pub when: String,
    /// Gleichwertige Alternativen für diesen Schritt
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub options: Option<Vec<WorkflowOption>>,
}

/// Eine Phase des Ablaufs mit ihren Schritten
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "workflow.ts")]
pub struct WorkflowPhase {
    pub id: String,
    pub title: String,
    pub hint: String,
    pub steps: Vec<WorkflowStep>,
}

/// Ein Schritt mit Befehl und optionalen Alternativen
fn mit_befehl(step: &str, command: &str, when: &str, options: Vec<WorkflowOption>) -> WorkflowStep {
    WorkflowStep {
        step: step.to_string(),
        command: Some(command.to_string()),
        place: None,
        when: when.to_string(),
        options: if options.is_empty() {
            None
        } else {
            Some(options)
        },
    }
}

/// Ein Schritt ohne Befehl: Ort in der Oberfläche oder ein Satz an Claude
fn mit_ort(step: &str, place: &str, when: &str) -> WorkflowStep {
    WorkflowStep {
        step: step.to_string(),
        command: None,
        place: Some(place.to_string()),
        when: when.to_string(),
        options: None,
    }
}

/// Eine Alternative eines Schritts
fn wahl(label: &str, command: &str, when: &str) -> WorkflowOption {
    WorkflowOption {
        label: label.to_string(),
        command: command.to_string(),
        when: when.to_string(),
    }
}

/// Hinweis des ersten Schritts und seiner ersten Alternative (wortgleich wie in TypeScript)
const BRAINSTORMING: &str = "Einfach: erkundet Idee, Anforderungen und Entwurf im Dialog (Superpowers-Plugin) und schreibt die Spec nach docs/superpowers/specs/. Danach weiter mit /superpowers:writing-plans.";

/// Hinweis des Umsetzen-Schritts und seiner ersten Alternative
const IMPLEMENT: &str = "Wählt das nächste Ticket, legt Branch und Worktree an (harness board start), setzt es test-first um und schließt mit Review ab. Ein bestimmtes Ticket: /ai-harness:harness-implement ARC-0042.";

/// Ablauf in Phasen, nur mit Befehlen für Claude Code. Die CLI ruft Claude im Hintergrund
/// selbst auf.
pub fn workflow_phases() -> Vec<WorkflowPhase> {
    vec![
        WorkflowPhase {
            id: "planen".to_string(),
            title: "Neue Funktion planen".to_string(),
            hint: "In Claude Code, einmal pro Funktion. Für kleine Aufgaben genügt der erste Schritt.".to_string(),
            steps: vec![
                mit_befehl(
                    "Anforderungen klären",
                    "/superpowers:brainstorming",
                    BRAINSTORMING,
                    vec![
                        wahl("Brainstorming", "/superpowers:brainstorming", BRAINSTORMING),
                        wahl("Grill", "/ai-harness:harness-grill", "Mittel: fragt in Runden nach, pflegt Glossar und ADRs. Am Ende steht eine Zusammenfassung. Claude legt sie als Spec unter docs/superpowers/specs/ ab; danach weiter mit /superpowers:writing-plans."),
                        wahl("Wayfinder", "/ai-harness:harness-wayfinder", "Komplex: für große Vorhaben über mehrere Sessions (AI Hero). Hält Ziel und Klärungsfragen in einer Karte fest; am Ende steht eine Zusammenfassung. Claude legt sie als Spec unter docs/superpowers/specs/ ab (je Funktion eine); danach weiter mit /superpowers:writing-plans."),
                    ],
                ),
                mit_befehl(
                    "Plan schreiben",
                    "/superpowers:writing-plans",
                    "Zerlegt die Spec in Tasks mit Dateien, Code, Tests und Befehlen und speichert den Plan unter docs/superpowers/plans/. Fragt der Skill danach, wie umgesetzt werden soll: zuerst die Tickets anlegen lassen.",
                    vec![],
                ),
                mit_ort(
                    "Tickets anlegen",
                    "In der Session sagen: „Lege für die Tasks des Plans Tickets an“",
                    "Claude legt je Task ein Ticket an (Titel, Abnahme, Schritte aus den Task-Schritten, Abhängigkeiten in Task-Reihenfolge) und schreibt Plan (Task-Text), Handbuch (Testbefehle, globale Vorgaben) und Diagramm mit harness board plan-json; danach stehen sie in Geplant. Plan und Handbuch stehen im Ticket – Worktrees und Agenten brauchen die Plan-Datei nicht. Kleinere Schnitte gehen auch auf der Seite „Docs“: Stelle markieren, „Zur Ticket-Auswahl“, Entwürfe prüfen, anlegen.",
                ),
                mit_ort(
                    "Für den Dispatcher vorbereiten",
                    "In der Session sagen: „Bereite ARC-0042 für den Dispatcher vor“",
                    "Optional. Claude prüft, was dem Ticket für den Dispatcher fehlt, ergänzt es, schätzt den Aufwand ein und setzt das Label auto mit profil:<klein|standard|schwer>.",
                ),
            ],
        },
        WorkflowPhase {
            id: "umsetzen".to_string(),
            title: "Ticket umsetzen".to_string(),
            hint: "In Claude Code, für jedes Ticket. Ohne Nummer nimmt Claude das nächste bereite Ticket.".to_string(),
            steps: vec![
                mit_befehl(
                    "Umsetzen",
                    "/ai-harness:harness-implement",
                    IMPLEMENT,
                    vec![
                        wahl("Ein Ticket", "/ai-harness:harness-implement", IMPLEMENT),
                        wahl("Ganzer Plan", "/superpowers:subagent-driven-development", "Setzt alle Tasks des Plans in dieser Session mit Subagenten um. Die Tickets dabei mit dem Board-Skill nachführen: übernehmen, Schritte abhaken, abschließen."),
                    ],
                ),
                mit_ort(
                    "Automatisch umsetzen lassen",
                    "Einstellungen → Dispatcher",
                    "Der Dispatcher setzt bereite Tickets mit Label auto selbst im eigenen Worktree um und mergt sie danach. Was er prüft und wie man ihn einrichtet, steht im Abschnitt „Dispatcher“ der Anleitung.",
                ),
                mit_ort(
                    "Schritte pflegen",
                    "Im Board im Ticket-Panel, „+ Schritt“",
                    "Schritte des Tickets lassen sich im Panel anlegen, bearbeiten, löschen und verschieben. Im Terminal: harness board step add|edit|rm|move.",
                ),
                mit_ort(
                    "Kommentare schreiben",
                    "Im Board im Ticket-Panel, Reiter „Kommentare“",
                    "Claude bekommt neue Kommentare automatisch bei der nächsten Eingabe und antwortet darauf.",
                ),
                mit_ort(
                    "Abschließen",
                    "In der Session sagen: „Schließe ARC-0042 ab“",
                    "Claude setzt das Ticket mit harness board done auf Erledigt. Gemergt wird mit „Mergen“ im Ticket-Panel oder harness board merge ARC-0042; der Dispatcher mergt seine Tickets selbst.",
                ),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_standard_hat_die_pflicht_phasen() {
        let phasen = workflow_phases();
        let ids: Vec<&str> = phasen.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["planen", "umsetzen"]);
    }

    /// Übernommen aus dem früheren Web-Test `GuideWorkflow.test.tsx`, der den
    /// Standard-Ablauf noch gegen das TypeScript-Literal geprüft hat.
    #[test]
    fn die_phase_planen_fuehrt_durch_superpowers() {
        let planen = workflow_phases()
            .into_iter()
            .find(|p| p.id == "planen")
            .expect("Phase planen");
        let schritte: Vec<&str> = planen.steps.iter().map(|s| s.step.as_str()).collect();
        assert_eq!(
            schritte,
            vec![
                "Anforderungen klären",
                "Plan schreiben",
                "Tickets anlegen",
                "Für den Dispatcher vorbereiten",
            ]
        );
        let json = serde_json::to_string(&workflow_phases()).unwrap();
        assert!(!json.contains("/speckit-"));
        assert!(!json.contains("harness-tasks"));
    }

    #[test]
    fn ein_schritt_hat_entweder_befehl_oder_ort() {
        for phase in workflow_phases() {
            for step in phase.steps {
                assert!(
                    step.command.is_some() != step.place.is_some(),
                    "Schritt „{}“ braucht genau eines von command und place",
                    step.step
                );
            }
        }
    }

    #[test]
    fn das_json_nennt_die_felder_in_der_reihenfolge_des_typescript_literals() {
        let json = serde_json::to_string_pretty(&workflow_phases()).unwrap();
        let erste = json.find("\"id\"").unwrap();
        assert!(erste < json.find("\"title\"").unwrap());
        assert!(json.find("\"title\"").unwrap() < json.find("\"hint\"").unwrap());
        // Ein Schritt mit Ort nennt place vor when und keinen Befehl
        let ort = json.find("\"place\"").unwrap();
        assert!(ort < json.rfind("\"when\"").unwrap());
        assert!(!json.contains("\"command\": null"));
        assert!(!json.contains("\"options\": null"));
    }
}
