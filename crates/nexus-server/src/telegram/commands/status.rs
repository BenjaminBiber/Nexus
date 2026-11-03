//! `/status`: Dispatcher, laufende Worker, Accounts und die Zähler aus „Meine Aufgaben“.

use async_trait::async_trait;
use nexus_core::settings::UserSettingsStore;
use nexus_core::Result;

use crate::telegram::commands::{board, report, tasks, Deps};
use crate::telegram::router::{BotContext, CommandHandler};
use crate::telegram::views::{status_message, StatusInput};

/// Der Befehl `/status`
#[derive(Debug)]
pub(crate) struct StatusCommand(pub(crate) Deps);

#[async_trait]
impl CommandHandler for StatusCommand {
    async fn run(&self, ctx: &BotContext, _args: &str) -> Result<()> {
        let state = self.0.state()?;
        // Der Zustand des Dispatchers ist der Kern der Antwort; ohne ihn gibt es nichts zu zeigen
        let status = match state.dispatch.status(None).await {
            Ok(status) => status,
            Err(error) => return report(ctx, &error).await,
        };
        // Board-Namen, Auslastung und Arbeitsliste sind Beigaben: fehlt eine, steht das in der
        // Antwort, statt den ganzen Befehl scheitern zu lassen
        let boards = board::own_boards(&state).await.unwrap_or_default();
        let board_name = status
            .board_id
            .and_then(|id| boards.iter().find(|board| board.id == id))
            .map(|board| board.name.clone());
        let accounts_enabled = UserSettingsStore::new(state.home.clone())
            .read()
            .claude_usage;
        let accounts = match accounts_enabled {
            true => state.accounts.usage.overview().await,
            false => Vec::new(),
        };
        let inbox = tasks::inbox_list(&state).await.ok();
        ctx.reply(&status_message(StatusInput {
            status: &status,
            board_name: board_name.as_deref(),
            accounts: &accounts,
            accounts_enabled,
            inbox: inbox.as_ref(),
            now: state.config.clock.now(),
        }))
        .await
    }
}
