//! `/ticket <ID>`: Status, gekürzte Beschreibung, Abnahme und die letzten Kommentare.

use async_trait::async_trait;
use nexus_core::Result;

use crate::telegram::commands::{report, Deps};
use crate::telegram::router::{BotContext, CommandHandler};
use crate::telegram::views::{ticket_message, unknown_ticket, TICKET_WITHOUT_ID};

/// Fehlercodes, die „die ID gibt es so nicht“ heißen; alles andere ist ein echter Fehler
const NOT_FOUND: &[&str] = &["ticket-not-found", "ticket-id-invalid"];

/// Der Befehl `/ticket`
#[derive(Debug)]
pub(crate) struct TicketCommand(pub(crate) Deps);

#[async_trait]
impl CommandHandler for TicketCommand {
    async fn run(&self, ctx: &BotContext, args: &str) -> Result<()> {
        // Telegram-Clients schreiben IDs gern klein; die Tickets heißen aber „NXS-0042“
        let id = args.split_whitespace().next().unwrap_or_default();
        if id.is_empty() {
            return ctx.reply(TICKET_WITHOUT_ID).await;
        }
        let state = self.0.state()?;
        let wanted = id.to_uppercase();
        let detail = state
            .board
            .run(|backend| async move { backend.services().tickets.get(&wanted).await })
            .await;
        match detail {
            Ok(detail) => {
                ctx.reply(&ticket_message(&detail, state.config.clock.now()))
                    .await
            }
            Err(error) if is_not_found(&error) => ctx.reply(&unknown_ticket(id)).await,
            Err(error) => report(ctx, &error).await,
        }
    }
}

/// Heißt der Fehler „diese ID gibt es nicht“?
fn is_not_found(error: &nexus_core::Error) -> bool {
    error.code().is_some_and(|code| NOT_FOUND.contains(&code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erkennt_die_fehler_einer_unbekannten_id() {
        let fehlt: nexus_core::Error =
            nexus_core::HarnessError::new("ticket-not-found", "gibt es nicht").into();
        let kaputt: nexus_core::Error =
            nexus_core::HarnessError::new("board-db-unreachable", "weg").into();

        assert!(is_not_found(&fehlt));
        assert!(!is_not_found(&kaputt));
    }
}
