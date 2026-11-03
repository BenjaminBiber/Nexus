//! Die lesenden Befehle des Bots und Pause/Weiter des Dispatchers.
//!
//! Jeder Handler ruft im Prozess dieselben Dienste wie die Routen (`state.dispatch`,
//! `state.board`, `state.accounts`) – nie über HTTP. Den Zustand hält er nur schwach: die
//! Handler hängen im Router des Bots, der Bot hängt im Zustand. Ist der Zustand weg, endet
//! der Server gerade und der Befehl antwortet mit einem Hinweis.
//!
//! Die Formatierung steckt in [`crate::telegram::views`]; hier steht nur, was gelesen wird.

mod board;
mod control;
mod status;
mod tasks;
mod ticket;

use std::sync::{Arc, Weak};

use nexus_core::{HarnessError, Result};

use crate::state::AppState;
use crate::telegram::router::{BotContext, UpdateRouter};

/// Die Befehle samt Beschreibung für `/hilfe` und `setMyCommands`, in dieser Reihenfolge
pub const COMMANDS: &[(&str, &str)] = &[
    ("status", "Dispatcher, Läufe, Accounts und Aufgaben"),
    ("board", "Spalten des Boards; mit Name oder ID ein anderes"),
    ("ticket", "Kurzansicht eines Tickets (/ticket NXS-0042)"),
    ("aufgaben", "Was gerade auf dich wartet"),
    ("pause", "Dispatcher pausieren (/pause 30, Vorgabe 60 min)"),
    ("weiter", "Dispatcher fortsetzen"),
];

/// Meldet die Handler dieses Tickets im Router an. Der Zustand wird schwach gehalten, weil
/// der Router im Bot und der Bot im Zustand liegt
pub fn register(router: &mut UpdateRouter, state: &Weak<AppState>) {
    let deps = Deps(state.clone());
    router.register_command(
        "status",
        description("status"),
        Arc::new(status::StatusCommand(deps.clone())),
    );
    router.register_command(
        "board",
        description("board"),
        Arc::new(board::BoardCommand(deps.clone())),
    );
    router.register_command(
        "ticket",
        description("ticket"),
        Arc::new(ticket::TicketCommand(deps.clone())),
    );
    router.register_command(
        "aufgaben",
        description("aufgaben"),
        Arc::new(tasks::TasksCommand(deps.clone())),
    );
    router.register_command(
        "pause",
        description("pause"),
        Arc::new(control::PauseCommand(deps.clone())),
    );
    router.register_command(
        "weiter",
        description("weiter"),
        Arc::new(control::ResumeCommand(deps.clone())),
    );
    router.register_callback(Arc::new(board::BoardButton(deps.clone())));
    router.register_callback(Arc::new(tasks::MoreButton(deps)));
}

/// Die Beschreibung eines Befehls aus [`COMMANDS`]
fn description(name: &str) -> &'static str {
    COMMANDS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, text)| *text)
        .unwrap_or_default()
}

/// Der Zugriff der Handler auf die Dienste des Servers, schwach gehalten
#[derive(Clone)]
pub(crate) struct Deps(Weak<AppState>);

impl std::fmt::Debug for Deps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Deps").finish_non_exhaustive()
    }
}

impl Deps {
    /// Der Zustand; ist er weg, endet der Server gerade
    pub(crate) fn state(&self) -> Result<Arc<AppState>> {
        self.0
            .upgrade()
            .ok_or_else(|| HarnessError::new("server-stopped", "Der Server wird beendet.").into())
    }
}

/// Antwort auf einen Befehl, der an einem Dienst gescheitert ist. Die Meldung geht in den
/// Chat, damit niemand auf eine Antwort wartet, die nie kommt; der Fehler selbst steht
/// zusätzlich im Protokoll des Servers (der Poller schreibt ihn)
pub(crate) async fn report(ctx: &BotContext, error: &nexus_core::Error) -> Result<()> {
    let message = match error.harness() {
        Some(harness) => harness.message.clone(),
        None => error.to_string(),
    };
    ctx.reply(&format!(
        "Das ging gerade nicht: {}",
        nexus_core::telegram::escape(&message)
    ))
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beschreibt_jeden_angemeldeten_befehl() {
        for (name, text) in COMMANDS {
            assert!(!text.is_empty(), "{name} braucht eine Beschreibung");
            assert_eq!(description(name), *text);
        }
    }
}
