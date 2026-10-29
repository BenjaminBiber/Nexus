//! Suche über alle Projekte und Boards der Board-Datenbank: Titel, Beschreibung, Kommentare,
//! Plan, Handbuch, Abnahme, Fragen (samt Entscheidung und Kommentaren) und Notizen. Je Ticket,
//! Frage oder Notiz ein Treffer (bestes Feld). Portiert aus
//! `packages/core/src/board/BoardSearchService.ts`.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::board::database::{BoardDatabase, BoardFilter};
use crate::board::search_types::{
    SearchBoardRef, SearchField, SearchHit, SearchKind, SearchProjectRef, SearchRequest,
    SearchResult, SearchRow, SEARCH_DEFAULT_LIMIT, SEARCH_KINDS, SEARCH_MAX_LENGTH,
    SEARCH_MAX_LIMIT, SEARCH_MAX_TERMS, SEARCH_MIN_LENGTH, SEARCH_PER_SOURCE,
};
use crate::error::{HarnessError, Result};
use crate::text::{natural_cmp, utf16_len, utf16_prefix, SearchSnippet, SnippetCut};
use crate::tx;

/// Höchstlänge des angezeigten Titels einer Notiz
const NOTE_TITLE_LENGTH: usize = 80;

/// Die Anfrage an die Suche
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchInput {
    pub q: String,
    /// Leer = alle Arten
    pub kinds: Vec<SearchKind>,
    pub limit: Option<usize>,
}

impl SearchInput {
    /// Eine Suche über alle Arten mit der Standardzahl an Treffern
    pub fn query(q: impl Into<String>) -> Self {
        Self {
            q: q.into(),
            ..Self::default()
        }
    }
}

/// Die Suche über alle Boards.
#[derive(Debug, Clone)]
pub struct BoardSearchService<D: BoardDatabase> {
    db: Arc<D>,
    snippets: SearchSnippet,
}

impl<D: BoardDatabase> BoardSearchService<D> {
    /// Suche auf dieser Datenbank
    pub fn new(db: Arc<D>) -> Self {
        Self {
            db,
            snippets: SearchSnippet::default(),
        }
    }

    /// Sucht die Begriffe der Anfrage; jeder Begriff muss im selben Feld vorkommen
    pub async fn search(&self, input: &SearchInput) -> Result<SearchResult> {
        let query = collapse_whitespace(&input.q);
        if utf16_len(&query) < SEARCH_MIN_LENGTH {
            return Err(HarnessError::new(
                "query-short",
                format!("Bitte mindestens {SEARCH_MIN_LENGTH} Zeichen eingeben."),
            )
            .into());
        }
        if utf16_len(&query) > SEARCH_MAX_LENGTH {
            return Err(HarnessError::new(
                "query-long",
                format!("Höchstens {SEARCH_MAX_LENGTH} Zeichen."),
            )
            .into());
        }
        let terms = unique_terms(&query);
        if terms.len() > SEARCH_MAX_TERMS {
            return Err(HarnessError::new(
                "query-long",
                format!("Höchstens {SEARCH_MAX_TERMS} Wörter."),
            )
            .into());
        }
        let kinds: Vec<SearchKind> = if input.kinds.is_empty() {
            SEARCH_KINDS.to_vec()
        } else {
            SEARCH_KINDS
                .into_iter()
                .filter(|kind| input.kinds.contains(kind))
                .collect()
        };
        let limit = input
            .limit
            .unwrap_or(SEARCH_DEFAULT_LIMIT)
            .clamp(1, SEARCH_MAX_LIMIT);

        let request = SearchRequest {
            terms: terms.clone(),
            kinds,
            per_source: SEARCH_PER_SOURCE as i32,
        };
        let snippets = self.snippets;
        let found = tx!(self.db, [request, terms, snippets], |u| {
            let found = u.search().find(&request).await?;
            let projects: HashMap<i32, (String, String)> = u
                .projects()
                .list()
                .await?
                .into_iter()
                .map(|project| (project.id, (project.key, project.name)))
                .collect();
            let boards: HashMap<i32, String> = u
                .boards()
                .list(&BoardFilter::default())
                .await?
                .into_iter()
                .map(|board| (board.id, board.name))
                .collect();

            // Je Ticket, Frage und Notiz nur die beste Zeile
            let mut rows = found.rows;
            rows.sort_by(compare);
            let mut seen: HashSet<(SearchKind, String)> = HashSet::new();
            let best: Vec<SearchRow> = rows
                .into_iter()
                .filter(|row| seen.insert((row.kind, row.id.clone())))
                .collect();

            let hits: Vec<SearchHit> = best
                .iter()
                .take(limit)
                .map(|row| {
                    let project = projects.get(&row.project_id);
                    let board = row
                        .board_id
                        .and_then(|id| boards.get(&id).map(|name| (id, name.clone())));
                    let snippet = snippets.build(
                        &row.text,
                        &terms,
                        &SnippetCut {
                            cut_before: row.text_start > 0,
                            cut_after: (row.text_start + utf16_len(&row.text) as i32)
                                < row.text_length,
                        },
                    );
                    SearchHit {
                        kind: row.kind,
                        id: row.id.clone(),
                        ticket_id: row.ticket_id.clone(),
                        field: row.field,
                        title: if row.kind == SearchKind::Note {
                            note_title(&row.title)
                        } else {
                            row.title.clone()
                        },
                        project: SearchProjectRef {
                            id: row.project_id,
                            key: project.map(|p| p.0.clone()).unwrap_or_default(),
                            name: project.map(|p| p.1.clone()).unwrap_or_default(),
                        },
                        board: board.map(|(id, name)| SearchBoardRef { id, name }),
                        status: row.status.clone(),
                        snippet: snippet.text,
                        matches: snippet.matches,
                        at: row.at.clone(),
                    }
                })
                .collect();
            Ok((hits, found.truncated || best.len() > limit))
        })
        .await?;

        Ok(SearchResult {
            query,
            terms,
            hits: found.0,
            truncated: found.1,
        })
    }
}

/// Kleinere Zahl steht weiter oben: Titeltreffer zuerst, Kommentare und Notizen zuletzt
fn field_rank(field: SearchField) -> u8 {
    match field {
        SearchField::Title => 0,
        SearchField::Acceptance | SearchField::Description | SearchField::Decision => 1,
        SearchField::Plan | SearchField::Handbook => 2,
        SearchField::Comment | SearchField::QuestionComment | SearchField::Note => 3,
    }
}

/// Tickets vor Fragen vor Notizen
fn kind_rank(kind: SearchKind) -> u8 {
    match kind {
        SearchKind::Ticket => 0,
        SearchKind::Question => 1,
        SearchKind::Note => 2,
    }
}

/// Feld, Art, neuester Zeitpunkt, höhere ID
fn compare(a: &SearchRow, b: &SearchRow) -> Ordering {
    field_rank(a.field)
        .cmp(&field_rank(b.field))
        .then_with(|| kind_rank(a.kind).cmp(&kind_rank(b.kind)))
        .then_with(|| {
            b.at.as_deref()
                .unwrap_or("")
                .cmp(a.at.as_deref().unwrap_or(""))
        })
        .then_with(|| natural_cmp(&b.id, &a.id))
}

/// Erste Zeile der Notiz, gekürzt
fn note_title(body: &str) -> String {
    let line = body
        .trim()
        .split('\n')
        .next()
        .unwrap_or("")
        .trim_end_matches('\r')
        .trim();
    if utf16_len(line) > NOTE_TITLE_LENGTH {
        format!("{}…", utf16_prefix(line, NOTE_TITLE_LENGTH - 1))
    } else {
        line.to_string()
    }
}

/// Fasst jede Folge von Leerraum zu einem Leerzeichen zusammen und schneidet die Ränder ab
fn collapse_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Die Wörter der Anfrage ohne Wiederholung: Reihenfolge des ersten Auftretens, Schreibweise
/// des letzten (wie `new Map(…)` in TypeScript)
fn unique_terms(query: &str) -> Vec<String> {
    let mut terms: Vec<(String, String)> = Vec::new();
    for term in query.split(' ') {
        let key = term.to_lowercase();
        match terms.iter_mut().find(|(known, _)| *known == key) {
            Some(entry) => entry.1 = term.to_string(),
            None => terms.push((key, term.to_string())),
        }
    }
    terms.into_iter().map(|(_, term)| term).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fasst_wiederholte_worter_zusammen_und_behalt_die_letzte_schreibweise() {
        assert_eq!(unique_terms("Login login LOGIN test"), ["LOGIN", "test"]);
        assert_eq!(unique_terms("a b"), ["a", "b"]);
    }

    #[test]
    fn kurzt_den_titel_einer_notiz_auf_die_erste_zeile() {
        assert_eq!(note_title("  Erste Zeile\nzweite  "), "Erste Zeile");
        let lang = "x".repeat(100);
        let gekurzt = note_title(&lang);
        assert_eq!(utf16_len(&gekurzt), NOTE_TITLE_LENGTH);
        assert!(gekurzt.ends_with('…'));
    }
}
