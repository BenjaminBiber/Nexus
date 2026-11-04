//! Die Befehle des Bots, Pause/Weiter des Dispatchers und die Knöpfe an „Meine Aufgaben“.
//!
//! Jeder Handler ruft im Prozess dieselben Dienste wie die Routen (`state.dispatch`,
//! `state.board`, `state.accounts`) – nie über HTTP. Den Zustand hält er nur schwach: die
//! Handler hängen im Router des Bots, der Bot hängt im Zustand. Ist der Zustand weg, endet
//! der Server gerade und der Befehl antwortet mit einem Hinweis.
//!
//! Die Formatierung steckt in [`crate::telegram::views`]; hier steht nur, was gelesen wird.

mod actions;
mod board;
mod control;
mod questions;
mod status;
mod tasks;
mod ticket;

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use nexus_core::clock::Clock;
use nexus_core::{HarnessError, Result};

use crate::state::AppState;
use crate::telegram::action_views::DIALOG_EXPIRED;
use crate::telegram::api::Message;
use crate::telegram::dialog::{DialogAnswer, DialogInput, DialogKind, Dialogs};
use crate::telegram::idea::flow::IdeaFlow;
use crate::telegram::router::{BotContext, MessageHandler, UpdateRouter};
use crate::telegram::voice::{ConfiguredSpeech, VoiceMessages};

/// Beantwortet einen Knopfdruck; die Knöpfe des ganzen Bots nutzen denselben Weg
pub(crate) use actions::answer;
/// Die eigenen Boards und das aktive aus `board.yaml`
pub(crate) use board::{active_board, own_boards};

/// Die Befehle samt Beschreibung für `/hilfe` und `setMyCommands`, in dieser Reihenfolge
pub const COMMANDS: &[(&str, &str)] = &[
    ("status", "Dispatcher, Läufe, Accounts und Aufgaben"),
    ("board", "Spalten des Boards; mit Name oder ID ein anderes"),
    ("ticket", "Kurzansicht eines Tickets (/ticket NXS-0042)"),
    ("aufgaben", "Was gerade auf dich wartet"),
    ("idee", "Idee erfassen (/idee Der Cache läuft nie ab)"),
    ("pause", "Dispatcher pausieren (/pause 30, Vorgabe 60 min)"),
    ("weiter", "Dispatcher fortsetzen"),
    ("abbrechen", "Einen offenen Dialog beenden"),
];

/// Meldet die Handler dieses Tickets im Router an. Der Zustand wird schwach gehalten, weil
/// der Router im Bot und der Bot im Zustand liegt; die Uhr lässt die Dialoge ablaufen
pub fn register(router: &mut UpdateRouter, state: &Weak<AppState>, clock: Clock) {
    let deps = Deps(state.clone());
    let dialogs = Arc::new(Dialogs::with_clock(clock));
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
    let ideas = Arc::new(IdeaFlow::new(deps.clone(), dialogs.clone()));
    router.register_command("idee", description("idee"), ideas.clone());
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
    router.register_command(
        "abbrechen",
        description("abbrechen"),
        Arc::new(actions::CancelCommand(dialogs.clone())),
    );
    router.register_callback(Arc::new(board::BoardButton(deps.clone())));
    router.register_callback(Arc::new(tasks::MoreButton(deps.clone())));
    let actions = Arc::new(actions::Actions::new(deps.clone(), dialogs.clone()));
    let questions = Arc::new(questions::Questions::new(deps.clone(), dialogs.clone()));
    // Der Ideen-Dialog steht vor den anderen Knöpfen, damit „Abbrechen“ eine wartende Idee
    // wegräumt, bevor [`actions`] die Nachricht ersetzt
    router.register_callback(ideas.clone());
    router.register_callback(actions.clone());
    router.register_callback(questions.clone());
    // Die Texte der Dialoge nimmt genau ein Handler ab und gibt sie weiter
    router.register_message(Arc::new(DialogAnswers {
        dialogs,
        actions,
        questions,
        ideas: ideas.clone(),
    }));
    // Freier Text ohne offenen Dialog ist eine neue Idee; deshalb steht er hinter den Dialogen
    router.register_message(ideas);
    // Eine Sprachnachricht wird vorher zu Text und läuft dann denselben Weg
    router.set_voice(Arc::new(VoiceMessages::new(Arc::new(ConfiguredSpeech(
        deps,
    )))));
}

/// Nimmt die Antwort auf den offenen Dialog eines Chats ab und gibt sie an den Handler, der
/// ihn aufgemacht hat. Höchstens ein Dialog je Chat, deshalb nimmt ihn auch nur einer ab.
struct DialogAnswers {
    dialogs: Arc<Dialogs>,
    actions: Arc<actions::Actions>,
    questions: Arc<questions::Questions>,
    ideas: Arc<IdeaFlow>,
}

#[async_trait]
impl MessageHandler for DialogAnswers {
    async fn run(&self, ctx: &BotContext, message: &Message) -> Result<bool> {
        // Eine Sprachnachricht kommt hier als ihr Transkript an (der Router erkennt sie
        // vorher); jeder andere Anhang lässt den Dialog offen
        let Some(text) = message.text.as_deref() else {
            return Ok(false);
        };
        let dialog = match self.dialogs.take(ctx.chat_id) {
            DialogAnswer::None => return Ok(false),
            DialogAnswer::Expired(_) => {
                ctx.reply(DIALOG_EXPIRED).await?;
                return Ok(true);
            }
            DialogAnswer::Open(dialog) => dialog,
        };
        let input = DialogInput::Text(text.to_string());
        match dialog.kind {
            DialogKind::QuestionText | DialogKind::QuestionDiscard => {
                self.questions.submit(ctx, &dialog, input).await?
            }
            DialogKind::Comment | DialogKind::ReturnFeedback => {
                self.actions.submit(ctx, &dialog, input).await?
            }
            DialogKind::IdeaAnswer => self.ideas.submit(ctx, &dialog, input).await?,
        }
        Ok(true)
    }
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
    ctx.reply(&trouble(error)).await
}

/// Derselbe Satz als Text – für Antworten, die eine schon geschickte Nachricht ersetzen
pub(crate) fn trouble(error: &nexus_core::Error) -> String {
    let message = match error.harness() {
        Some(harness) => harness.message.clone(),
        None => error.to_string(),
    };
    format!(
        "Das ging gerade nicht: {}",
        nexus_core::telegram::escape(&message)
    )
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
