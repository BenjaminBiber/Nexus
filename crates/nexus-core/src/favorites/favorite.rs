//! Gemerkte Sessions eines Projekts. Portiert aus `packages/core/src/favorites/Favorite.ts`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::sessions::SessionState;

/// Gemerkte Session eines Projekts: Notiz plus Momentaufnahme, damit sie auch nach der
/// 7-Tage-Frist erhalten bleibt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "session.ts")]
pub struct Favorite {
    pub project_id: String,
    pub session_id: String,
    pub note: String,
    pub tickets: Vec<String>,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    pub marked_at: String,
    pub last_state: SessionState,
    pub last_event_at: String,
    /// Kennungen fortgesetzter Sessions, die diesem Favoriten zugeordnet wurden
    pub resumed_as: Vec<String>,
}

/// Momentaufnahme einer Session beim Markieren oder Aktualisieren
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "session.ts")]
pub struct SessionSnapshot {
    pub session_id: String,
    pub tickets: Vec<String>,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    pub state: SessionState,
    pub last_event_at: String,
}
