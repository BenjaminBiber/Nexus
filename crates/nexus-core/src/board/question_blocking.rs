//! Ermittelt die offenen Fragen, auf die Tickets warten (abgeleitete Sperre).
//! Portiert aus `packages/core/src/board/BoardQuestionBlocking.ts`.

use std::collections::{HashMap, HashSet};

use crate::board::database::{BoardQuestionFilter, BoardUnitOfWork};
use crate::board::question_types::BoardOpenQuestion;
use crate::error::Result;
use crate::questions::QuestionState;

/// Ab so vielen Tickets werden alle offenen Fragen geladen statt nach Tickets gefiltert
/// (Grenze der SQL-Parameter)
const MANY_TICKETS: usize = 500;

/// Offene Fragen je Ticket.
pub struct BoardQuestionBlocking;

impl BoardQuestionBlocking {
    /// Offene Fragen je Ticket, sortiert nach Fragenummer; Tickets ohne offene Frage fehlen
    pub async fn open_by_ticket(
        u: &mut dyn BoardUnitOfWork,
        ticket_ids: &[String],
    ) -> Result<HashMap<String, Vec<BoardOpenQuestion>>> {
        let mut result: HashMap<String, Vec<BoardOpenQuestion>> = HashMap::new();
        if ticket_ids.is_empty() {
            return Ok(result);
        }
        let wanted: HashSet<&str> = ticket_ids.iter().map(String::as_str).collect();
        let filter = BoardQuestionFilter {
            ticket_ids: (ticket_ids.len() <= MANY_TICKETS).then(|| ticket_ids.to_vec()),
            states: Some(vec![QuestionState::Open]),
            ..BoardQuestionFilter::default()
        };
        for question in u.questions().list(&filter).await? {
            for ticket_id in &question.ticket_ids {
                if !wanted.contains(ticket_id.as_str()) {
                    continue;
                }
                result
                    .entry(ticket_id.clone())
                    .or_default()
                    .push(BoardOpenQuestion {
                        id: question.id.clone(),
                        title: question.title.clone(),
                    });
            }
        }
        Ok(result)
    }
}
