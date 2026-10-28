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
            return Err(
                HarnessError::new("comment-empty", "Der Kommentar darf nicht leer sein.").into(),
            );
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

#[cfg(test)]
mod tests {
    use crate::board::basics::BoardActor;
    use crate::board::database::BoardDatabase;
    use crate::testing::{BoardFixture, USER_ID};
    use crate::tx;

    /// Der Mensch, der nicht aus einer Session schreibt
    fn florian() -> BoardActor {
        BoardActor {
            user_id: USER_ID.into(),
            name: "Florian".into(),
        }
    }

    /// Legt die Tickets ARC-0001 … ARC-000n an
    async fn tickets(f: &BoardFixture, count: i32) {
        for number in 1..=count {
            let ticket = f.new_ticket(number).await;
            f.insert_ticket(&ticket).await;
        }
    }

    #[tokio::test]
    async fn hangt_kommentare_an_erhoht_die_version_schreibt_keinen_verlauf() {
        let f = BoardFixture::create().await;
        tickets(&f, 1).await;

        let comment = f
            .comments
            .add("arc-1", "  Schritt 2 fertig ", &f.actor, false)
            .await
            .unwrap();

        assert_eq!(comment.ticket_id, "ARC-0001");
        assert_eq!(comment.body, "Schritt 2 fertig");
        assert_eq!(comment.author_name, "claude");
        assert_eq!(comment.author_id, f.actor.user_id);
        let ticket = tx!(f.db, |u| { u.tickets().by_id("ARC-0001", false).await })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ticket.rev, 2);
        // Der Eintrag „angelegt“ entsteht erst mit `TicketService::create` aus Teil 2; der
        // Kommentar selbst schreibt keinen Verlauf
        assert!(tx!(f.db, |u| { u.history().list("ARC-0001").await })
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            f.comments
                .list("ARC-0001")
                .await
                .unwrap()
                .iter()
                .map(|comment| comment.body.as_str())
                .collect::<Vec<_>>(),
            ["Schritt 2 fertig"]
        );
    }

    #[tokio::test]
    async fn lehnt_leere_kommentare_und_unbekannte_tickets_ab() {
        let f = BoardFixture::create().await;
        tickets(&f, 1).await;

        assert_eq!(
            f.comments
                .add("ARC-0001", " ", &f.actor, false)
                .await
                .unwrap_err()
                .code(),
            Some("comment-empty")
        );
        assert_eq!(
            f.comments
                .add("ARC-0009", "x", &f.actor, false)
                .await
                .unwrap_err()
                .code(),
            Some("ticket-not-found")
        );
    }

    #[tokio::test]
    async fn liefert_nur_kommentare_ohne_lesemarke_eigene_gelten_als_gelesen() {
        let f = BoardFixture::create().await;
        tickets(&f, 2).await;
        f.comments
            .add("ARC-0001", "Bitte prüfen", &florian(), false)
            .await
            .unwrap();
        let own = f
            .comments
            .add("ARC-0001", "Zwischenstand", &f.actor, true)
            .await
            .unwrap();
        f.comments
            .add("ARC-0002", "Auch hier", &florian(), false)
            .await
            .unwrap();

        assert_eq!(own.read_by_agent_at, Some(own.created_at.clone()));
        let unread = f
            .comments
            .unread_for_agent(&["ARC-0001".to_string(), "ARC-0002".to_string()])
            .await
            .unwrap();
        assert_eq!(
            unread
                .iter()
                .map(|comment| (comment.ticket_id.as_str(), comment.body.as_str()))
                .collect::<Vec<_>>(),
            [("ARC-0001", "Bitte prüfen"), ("ARC-0002", "Auch hier")]
        );

        f.advance(5);
        let ids: Vec<i64> = unread.iter().map(|comment| comment.id).collect();
        f.comments.mark_read_by_agent(&ids).await.unwrap();

        assert!(f
            .comments
            .unread_for_agent(&["ARC-0001".to_string(), "ARC-0002".to_string()])
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            f.comments.list("ARC-0001").await.unwrap()[0]
                .read_by_agent_at
                .as_deref(),
            Some("2026-10-13T10:05:00.000Z")
        );
    }

    #[tokio::test]
    async fn kommt_mit_leeren_listen_aus() {
        let f = BoardFixture::create().await;

        assert!(f.comments.unread_for_agent(&[]).await.unwrap().is_empty());
        assert!(f.comments.mark_read_by_agent(&[]).await.is_ok());
    }
}
