//! Dokumente des Projekts: Markdown-Abschnitte, Fingerabdruck, Kommentare neben dem
//! Dokument, Ticket-Entwürfe und der Scanner der Superpowers-Dokumente.
//! Portiert aus `packages/core/src/docs/`.

pub mod doc_comment;
pub mod doc_comment_store;
pub mod doc_hash;
pub mod markdown_sections;
pub mod superpowers_doc_scanner;
pub mod ticket_draft;
pub mod ticket_draft_validator;

pub use doc_comment::{
    DocComment, DocCommentPatch, DocCommentStatus, NewDocComment, OpenDocComment,
};
pub use doc_comment_store::DocCommentStore;
pub use doc_hash::DocHash;
pub use markdown_sections::{MarkdownSection, MarkdownSections};
pub use superpowers_doc_scanner::{
    is_doc_file_name, SuperpowersDoc, SuperpowersDocKind, SuperpowersDocScanner, SuperpowersTopic,
};
pub use ticket_draft::{TicketDraft, TicketDraftCheck, TicketDraftSource};
pub use ticket_draft_validator::{TicketDraftValidator, MAX_DOC_DRAFTS, MAX_DRAFTS};
