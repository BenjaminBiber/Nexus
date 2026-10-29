//! Zeitleiste eines Tickets aus der Board-Datenbank: Verlauf, Kommentare (samt
//! Dispatcher-Ereignissen) und verknüpfte Fragen. Lokale Quellen (Lauf-Protokolle, Git)
//! ergänzt der Server. Portiert aus `packages/core/src/board/TicketTimelineService.ts`.

use std::collections::HashSet;
use std::sync::Arc;

use crate::board::database::{BoardDatabase, BoardQuestionFilter};
use crate::board::question_types::BoardQuestionHistoryType;
use crate::board::ticket_ids::TicketIds;
use crate::board::ticket_timeline::{
    sort_timeline, TicketTimeline, TimelineAgentNames, TimelineEvent, TimelineKind, TimelineRef,
};
use crate::board::timeline_classifier::TimelineClassifier;
use crate::error::{HarnessError, Result};
use crate::tx;

/// Einträge im Verlauf einer Frage, die für die Zeitleiste eines betroffenen Tickets zählen
const QUESTION_TYPES: [BoardQuestionHistoryType; 6] = [
    BoardQuestionHistoryType::Created,
    BoardQuestionHistoryType::Research,
    BoardQuestionHistoryType::Tickets,
    BoardQuestionHistoryType::Decided,
    BoardQuestionHistoryType::Discarded,
    BoardQuestionHistoryType::Reopened,
];

/// Die Zeitleiste eines Tickets.
#[derive(Debug, Clone)]
pub struct TicketTimelineService<D: BoardDatabase> {
    db: Arc<D>,
    names: Option<TimelineAgentNames>,
}

impl<D: BoardDatabase> TicketTimelineService<D> {
    /// Zeitleiste auf dieser Datenbank; `names` aus der Dispatch-Konfiguration erkennt
    /// Agenten (`None` = nur „claude“ und Art des Autors)
    pub fn new(db: Arc<D>, names: Option<TimelineAgentNames>) -> Self {
        Self { db, names }
    }

    /// Liest alles in einer Transaktion; eine unbekannte ID ergibt „ticket-not-found“
    pub async fn get(&self, id: &str) -> Result<TicketTimeline> {
        let ticket_id = TicketIds::canonical(id)?;
        let classifier = TimelineClassifier::new(self.names.clone());
        tx!(self.db, [ticket_id, classifier], |u| {
            if u.tickets().by_id(&ticket_id, false).await?.is_none() {
                return Err(HarnessError::new(
                    "ticket-not-found",
                    format!("Ticket {ticket_id} gibt es nicht."),
                )
                .into());
            }
            let history = u.history().list(&ticket_id).await?;
            let comments = u.comments().list(&ticket_id).await?;
            let questions = u
                .questions()
                .list(&BoardQuestionFilter {
                    ticket_ids: Some(vec![ticket_id.clone()]),
                    ..BoardQuestionFilter::default()
                })
                .await?;
            let linked: HashSet<String> = questions
                .iter()
                .map(|question| question.id.clone())
                .collect();

            let mut events: Vec<TimelineEvent> = Vec::new();
            for entry in &history {
                let event = classifier.history(entry);
                // Zustandswechsel verknüpfter Fragen kommen aus deren eigenem Verlauf
                // (mit Art des Autors)
                let verknupft = entry.field == "question"
                    && event
                        .reference
                        .as_ref()
                        .and_then(|r| r.question_id.as_deref())
                        .is_some_and(|id| linked.contains(id));
                if verknupft {
                    continue;
                }
                events.push(event);
            }
            for question in &questions {
                for entry in u.question_history().list(&question.id).await? {
                    if !QUESTION_TYPES.contains(&entry.entry_type) {
                        continue;
                    }
                    events.push(TimelineEvent {
                        at: entry.at.clone(),
                        kind: TimelineKind::Question,
                        event_type: format!("question:{}", entry.entry_type),
                        actor: classifier.actor(&entry.by_name, entry.by_kind),
                        field: None,
                        from: None,
                        to: None,
                        note: None,
                        text: Some(format!(
                            "{} „{}“: {}",
                            question.id, question.title, entry.text
                        )),
                        reference: Some(TimelineRef {
                            question_id: Some(question.id.clone()),
                            ..TimelineRef::default()
                        }),
                    });
                }
            }
            events.extend(comments.iter().map(|comment| classifier.comment(comment)));
            sort_timeline(&mut events);
            Ok(TicketTimeline {
                ticket_id: ticket_id.clone(),
                events,
            })
        })
        .await
    }
}

#[cfg(test)]
mod tests;
