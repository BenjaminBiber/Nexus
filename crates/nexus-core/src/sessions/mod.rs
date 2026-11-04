//! Sessions eines Projekts: die Nummern, der Sitzungsname, die Fachobjekte samt Hook-Ereignis,
//! der Zustandsspeicher, die Datei `sessions.json`, das Antwortformat und die Detailansicht
//! (Transcript, Änderungen, Laufdaten).
//! Portiert aus `packages/core/src/sessions`.

pub mod detail;
pub mod dto;
pub mod numbers;
pub mod persistence;
pub mod session;
pub mod store;
pub mod title;

pub use dto::{SessionDto, SubagentDto, TicketInfo};
pub use numbers::SessionNumbers;
pub use persistence::SessionPersistence;
pub use session::{
    DisplayState, HookEvent, HookKind, Session, SessionClient, SessionState, SessionView, Subagent,
    SubagentState, HOOK_KINDS,
};
pub use store::{SessionStore, RETENTION_MS, STALE_MS};
pub use title::SessionTitle;
