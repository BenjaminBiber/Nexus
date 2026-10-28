//! Kommentare an Tickets: nur anhängen, nie ändern. Ein Kommentar erhöht die Version des
//! Tickets. Portiert aus `packages/core/src/board/CommentService.ts`.

use std::sync::Arc;

use crate::board::basics::BoardActor;
use crate::board::database::{BoardDatabase, NewComment};
use crate::board::limits::BoardLimits;
use crate::board::ticket_service::TicketService;
use crate::board::types::{Comment, Ticket};
use crate::clock::Clock;
use crate::error::{HarnessError, Result};
use crate::tx;

/// Kommentare eines Tickets.
#[derive(Debug, Clone)]
pub struct CommentService<D: BoardDatabase> {
    db: Arc<D>,
    clock: Clock,
}

impl<D: BoardDatabase> CommentService<D> {
    /// Dienst auf dieser Datenbank mit dieser Uhr
    pub fn new(db: Arc<D>, clock: Clock) -> Self {
        Self { db, clock }
    }

    /// Kommentare dieser Tickets, die noch in keine Session eingespeist wurden
    pub async fn unread_for_agent(&self, ticket_ids: &[String]) -> Result<Vec<Comment>> {
        if ticket_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ticket_ids = ticket_ids.to_vec();
        tx!(self.db, [ticket_ids], |u| {
            u.comments().unread_by_agent(&ticket_ids).await
        })
        .await
    }

    /// Markiert Kommentare als eingespeist (ohne neue Version des Tickets)
    pub async fn mark_read_by_agent(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let ids = ids.to_vec();
        let clock = self.clock.clone();
        tx!(self.db, [ids, clock], |u| {
            u.comments().mark_read_by_agent(&ids, &clock.iso()).await
        })
        .await
    }

    /// Hängt einen Kommentar an; `read_by_agent`: aus einer Session geschrieben, gilt sofort
    /// als eingespeist
    pub async fn add(
        &self,
        ticket_id: &str,
        body: &str,
        actor: &BoardActor,
        read_by_agent: bool,
    ) -> Result<Comment> {
        let text = body.trim().to_string();
        if text.is_empty() {
            return Err(HarnessError::new(
                "comment-empty",
                "Der Kommentar darf nicht leer sein.",
            )
            .into());
        }
        let ticket_id = ticket_id.to_string();
        let clock = self.clock.clone();
        let actor = actor.clone();
        tx!(self.db, [ticket_id, text, clock, actor], |u| {
            BoardLimits::actor(&actor)?;
            let ticket = TicketService::load(u, &ticket_id, None, true).await?;
            let at = clock.iso();
            u.tickets()
                .update(&Ticket {
                    rev: ticket.rev + 1,
                    updated_at: at.clone(),
                    ..ticket.clone()
                })
                .await?;
            u.comments()
                .insert(&NewComment {
                    ticket_id: ticket.id.clone(),
                    author_id: actor.user_id.clone(),
                    author_name: actor.name.clone(),
                    body: text.clone(),
                    created_at: at.clone(),
                    read_by_agent_at: read_by_agent.then(|| at.clone()),
                })
                .await
        })
        .await
    }

    /// Alle Kommentare eines Tickets in zeitlicher Folge
    pub async fn list(&self, ticket_id: &str) -> Result<Vec<Comment>> {
        let ticket_id = ticket_id.to_string();
        tx!(self.db, [ticket_id], |u| {
            let ticket = TicketService::load(u, &ticket_id, None, false).await?;
            u.comments().list(&ticket.id).await
        })
        .await
    }
}
