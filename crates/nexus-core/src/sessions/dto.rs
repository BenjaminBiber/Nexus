//! Antwortformat der Sessions (Liste, Zuordnung, Detailansicht); portiert aus
//! `packages/server/src/sessions/SessionDtoMapper.ts`. Liegt im Fachkern, damit ts-rs die
//! Typen zusammen mit der Detailansicht erzeugt.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::session::{
    DisplayState, SessionClient, SessionState, SessionView, Subagent, SubagentState,
};

/// Titel eines Tickets, das die Board-Datenbank nicht kennt
pub const UNKNOWN_TITLE: &str = "(unbekannt)";
/// Status eines Tickets, das die Board-Datenbank nicht kennt
pub const UNKNOWN_STATUS: &str = "backlog";

/// Angaben eines Tickets an einer Session oder einem Favoriten
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "session.ts")]
pub struct TicketInfo {
    pub id: String,
    pub title: String,
    pub status: String,
}

impl TicketInfo {
    /// Die Angaben aus der Zuordnung; ein unbekanntes Ticket wird als `(unbekannt)` gezeigt
    pub fn of(id: &str, info: &HashMap<String, TicketInfo>) -> TicketInfo {
        info.get(id).cloned().unwrap_or_else(|| TicketInfo {
            id: id.to_string(),
            title: UNKNOWN_TITLE.to_string(),
            status: UNKNOWN_STATUS.to_string(),
        })
    }

    /// Die Angaben zu allen Tickets in ihrer Reihenfolge
    pub fn all(ids: &[String], info: &HashMap<String, TicketInfo>) -> Vec<TicketInfo> {
        ids.iter().map(|id| TicketInfo::of(id, info)).collect()
    }
}

/// Ein Subagent im Antwortformat (fehlendes Ende als `null`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "session.ts")]
pub struct SubagentDto {
    pub id: String,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub kind: String,
    pub state: SubagentState,
    pub started_at: String,
    pub ended_at: Option<String>,
}

/// Eine Session im Antwortformat (fehlende Angaben als `null`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "session.ts")]
pub struct SessionDto {
    pub id: String,
    pub project_id: String,
    pub tickets: Vec<String>,
    pub ticket_info: Vec<TicketInfo>,
    pub cwd: String,
    pub state: SessionState,
    pub display_state: DisplayState,
    pub listening: bool,
    pub started_at: String,
    pub last_event_at: String,
    pub ended_at: Option<String>,
    pub runtime_ms: i64,
    pub title: Option<String>,
    pub model: Option<String>,
    pub subagents: Vec<SubagentDto>,
    /// Quelle der Session laut Hooks; ohne Angabe `unknown`
    pub client: SessionClient,
    /// Lauf-Protokoll des Dispatchers, nur bei Sessions von Nexus
    pub run_name: Option<String>,
}

impl SessionDto {
    /// Bildet eine Session-Ansicht auf das Antwortformat ab
    pub fn of(view: &SessionView, info: &HashMap<String, TicketInfo>) -> SessionDto {
        SessionDto {
            id: view.id.clone(),
            project_id: view.project_id.clone(),
            tickets: view.tickets.clone(),
            ticket_info: TicketInfo::all(&view.tickets, info),
            cwd: view.cwd.clone(),
            state: view.state,
            display_state: view.display_state,
            listening: view.listening,
            started_at: view.started_at.clone(),
            last_event_at: view.last_event_at.clone(),
            ended_at: view.ended_at.clone(),
            runtime_ms: view.runtime_ms,
            title: view.title.clone(),
            model: view.model.clone(),
            subagents: view.subagents.iter().map(subagent).collect(),
            client: client_of(view),
            run_name: view.run_name.clone(),
        }
    }
}

/// Quelle laut Hooks: ein Lauf des Dispatchers ist immer `nexus`
fn client_of(view: &SessionView) -> SessionClient {
    if view.run_name.is_some() {
        return SessionClient::Nexus;
    }
    view.client.unwrap_or_default()
}

/// Ein Subagent im Antwortformat
fn subagent(agent: &Subagent) -> SubagentDto {
    SubagentDto {
        id: agent.id.clone(),
        kind: agent.kind.clone(),
        state: agent.state,
        started_at: agent.started_at.clone(),
        ended_at: agent.ended_at.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::session::Session;
    use super::*;

    /// Ansicht einer Session ohne Angaben zur Herkunft
    fn view() -> SessionView {
        SessionView {
            session: Session {
                id: "s1".into(),
                project_id: "p".into(),
                tickets: vec!["T-0001".into()],
                cwd: "D:/wt".into(),
                state: SessionState::Idle,
                started_at: "2026-10-10T10:00:00.000Z".into(),
                last_event_at: "2026-10-10T10:00:00.000Z".into(),
                ended_at: None,
                title: None,
                model: None,
                subagents: Vec::new(),
                transcript_path: None,
                client: None,
                run_name: None,
            },
            display_state: DisplayState::Idle,
            listening: true,
            runtime_ms: 0,
        }
    }

    #[test]
    fn liefert_ohne_herkunft_unknown_und_keinen_lauf() {
        let json = serde_json::to_value(SessionDto::of(&view(), &HashMap::new())).unwrap();
        assert_eq!(json["client"], "unknown");
        assert_eq!(json["runName"], serde_json::Value::Null);
        assert_eq!(json["ticketInfo"][0]["title"], UNKNOWN_TITLE);
    }

    #[test]
    fn eine_session_mit_lauf_ist_von_nexus() {
        let mut view = view();
        view.session.client = Some(SessionClient::ClaudeCode);
        view.session.run_name = Some("T-0001-20261104-091500.log".into());
        let dto = SessionDto::of(&view, &HashMap::new());
        assert_eq!(dto.client, SessionClient::Nexus);
        assert_eq!(dto.run_name.as_deref(), Some("T-0001-20261104-091500.log"));
    }
}
