//! Typen, Regeln und Dienste des Boards: Enums, Grenzen, IDs, Labels, die Konfig, die DTOs
//! und die Fachlogik (Projekte, Boards, Tickets, Schritte, Kommentare, Notizen, Ansichten,
//! Abhängigkeiten) samt der Sammelstruktur [`BoardServices`].

pub mod basics;
pub mod board_service;
pub mod comment_service;
pub mod config;
pub mod config_store;
pub mod database;
pub mod dependency_cleanup;
pub mod dependency_graph;
pub mod dependency_graph_service;
pub mod dependency_rules;
pub mod dispatch_run_types;
pub mod in_memory;
pub mod inbox;
pub mod labels;
pub mod limits;
pub mod note_service;
pub mod note_similarity;
pub mod planning_candidate;
pub mod planning_service;
pub mod project_service;
pub mod question_blocking;
pub mod question_ids;
pub mod question_ticket_cleanup;
pub mod question_types;
pub mod repo_board_resolver;
pub mod repo_file;
pub mod repo_paths;
pub mod search_match;
pub mod search_types;
pub mod services;
pub mod spec_progress;
pub mod status_parser;
pub mod ticket_history_diff;
pub mod ticket_ids;
pub mod ticket_ordering;
pub mod ticket_service;
pub mod ticket_step_edits;
pub mod types;
pub mod view_service;

pub use basics::*;
pub use board_service::{BoardDeleteResult, BoardListFilter, BoardService, BoardSummary};
pub use comment_service::CommentService;
pub use config::{is_guid, BoardConfig, BoardProvider, BOARD_PROVIDERS};
pub use config_store::BoardConfigStore;
pub use database::*;
pub use dependency_cleanup::DependencyCleanup;
pub use dependency_graph::{
    DependencyGraph, DependencyGraphResult, GraphEdge, GraphNode, GraphOptions, GraphScope,
};
pub use dependency_graph_service::{
    DependencyGraphService, DependencyGraphView, GraphNodeView, GraphRequest,
};
pub use dependency_rules::{DependencyGraphMap, DependencyRules};
pub use dispatch_run_types::*;
pub use in_memory::{InMemoryBoardDatabase, InMemoryBoardSearch, SearchableState};
pub use inbox::types::*;
pub use labels::BoardLabels;
pub use limits::BoardLimits;
pub use note_service::{AddNoteOptions, NewNoteText, NoteService, TakeNoteOptions, TakenNote};
pub use note_similarity::{note_title, NoteSimilarity};
pub use planning_candidate::PlanningCandidate;
pub use planning_service::{PlanInput, PlanningService};
pub use project_service::ProjectService;
pub use question_blocking::BoardQuestionBlocking;
pub use question_ids::QuestionIds;
pub use question_ticket_cleanup::{DeletedTicket, QuestionTicketCleanup};
pub use question_types::*;
pub use repo_board_resolver::{RepoBoardOwner, RepoBoardResolver};
pub use repo_file::{BoardRepoEntries, BoardRepoFile};
pub use repo_paths::{
    BoardRepoPaths, BoardRepoSource, BoardRepoStore, BoardWithRepo, MemoryBoardRepoStore,
    RepoPathTarget,
};
pub use search_match::{BoardSearchMatch, SearchExcerpt};
pub use search_types::*;
pub use services::BoardServices;
pub use spec_progress::{SpecProgress, SpecProgressCount};
pub use status_parser::StatusParser;
pub use ticket_history_diff::TicketHistoryDiff;
pub use ticket_ids::{TicketId, TicketIds};
pub use ticket_ordering::{BoardColumn, TicketOrdering, COLUMN_ORDER};
pub use ticket_service::{
    AddStepOptions, ClaimOptions, MoveOptions, NewTicketInput, TicketDetail, TicketListFilter,
    TicketPatch, TicketService, TicketSummary, TicketView, WriteOptions,
};
pub use ticket_step_edits::{StepChange, StepEdit, StepGuard, StepPosition, TicketStepEdits};
pub use types::*;
pub use view_service::{BoardCard, BoardView, BoardViewColumn, BoardViewService};
