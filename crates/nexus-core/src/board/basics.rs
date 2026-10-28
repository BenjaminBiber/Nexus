//! Enums, Höchstlängen und kleine Regeln des Boards; portiert aus `BoardTypes.ts`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{Error, HarnessError};
use crate::text::join_names;

/// Spalten des Boards in der Reihenfolge der Anzeige
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    Backlog,
    InPlanning,
    Planned,
    InProgress,
    Blocked,
    Done,
    NotNeeded,
}

impl TicketStatus {
    /// Alle Status in der Reihenfolge von TypeScript (`TICKET_STATUSES`)
    pub const ALL: [TicketStatus; 7] = [
        TicketStatus::Backlog,
        TicketStatus::InPlanning,
        TicketStatus::Planned,
        TicketStatus::InProgress,
        TicketStatus::Blocked,
        TicketStatus::Done,
        TicketStatus::NotNeeded,
    ];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            TicketStatus::Backlog => "backlog",
            TicketStatus::InPlanning => "in_planning",
            TicketStatus::Planned => "planned",
            TicketStatus::InProgress => "in_progress",
            TicketStatus::Blocked => "blocked",
            TicketStatus::Done => "done",
            TicketStatus::NotNeeded => "not_needed",
        }
    }

    /// Spaltentitel der Anzeige (`STATUS_LABELS`)
    pub fn label(&self) -> &'static str {
        match self {
            TicketStatus::Backlog => "Backlog",
            TicketStatus::InPlanning => "In Planung",
            TicketStatus::Planned => "Geplant",
            TicketStatus::InProgress => "In Arbeit",
            TicketStatus::Blocked => "Blockiert",
            TicketStatus::Done => "Erledigt",
            TicketStatus::NotNeeded => "Nicht mehr nötig",
        }
    }
}

/// Titel der abgeleiteten Spalte „Wartet“; sie steht nach dem Backlog und ist kein Status
pub const WAITING_LABEL: &str = "Wartet";

/// Alle Status in der Reihenfolge von TypeScript (`TICKET_STATUSES`)
pub const TICKET_STATUSES: [TicketStatus; 7] = TicketStatus::ALL;

/// Meldung für eine Spaltenangabe, die kein Status ist – an einer Stelle, damit Parser und
/// `FromStr` wortgleich bleiben
pub(crate) fn status_invalid(input: &str) -> HarnessError {
    HarnessError::new(
        "status-invalid",
        format!(
            "Status „{input}“ ist ungültig. Erlaubt: {}.",
            join_names(TicketStatus::ALL.map(|status| status.as_str()))
        ),
    )
}

impl fmt::Display for TicketStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TicketStatus {
    type Err = Error;

    /// Nimmt nur die gespeicherte Schreibweise; Spaltenangaben aus CLI und Oberfläche
    /// gehen über [`crate::board::StatusParser`]
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        TicketStatus::ALL
            .into_iter()
            .find(|status| status.as_str() == input)
            .ok_or_else(|| status_invalid(input).into())
    }
}

/// Priorität eines Tickets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Normal,
    High,
    Urgent,
}

impl Priority {
    /// Alle Prioritäten in der Reihenfolge von TypeScript
    pub const ALL: [Priority; 4] = [
        Priority::Low,
        Priority::Normal,
        Priority::High,
        Priority::Urgent,
    ];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Low => "low",
            Priority::Normal => "normal",
            Priority::High => "high",
            Priority::Urgent => "urgent",
        }
    }

    /// Platz in der Sortierung (`PRIORITY_RANK`); kleinere Zahl steht weiter oben
    pub fn rank(&self) -> u8 {
        match self {
            Priority::Urgent => 0,
            Priority::High => 1,
            Priority::Normal => 2,
            Priority::Low => 3,
        }
    }
}

/// Alle Prioritäten in der Reihenfolge von TypeScript (`PRIORITIES`)
pub const PRIORITIES: [Priority; 4] = Priority::ALL;

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Priority {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Priority::ALL
            .into_iter()
            .find(|priority| priority.as_str() == input)
            .ok_or_else(|| {
                HarnessError::new(
                    "priority-invalid",
                    format!(
                        "Priorität „{input}“ ist ungültig. Erlaubt: {}.",
                        join_names(Priority::ALL.map(|priority| priority.as_str()))
                    ),
                )
                .into()
            })
    }
}

/// Stand einer Notiz: offen, in ein Ticket übernommen oder verworfen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteState {
    Open,
    Taken,
    Dropped,
}

impl NoteState {
    /// Alle Stände in der Reihenfolge von TypeScript (`NOTE_STATES`)
    pub const ALL: [NoteState; 3] = [NoteState::Open, NoteState::Taken, NoteState::Dropped];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            NoteState::Open => "open",
            NoteState::Taken => "taken",
            NoteState::Dropped => "dropped",
        }
    }
}

impl fmt::Display for NoteState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for NoteState {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        NoteState::ALL
            .into_iter()
            .find(|state| state.as_str() == input)
            .ok_or_else(|| {
                HarnessError::new(
                    "state-invalid",
                    format!(
                        "Stand „{input}“ ist ungültig. Erlaubt: {}.",
                        join_names(NoteState::ALL.map(|state| state.as_str()))
                    ),
                )
                .into()
            })
    }
}

/// Wer handelt: Benutzer-GUID aus der Konfig und der Name im Verlauf (z. B. „claude“)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardActor {
    pub user_id: String,
    pub name: String,
}

/// Dauer, nach der ein Planungs-Claim verfällt
pub const PLANNING_CLAIM_TTL_MS: i64 = 15 * 60 * 1000;

// Höchstlängen der Texte (passen zu den Spalten in board.*); länger ergibt „value-too-long“
pub const MAX_TITLE_LENGTH: usize = 400;
pub const MAX_BLOCKED_REASON_LENGTH: usize = 500;
/// Namen von Handelnden, Zuständigen und Agenten
pub const MAX_PERSON_NAME_LENGTH: usize = 100;
pub const MAX_LABEL_LENGTH: usize = 100;
pub const MAX_PROJECT_NAME_LENGTH: usize = 200;
pub const MAX_BOARD_NAME_LENGTH: usize = 200;
pub const MAX_NOTE_REASON_LENGTH: usize = 1000;
/// Labels einer Notiz, kommagetrennt gespeichert
pub const MAX_NOTE_LABELS_LENGTH: usize = 1000;
/// Titel einer Notiz: eine Zeile, die ohne Kontext verständlich ist
pub const MAX_NOTE_TITLE_LENGTH: usize = 200;
/// Tickets, bei denen eine Notiz erneut aufgefallen ist, kommagetrennt gespeichert
pub const MAX_NOTE_SEEN_IN_LENGTH: usize = 1000;
/// Spec-Ordner (z. B. 001-login) und Aufgabennummer (T012) am Ticket
pub const MAX_SPEC_LENGTH: usize = 200;
pub const MAX_SPEC_TASK_LENGTH: usize = 20;
/// Branch und Worktree-Pfad am Ticket
pub const MAX_BRANCH_LENGTH: usize = 400;
pub const MAX_WORKTREE_LENGTH: usize = 500;
/// Text eines Arbeitsschritts und Kennung einer Claude-Session
pub const MAX_STEP_TEXT_LENGTH: usize = 1000;
pub const MAX_SESSION_ID_LENGTH: usize = 100;

/// Vorsilbe des Blockiert-Grunds „fertig, ein Mensch muss es noch ansehen“;
/// nur so blockierte Tickets lassen sich abnehmen
pub const HUMAN_INTERACTION_PREFIX: &str = "Human Interaction:";

/// Wartet das Ticket mit diesem Blockiert-Grund auf die Abnahme durch einen Menschen?
/// („Human Interaction“ ohne Doppelpunkt zählt mit)
pub fn is_human_interaction(reason: Option<&str>) -> bool {
    let text = reason.unwrap_or("").trim();
    text.starts_with(HUMAN_INTERACTION_PREFIX)
        || text == &HUMAN_INTERACTION_PREFIX[..HUMAN_INTERACTION_PREFIX.len() - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_behalt_reihenfolge_schreibweise_und_titel() {
        assert_eq!(
            TicketStatus::ALL.map(|status| status.as_str()),
            [
                "backlog",
                "in_planning",
                "planned",
                "in_progress",
                "blocked",
                "done",
                "not_needed"
            ]
        );
        assert_eq!(TicketStatus::InPlanning.label(), "In Planung");
        assert_eq!(TicketStatus::NotNeeded.label(), "Nicht mehr nötig");
        assert_eq!(WAITING_LABEL, "Wartet");
    }

    #[test]
    fn status_fahrt_als_json_rund() {
        let json = serde_json::to_string(&TicketStatus::InPlanning).unwrap();
        assert_eq!(json, r#""in_planning""#);
        assert_eq!(
            serde_json::from_str::<TicketStatus>(&json).unwrap(),
            TicketStatus::InPlanning
        );
        assert_eq!(
            serde_json::from_str::<TicketStatus>(r#""not_needed""#).unwrap(),
            TicketStatus::NotNeeded
        );
        assert_eq!(
            "not_needed".parse::<TicketStatus>().unwrap(),
            TicketStatus::NotNeeded
        );
        assert_eq!(TicketStatus::NotNeeded.to_string(), "not_needed");
    }

    #[test]
    fn unbekannter_status_wird_mit_status_invalid_abgelehnt() {
        let error = "nicht-mehr-nötig".parse::<TicketStatus>().unwrap_err();
        assert_eq!(error.code(), Some("status-invalid"));
        assert_eq!(
            error.to_string(),
            "Status „nicht-mehr-nötig“ ist ungültig. Erlaubt: backlog, in_planning, planned, in_progress, blocked, done, not_needed."
        );
    }

    #[test]
    fn prioritaten_sortieren_dringend_nach_oben() {
        assert_eq!(
            PRIORITIES.map(|priority| priority.as_str()),
            ["low", "normal", "high", "urgent"]
        );
        let mut sortiert = PRIORITIES;
        sortiert.sort_by_key(Priority::rank);
        assert_eq!(
            sortiert,
            [
                Priority::Urgent,
                Priority::High,
                Priority::Normal,
                Priority::Low
            ]
        );
    }

    #[test]
    fn unbekannte_prioritat_wird_mit_priority_invalid_abgelehnt() {
        let error = "mittel".parse::<Priority>().unwrap_err();
        assert_eq!(error.code(), Some("priority-invalid"));
        assert_eq!(
            error.to_string(),
            "Priorität „mittel“ ist ungültig. Erlaubt: low, normal, high, urgent."
        );
    }

    #[test]
    fn notiz_stande_fahren_rund() {
        assert_eq!(
            NoteState::ALL.map(|state| state.as_str()),
            ["open", "taken", "dropped"]
        );
        assert_eq!("taken".parse::<NoteState>().unwrap(), NoteState::Taken);
        assert_eq!(
            serde_json::to_string(&NoteState::Dropped).unwrap(),
            r#""dropped""#
        );
        let error = "offen".parse::<NoteState>().unwrap_err();
        assert_eq!(error.code(), Some("state-invalid"));
        assert_eq!(
            error.to_string(),
            "Stand „offen“ ist ungültig. Erlaubt: open, taken, dropped."
        );
    }

    #[test]
    fn hochstlangen_passen_zu_den_spalten() {
        assert_eq!(MAX_TITLE_LENGTH, 400);
        assert_eq!(MAX_BLOCKED_REASON_LENGTH, 500);
        assert_eq!(MAX_PERSON_NAME_LENGTH, 100);
        assert_eq!(MAX_SESSION_ID_LENGTH, 100);
        assert_eq!(PLANNING_CLAIM_TTL_MS, 900_000);
    }

    #[test]
    fn erkennt_den_grund_fur_menschliche_abnahme() {
        assert!(is_human_interaction(Some(
            "Human Interaction: bitte Oberfläche ansehen"
        )));
        // ohne Doppelpunkt zählt mit, auch mit Leerraum ringsum
        assert!(is_human_interaction(Some("  Human Interaction  ")));
        assert!(is_human_interaction(Some("Human Interaction:")));
        assert!(!is_human_interaction(Some(
            "human interaction: klein geschrieben"
        )));
        assert!(!is_human_interaction(Some("Warte auf Zugang")));
        assert!(!is_human_interaction(Some("")));
        assert!(!is_human_interaction(None));
    }

    #[test]
    fn board_akteur_nutzt_camelcase() {
        let actor = BoardActor {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            name: "claude".into(),
        };
        assert_eq!(
            serde_json::to_value(&actor).unwrap(),
            serde_json::json!({ "userId": "11111111-1111-1111-1111-111111111111", "name": "claude" })
        );
        assert_eq!(
            serde_json::from_value::<BoardActor>(
                serde_json::json!({ "userId": "11111111-1111-1111-1111-111111111111", "name": "claude" })
            )
            .unwrap(),
            actor
        );
    }
}
