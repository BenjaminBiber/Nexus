//! Ein Kommentar zu einer Textstelle eines Dokuments.
//! Portiert aus `packages/core/src/docs/DocComment.ts`.

use serde::{Deserialize, Serialize};

/// Bearbeitungsstand eines Kommentars: offen oder erledigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocCommentStatus {
    Open,
    Resolved,
}

/// Ein gespeicherter Kommentar zu einer Textstelle eines Dokuments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocComment {
    pub id: String,
    pub status: DocCommentStatus,
    pub text: String,
    pub created_at: String,
    pub quote: String,
    pub heading: Option<String>,
    pub lines: Option<[i64; 2]>,
    pub resolution: Option<String>,
}

/// Angaben, aus denen ein neuer Kommentar entsteht.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDocComment {
    pub text: String,
    pub quote: String,
    pub heading: Option<String>,
    pub lines: Option<[i64; 2]>,
}

/// Änderbare Felder eines bestehenden Kommentars; ein nicht gesetztes Feld bleibt, wie es war
/// (`undefined` in TypeScript). `resolution: Some(None)` setzt die Antwort ausdrücklich auf leer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocCommentPatch {
    pub text: Option<String>,
    pub status: Option<DocCommentStatus>,
    pub resolution: Option<Option<String>>,
}

/// Ein offener Kommentar samt Pfad des Dokuments, zu dem er gehört.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDocComment {
    pub path: String,
    pub comment: DocComment,
}
