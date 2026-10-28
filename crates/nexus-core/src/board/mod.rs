//! Typen und Regeln des Boards: Enums, Grenzen, IDs, Labels, die Konfig und die DTOs.

pub mod basics;
pub mod config;
pub mod config_store;
pub mod database;
pub mod dispatch_run_types;
pub mod inbox;
pub mod labels;
pub mod limits;
pub mod question_ids;
pub mod question_types;
pub mod repo_file;
pub mod search_match;
pub mod search_types;
pub mod status_parser;
pub mod ticket_ids;
pub mod types;

pub use basics::*;
pub use config::{is_guid, BoardConfig, BoardProvider, BOARD_PROVIDERS};
pub use config_store::BoardConfigStore;
pub use database::*;
pub use dispatch_run_types::*;
pub use inbox::types::*;
pub use labels::BoardLabels;
pub use limits::BoardLimits;
pub use question_ids::QuestionIds;
pub use question_types::*;
pub use repo_file::{BoardRepoEntries, BoardRepoFile};
pub use search_match::{BoardSearchMatch, SearchExcerpt};
pub use search_types::*;
pub use status_parser::StatusParser;
pub use ticket_ids::{TicketId, TicketIds};
pub use types::*;
