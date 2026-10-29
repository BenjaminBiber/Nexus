//! Zeitleiste eines Tickets: Ereignisse aus Verlauf, Kommentaren, Fragen und (im Server)
//! lokalen Quellen. Portiert aus `packages/core/src/board/TicketTimeline.ts`.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Art eines Ereignisses der Zeitleiste
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "timeline.ts")]
pub enum TimelineKind {
    Status,
    Claim,
    Run,
    Question,
    Git,
    Comment,
    Content,
    Other,
}

impl TimelineKind {
    /// Alle Arten in der Reihenfolge von TypeScript (`TIMELINE_KINDS`)
    pub const ALL: [TimelineKind; 8] = [
        TimelineKind::Status,
        TimelineKind::Claim,
        TimelineKind::Run,
        TimelineKind::Question,
        TimelineKind::Git,
        TimelineKind::Comment,
        TimelineKind::Content,
        TimelineKind::Other,
    ];

    /// Schreibweise in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            TimelineKind::Status => "status",
            TimelineKind::Claim => "claim",
            TimelineKind::Run => "run",
            TimelineKind::Question => "question",
            TimelineKind::Git => "git",
            TimelineKind::Comment => "comment",
            TimelineKind::Content => "content",
            TimelineKind::Other => "other",
        }
    }
}

impl fmt::Display for TimelineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Alle Arten in der Reihenfolge von TypeScript
pub const TIMELINE_KINDS: [TimelineKind; 8] = TimelineKind::ALL;

/// Ob ein Mensch oder ein Agent gehandelt hat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "timeline.ts")]
pub enum TimelineActorType {
    Person,
    Agent,
}

impl TimelineActorType {
    /// Schreibweise in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            TimelineActorType::Person => "person",
            TimelineActorType::Agent => "agent",
        }
    }
}

impl fmt::Display for TimelineActorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Wer gehandelt hat; `actor_type` ist eine Einschätzung am Namen bzw. an der Art des Autors
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "timeline.ts")]
pub struct TimelineActor {
    pub name: String,
    /// Heißt in JSON `type`; `type` ist in Rust ein Schlüsselwort
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub actor_type: TimelineActorType,
}

/// Verweise eines Ereignisses (Frage, Commit, Lauf-Protokoll, Profil, Exit-Code)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "timeline.ts")]
pub struct TimelineRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub question_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub run_log: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub profile: Option<String>,
    /// Darf auch ausdrücklich `null` sein (Lauf ohne Exit-Code)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exit_code: Option<Option<i32>>,
}

/// Ein Ereignis der Zeitleiste
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "timeline.ts")]
pub struct TimelineEvent {
    pub at: String,
    pub kind: TimelineKind,
    /// Genauer: `field:status`, `comment`, `dispatch:start`, `run-log`, `commit`, `merge`,
    /// `question:decided` …
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub event_type: String,
    pub actor: TimelineActor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub field: Option<String>,
    /// Die drei Felder des Verlaufs dürfen auch ausdrücklich `null` sein
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub from: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub to: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub note: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(rename = "ref")]
    #[ts(rename = "ref")]
    pub reference: Option<TimelineRef>,
}

/// Namen, an denen Agenten erkannt werden (aus der Dispatch-Konfiguration)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "timeline.ts")]
pub struct TimelineAgentNames {
    pub dispatcher_name: String,
    pub agent_prefix: String,
}

/// Die Zeitleiste eines Tickets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "timeline.ts")]
pub struct TicketTimeline {
    pub ticket_id: String,
    pub events: Vec<TimelineEvent>,
}

/// Sortiert aufsteigend nach Zeit; gleiche Zeit behält die Reihenfolge der Eingabe (stabil)
pub fn sort_timeline(events: &mut [TimelineEvent]) {
    events.sort_by(|a, b| a.at.cmp(&b.at));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Ereignis zu diesem Zeitpunkt mit diesem Typ
    fn event(at: &str, event_type: &str) -> TimelineEvent {
        TimelineEvent {
            at: at.to_string(),
            kind: TimelineKind::Other,
            event_type: event_type.to_string(),
            actor: TimelineActor {
                name: "Florian".into(),
                actor_type: TimelineActorType::Person,
            },
            field: None,
            from: None,
            to: None,
            note: None,
            text: None,
            reference: None,
        }
    }

    #[test]
    fn sortiert_aufsteigend_und_halt_gleiche_zeiten_in_der_reihenfolge_der_eingabe() {
        let mut events = vec![
            event("2026-10-13T12:00:00.000Z", "c"),
            event("2026-10-13T10:00:00.000Z", "a"),
            event("2026-10-13T12:00:00.000Z", "d"),
            event("2026-10-13T11:00:00.000Z", "b"),
        ];
        sort_timeline(&mut events);
        assert_eq!(
            events
                .iter()
                .map(|e| e.event_type.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c", "d"]
        );
    }

    #[test]
    fn schreibt_ein_leeres_feld_als_null_und_ein_fehlendes_gar_nicht() {
        let mut history = event("2026-10-13T10:00:00.000Z", "field:branch");
        history.field = Some("branch".into());
        history.from = Some(None);
        history.to = Some(Some("ticket/x".into()));
        let json = serde_json::to_string(&history).expect("das Ereignis ist serialisierbar");
        assert!(json.contains(r#""from":null"#), "{json}");
        assert!(json.contains(r#""to":"ticket/x""#), "{json}");
        // „note“, „text“ und „ref“ fehlen ganz, wie in TypeScript
        assert!(!json.contains("note"), "{json}");
        assert!(!json.contains("text"), "{json}");
        assert!(!json.contains("ref"), "{json}");
    }
}
