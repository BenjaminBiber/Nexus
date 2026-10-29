//! Benachrichtigungen des Servers: Aufmerksamkeit der Sessions, Fragen-Beobachtung, Texte,
//! Toast-XML, ntfy und die Zustellung an Tab bzw. System.
//!
//! Portiert aus `packages/server/src/notifications/*`. Die Systemmeldung selbst liegt in
//! [`nexus_platform::notifier`]; hier entsteht nur, was gemeldet wird.

pub mod attention;
pub mod db_question_watch;
pub mod model;
pub mod ntfy;
pub mod question_changes;
pub mod service;
pub mod texts;
pub mod toast_xml;

pub use attention::{AttentionKind, AttentionTracker, PendingSession};
pub use db_question_watch::{DbQuestionSink, DbQuestionWatch, Query};
pub use model::{ToastKind, ToastModel, ToastOptions};
pub use ntfy::{NtfyFetch, NtfyResponse, NtfySender, NtfyTarget};
pub use question_changes::{QuestionChangeDetector, QuestionChangeKind, QuestionSnapshot};
pub use service::{NotificationService, NotificationSetup, QuestionChange, SessionChange};
pub use texts::{TicketRef, ToastTexts};
pub use toast_xml::ToastXmlBuilder;
