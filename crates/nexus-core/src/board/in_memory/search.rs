//! Suche im Speicher; portiert aus `packages/core/src/board/InMemoryBoardSearch.ts`.
//!
//! Dieselbe Bedeutung wie `MssqlSearchRepository`: alle Begriffe im selben Feld, Groß/Klein egal,
//! je Quelle (Tabelle und Feld) höchstens `per_source` Zeilen, neueste zuerst; geliefert wird
//! das ganze Feld als Ausschnitt.

use std::cmp::{Ordering, Reverse};
use std::collections::BTreeMap;

use crate::board::question_types::{BoardQuestion, BoardQuestionComment};
use crate::board::search_match::BoardSearchMatch;
use crate::board::search_types::{SearchField, SearchKind, SearchRequest, SearchRow, SearchRows};
use crate::board::types::{Board, Comment, Note, Ticket};
use crate::text::utf16_len;

/// Die Daten, die die Suche im Speicher liest
#[derive(Debug, Clone, Copy)]
pub struct SearchableState<'a> {
    pub boards: &'a [Board],
    pub tickets: &'a [Ticket],
    pub comments: &'a [Comment],
    pub questions: &'a [BoardQuestion],
    pub question_comments: &'a [BoardQuestionComment],
    pub notes: &'a [Note],
}

/// Eine gefundene Zeile ohne Feld und Text – das trägt die jeweilige Quelle bei
/// (in TypeScript `Candidate`)
#[derive(Debug, Clone)]
struct Candidate {
    kind: SearchKind,
    id: String,
    ticket_id: Option<String>,
    title: String,
    project_id: i32,
    board_id: Option<i32>,
    status: String,
    at: Option<String>,
}

/// Suche im Speicher über Tickets, Fragen und Notizen
#[derive(Debug, Clone, Default)]
pub struct InMemoryBoardSearch;

impl InMemoryBoardSearch {
    /// Eine neue Suche; sie hält keinen Zustand
    pub fn new() -> Self {
        Self
    }

    /// Alle Zeilen, in denen jeder Begriff im selben Feld vorkommt
    pub fn find(&self, state: &SearchableState<'_>, request: &SearchRequest) -> SearchRows {
        let matcher = BoardSearchMatch::new(&request.terms);
        let mut treffer = Found {
            rows: Vec::new(),
            truncated: false,
            per_source: request.per_source.max(0) as usize,
            matcher: &matcher,
        };

        if request.kinds.contains(&SearchKind::Ticket) {
            let mut tickets: Vec<&Ticket> = state.tickets.iter().collect();
            tickets.sort_by(|a, b| natural_desc(&a.id, &b.id));
            for (field, text) in ticket_fields() {
                treffer.collect(
                    field,
                    &tickets,
                    |ticket| Some(text(ticket)),
                    |ticket| ticket_row(ticket),
                );
            }
            let by_id: BTreeMap<&str, &Ticket> = state
                .tickets
                .iter()
                .map(|ticket| (ticket.id.as_str(), ticket))
                .collect();
            let mut comments: Vec<&Comment> = state
                .comments
                .iter()
                .filter(|comment| by_id.contains_key(comment.ticket_id.as_str()))
                .collect();
            comments.sort_by_key(|comment| Reverse(comment.id));
            treffer.collect(
                SearchField::Comment,
                &comments,
                |comment| Some(comment.body.as_str()),
                |comment| Candidate {
                    at: Some(comment.created_at.clone()),
                    ..ticket_row(by_id[comment.ticket_id.as_str()])
                },
            );
        }

        if request.kinds.contains(&SearchKind::Question) {
            let mut questions: Vec<&BoardQuestion> = state.questions.iter().collect();
            questions.sort_by(|a, b| natural_desc(&a.id, &b.id));
            treffer.collect(
                SearchField::Title,
                &questions,
                |question| Some(question.title.as_str()),
                |question| question_row(question),
            );
            treffer.collect(
                SearchField::Description,
                &questions,
                |question| Some(question.description.as_str()),
                |question| question_row(question),
            );
            // Die Entscheidung wird als ein Feld durchsucht: Text und Begründung, durch
            // einen Zeilenumbruch getrennt (wie in TypeScript)
            let decisions: Vec<(&BoardQuestion, Option<String>)> = questions
                .iter()
                .map(|question| {
                    let text = question.decision.as_ref().map(|decision| {
                        format!(
                            "{}\n{}",
                            decision.text.as_deref().unwrap_or(""),
                            decision.reason
                        )
                    });
                    (*question, text)
                })
                .collect();
            treffer.collect(
                SearchField::Decision,
                &decisions,
                |(_, text)| text.as_deref(),
                |(question, _)| Candidate {
                    at: question
                        .decision
                        .as_ref()
                        .map(|decision| decision.at.clone()),
                    ..question_row(question)
                },
            );
            let by_id: BTreeMap<&str, &BoardQuestion> = state
                .questions
                .iter()
                .map(|question| (question.id.as_str(), question))
                .collect();
            let mut comments: Vec<&BoardQuestionComment> = state
                .question_comments
                .iter()
                .filter(|comment| by_id.contains_key(comment.question_id.as_str()))
                .collect();
            comments.sort_by_key(|comment| Reverse(comment.id));
            treffer.collect(
                SearchField::QuestionComment,
                &comments,
                |comment| Some(comment.body.as_str()),
                |comment| Candidate {
                    at: Some(comment.created_at.clone()),
                    ..question_row(by_id[comment.question_id.as_str()])
                },
            );
        }

        if request.kinds.contains(&SearchKind::Note) {
            let projects: BTreeMap<i32, i32> = state
                .boards
                .iter()
                .map(|board| (board.id, board.project_id))
                .collect();
            let mut notes: Vec<&Note> = state.notes.iter().collect();
            notes.sort_by_key(|note| Reverse(note.id));
            let texts: Vec<(&Note, String)> = notes
                .iter()
                .map(|note| (*note, format!("{}\n{}", note.title, note.body)))
                .collect();
            treffer.collect(
                SearchField::Note,
                &texts,
                |(_, text)| Some(text.as_str()),
                |(note, _)| Candidate {
                    kind: SearchKind::Note,
                    id: note.id.to_string(),
                    ticket_id: note.ticket_id.clone(),
                    title: if note.title.is_empty() {
                        crate::text::utf16_prefix(&note.body, 200).to_string()
                    } else {
                        note.title.clone()
                    },
                    project_id: projects.get(&note.board_id).copied().unwrap_or(0),
                    board_id: Some(note.board_id),
                    status: note.state.to_string(),
                    at: Some(note.created_at.clone()),
                },
            );
        }

        SearchRows {
            rows: treffer.rows,
            truncated: treffer.truncated,
        }
    }
}

/// Ein durchsuchtes Feld eines Tickets: Name und Zugriff auf den Text
type TicketField = (SearchField, for<'a> fn(&'a Ticket) -> &'a str);

/// Die durchsuchten Felder eines Tickets in der Reihenfolge von TypeScript
fn ticket_fields() -> [TicketField; 5] {
    [
        (SearchField::Title, |ticket| ticket.title.as_str()),
        (SearchField::Description, |ticket| {
            ticket.description.as_str()
        }),
        (SearchField::Acceptance, |ticket| ticket.acceptance.as_str()),
        (SearchField::Plan, |ticket| ticket.plan.as_str()),
        (SearchField::Handbook, |ticket| ticket.handbook.as_str()),
    ]
}

/// Die gesammelten Zeilen einer Suche samt Grenze je Quelle
struct Found<'a> {
    rows: Vec<SearchRow>,
    truncated: bool,
    per_source: usize,
    matcher: &'a BoardSearchMatch,
}

impl Found<'_> {
    /// Eine Quelle auswerten: bis `per_source` Treffer übernehmen, ein weiterer meldet „gekürzt“
    fn collect<T>(
        &mut self,
        field: SearchField,
        items: &[T],
        text: impl Fn(&T) -> Option<&str>,
        row: impl Fn(&T) -> Candidate,
    ) {
        let found: Vec<&T> = items
            .iter()
            .filter(|item| self.matcher.matches(text(item)))
            .take(self.per_source + 1)
            .collect();
        if found.len() > self.per_source {
            self.truncated = true;
        }
        for item in found.into_iter().take(self.per_source) {
            let value = text(item).expect("ein Treffer hat immer einen Text");
            let candidate = row(item);
            self.rows.push(SearchRow {
                kind: candidate.kind,
                id: candidate.id,
                ticket_id: candidate.ticket_id,
                field,
                title: candidate.title,
                project_id: candidate.project_id,
                board_id: candidate.board_id,
                status: candidate.status,
                text: value.to_string(),
                text_start: 0,
                text_length: utf16_len(value) as i32,
                at: candidate.at,
            });
        }
    }
}

/// Die gemeinsamen Angaben eines Ticket-Treffers
fn ticket_row(ticket: &Ticket) -> Candidate {
    Candidate {
        kind: SearchKind::Ticket,
        id: ticket.id.clone(),
        ticket_id: Some(ticket.id.clone()),
        title: ticket.title.clone(),
        project_id: ticket.project_id,
        board_id: Some(ticket.board_id),
        status: ticket.status.to_string(),
        at: None,
    }
}

/// Die gemeinsamen Angaben eines Fragen-Treffers
fn question_row(question: &BoardQuestion) -> Candidate {
    Candidate {
        kind: SearchKind::Question,
        id: question.id.clone(),
        ticket_id: None,
        title: question.title.clone(),
        project_id: question.project_id,
        board_id: None,
        status: question.state.to_string(),
        at: None,
    }
}

/// Absteigend nach ID, Zahlen im Text numerisch – wie `localeCompare(…, { numeric: true })`
/// in TypeScript (`ARC-0010` steht damit vor `ARC-0009`)
fn natural_desc(a: &str, b: &str) -> Ordering {
    natural_asc(b, a)
}

/// Aufsteigend nach ID; Ziffernfolgen zählen als Zahl, alles andere zeichenweise
fn natural_asc(a: &str, b: &str) -> Ordering {
    let (mut left, mut right) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let a_digits = take_digits(&mut left);
                let b_digits = take_digits(&mut right);
                let a_value = a_digits.trim_start_matches('0');
                let b_value = b_digits.trim_start_matches('0');
                let order = a_value
                    .len()
                    .cmp(&b_value.len())
                    .then_with(|| a_value.cmp(b_value));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                left.next();
                right.next();
                if x != y {
                    return x.cmp(&y);
                }
            }
        }
    }
}

/// Die nächste Ziffernfolge
fn take_digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(c) = chars.peek().copied() {
        if !c.is_ascii_digit() {
            break;
        }
        digits.push(c);
        chars.next();
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sortiert_ids_absteigend_mit_zahlen_als_zahl() {
        let mut ids = ["ARC-0002", "ARC-0010", "ARC-0001", "BRC-0001"];
        ids.sort_by(|a, b| natural_desc(a, b));
        assert_eq!(ids, ["BRC-0001", "ARC-0010", "ARC-0002", "ARC-0001"]);
    }

    #[test]
    fn zahlen_ohne_fuhrende_nullen_zahlen_numerisch() {
        assert_eq!(natural_asc("x9", "x10"), Ordering::Less);
        assert_eq!(natural_asc("x10", "x9"), Ordering::Greater);
        assert_eq!(natural_asc("x007", "x7"), Ordering::Equal);
    }
}
