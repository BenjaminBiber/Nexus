//! Sammelt „Meine Aufgaben“: alles auf den eigenen Boards, was gerade auf den Benutzer wartet
//! (blockierte Tickets, Tickets in Planung, offene Fragen nach der Regel „Eigene“, offene
//! Notizen). Liefert die Einträge nach Dringlichkeit sortiert. Portiert aus
//! `packages/core/src/board/inbox/InboxService.ts`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::board::basics::{NoteState, TicketStatus};
use crate::board::database::{
    BoardDatabase, BoardFilter, BoardQuestionFilter, NoteFilter, TicketFilter,
};
use crate::board::inbox::ranking::InboxRanking;
use crate::board::inbox::types::{InboxCounts, InboxItem, InboxKind, InboxList};
use crate::board::note_similarity::note_title;
use crate::board::types::{Board, Note};
use crate::error::{HarnessError, Result};
use crate::questions::QuestionState;
use crate::text::{utf16_len, utf16_prefix};
use crate::tx;

/// Höchstlänge des Titels eines Eintrags
const TITLE_LENGTH: usize = 120;
/// Höchstlänge des Details eines Eintrags
const DETAIL_LENGTH: usize = 300;

/// Auswahl der Arbeitsliste
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InboxFilter {
    pub board_id: Option<i32>,
}

/// Die persönliche Arbeitsliste „Meine Aufgaben“.
#[derive(Debug, Clone)]
pub struct InboxService<D: BoardDatabase> {
    db: Arc<D>,
}

impl<D: BoardDatabase> InboxService<D> {
    /// Arbeitsliste auf dieser Datenbank
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    /// Alles, was auf den Benutzer wartet, nach Dringlichkeit sortiert
    pub async fn list(&self, owner_id: &str, filter: &InboxFilter) -> Result<Vec<InboxItem>> {
        let owner_id = owner_id.to_string();
        let filter = *filter;
        tx!(self.db, [owner_id, filter], |u| {
            let owned = u
                .boards()
                .list(&BoardFilter {
                    owner_id: Some(owner_id.clone()),
                    ..BoardFilter::default()
                })
                .await?;
            if let Some(board_id) = filter.board_id {
                if !owned.iter().any(|board| board.id == board_id) {
                    return Err(HarnessError::new(
                        "board-not-owned",
                        format!("Board {board_id} gehört nicht dir."),
                    )
                    .into());
                }
            }
            if owned.is_empty() {
                return Ok(Vec::new());
            }
            let own_ids: HashSet<i32> = owned.iter().map(|board| board.id).collect();
            let boards: HashMap<i32, Board> = owned
                .iter()
                .map(|board| (board.id, board.clone()))
                .collect();
            let projects: HashMap<i32, String> = u
                .projects()
                .list()
                .await?
                .into_iter()
                .map(|project| (project.id, project.key))
                .collect();
            let tickets = u
                .tickets()
                .list(&TicketFilter {
                    board_ids: Some(owned.iter().map(|board| board.id).collect()),
                    without_texts: true,
                    ..TicketFilter::default()
                })
                .await?;
            let board_of: HashMap<&str, i32> = tickets
                .iter()
                .map(|ticket| (ticket.id.as_str(), ticket.board_id))
                .collect();

            // Offene Fragen, die den Benutzer betreffen; ihre offenen Tickets erscheinen nur
            // über die Frage
            let questions: Vec<_> = u
                .questions()
                .list(&BoardQuestionFilter {
                    states: Some(vec![QuestionState::Open]),
                    ..BoardQuestionFilter::default()
                })
                .await?
                .into_iter()
                .filter(|question| {
                    // Die Regel „Eigene“ aus `QuestionConcern`: selbst gestellt (auch von
                    // einem Agent unter dem eigenen Benutzer) oder mit einem Ticket auf einem
                    // eigenen Board verknüpft
                    question.created_by_id == owner_id
                        || question.ticket_ids.iter().any(|id| {
                            board_of
                                .get(id.as_str())
                                .is_some_and(|board| own_ids.contains(board))
                        })
                })
                .collect();
            let referenced: Vec<String> = {
                let mut seen: HashSet<&str> = HashSet::new();
                questions
                    .iter()
                    .flat_map(|question| question.ticket_ids.iter())
                    .filter(|id| seen.insert(id.as_str()))
                    .cloned()
                    .collect()
            };
            let status: HashMap<String, TicketStatus> = if referenced.is_empty() {
                HashMap::new()
            } else {
                u.tickets()
                    .list(&TicketFilter {
                        ids: Some(referenced.clone()),
                        without_texts: true,
                        ..TicketFilter::default()
                    })
                    .await?
                    .into_iter()
                    .map(|ticket| (ticket.id, ticket.status))
                    .collect()
            };

            let mut covered: HashSet<String> = HashSet::new();
            let mut items: Vec<InboxItem> = Vec::new();
            for question in &questions {
                let open: Vec<&String> = question
                    .ticket_ids
                    .iter()
                    .filter(|id| {
                        status.get(id.as_str()).is_some_and(|status| {
                            *status != TicketStatus::Done && *status != TicketStatus::NotNeeded
                        })
                    })
                    .collect();
                for id in &open {
                    covered.insert((*id).clone());
                }
                let board_id = question
                    .ticket_ids
                    .iter()
                    .find_map(|id| board_of.get(id.as_str()).copied());
                if filter.board_id.is_some() && board_id != filter.board_id {
                    continue;
                }
                items.push(item(
                    InboxKind::Question,
                    format!("question:{}", question.id),
                    &question.title,
                    question.occasion.as_deref(),
                    board_id.and_then(|id| boards.get(&id)),
                    projects
                        .get(&question.project_id)
                        .map_or("?", String::as_str),
                    ItemLinks {
                        question_id: Some(question.id.clone()),
                        waiting_tickets: open.len() as i32,
                        ..ItemLinks::default()
                    },
                    &question.created_at,
                ));
            }

            for ticket in &tickets {
                let Some(kind) =
                    InboxRanking::classify(ticket.status, ticket.blocked_reason.as_deref())
                else {
                    continue;
                };
                if covered.contains(&ticket.id) {
                    continue;
                }
                if filter.board_id.is_some() && Some(ticket.board_id) != filter.board_id {
                    continue;
                }
                // `since` ist die letzte Änderung des Tickets: eine Verlaufsabfrage je Ticket
                // nur für den Zeitpunkt des Statuswechsels lohnt nicht, und die Änderungszeit
                // kommt ihm bei wartenden Tickets nahe
                items.push(item(
                    kind,
                    format!("ticket:{}", ticket.id),
                    &ticket.title,
                    if kind == InboxKind::Planning {
                        None
                    } else {
                        ticket.blocked_reason.as_deref()
                    },
                    boards.get(&ticket.board_id),
                    projects.get(&ticket.project_id).map_or("?", String::as_str),
                    ItemLinks {
                        ticket_id: Some(ticket.id.clone()),
                        priority: Some(ticket.priority.to_string()),
                        ..ItemLinks::default()
                    },
                    &ticket.updated_at,
                ));
            }

            for board in &owned {
                if filter.board_id.is_some() && Some(board.id) != filter.board_id {
                    continue;
                }
                let notes = u
                    .notes()
                    .list(&NoteFilter {
                        board_id: board.id,
                        states: Some(vec![NoteState::Open]),
                        source: None,
                    })
                    .await?;
                for note in notes {
                    items.push(note_item(
                        &note,
                        board,
                        projects.get(&board.project_id).map_or("?", String::as_str),
                    ));
                }
            }

            InboxRanking::sort(&mut items);
            Ok(items)
        })
        .await
    }

    /// Zähler je Art und gesamt
    pub fn summarize(items: Vec<InboxItem>) -> InboxList {
        let mut counts = InboxCounts {
            merge: 0,
            setup: 0,
            question: 0,
            accept: 0,
            blocked: 0,
            planning: 0,
            note: 0,
        };
        for item in &items {
            match item.kind {
                InboxKind::Merge => counts.merge += 1,
                InboxKind::Setup => counts.setup += 1,
                InboxKind::Question => counts.question += 1,
                InboxKind::Accept => counts.accept += 1,
                InboxKind::Blocked => counts.blocked += 1,
                InboxKind::Planning => counts.planning += 1,
                InboxKind::Note => counts.note += 1,
            }
        }
        InboxList {
            total: items.len() as i32,
            items,
            counts,
        }
    }
}

/// Die Verweise eines Eintrags; was nicht zur Art gehört, bleibt leer
#[derive(Debug, Clone, Default)]
struct ItemLinks {
    ticket_id: Option<String>,
    question_id: Option<String>,
    note_id: Option<i32>,
    waiting_tickets: i32,
    priority: Option<String>,
}

/// Ein Eintrag der Arbeitsliste
#[allow(clippy::too_many_arguments)]
fn item(
    kind: InboxKind,
    key: String,
    title: &str,
    detail: Option<&str>,
    board: Option<&Board>,
    project_key: &str,
    links: ItemLinks,
    since: &str,
) -> InboxItem {
    InboxItem {
        key,
        kind,
        title: title.to_string(),
        detail: detail
            .filter(|detail| !detail.is_empty())
            .map(|detail| clip(detail, DETAIL_LENGTH)),
        board_id: board.map(|board| board.id),
        board_name: board.map(|board| board.name.clone()),
        project_key: project_key.to_string(),
        ticket_id: links.ticket_id,
        question_id: links.question_id,
        note_id: links.note_id,
        waiting_tickets: links.waiting_tickets,
        priority: links.priority,
        since: since.to_string(),
        rank: InboxRanking::rank(kind),
    }
}

/// Eine offene Notiz als Eintrag
fn note_item(note: &Note, board: &Board, project_key: &str) -> InboxItem {
    // Ältere Notizen ohne Titel: erste Zeile des Inhalts als Titel, der Rest als Detail
    let body = note.body.trim();
    let lines: Vec<&str> = body
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect();
    let rest: Vec<&str> = if note.title.trim().is_empty() {
        lines.into_iter().skip(1).collect()
    } else {
        lines
    };
    let rest = collapse_whitespace(&rest.join(" "));
    let seen = if note.seen_count > 1 {
        format!("{}× aufgefallen", note.seen_count)
    } else {
        String::new()
    };
    let detail = [seen.as_str(), rest.as_str()]
        .into_iter()
        .filter(|teil| !teil.is_empty())
        .collect::<Vec<&str>>()
        .join(" · ");
    item(
        InboxKind::Note,
        format!("note:{}", note.id),
        &clip(&note_title(note), TITLE_LENGTH),
        (!detail.is_empty()).then_some(detail.as_str()),
        Some(board),
        project_key,
        ItemLinks {
            note_id: Some(note.id),
            ..ItemLinks::default()
        },
        &note.created_at,
    )
}

/// Kürzt den Text auf `max` Einheiten und setzt Auslassungspunkte
fn clip(text: &str, max: usize) -> String {
    if utf16_len(text) > max {
        format!("{}…", utf16_prefix(text, max - 1))
    } else {
        text.to_string()
    }
}

/// Fasst jede Folge von Leerraum zu einem Leerzeichen zusammen und schneidet die Ränder ab
fn collapse_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<&str>>().join(" ")
}

#[cfg(test)]
mod tests;
