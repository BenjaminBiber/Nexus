//! Antwortformat der Sessions; die Typen liegen im Fachkern (`nexus_core::sessions::dto`),
//! damit ts-rs sie mit der Detailansicht erzeugt. Hier bleiben die bisherigen Namen.

pub use nexus_core::sessions::dto::{
    SessionDto, SubagentDto, TicketInfo, UNKNOWN_STATUS, UNKNOWN_TITLE,
};
