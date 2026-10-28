//! Zustände einer Frage; portiert aus `Question.ts`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Error, HarnessError};
use crate::text::join_names;

/// Zustand einer Frage; „bereit“, „wartet auf“ und „recherchiert“ werden berechnet, nicht gespeichert
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "question.ts")]
pub enum QuestionState {
    Open,
    Answered,
    Discarded,
}

impl QuestionState {
    /// Alle Zustände in der Reihenfolge von TypeScript (`QUESTION_STATES`)
    pub const ALL: [QuestionState; 3] = [
        QuestionState::Open,
        QuestionState::Answered,
        QuestionState::Discarded,
    ];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionState::Open => "open",
            QuestionState::Answered => "answered",
            QuestionState::Discarded => "discarded",
        }
    }

    /// Anzeigetext des Zustands (`QUESTION_STATE_LABELS`)
    pub fn label(&self) -> &'static str {
        match self {
            QuestionState::Open => "offen",
            QuestionState::Answered => "beantwortet",
            QuestionState::Discarded => "verworfen",
        }
    }
}

impl fmt::Display for QuestionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for QuestionState {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        QuestionState::ALL
            .into_iter()
            .find(|state| state.as_str() == input)
            .ok_or_else(|| {
                HarnessError::new(
                    "question-state-invalid",
                    format!(
                        "Zustand „{input}“ ist ungültig. Erlaubt: {}.",
                        join_names(QuestionState::ALL.map(|state| state.as_str()))
                    ),
                )
                .into()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zustande_haben_schreibweise_und_anzeigetext() {
        assert_eq!(
            QuestionState::ALL.map(|state| state.as_str()),
            ["open", "answered", "discarded"]
        );
        assert_eq!(QuestionState::Open.label(), "offen");
        assert_eq!(QuestionState::Answered.label(), "beantwortet");
        assert_eq!(QuestionState::Discarded.label(), "verworfen");
    }

    #[test]
    fn zustand_fahrt_als_json_rund() {
        let json = serde_json::to_string(&QuestionState::Discarded).unwrap();
        assert_eq!(json, r#""discarded""#);
        assert_eq!(
            serde_json::from_str::<QuestionState>(&json).unwrap(),
            QuestionState::Discarded
        );
        assert_eq!(
            "answered".parse::<QuestionState>().unwrap(),
            QuestionState::Answered
        );
    }

    #[test]
    fn unbekannter_zustand_wird_abgelehnt() {
        let error = "verworfen".parse::<QuestionState>().unwrap_err();
        assert_eq!(error.code(), Some("question-state-invalid"));
        assert_eq!(
            error.to_string(),
            "Zustand „verworfen“ ist ungültig. Erlaubt: open, answered, discarded."
        );
    }
}
