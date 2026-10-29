//! Die Bausteine der Hooks: Normalisierung der Ereignisse, die Env-Datei der Session, die
//! Marken der Fragen-Einspeisung, die Zuordnung zu Tickets und der eingespeiste Kontexttext.
//! Portiert aus `packages/core/src/hooks`. Die Ausführung im Binary folgt in einem eigenen
//! Ticket.

pub mod context_builder;
pub mod env_file_writer;
pub mod event_factory;
pub mod question_context_marks;
pub mod ticket_assigner;

pub use context_builder::{ContextBuilder, ContextKind, HookContext};
pub use env_file_writer::EnvFileWriter;
pub use event_factory::HookEventFactory;
pub use question_context_marks::QuestionContextMarks;
pub use ticket_assigner::{AssignInput, TicketAssigner};
