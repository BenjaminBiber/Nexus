//! Typen und Regeln des Boards: Enums, Grenzen, IDs, Labels und die Konfig.

pub mod basics;
pub mod config;
pub mod config_store;
pub mod labels;
pub mod limits;
pub mod question_ids;
pub mod repo_file;
pub mod status_parser;
pub mod ticket_ids;

pub use basics::*;
pub use config::{is_guid, BoardConfig, BoardProvider, BOARD_PROVIDERS};
pub use config_store::BoardConfigStore;
pub use labels::BoardLabels;
pub use limits::BoardLimits;
pub use question_ids::QuestionIds;
pub use repo_file::{BoardRepoEntries, BoardRepoFile};
pub use status_parser::StatusParser;
pub use ticket_ids::{TicketId, TicketIds};
