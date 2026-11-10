//! Die Standardvorlagen der Dispatcher-Prompts und ihre Platzhalter; portiert aus
//! `packages/core/src/dispatch/DispatchPromptDefaults.ts`.
//!
//! Die Texte liegen als Golden-Dateien in `prompts/*.txt` und sind unverändert aus dem
//! TypeScript-Stand erzeugt: ein einmaliges tsx-Skript im Worktree schrieb
//! `DISPATCH_PROMPT_DEFAULTS`, `LINK_RULE`, `TOOL_RULE` und `ACCEPTANCE_RULE` in diese Dateien
//! (`npx tsx <skript>`; das Skript selbst ist nicht eingecheckt). Geprüft werden sie Zeichen
//! für Zeichen in `tests/dispatch_prompt_output.rs`.
//! Sie sind der Vertrag zwischen Dispatcher und Agent und werden nie „verbessert“.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dispatch::prompt_check::PromptKind;

/// Keine Links aus dem Worktree heraus: beim Aufräumen könnte sonst durch sie hindurch
/// gelöscht werden
pub const LINK_RULE: &str = include_str!("prompts/link_rule.txt");

/// Dateien mit den Werkzeugen statt über die Shell: robuster (keine Zeilenende-Fallen) und
/// im Lauf-Protokoll lesbar
pub const TOOL_RULE: &str = include_str!("prompts/tool_rule.txt");

/// Ausgänge eines Umsetzungslaufs (`{{outcomes}}`); enthält selbst `{{ticketId}}` und `{{agent}}`
pub const OUTCOMES: &str = include_str!("prompts/outcomes.txt");

/// Ausgänge im Autopilot: ohne „Human Interaction“, dafür mit dem Hinweis, dass es keine
/// Abnahme durch einen Menschen gibt
pub const OUTCOMES_AUTOPILOT: &str = include_str!("prompts/outcomes_autopilot.txt");

/// Abnahme im plan.json (`planning.acceptance`): plan-json schreibt sie ans Ticket
pub const ACCEPTANCE_RULE: &str = include_str!("prompts/acceptance_rule.txt");

/// Standardvorlage eines Umsetzungslaufs
pub const WORK_TEMPLATE: &str = include_str!("prompts/work.txt");
/// Standardvorlage eines Planungslaufs
pub const PLAN_TEMPLATE: &str = include_str!("prompts/plan.txt");
/// Standardvorlage eines Recherche-Laufs
pub const RESEARCH_TEMPLATE: &str = include_str!("prompts/research.txt");
/// Standardvorlage der Merge-Session
pub const MERGE_TEMPLATE: &str = include_str!("prompts/merge.txt");

/// Die Standardvorlage einer Laufart (`DISPATCH_PROMPT_DEFAULTS`)
pub fn default_template(kind: PromptKind) -> &'static str {
    match kind {
        PromptKind::Work => WORK_TEMPLATE,
        PromptKind::Plan => PLAN_TEMPLATE,
        PromptKind::Research => RESEARCH_TEMPLATE,
        PromptKind::Merge => MERGE_TEMPLATE,
    }
}

/// Ein Platzhalter einer Vorlage, für die Oberfläche
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct PromptPlaceholder {
    pub name: String,
    /// Kurze Beschreibung, was an dieser Stelle eingesetzt wird
    pub description: String,
}

/// Ein Platzhalter aus Name und Beschreibung
fn placeholder(name: &str, description: &str) -> PromptPlaceholder {
    PromptPlaceholder {
        name: name.to_string(),
        description: description.to_string(),
    }
}

const TICKET_ID_DESC: &str = "Kennung des Tickets, z. B. ARC-0009";
const QUESTION_ID_DESC: &str = "Kennung der Frage, z. B. ARC-F0007";
const TITLE_DESC: &str = "Titel des Tickets bzw. der Frage";
const AGENT_DESC: &str = "Name des Agenten, der den Lauf ausführt";
const BRANCH_DESC: &str = "Branch des Tickets";
const LINK_DESC: &str = "Regel gegen Links auf Hauptordner und andere Worktrees";
const TOOL_DESC: &str = "Regeln zum Umgang mit Dateien und Befehlen";
const EXTRA_DESC: &str =
    "Zusatz-Anweisung aus der Einstellung des Dispatchers; leer, wenn keine eingestellt ist";

/// Welche Platzhalter in welcher Laufart zur Verfügung stehen – Grundlage der Oberfläche für
/// eigene Vorlagen (`DISPATCH_PROMPT_PLACEHOLDERS`)
pub fn placeholders(kind: PromptKind) -> Vec<PromptPlaceholder> {
    match kind {
        PromptKind::Work => vec![
            placeholder("agent", AGENT_DESC),
            placeholder("ticketId", TICKET_ID_DESC),
            placeholder("title", TITLE_DESC),
            placeholder("branch", BRANCH_DESC),
            placeholder(
                "worktreeSetup",
                "Hinweis auf die vom Dispatcher ausgeführte Einrichtung; leer, wenn keine eingestellt ist",
            ),
            placeholder("linkRule", LINK_DESC),
            placeholder("toolRule", TOOL_DESC),
            placeholder(
                "outcomes",
                "Die möglichen Ausgänge des Laufs mit ihren Board-Befehlen; im Autopilot ohne Human Interaction",
            ),
            placeholder(
                "answers",
                "Entschiedene Fragen dieses Tickets; leer, wenn es keine gibt",
            ),
            placeholder(
                "feedback",
                "Rückmeldungen aus dem Review; leer, wenn es keine gibt",
            ),
            placeholder("promptExtra", EXTRA_DESC),
        ],
        PromptKind::Research => vec![
            placeholder("agent", AGENT_DESC),
            placeholder("questionId", QUESTION_ID_DESC),
            placeholder("title", TITLE_DESC),
            placeholder("promptExtra", EXTRA_DESC),
        ],
        PromptKind::Plan => vec![
            placeholder("agent", AGENT_DESC),
            placeholder("ticketId", TICKET_ID_DESC),
            placeholder("title", TITLE_DESC),
            placeholder(
                "acceptanceRule",
                "Anweisung, im plan.json auch die Abnahme zu schreiben; leer, wenn „Abnahme beim Planen mit ausfüllen“ aus ist",
            ),
            placeholder("promptExtra", EXTRA_DESC),
        ],
        PromptKind::Merge => vec![
            placeholder("agent", AGENT_DESC),
            placeholder("branch", BRANCH_DESC),
            placeholder("ticketId", TICKET_ID_DESC),
            placeholder("title", TITLE_DESC),
            placeholder("target", "Zielbranch des Merges, z. B. main"),
            placeholder("linkRule", LINK_DESC),
            placeholder("toolRule", TOOL_DESC),
            placeholder("promptExtra", EXTRA_DESC),
        ],
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use regex::Regex;

    use super::*;
    use crate::dispatch::prompt_check::PromptTemplateCheck;

    #[test]
    fn jede_standardvorlage_enthalt_alle_pflichtteile() {
        let check = PromptTemplateCheck::new();
        for kind in PromptTemplateCheck::KINDS {
            assert_eq!(
                check.missing(kind, default_template(kind)),
                vec![],
                "{kind}"
            );
        }
    }

    #[test]
    fn die_platzhalterliste_nennt_genau_die_platzhalter_der_vorlage() {
        let pattern = Regex::new(r"\{\{(\w+)\}\}").unwrap();
        for kind in PromptTemplateCheck::KINDS {
            let used: BTreeSet<String> = pattern
                .captures_iter(default_template(kind))
                .map(|capture| capture[1].to_string())
                .collect();
            let listed: BTreeSet<String> = placeholders(kind).into_iter().map(|p| p.name).collect();
            assert_eq!(used, listed, "{kind}");
        }
    }

    #[test]
    fn jeder_platzhalter_ist_beschrieben() {
        for kind in PromptTemplateCheck::KINDS {
            for placeholder in placeholders(kind) {
                assert!(
                    !placeholder.description.trim().is_empty(),
                    "{kind}/{}",
                    placeholder.name
                );
            }
        }
    }

    #[test]
    fn die_golden_dateien_haben_lf_und_keinen_abschliessenden_umbruch() {
        for text in [
            WORK_TEMPLATE,
            PLAN_TEMPLATE,
            RESEARCH_TEMPLATE,
            MERGE_TEMPLATE,
            LINK_RULE,
            TOOL_RULE,
            ACCEPTANCE_RULE,
        ] {
            assert!(!text.contains('\r'), "Zeilenende CRLF in einer Vorlage");
            assert!(
                !text.ends_with('\n'),
                "abschließender Umbruch in einer Vorlage"
            );
        }
    }
}
