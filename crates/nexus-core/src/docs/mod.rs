//! Dokumente des Projekts: Kommentare zu Spec- und Plan-Dokumenten, Glossar und ADRs.
//! Spiegelt `packages/core/src/docs`.

pub mod doc_comment;
pub mod doc_comment_store;

pub use doc_comment::{
    DocComment, DocCommentPatch, DocCommentStatus, NewDocComment, OpenDocComment,
};
pub use doc_comment_store::DocCommentStore;
