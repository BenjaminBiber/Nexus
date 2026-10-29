//! Die Docs-Dienste des Servers: Quelle der Dokumente, Lesen und Schreiben, gespeicherte
//! Ticket-Entwürfe und Tickets aus Dokumenten.
//! Portiert aus `packages/server/src/docs/`.

pub mod doc_draft_store;
pub mod doc_ticket_service;
pub mod doc_ticket_source;
pub mod docs_root_resolver;
pub mod docs_service;
pub mod ticket_drafts_saved;

pub use doc_draft_store::{DocDraftStore, TicketDraftResult};
pub use doc_ticket_service::{DocTicketRef, DocTicketRequest, DocTicketResult, DocTicketService};
pub use docs_root_resolver::{DocsRootResolver, DocsSource, DocsSourceReason};
pub use docs_service::{
    DocFileContentDto, DocFileDto, DocsListDto, DocsService, SearchHitDto, WriteSectionInput,
    WriteSectionResult,
};
pub use ticket_drafts_saved::SavedTicketDrafts;
