//! Suche über alle Boards: Anfrage an das Repository, gefundene Zeilen und Treffer
//! für die Oberfläche. Portiert aus `packages/core/src/board/BoardSearchTypes.ts`.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Durchsuchte Gegenstände: Tickets, Fragen und Notizen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "search.ts")]
pub enum SearchKind {
    Ticket,
    Question,
    Note,
}

impl SearchKind {
    /// Alle Arten in der Reihenfolge von TypeScript (`SEARCH_KINDS`)
    pub const ALL: [SearchKind; 3] = [SearchKind::Ticket, SearchKind::Question, SearchKind::Note];

    /// Schreibweise in JSON und in der Anfrage
    pub fn as_str(&self) -> &'static str {
        match self {
            SearchKind::Ticket => "ticket",
            SearchKind::Question => "question",
            SearchKind::Note => "note",
        }
    }
}

impl fmt::Display for SearchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Alle Arten in der Reihenfolge von TypeScript
pub const SEARCH_KINDS: [SearchKind; 3] = SearchKind::ALL;

/// Feld, in dem ein Treffer steht
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "search.ts")]
pub enum SearchField {
    Title,
    Description,
    Comment,
    Plan,
    Handbook,
    Acceptance,
    Decision,
    QuestionComment,
    Note,
}

impl SearchField {
    /// Alle Felder in der Reihenfolge von TypeScript
    pub const ALL: [SearchField; 9] = [
        SearchField::Title,
        SearchField::Description,
        SearchField::Comment,
        SearchField::Plan,
        SearchField::Handbook,
        SearchField::Acceptance,
        SearchField::Decision,
        SearchField::QuestionComment,
        SearchField::Note,
    ];

    /// Schreibweise in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            SearchField::Title => "title",
            SearchField::Description => "description",
            SearchField::Comment => "comment",
            SearchField::Plan => "plan",
            SearchField::Handbook => "handbook",
            SearchField::Acceptance => "acceptance",
            SearchField::Decision => "decision",
            SearchField::QuestionComment => "question-comment",
            SearchField::Note => "note",
        }
    }
}

impl fmt::Display for SearchField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// Grenzen der Eingabe und der Treffer
pub const SEARCH_MIN_LENGTH: usize = 2;
pub const SEARCH_MAX_LENGTH: usize = 100;
pub const SEARCH_MAX_TERMS: usize = 5;
pub const SEARCH_PER_SOURCE: usize = 30;
pub const SEARCH_MAX_LIMIT: usize = 100;
pub const SEARCH_DEFAULT_LIMIT: usize = 50;
/// Zeichen vor und nach der Fundstelle, die das Repository als Ausschnitt liefert
pub const SEARCH_WINDOW: usize = 150;

/// Anfrage an das Repository: alle Begriffe müssen im selben Feld vorkommen
/// (Groß/Klein egal)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "search.ts")]
pub struct SearchRequest {
    pub terms: Vec<String>,
    pub kinds: Vec<SearchKind>,
    /// Höchstens so viele Zeilen je Quelle (Tabelle und Feld)
    pub per_source: i32,
}

/// Gefundene Zeile: ein Feld eines Tickets, einer Frage oder einer Notiz
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "search.ts")]
pub struct SearchRow {
    pub kind: SearchKind,
    pub id: String,
    /// Ticket zum Öffnen (Ticket, Kommentar, übernommene Notiz); sonst null
    pub ticket_id: Option<String>,
    pub field: SearchField,
    pub title: String,
    pub project_id: i32,
    /// Fragen gehören zu keinem Board
    pub board_id: Option<i32>,
    pub status: String,
    /// Ausschnitt des Feldes um die erste Fundstelle (oder das ganze Feld)
    pub text: String,
    /// Position des Ausschnitts im Feld (0 = Anfang)
    pub text_start: i32,
    /// Länge des ganzen Feldes
    pub text_length: i32,
    pub at: Option<String>,
}

/// Gefundene Zeilen samt Hinweis, ob eine Quelle ihre Höchstzahl erreicht hat
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "search.ts")]
pub struct SearchRows {
    pub rows: Vec<SearchRow>,
    /// Mindestens eine Quelle hat ihre Höchstzahl erreicht
    pub truncated: bool,
}

/// Projekt eines Treffers, wie es die Oberfläche anzeigt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "search.ts")]
pub struct SearchProjectRef {
    pub id: i32,
    pub key: String,
    pub name: String,
}

/// Board eines Treffers, wie es die Oberfläche anzeigt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "search.ts")]
pub struct SearchBoardRef {
    pub id: i32,
    pub name: String,
}

/// Ein Treffer für die Oberfläche: Zeile samt Projekt, Board und Hervorhebungen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "search.ts")]
pub struct SearchHit {
    pub kind: SearchKind,
    pub id: String,
    pub ticket_id: Option<String>,
    pub field: SearchField,
    pub title: String,
    pub project: SearchProjectRef,
    pub board: Option<SearchBoardRef>,
    pub status: String,
    pub snippet: String,
    /// Hervorzuhebende Bereiche im Ausschnitt [Anfang, Ende) als Zeichenpositionen
    pub matches: Vec<(u32, u32)>,
    pub at: Option<String>,
}

/// Ergebnis einer Suche
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "search.ts")]
pub struct SearchResult {
    pub query: String,
    pub terms: Vec<String>,
    pub hits: Vec<SearchHit>,
    pub truncated: bool,
}
