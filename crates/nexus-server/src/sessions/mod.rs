//! Sessions des Servers: der Zustandsspeicher mit entprelltem Schreiben, das Antwortformat,
//! die Ticketangaben aus der Board-Datenbank
//! und die Detailansicht (Transcript-Stände, Änderungen, Laufdaten, Beobachtung).
//!
//! Portiert aus `packages/server/src/sessions/`.

pub mod session_details;
pub mod session_dto;
pub mod session_service;
pub mod ticket_info_lookup;
pub mod transcript_watch;

pub use session_details::SessionDetails;
pub use session_dto::{SessionDto, SubagentDto, TicketInfo, UNKNOWN_STATUS, UNKNOWN_TITLE};
pub use session_service::{SessionService, SAVE_DELAY_MS};
pub use ticket_info_lookup::{TicketInfoLookup, TicketSummaries, DEFAULT_TIMEOUT_MS};
pub use transcript_watch::TranscriptWatch;
