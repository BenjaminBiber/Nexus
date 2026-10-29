//! Sessions des Servers: der Zustandsspeicher mit entprelltem Schreiben, das Antwortformat
//! und die Ticketangaben aus der Board-Datenbank.
//!
//! Portiert aus `packages/server/src/sessions/`.

pub mod session_dto;
pub mod session_service;
pub mod ticket_info_lookup;

pub use session_dto::{SessionDto, SubagentDto, TicketInfo, UNKNOWN_STATUS, UNKNOWN_TITLE};
pub use session_service::{SessionService, SAVE_DELAY_MS};
pub use ticket_info_lookup::{TicketInfoLookup, TicketSummaries, DEFAULT_TIMEOUT_MS};
