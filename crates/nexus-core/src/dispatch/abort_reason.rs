//! Ordnet die Abbruchgründe des Dispatchers einer festen Kategorie zu; portiert aus
//! `packages/core/src/dispatch/AbortReasonClassifier.ts`.
//!
//! Die Gründe formulieren `RunOutputReader`, `ResultHandler`, `TicketStarter` und
//! `DispatchService`. Sie stehen so in `board.DispatchRuns.abortReason`, die Kategorie
//! daneben in `abortCategory` – beide Schreibweisen sind Vertrag.

use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Gründe, nach denen der Worker verschwunden ist (drei Formulierungen, eine Kategorie)
static VANISHED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(Worker lieferte .* kein Ergebnis|Worker-Prozess endete ohne Ergebnis|Kein Lebenszeichen seit)",
    )
    .expect("festes Muster")
});

/// Kategorie eines Abbruchs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "dispatch.ts")]
pub enum AbortCategory {
    Timeout,
    MaxTurns,
    Context,
    ExitCode,
    ClaudeError,
    NoOutcome,
    UsageLimit,
    StartFailed,
    Vanished,
    Other,
}

impl AbortCategory {
    /// Alle Kategorien in der Reihenfolge von TypeScript
    /// (`AbortReasonClassifier.CATEGORIES`)
    pub const ALL: [AbortCategory; 10] = [
        AbortCategory::Timeout,
        AbortCategory::MaxTurns,
        AbortCategory::Context,
        AbortCategory::ExitCode,
        AbortCategory::ClaudeError,
        AbortCategory::NoOutcome,
        AbortCategory::UsageLimit,
        AbortCategory::StartFailed,
        AbortCategory::Vanished,
        AbortCategory::Other,
    ];

    /// Schreibweise in JSON und in der Spalte `abortCategory`
    pub fn as_str(&self) -> &'static str {
        match self {
            AbortCategory::Timeout => "timeout",
            AbortCategory::MaxTurns => "max-turns",
            AbortCategory::Context => "context",
            AbortCategory::ExitCode => "exit-code",
            AbortCategory::ClaudeError => "claude-error",
            AbortCategory::NoOutcome => "no-outcome",
            AbortCategory::UsageLimit => "usage-limit",
            AbortCategory::StartFailed => "start-failed",
            AbortCategory::Vanished => "vanished",
            AbortCategory::Other => "other",
        }
    }

    /// Deutsche Beschriftung der Kategorie (`LABELS`)
    pub fn label(&self) -> &'static str {
        match self {
            AbortCategory::Timeout => "Zeitlimit",
            AbortCategory::MaxTurns => "Schrittlimit",
            AbortCategory::Context => "Kontext voll",
            AbortCategory::ExitCode => "Exit-Code",
            AbortCategory::ClaudeError => "Fehler von Claude",
            AbortCategory::NoOutcome => "Ohne Ausgang",
            AbortCategory::UsageLimit => "Nutzungslimit",
            AbortCategory::StartFailed => "Start gescheitert",
            AbortCategory::Vanished => "Worker verschwunden",
            AbortCategory::Other => "Sonstiges",
        }
    }
}

impl fmt::Display for AbortCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Alle Kategorien in der Reihenfolge von TypeScript
pub const ABORT_CATEGORIES: [AbortCategory; 10] = AbortCategory::ALL;

/// Ordnet die Abbruchgründe des Dispatchers einer festen Kategorie zu.
#[derive(Debug, Clone, Copy, Default)]
pub struct AbortReasonClassifier;

impl AbortReasonClassifier {
    /// Kategorie zum Grundtext; unbekannte Texte ergeben [`AbortCategory::Other`]
    pub fn category(reason: &str) -> AbortCategory {
        let text = reason.trim();
        if text.starts_with("Zeitlimit von ") {
            AbortCategory::Timeout
        } else if text.starts_with("Maximale Zahl an Schritten") {
            AbortCategory::MaxTurns
        } else if text.starts_with("Kontext voll") {
            AbortCategory::Context
        } else if text.starts_with("Claude endete mit Exit-Code") {
            AbortCategory::ExitCode
        } else if text.starts_with("Claude meldet einen Fehler") {
            AbortCategory::ClaudeError
        } else if text.starts_with("Lauf endete ohne Abschluss") {
            AbortCategory::NoOutcome
        } else if text.starts_with("Nutzungslimit") {
            AbortCategory::UsageLimit
        } else if text.starts_with("Worker konnte nicht gestartet werden") {
            AbortCategory::StartFailed
        } else if VANISHED.is_match(text) {
            AbortCategory::Vanished
        } else {
            AbortCategory::Other
        }
    }

    /// Deutsche Beschriftung einer Kategorie, wie sie in der Datenbank steht; eine
    /// unbekannte Kategorie wird mit ihrem eigenen Namen beschriftet
    pub fn label(category: &str) -> String {
        AbortCategory::ALL
            .into_iter()
            .find(|known| known.as_str() == category)
            .map_or_else(|| category.to_string(), |known| known.label().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Gründe stehen hier wörtlich so, wie `RunOutputReader.abortReason` sie formuliert;
    /// der Leser selbst kommt erst in Phase 4 nach Rust.
    #[test]
    fn ordnet_die_grunde_des_runoutputreader_genau_einer_kategorie_zu() {
        assert_eq!(
            AbortReasonClassifier::category("Zeitlimit von 90 min überschritten"),
            AbortCategory::Timeout
        );
        assert_eq!(
            AbortReasonClassifier::category("Maximale Zahl an Schritten erreicht (80)"),
            AbortCategory::MaxTurns
        );
        assert_eq!(
            AbortReasonClassifier::category("Kontext voll: Prompt is too long"),
            AbortCategory::Context
        );
        assert_eq!(
            AbortReasonClassifier::category("Claude endete mit Exit-Code 2"),
            AbortCategory::ExitCode
        );
        assert_eq!(
            AbortReasonClassifier::category("Claude meldet einen Fehler: error_during_execution"),
            AbortCategory::ClaudeError
        );
        assert_eq!(
            AbortReasonClassifier::category("Lauf endete ohne Abschluss"),
            AbortCategory::NoOutcome
        );
    }

    #[test]
    fn kennt_die_grunde_von_resulthandler_ticketstarter_und_dispatchservice() {
        assert_eq!(
            AbortReasonClassifier::category("Nutzungslimit zum 3. Mal in Folge"),
            AbortCategory::UsageLimit
        );
        assert_eq!(
            AbortReasonClassifier::category(
                "Worker konnte nicht gestartet werden: git-bash-missing"
            ),
            AbortCategory::StartFailed
        );
        assert_eq!(
            AbortReasonClassifier::category(
                "Worker lieferte 10 min nach dem Zeitlimit noch kein Ergebnis (Prozess hängt oder Prozess-ID neu vergeben)"
            ),
            AbortCategory::Vanished
        );
        assert_eq!(
            AbortReasonClassifier::category(
                "Worker-Prozess endete ohne Ergebnis (abgestürzt oder Rechner neu gestartet)"
            ),
            AbortCategory::Vanished
        );
        assert_eq!(
            AbortReasonClassifier::category(
                "Kein Lebenszeichen seit mehr als 30 min (Worker vermutlich abgestürzt oder Rechner neu gestartet)"
            ),
            AbortCategory::Vanished
        );
    }

    #[test]
    fn liefert_other_fur_unbekanntes_und_ein_label_je_kategorie() {
        assert_eq!(
            AbortReasonClassifier::category("irgendwas"),
            AbortCategory::Other
        );
        for category in AbortCategory::ALL {
            assert!(!AbortReasonClassifier::label(category.as_str())
                .trim()
                .is_empty());
        }
        // Eine Kategorie, die es (noch) nicht gibt, trägt ihren eigenen Namen
        assert_eq!(AbortReasonClassifier::label("neu"), "neu");
    }

    #[test]
    fn kennt_die_kategorie_kontext_voll() {
        assert!(ABORT_CATEGORIES.contains(&AbortCategory::Context));
        assert_eq!(
            AbortReasonClassifier::category("Kontext voll: Prompt is too long"),
            AbortCategory::Context
        );
        assert_eq!(AbortReasonClassifier::label("context"), "Kontext voll");
    }
}
