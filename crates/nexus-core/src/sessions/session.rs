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

/// Woher eine Session stammt: Dispatcher von Nexus, Claude Code (CLI) oder der Code-Tab der
/// Claude-Desktop-App
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "sessionDetail.ts")]
pub enum SessionClient {
    Nexus,
    ClaudeCode,
    ClaudeDesktop,
    #[default]
    Unknown,
}

impl SessionClient {
    /// Schreibweise in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionClient::Nexus => "nexus",
            SessionClient::ClaudeCode => "claude-code",
            SessionClient::ClaudeDesktop => "claude-desktop",
            SessionClient::Unknown => "unknown",
        }
    }

    /// Die Quelle zu ihrer Schreibweise; unbekannte Namen ergeben nichts
    pub fn from_str_opt(name: &str) -> Option<SessionClient> {
        [
            SessionClient::Nexus,
            SessionClient::ClaudeCode,
            SessionClient::ClaudeDesktop,
            SessionClient::Unknown,
        ]
        .into_iter()
        .find(|client| client.as_str() == name)
    }

    /// Quelle zum Einstiegspunkt von Claude Code (`entrypoint` im Transcript,
    /// `CLAUDE_CODE_ENTRYPOINT` im Hook): die Desktop-App, sonst Claude Code
    pub fn from_entrypoint(entrypoint: &str) -> SessionClient {
        if entrypoint == "claude-desktop" {
            SessionClient::ClaudeDesktop
        } else {
            SessionClient::ClaudeCode
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
    /// Pfad des Transcripts (JSONL von Claude Code), wie der Hook ihn meldet
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub transcript_path: Option<String>,
    /// Quelle der Session; `nexus` bleibt, wenn ein Lauf des Dispatchers sie gestartet hat
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub client: Option<SessionClient>,
    /// Name des Lauf-Protokolls des Dispatchers (`NEXUS_RUN`), z. B.
    /// `ARC-0001-20261104-091500.log`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub run_name: Option<String>,
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
    /// Pfad des Transcripts (`transcript_path` der Eingabe von Claude Code)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub transcript_path: Option<String>,
    /// Quelle laut Umgebung des Hooks (`CLAUDE_CODE_ENTRYPOINT`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub client: Option<SessionClient>,
    /// Lauf-Protokoll des Dispatchers (`NEXUS_RUN` bzw. `HARNESS_RUN`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub run: Option<String>,
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
            transcript_path: None,
            client: None,
            run: None,
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

    #[test]
    fn schreibt_transcript_quelle_und_lauf_des_ereignisses() {
        let event = HookEvent {
            transcript_path: Some("C:/t/s.jsonl".into()),
            client: Some(SessionClient::ClaudeDesktop),
            run: Some("ARC-0001-20261104-091500.log".into()),
            ..HookEvent::new(HookKind::Stop, "abc", "D:/wt")
        };
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"{"kind":"stop","sessionId":"abc","cwd":"D:/wt","transcriptPath":"C:/t/s.jsonl","client":"claude-desktop","run":"ARC-0001-20261104-091500.log"}"#
        );
    }

    #[test]
    fn leitet_die_quelle_aus_dem_einstiegspunkt_ab() {
        assert_eq!(
            SessionClient::from_entrypoint("claude-desktop"),
            SessionClient::ClaudeDesktop
        );
        assert_eq!(
            SessionClient::from_entrypoint("cli"),
            SessionClient::ClaudeCode
        );
        assert_eq!(
            SessionClient::from_entrypoint("sdk-cli"),
            SessionClient::ClaudeCode
        );
        assert_eq!(
            SessionClient::from_str_opt("claude-code"),
            Some(SessionClient::ClaudeCode)
        );
        assert_eq!(SessionClient::from_str_opt("quatsch"), None);
    }
}
