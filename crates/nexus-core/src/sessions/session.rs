//! Fachobjekte der Sessions: Zustand, Subagent, Session samt berechneter Ansicht und das
//! normalisierte Hook-Ereignis. Portiert aus `packages/core/src/sessions/Session.ts`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Zustand einer Session, wie ihn die Hook-Ereignisse setzen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "session.ts")]
pub enum SessionState {
    Idle,
    Running,
    Waiting,
    Ended,
}

impl SessionState {
    /// Schreibweise in JSON und in den Dateien
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionState::Idle => "idle",
            SessionState::Running => "running",
            SessionState::Waiting => "waiting",
            SessionState::Ended => "ended",
        }
    }
}

/// Angezeigter Zustand: der Zustand der Session oder „veraltet“, wenn lange nichts kam
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "session.ts")]
pub enum DisplayState {
    Idle,
    Running,
    Waiting,
    Ended,
    Stale,
}

impl DisplayState {
    /// Schreibweise in JSON und in den Dateien
    pub fn as_str(&self) -> &'static str {
        match self {
            DisplayState::Idle => "idle",
            DisplayState::Running => "running",
            DisplayState::Waiting => "waiting",
            DisplayState::Ended => "ended",
            DisplayState::Stale => "stale",
        }
    }
}

impl From<SessionState> for DisplayState {
    fn from(state: SessionState) -> Self {
        match state {
            SessionState::Idle => DisplayState::Idle,
            SessionState::Running => DisplayState::Running,
            SessionState::Waiting => DisplayState::Waiting,
            SessionState::Ended => DisplayState::Ended,
        }
    }
}

/// Zustand eines Subagenten
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "session.ts")]
pub enum SubagentState {
    Running,
    Ended,
}

/// Ein Subagent einer Session mit Beginn und (nach dem Ende) Endzeitpunkt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "session.ts")]
pub struct Subagent {
    pub id: String,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub kind: String,
    pub state: SubagentState,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ended_at: Option<String>,
}

/// Ein Claude-Lauf mit Zuordnung zu Tickets und seinen Subagenten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "session.ts")]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub tickets: Vec<String>,
    pub cwd: String,
    pub state: SessionState,
    pub started_at: String,
    pub last_event_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ended_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
    pub subagents: Vec<Subagent>,
}

/// Beim Lesen berechnete Angaben: Anzeigezustand, Lauschen und Laufzeit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "session.ts")]
pub struct SessionView {
    #[serde(flatten)]
    #[ts(flatten)]
    pub session: Session,
    pub display_state: DisplayState,
    pub listening: bool,
    pub runtime_ms: i64,
}

impl std::ops::Deref for SessionView {
    type Target = Session;

    fn deref(&self) -> &Session {
        &self.session
    }
}

/// Die Ereignisse, die Claude Code über die Hooks meldet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "session.ts")]
pub enum HookKind {
    SessionStart,
    UserPrompt,
    Notification,
    Stop,
    SubagentStart,
    SubagentStop,
    SessionEnd,
}

impl HookKind {
    /// Schreibweise auf der Kommandozeile und in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            HookKind::SessionStart => "session-start",
            HookKind::UserPrompt => "user-prompt",
            HookKind::Notification => "notification",
            HookKind::Stop => "stop",
            HookKind::SubagentStart => "subagent-start",
            HookKind::SubagentStop => "subagent-stop",
            HookKind::SessionEnd => "session-end",
        }
    }

    /// Die Art zu ihrer Schreibweise; unbekannte Namen ergeben nichts
    pub fn from_str_opt(name: &str) -> Option<HookKind> {
        HOOK_KINDS.into_iter().find(|kind| kind.as_str() == name)
    }
}

/// Alle Ereignisarten in der Reihenfolge von TypeScript (`HOOK_KINDS`)
pub const HOOK_KINDS: [HookKind; 7] = [
    HookKind::SessionStart,
    HookKind::UserPrompt,
    HookKind::Notification,
    HookKind::Stop,
    HookKind::SubagentStart,
    HookKind::SubagentStop,
    HookKind::SessionEnd,
];

/// Normalisiertes Ereignis eines Hooks (CLI → Server).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "session.ts")]
pub struct HookEvent {
    pub kind: HookKind,
    pub session_id: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tickets: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub notification_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
    /// Quelle des Sessionstarts bei Claude Code, z. B. startup oder resume
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<String>,
}

impl HookEvent {
    /// Ein Ereignis, von dem nur Art, Session und Verzeichnis feststehen
    pub fn new(kind: HookKind, session_id: impl Into<String>, cwd: impl Into<String>) -> Self {
        Self {
            kind,
            session_id: session_id.into(),
            cwd: cwd.into(),
            at: None,
            tickets: None,
            agent_id: None,
            agent_type: None,
            notification_type: None,
            title: None,
            model: None,
            source: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nennt_die_ereignisarten_in_der_reihenfolge_von_typescript() {
        assert_eq!(
            HOOK_KINDS.map(|kind| kind.as_str()),
            [
                "session-start",
                "user-prompt",
                "notification",
                "stop",
                "subagent-start",
                "subagent-stop",
                "session-end",
            ]
        );
        assert_eq!(
            HookKind::from_str_opt("subagent-stop"),
            Some(HookKind::SubagentStop)
        );
        assert_eq!(HookKind::from_str_opt("quatsch"), None);
    }

    #[test]
    fn schreibt_das_ereignis_mit_den_feldnamen_des_hook_vertrags() {
        let event = HookEvent {
            tickets: Some(vec!["ARC-0001".into()]),
            agent_type: Some("Explore".into()),
            ..HookEvent::new(HookKind::SubagentStart, "abc", "D:/wt")
        };
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"{"kind":"subagent-start","sessionId":"abc","cwd":"D:/wt","tickets":["ARC-0001"],"agentType":"Explore"}"#
        );
    }
}
