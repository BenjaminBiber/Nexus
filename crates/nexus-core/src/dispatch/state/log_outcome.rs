//! Liest den Ausgang eines Laufs aus dem Text seines Protokolls (Zeile
//! „[dispatch] <ISO> Ende: Exit-Code N[, Zeitlimit]“ des Workers). Portiert aus
//! `packages/core/src/dispatch/state/RunLogOutcome.ts`.

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::clock::iso;

/// Stand eines Laufs laut Ende seines Protokolls
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub enum RunState {
    /// Das Protokoll hat eine Ende-Zeile
    Finished,
    /// Noch kein Ende im Protokoll (läuft oder wurde hart beendet)
    #[default]
    Running,
}

/// Ausgang eines Laufs laut Ende seines Protokolls
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct RunOutcome {
    pub state: RunState,
    pub exit_code: Option<i64>,
    pub timed_out: bool,
    pub ended_at: Option<String>,
}

/// Ende-Zeile des Workers
fn end_pattern() -> Regex {
    Regex::new(r"\[dispatch\] (\S+) Ende: Exit-Code (-?\d+|null)(, Zeitlimit)?")
        .expect("festes Muster")
}

/// Meldung des Workers, wenn das Zeitlimit greift
const TIMEOUT_MARKER: &str = "[dispatch] Zeitlimit erreicht";

/// Liest den Ausgang aus dem Text eines Lauf-Protokolls.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunLogOutcome;

impl RunLogOutcome {
    /// Neuer Leser (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Ausgang laut Text; gezählt wird die letzte Ende-Zeile
    pub fn read(&self, text: &str) -> RunOutcome {
        let limit = text.contains(TIMEOUT_MARKER);
        let Some(last) = end_pattern().captures_iter(text).last() else {
            return RunOutcome {
                state: RunState::Running,
                exit_code: None,
                timed_out: limit,
                ended_at: None,
            };
        };
        let code = &last[2];
        RunOutcome {
            state: RunState::Finished,
            exit_code: if code == "null" {
                None
            } else {
                code.parse().ok()
            },
            timed_out: last.get(3).is_some() || limit,
            ended_at: DateTime::parse_from_rfc3339(&last[1])
                .ok()
                .map(|at| iso(at.with_timezone(&Utc))),
        }
    }
}
