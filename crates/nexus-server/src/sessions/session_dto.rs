//! Antwortformat der Sessions; portiert aus
//! `packages/server/src/sessions/SessionDtoMapper.ts`.

use std::collections::HashMap;

use nexus_core::sessions::{DisplayState, SessionState, SessionView, Subagent, SubagentState};
use serde::{Deserialize, Serialize};

/// Titel eines Tickets, das die Board-Datenbank nicht kennt
pub const UNKNOWN_TITLE: &str = "(unbekannt)";
/// Status eines Tickets, das die Board-Datenbank nicht kennt
pub const UNKNOWN_STATUS: &str = "backlog";

/// Angaben eines Tickets an einer Session oder einem Favoriten
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentDto {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub state: SubagentState,
    pub started_at: String,
    pub ended_at: Option<String>,
}

/// Eine Session im Antwortformat (fehlende Angaben als `null`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
        }
    }
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
