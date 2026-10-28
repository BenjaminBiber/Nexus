//! Entfernt Verweise anderer Tickets auf ein gelöschtes Ticket, erhöht deren Version und
//! schreibt den Verlauf. Portiert aus `packages/core/src/board/DependencyCleanup.ts`.

use std::collections::HashSet;

use crate::board::basics::BoardActor;
use crate::board::database::BoardUnitOfWork;
use crate::board::ticket_history_diff::TicketHistoryDiff;
use crate::board::types::Ticket;
use crate::error::{Error, Result};

/// Räumt Voraussetzungen auf ein gelöschtes Ticket ab.
pub struct DependencyCleanup;

impl DependencyCleanup {
    /// `skip`: Tickets, die in derselben Aktion ohnehin gelöscht werden (kein Verlauf nötig)
    pub async fn remove(
        u: &mut dyn BoardUnitOfWork,
        removed_id: &str,
        actor: &BoardActor,
        at: &str,
        skip: &HashSet<String>,
    ) -> Result<Vec<String>> {
        let touched = u.tickets().remove_dependency_on(removed_id).await?;
        for ticket_id in &touched {
            if skip.contains(ticket_id) {
                continue;
            }
            // remove_dependency_on hat den Verweis schon entfernt; „before“ stellt den alten
            // Stand für den Verlauf her
            let current = u
                .tickets()
                .by_id(ticket_id, true)
                .await?
                .ok_or_else(|| Error::unexpected_message(format!("Ticket {ticket_id} fehlt")))?;
            let mut before = current.clone();
            before.depends_on.push(removed_id.to_string());
            let after = Ticket {
                rev: current.rev + 1,
                updated_at: at.to_string(),
                ..current
            };
            u.tickets().update(&after).await?;
            let entries =
                TicketHistoryDiff::diff(&before, &after, actor, at, Some("Voraussetzung gelöscht"));
            u.history().insert(&entries).await?;
        }
        Ok(touched)
    }
}
