//! Pflichtteile einer Prompt-Vorlage; portiert aus
//! `packages/core/src/dispatch/PromptTemplateCheck.ts`.
//!
//! Diese Teile sind der Vertrag zwischen Dispatcher und Agent; fehlen sie, enden Läufe still
//! als „ohne Ergebnis“.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Laufart einer Prompt-Vorlage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "dispatch.ts")]
pub enum PromptKind {
    Work,
    Plan,
    Research,
    Merge,
}

impl PromptKind {
    /// Alle Laufarten in der Reihenfolge von TypeScript (`PromptTemplateCheck.KINDS`)
    pub const ALL: [PromptKind; 4] = [
        PromptKind::Work,
        PromptKind::Plan,
        PromptKind::Research,
        PromptKind::Merge,
    ];

    /// Schreibweise in JSON, in der Einstellung und in Meldungen
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptKind::Work => "work",
            PromptKind::Plan => "plan",
            PromptKind::Research => "research",
            PromptKind::Merge => "merge",
        }
    }

    /// Die Laufart zu diesem Schlüssel aus der Einstellung
    pub fn parse(value: &str) -> Option<PromptKind> {
        PromptKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
    }
}

impl std::fmt::Display for PromptKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ein Pflichtteil mit Kennung und Beschriftung für Oberfläche und Meldungen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct RequiredPart {
    pub id: String,
    pub label: String,
}

/// Ein Pflichtteil samt Erkennungsmuster
struct Rule {
    id: &'static str,
    label: &'static str,
    test: Regex,
}

/// Platzhalter in der Vorlage, nicht das fertige Ergebnis
fn ph(name: &str) -> String {
    format!(r"\{{\{{{name}\}}\}}")
}

/// Toleriert mehr Leerraum und zusätzliche Flags zwischen den Wörtern eines Befehls
fn cmd(words: &[&str]) -> Regex {
    Regex::new(&words.join(r"\s+(?:--?\S+\s+)*")).unwrap()
}

/// Wie [`cmd`], erkennt aber auch den Platzhalter `{{outcomes}}`: der setzt die Ausgänge des
/// Laufs mit allen Board-Befehlen ein
fn cmd_or_outcomes(words: &[&str]) -> Regex {
    let command = words.join(r"\s+(?:--?\S+\s+)*");
    Regex::new(&format!("(?:{command})|{}", ph("outcomes"))).unwrap()
}

/// Ein Pflichtteil, der nur auf einen Platzhalter prüft
fn placeholder_rule(name: &'static str, label: &'static str) -> Rule {
    Rule {
        id: name,
        label,
        test: Regex::new(&ph(name)).unwrap(),
    }
}

static RULES_WORK: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    vec![
        placeholder_rule("ticketId", "Platzhalter {{ticketId}}"),
        placeholder_rule("agent", "Platzhalter {{agent}}"),
        Rule {
            id: "done",
            label: "Ausgang erledigt: harness board done {{ticketId}}",
            test: cmd_or_outcomes(&["harness", "board", "done", &ph("ticketId")]),
        },
        Rule {
            id: "blocked",
            label: "Ausgang blockiert: harness board set {{ticketId}} --status blocked",
            test: cmd_or_outcomes(&[
                "harness",
                "board",
                "set",
                &ph("ticketId"),
                "--status",
                "blocked",
            ]),
        },
        Rule {
            id: "question",
            label: "Ausgang Frage: harness board question add … --ticket {{ticketId}}",
            test: cmd_or_outcomes(&[
                "harness",
                "board",
                "question",
                "add",
                r"[\s\S]*?--ticket",
                &ph("ticketId"),
            ]),
        },
        Rule {
            id: "by-agent",
            label: "Kennzeichnung --by {{agent}}",
            test: cmd_or_outcomes(&["--by", &ph("agent")]),
        },
    ]
});

static RULES_PLAN: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    vec![
        placeholder_rule("ticketId", "Platzhalter {{ticketId}}"),
        placeholder_rule("agent", "Platzhalter {{agent}}"),
        Rule {
            id: "plan-json",
            label: "Speichern: harness board plan-json {{ticketId}}",
            test: cmd(&["harness", "board", "plan-json", &ph("ticketId")]),
        },
        Rule {
            id: "plan-file",
            label: "Datei plan.json",
            test: Regex::new(r"plan\.json").unwrap(),
        },
    ]
});

static RULES_RESEARCH: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    vec![
        placeholder_rule("questionId", "Platzhalter {{questionId}}"),
        placeholder_rule("agent", "Platzhalter {{agent}}"),
        Rule {
            id: "question-research",
            label: "Speichern: harness board question research {{questionId}}",
            test: cmd(&[
                "harness",
                "board",
                "question",
                "research",
                &ph("questionId"),
            ]),
        },
        Rule {
            id: "research-file",
            label: "Datei research.json",
            test: Regex::new(r"research\.json").unwrap(),
        },
    ]
});

static RULES_MERGE: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    vec![
        placeholder_rule("ticketId", "Platzhalter {{ticketId}}"),
        placeholder_rule("agent", "Platzhalter {{agent}}"),
        placeholder_rule("branch", "Platzhalter {{branch}}"),
        placeholder_rule("target", "Platzhalter {{target}}"),
        Rule {
            id: "merge-abort",
            label: "Rückzug: git merge --abort",
            test: cmd(&["git", "merge", "--abort"]),
        },
    ]
});

/// Die Regeln einer Laufart
fn rules(kind: PromptKind) -> &'static [Rule] {
    match kind {
        PromptKind::Work => &RULES_WORK,
        PromptKind::Plan => &RULES_PLAN,
        PromptKind::Research => &RULES_RESEARCH,
        PromptKind::Merge => &RULES_MERGE,
    }
}

/// Prüft eine Prompt-Vorlage auf die Pflichtteile ihrer Laufart.
#[derive(Debug, Clone, Copy, Default)]
pub struct PromptTemplateCheck;

impl PromptTemplateCheck {
    /// Alle vier Laufarten (`PromptTemplateCheck.KINDS`)
    pub const KINDS: [PromptKind; 4] = PromptKind::ALL;

    /// Neue Prüfung (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Alle Pflichtteile einer Laufart, für Oberfläche und Doku
    pub fn parts(&self, kind: PromptKind) -> Vec<RequiredPart> {
        rules(kind)
            .iter()
            .map(|rule| RequiredPart {
                id: rule.id.to_string(),
                label: rule.label.to_string(),
            })
            .collect()
    }

    /// Die Pflichtteile, die in der Vorlage nicht vorkommen
    pub fn missing(&self, kind: PromptKind, template: &str) -> Vec<RequiredPart> {
        rules(kind)
            .iter()
            .filter(|rule| !rule.test.is_match(template))
            .map(|rule| RequiredPart {
                id: rule.id.to_string(),
                label: rule.label.to_string(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// Vollständige Beispielvorlagen – enthalten alle Pflichtteile der jeweiligen Laufart.
    fn sample(kind: PromptKind) -> String {
        match kind {
            PromptKind::Work => [
                "Du bist {{agent}}. Setze Ticket {{ticketId}} um.",
                "erledigt: harness board done {{ticketId}} --acceptance \"<woran man es sieht>\" --by {{agent}};",
                "blockiert: harness board set {{ticketId}} --status blocked --blocked-reason \"<Grund>\" --by {{agent}};",
                "Frage: harness board question add \"<Frage>\" --description \"<Kontext>\" --ticket {{ticketId}} --author {{agent}}",
            ]
            .join("\n"),
            PromptKind::Plan => [
                "Du bist {{agent}}, ein Planungs-Agent. Plane Ticket {{ticketId}}.",
                "Plan als JSON in die Datei plan.json schreiben, dann speichern:",
                "harness board plan-json {{ticketId}} --file plan.json --by {{agent}}",
            ]
            .join("\n"),
            PromptKind::Research => [
                "Du bist {{agent}}, ein Recherche-Agent. Recherchiere Frage {{questionId}}.",
                "Ergebnis in die Datei research.json schreiben, dann speichern:",
                "harness board question research {{questionId}} --from research.json --author {{agent}}",
            ]
            .join("\n"),
            PromptKind::Merge => [
                "Du bist {{agent}}. Der Branch {{branch}} von Ticket {{ticketId}} laesst sich nicht nach {{target}} mergen.",
                "Gelingt es nicht: git merge --abort und den Grund als Kommentar.",
            ]
            .join("\n"),
        }
    }

    /// Entfernt den vom Pflichtteil geprüften Text aus der Beispielvorlage
    fn remove_part(template: &str, id: &str) -> String {
        let pattern = match id {
            "ticketId" => r"\{\{ticketId\}\}",
            "agent" => r"\{\{agent\}\}",
            "questionId" => r"\{\{questionId\}\}",
            "branch" => r"\{\{branch\}\}",
            "target" => r"\{\{target\}\}",
            "done" => r"harness board done \{\{ticketId\}\}",
            "blocked" => r"harness board set \{\{ticketId\}\} --status blocked",
            "question" => r"harness board question add[\s\S]*?--ticket \{\{ticketId\}\}",
            "by-agent" => r"--by \{\{agent\}\}",
            "plan-json" => r"harness board plan-json \{\{ticketId\}\}",
            "plan-file" => r"plan\.json",
            "question-research" => r"harness board question research \{\{questionId\}\}",
            "research-file" => r"research\.json",
            "merge-abort" => r"git merge --abort",
            other => panic!("Testlücke: kein Entferner für Pflichtteil {other}"),
        };
        Regex::new(pattern)
            .unwrap()
            .replace_all(template, "X")
            .into_owned()
    }

    #[test]
    fn kennt_genau_die_vier_laufarten() {
        let kinds: Vec<&str> = PromptTemplateCheck::KINDS
            .iter()
            .map(PromptKind::as_str)
            .collect();
        assert_eq!(kinds, ["work", "plan", "research", "merge"]);
    }

    #[test]
    fn meldet_fur_eine_vollstandige_vorlage_nichts() {
        let check = PromptTemplateCheck::new();
        for kind in PromptTemplateCheck::KINDS {
            assert_eq!(check.missing(kind, &sample(kind)), vec![], "{kind}");
        }
    }

    #[test]
    fn liefert_eindeutige_ids_und_nicht_leere_labels() {
        let check = PromptTemplateCheck::new();
        for kind in PromptTemplateCheck::KINDS {
            let parts = check.parts(kind);
            assert!(!parts.is_empty(), "{kind}");
            let ids: HashSet<&str> = parts.iter().map(|part| part.id.as_str()).collect();
            assert_eq!(ids.len(), parts.len(), "{kind}");
            for part in &parts {
                assert!(!part.label.trim().is_empty(), "{kind}/{}", part.id);
            }
        }
    }

    #[test]
    fn meldet_fur_eine_leere_vorlage_alle_pflichtteile() {
        let check = PromptTemplateCheck::new();
        for kind in PromptTemplateCheck::KINDS {
            let missing: Vec<String> = check.missing(kind, "").into_iter().map(|p| p.id).collect();
            let all: Vec<String> = check.parts(kind).into_iter().map(|p| p.id).collect();
            assert_eq!(missing, all, "{kind}");
        }
    }

    #[test]
    fn erkennt_jeden_fehlenden_pflichtteil_einzeln() {
        let check = PromptTemplateCheck::new();
        for kind in PromptTemplateCheck::KINDS {
            for part in check.parts(kind) {
                let without = remove_part(&sample(kind), &part.id);
                let missing: Vec<String> = check
                    .missing(kind, &without)
                    .into_iter()
                    .map(|p| p.id)
                    .collect();
                assert!(missing.contains(&part.id), "{kind}/{}", part.id);
            }
        }
    }

    #[test]
    fn die_standardvorlagen_bestehen_die_prufung() {
        let check = PromptTemplateCheck::new();
        for kind in PromptTemplateCheck::KINDS {
            let template = crate::dispatch::prompt_defaults::default_template(kind);
            assert_eq!(check.missing(kind, template), vec![], "{kind}");
        }
    }

    #[test]
    fn der_platzhalter_ausgange_erfullt_die_ausgang_pflichtteile() {
        let check = PromptTemplateCheck::new();
        let template = "Du bist {{agent}}, Ticket {{ticketId}}.\n{{outcomes}}";
        assert_eq!(check.missing(PromptKind::Work, template), vec![]);
        // Ohne Platzhalter und ohne Befehle fehlen sie weiterhin
        let missing: Vec<String> = check
            .missing(PromptKind::Work, "Du bist {{agent}}, Ticket {{ticketId}}.")
            .into_iter()
            .map(|part| part.id)
            .collect();
        assert_eq!(missing, ["done", "blocked", "question", "by-agent"]);
    }

    #[test]
    fn akzeptiert_zusatzlichen_leerraum_und_weitere_flags() {
        let check = PromptTemplateCheck::new();
        let tolerant = sample(PromptKind::Work).replace(
            "harness board done {{ticketId}}",
            "harness   board\tdone   {{ticketId}}",
        );
        assert_eq!(check.missing(PromptKind::Work, &tolerant), vec![]);
        let flags = sample(PromptKind::Plan)
            .replace("plan-json {{ticketId}}", "plan-json --quiet {{ticketId}}");
        assert_eq!(check.missing(PromptKind::Plan, &flags), vec![]);
    }
}
