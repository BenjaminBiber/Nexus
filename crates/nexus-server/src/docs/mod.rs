//! Die Docs-Dienste des Servers: Quelle der Dokumente, Lesen und Schreiben, gespeicherte
//! Ticket-Entwürfe und Tickets aus Dokumenten, dazu die Dienste, die Claude rufen
//! (Ticket-Entwürfe erzeugen, Fragen herauslesen, einen Bereich umschreiben).
//! Portiert aus `packages/server/src/docs/`.

pub mod doc_draft_store;
pub mod doc_question_service;
pub mod doc_revision_service;
pub mod doc_ticket_service;
pub mod doc_ticket_source;
pub mod docs_claude;
pub mod docs_root_resolver;
pub mod docs_service;
pub mod ticket_draft_service;
pub mod ticket_drafts_saved;

pub use doc_draft_store::{DocDraftStore, TicketDraftResult};
pub use doc_question_service::{
    DocQuestionResult, DocQuestionService, QuestionDraft, QuestionDraftResult, MAX_QUESTION_DRAFTS,
};
pub use doc_revision_service::{DocRevisionService, ReviseDocInput, ReviseDocResultDto};
pub use doc_ticket_service::{DocTicketRef, DocTicketRequest, DocTicketResult, DocTicketService};
pub use docs_root_resolver::{DocsRootResolver, DocsSource, DocsSourceReason};
pub use docs_service::{
    DocFileContentDto, DocFileDto, DocsListDto, DocsService, SearchHitDto, WriteSectionInput,
    WriteSectionResult,
};
pub use ticket_draft_service::{TicketDraftRequest, TicketDraftSelection, TicketDraftService};
pub use ticket_drafts_saved::SavedTicketDrafts;
