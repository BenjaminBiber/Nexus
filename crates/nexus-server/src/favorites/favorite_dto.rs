//! Antwortformat der Favoriten; portiert aus
//! `packages/server/src/favorites/FavoriteDtoMapper.ts`.
//!
//! Ist die Session noch bekannt, gilt ihr aktueller Zustand; sonst der der Momentaufnahme.

use std::collections::HashMap;
use std::path::Path;

use nexus_core::favorites::{Favorite, ResumeCommand};
use nexus_core::sessions::{SessionState, SessionView};
use serde::{Deserialize, Serialize};

use crate::sessions::session_dto::TicketInfo;

/// Ein Favorit im Antwortformat
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteDto {
    pub session_id: String,
    pub note: String,
    pub tickets: Vec<String>,
    pub ticket_info: Vec<TicketInfo>,
    pub cwd: String,
    pub cwd_exists: bool,
    pub title: Option<String>,
    pub marked_at: String,
    pub state: SessionState,
    /// Zustand der laufenden Session; ohne sie der letzte bekannte Zustand
    pub display_state: String,
    pub live: bool,
    pub last_event_at: String,
    pub resumed_as: Vec<String>,
    pub command: String,
}

impl FavoriteDto {
    /// Ein Favorit mit optionaler laufender Session und Ticketangaben
    pub fn of(
        favorite: &Favorite,
        live: Option<&SessionView>,
        info: &HashMap<String, TicketInfo>,
    ) -> FavoriteDto {
        let tickets = live
            .map(|view| view.tickets.clone())
            .unwrap_or_else(|| favorite.tickets.clone());
        FavoriteDto {
            session_id: favorite.session_id.clone(),
            note: favorite.note.clone(),
            ticket_info: TicketInfo::all(&tickets, info),
            tickets,
            cwd: favorite.cwd.clone(),
            cwd_exists: Path::new(&favorite.cwd).exists(),
            title: live
                .and_then(|view| view.title.clone())
                .or_else(|| favorite.title.clone()),
            marked_at: favorite.marked_at.clone(),
            state: live.map(|view| view.state).unwrap_or(favorite.last_state),
            display_state: live
                .map(|view| view.display_state.as_str().to_string())
                .unwrap_or_else(|| favorite.last_state.as_str().to_string()),
            live: live.is_some(),
            last_event_at: live
                .map(|view| view.last_event_at.clone())
                .unwrap_or_else(|| favorite.last_event_at.clone()),
            resumed_as: favorite.resumed_as.clone(),
            command: ResumeCommand::build(&favorite.cwd, &favorite.session_id),
        }
    }
}
