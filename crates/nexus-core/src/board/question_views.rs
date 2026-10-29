//! Berechnet die Anzeigefassung von Fragen: betroffene Tickets mit Titel, offene
//! Abhängigkeiten, bereit, recherchiert.
//! Portiert aus `packages/core/src/board/BoardQuestionViews.ts`.

use std::collections::HashMap;

use crate::board::basics::TicketStatus;
use crate::board::database::{BoardQuestionFilter, BoardUnitOfWork, TicketFilter};
use crate::board::question_ids::QuestionIds;
use crate::board::question_types::{
    BoardQuestion, BoardQuestionDetail, BoardQuestionTicketRef, BoardQuestionView,
    BoardQuestionWaiting,
};
use crate::board::types::Ticket;
use crate::error::Result;
use crate::questions::QuestionState;

/// Anzeigefassungen der Fragen.
pub struct BoardQuestionViews;

impl BoardQuestionViews {
    /// Anzeigefassungen in einer Transaktion; Ziele der Abhängigkeiten werden
    /// projektübergreifend aufgelöst
    pub async fn build(
        u: &mut dyn BoardUnitOfWork,
        questions: &[BoardQuestion],
    ) -> Result<Vec<BoardQuestionView>> {
        if questions.is_empty() {
            return Ok(Vec::new());
        }
        let mut ticket_refs: Vec<String> = Vec::new();
        let mut question_refs: Vec<String> = Vec::new();
        for question in questions {
            for id in &question.ticket_ids {
                if !ticket_refs.contains(id) {
                    ticket_refs.push(id.clone());
                }
            }
            for id in &question.depends_on {
                let list = if QuestionIds::is_question(id) {
                    &mut question_refs
                } else {
                    &mut ticket_refs
                };
                if !list.contains(id) {
                    list.push(id.clone());
                }
            }
        }
        let tickets: HashMap<String, Ticket> = if ticket_refs.is_empty() {
            HashMap::new()
        } else {
            u.tickets()
                .list(&TicketFilter {
                    ids: Some(ticket_refs),
                    ..TicketFilter::default()
                })
                .await?
                .into_iter()
                .map(|ticket| (ticket.id.clone(), ticket))
                .collect()
        };
        let others: HashMap<String, QuestionState> = if question_refs.is_empty() {
            HashMap::new()
        } else {
            u.questions()
                .list(&BoardQuestionFilter {
                    ids: Some(question_refs),
                    ..BoardQuestionFilter::default()
                })
                .await?
                .into_iter()
                .map(|question| (question.id, question.state))
                .collect()
        };
        let keys: HashMap<i32, String> = u
            .projects()
            .list()
            .await?
            .into_iter()
            .map(|project| (project.id, project.key))
            .collect();
        let ids: Vec<String> = questions.iter().map(|q| q.id.clone()).collect();
        let research = u.question_research().summaries(&ids).await?;
        Ok(questions
            .iter()
            .map(|question| {
                let waiting_on = Self::waiting_on(question, &tickets, &others);
                let summary = research.get(&question.id);
                BoardQuestionView {
                    project_key: keys
                        .get(&question.project_id)
                        .cloned()
                        .unwrap_or_else(|| "?".to_string()),
                    tickets: question
                        .ticket_ids
                        .iter()
                        .filter_map(|id| {
                            tickets.get(id).map(|ticket| BoardQuestionTicketRef {
                                id: id.clone(),
                                title: ticket.title.clone(),
                                status: ticket.status,
                                board_id: ticket.board_id,
                            })
                        })
                        .collect(),
                    ready: question.state == QuestionState::Open && waiting_on.is_empty(),
                    researched: summary.map(|s| s.count).unwrap_or(0) > 0,
                    option_count: summary.map(|s| s.option_count).unwrap_or(0),
                    waiting_on,
                    question: question.clone(),
                }
            })
            .collect())
    }

    /// Anzeigefassung samt Recherchen, Kommentaren und Verlauf
    pub async fn detail(
        u: &mut dyn BoardUnitOfWork,
        question: &BoardQuestion,
    ) -> Result<BoardQuestionDetail> {
        let view = Self::build(u, std::slice::from_ref(question))
            .await?
            .remove(0);
        Ok(BoardQuestionDetail {
            research: u.question_research().list(&question.id).await?,
            comments: u.question_comments().list(&question.id).await?,
            history: u.question_history().list(&question.id).await?,
            view,
        })
    }

    /// Offene Abhängigkeiten: Frage noch offen, Ticket weder erledigt noch „nicht mehr nötig“;
    /// unbekannte Ziele zählen nicht
    fn waiting_on(
        question: &BoardQuestion,
        tickets: &HashMap<String, Ticket>,
        others: &HashMap<String, QuestionState>,
    ) -> Vec<BoardQuestionWaiting> {
        let mut open = Vec::new();
        for id in &question.depends_on {
            if let Some(ticket) = tickets.get(id) {
                if !matches!(ticket.status, TicketStatus::Done | TicketStatus::NotNeeded) {
                    open.push(BoardQuestionWaiting {
                        id: id.clone(),
                        state: ticket.status.to_string(),
                    });
                }
            }
            if let Some(state) = others.get(id) {
                if *state == QuestionState::Open {
                    open.push(BoardQuestionWaiting {
                        id: id.clone(),
                        state: state.as_str().to_string(),
                    });
                }
            }
        }
        open
    }
}
