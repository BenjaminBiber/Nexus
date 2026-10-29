//! Die Zeilen, die der Schattenbetrieb nach `dispatch/shadow/` schreibt, und ihre Felder.
//!
//! Jede Zeile ist für sich lesbar: Datum, Lauf, Ticket und die Abweichungen mit beiden
//! Werten. Der Bericht (`nexus dispatch-compare`) liest nichts anderes.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Eine Abweichung zwischen dem, was Node getan hat, und dem, was Rust gerechnet hat.
/// Fehlt ein Wert auf einer Seite, steht dort `None`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ShadowDiff {
    /// Name des Feldes, so wie es in der Lauf-Datei bzw. der Lauf-Zeile heißt
    pub field: String,
    pub node: Option<String>,
    pub rust: Option<String>,
}

impl ShadowDiff {
    /// Abweichung eines Feldes
    pub fn new(field: impl Into<String>, node: Option<String>, rust: Option<String>) -> Self {
        Self {
            field: field.into(),
            node,
            rust,
        }
    }
}

/// Vergleich einer Entscheidung: Node hat einen Lauf gestartet, Rust hat ihn nachgerechnet
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ShadowDecision {
    /// Zeitpunkt des Vergleichs
    pub at: String,
    /// ID des Laufs aus `dispatch/runs`
    pub run_id: String,
    pub ticket_id: String,
    /// Art des Laufs (`work`, `merge`, `research`, `plan`)
    pub kind: String,
    pub equal: bool,
    #[serde(default)]
    pub diffs: Vec<ShadowDiff>,
    /// Unified-Diff der Arbeitsanweisung, wenn sie abweicht
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt_diff: Option<String>,
    /// Warum nichts nachgerechnet werden konnte (Ticket weg, Repo fehlt …); dann ist der
    /// Lauf weder gleich noch abweichend, sondern bleibt ohne Urteil
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub skipped: Option<String>,
}

/// Vergleich einer Auswertung: Node hat eine Zeile in `board.DispatchRuns` geschrieben, Rust
/// hat sie aus Lauf und Ergebnis nachgerechnet
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ShadowEvaluation {
    pub at: String,
    pub run_id: String,
    /// ID der Zeile in `board.DispatchRuns`; `None`, solange sie fehlt
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub row_id: Option<String>,
    pub ticket_id: String,
    pub kind: String,
    pub equal: bool,
    /// Die Zeile ist auch nach der Wartefrist nicht erschienen
    #[serde(default)]
    pub missing_row: bool,
    #[serde(default)]
    pub diffs: Vec<ShadowDiff>,
}

/// Abweichung der Bereitschaft eines Tickets zwischen Node-Sidecar und Rust; gleiche Urteile
/// schreibt der Schattenbetrieb nicht mit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ShadowReadiness {
    pub at: String,
    pub ticket_id: String,
    /// Urteil des Node-Sidecars (`bereit`, „wartet …“); `None` = Ticket fehlt dort
    pub node: Option<String>,
    /// Urteil von Rust; `None` = Ticket fehlt hier
    pub rust: Option<String>,
    /// Das Ticket hat sich kurz vorher geändert – die Abweichung ist vermutlich nur ein
    /// Zeitunterschied
    #[serde(default)]
    pub timing: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_entscheidung_ohne_abweichung_bleibt_knapp() {
        let line = ShadowDecision {
            at: "2026-11-01T10:00:00.000Z".into(),
            run_id: "ARC-0001-work-20261101-100000".into(),
            ticket_id: "ARC-0001".into(),
            kind: "work".into(),
            equal: true,
            diffs: Vec::new(),
            prompt_diff: None,
            skipped: None,
        };
        let text = serde_json::to_string(&line).expect("JSON");
        assert!(text.contains(r#""equal":true"#));
        assert!(!text.contains("promptDiff"));
        assert!(!text.contains("skipped"));
    }

    #[test]
    fn eine_abweichung_nennt_beide_werte() {
        let diff = ShadowDiff::new("run.profile", Some("leicht".into()), Some("schwer".into()));
        assert_eq!(
            serde_json::to_string(&diff).expect("JSON"),
            r#"{"field":"run.profile","node":"leicht","rust":"schwer"}"#
        );
    }
}
