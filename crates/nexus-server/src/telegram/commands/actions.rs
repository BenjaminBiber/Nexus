//! Die Knöpfe an „Meine Aufgaben“: Abnahme, Rückgabe, erneuter Merge, Grund, Kommentar,
//! Planung und Notizen – dazu `/abbrechen`.
//!
//! Jeder Knopf läuft über dieselben Dienste wie die Routen (`state.dispatch`,
//! `state.board`); Handelnder ist der Board-Actor des Servers, und jeder Kommentar trägt
//! den Zusatz „(über Telegram)“. Beim Klick wird der Zustand neu gelesen: Trägt das Ticket
//! den Knopf nicht mehr, ändert sich nichts, und der Hinweis erscheint als Fenster am Knopf
//! und als neuer Text der Nachricht. Nach einer gelaufenen Aktion ersetzt das Ergebnis die
//! Nachricht, die Knöpfe fallen weg.
//!
//! Übernehmen und erneut mergen brauchen zwei Klicks: der erste ersetzt die Knöpfe durch
//! die Sicherheitsfrage, erst „Ja“ führt die Aktion aus. Rückmeldung und Kommentar kommen
//! als Text; dafür hält [`crate::telegram::dialog`] je Chat einen offenen Dialog.

use std::sync::Arc;

use async_trait::async_trait;
use nexus_core::board::basics::{NoteState, TicketStatus};
use nexus_core::board::inbox::ranking::InboxRanking;
use nexus_core::board::inbox::types::InboxKind;
use nexus_core::board::note_service::TakeNoteOptions;
use nexus_core::board::ticket_service::{TicketDetail, TicketPatch, WriteOptions};
use nexus_core::board::types::Note;
use nexus_core::telegram::CallbackAction;
use nexus_core::Result;

use crate::state::AppState;
use crate::telegram::action_views::{
    accept_question, dialog_message, diff_message, done_message, hint_message, merge_question,
    reason_message, via_telegram, DIALOG_CANCELLED, DIALOG_EXPIRED, NOTHING_CHANGED, NO_DIALOG,
};
use crate::telegram::api::{InlineKeyboard, Message};
use crate::telegram::commands::{report, Deps};
use crate::telegram::dialog::{Dialog, DialogAnswer, DialogInput, DialogKind, Dialogs};
use crate::telegram::router::{BotContext, CallbackHandler, CommandHandler, MessageHandler};

/// Die Knöpfe der Aufgaben und die Antworten auf ihre Dialoge
pub(crate) struct Actions {
    deps: Deps,
    dialogs: Arc<Dialogs>,
}

impl std::fmt::Debug for Actions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Actions")
            .field("dialogs", &self.dialogs)
            .finish_non_exhaustive()
    }
}

impl Actions {
    /// Knöpfe auf diesen Diensten mit diesen Dialogen
    pub(crate) fn new(deps: Deps, dialogs: Arc<Dialogs>) -> Self {
        Self { deps, dialogs }
    }

    // ---------------------------------------------------------------- Abnahme und Merge

    /// Erster Klick auf „Übernehmen“ bzw. „Erneut mergen“: die Sicherheitsfrage ersetzt die
    /// Knöpfe des Eintrags; `question` baut sie, `kind` ist der Zustand, den sie verlangt
    async fn ask(
        &self,
        ctx: &BotContext,
        query_id: &str,
        id: &str,
        kind: InboxKind,
        question: Question,
    ) -> Result<()> {
        let state = self.deps.state()?;
        let detail = match self.ticket(ctx, query_id, &state, id, &[kind]).await? {
            Some(detail) => detail,
            None => return Ok(()),
        };
        answer(ctx, query_id, None).await?;
        let (text, keyboard) = question(&detail.ticket.id);
        ctx.update(&text, Some(keyboard)).await
    }

    /// „Ja, übernehmen“: Abnahme wie `POST /api/dispatch/accept/:id`, danach der Merge
    async fn accept(&self, ctx: &BotContext, query_id: &str, id: &str) -> Result<()> {
        let state = self.deps.state()?;
        let detail = match self
            .ticket(ctx, query_id, &state, id, &[InboxKind::Accept])
            .await?
        {
            Some(detail) => detail,
            None => return Ok(()),
        };
        answer(ctx, query_id, None).await?;
        match state.dispatch.accept(&detail.ticket.id, None).await {
            Ok(accepted) => {
                self.done(ctx, &state, "Übernommen", Some(&accepted.result.text))
                    .await
            }
            Err(error) => report(ctx, &error).await,
        }
    }

    /// „Ja, mergen“: erneuter Merge wie `POST /api/dispatch/merge/:id`
    async fn merge(&self, ctx: &BotContext, query_id: &str, id: &str) -> Result<()> {
        let state = self.deps.state()?;
        let detail = match self
            .ticket(ctx, query_id, &state, id, &[InboxKind::Merge])
            .await?
        {
            Some(detail) => detail,
            None => return Ok(()),
        };
        answer(ctx, query_id, None).await?;
        match state.dispatch.retry_merge(&detail.ticket.id).await {
            Ok(merged) => {
                self.done(ctx, &state, "Merge angestoßen", Some(&merged.result.text))
                    .await
            }
            Err(error) => report(ctx, &error).await,
        }
    }

    // ---------------------------------------------------------------- Nur lesende Knöpfe

    /// „Geänderte Dateien“: die Dateiliste des Reviews als eigene Nachricht; die Knöpfe des
    /// Eintrags bleiben, weil nichts geschehen ist
    async fn diff(&self, ctx: &BotContext, query_id: &str, id: &str) -> Result<()> {
        let state = self.deps.state()?;
        answer(ctx, query_id, None).await?;
        match state.dispatch.review(id).await {
            Ok(view) => ctx.reply(&diff_message(&view)).await,
            Err(error) => report(ctx, &error).await,
        }
    }

    /// „Grund“: der Blockierungsgrund als eigene Nachricht
    async fn reason(&self, ctx: &BotContext, query_id: &str, id: &str) -> Result<()> {
        let state = self.deps.state()?;
        answer(ctx, query_id, None).await?;
        match ticket_detail(&state, id).await {
            Ok(detail) => ctx.reply(&reason_message(&detail.ticket)).await,
            Err(error) => report(ctx, &error).await,
        }
    }

    // ---------------------------------------------------------------- Planung und Notizen

    /// „Planen starten“: wie `POST /api/dispatch/plan/:ticket_id`
    async fn replan(&self, ctx: &BotContext, query_id: &str, id: &str) -> Result<()> {
        let state = self.deps.state()?;
        let detail = match self
            .ticket(ctx, query_id, &state, id, &[InboxKind::Planning])
            .await?
        {
            Some(detail) => detail,
            None => return Ok(()),
        };
        let id = detail.ticket.id.clone();
        answer(ctx, query_id, None).await?;
        match state.dispatch.start_planning(&id).await {
            Ok(_) => {
                self.done(
                    ctx,
                    &state,
                    "Planung gestartet",
                    Some(&format!("{id}: Der Agent plant jetzt.")),
                )
                .await
            }
            Err(error) => report(ctx, &error).await,
        }
    }

    /// „Ticket daraus“: wie `POST /api/db/notes/:id/take`
    async fn take_note(&self, ctx: &BotContext, query_id: &str, note_id: i32) -> Result<()> {
        let state = self.deps.state()?;
        if self.note(ctx, query_id, &state, note_id).await?.is_none() {
            return Ok(());
        }
        answer(ctx, query_id, None).await?;
        let taken = state
            .board
            .run(|backend| async move {
                backend
                    .services()
                    .notes
                    .take(note_id, &backend.actor(None), &TakeNoteOptions::default())
                    .await
            })
            .await;
        match taken {
            Ok(taken) => {
                self.done(
                    ctx,
                    &state,
                    "Ticket aus der Notiz",
                    Some(&format!("{} angelegt.", taken.ticket.ticket.id)),
                )
                .await
            }
            Err(error) => report(ctx, &error).await,
        }
    }

    /// „Verwerfen“: wie `POST /api/db/notes/:id/drop`
    async fn drop_note(&self, ctx: &BotContext, query_id: &str, note_id: i32) -> Result<()> {
        let state = self.deps.state()?;
        if self.note(ctx, query_id, &state, note_id).await?.is_none() {
            return Ok(());
        }
        answer(ctx, query_id, None).await?;
        let reason = via_telegram("Verworfen");
        let dropped = state
            .board
            .run(
                |backend| async move { backend.services().notes.drop_note(note_id, &reason).await },
            )
            .await;
        match dropped {
            Ok(_) => self.done(ctx, &state, "Notiz verworfen", None).await,
            Err(error) => report(ctx, &error).await,
        }
    }

    // ---------------------------------------------------------------- Dialoge

    /// „Zurück an den Agenten“, „Zurück nach Geplant“ und „Kommentar schreiben“: fragt nach
    /// dem Text; die nächste Nachricht des Chats ist die Antwort
    async fn open_dialog(
        &self,
        ctx: &BotContext,
        query_id: &str,
        id: &str,
        kind: DialogKind,
    ) -> Result<()> {
        let state = self.deps.state()?;
        let allowed: &[InboxKind] = match kind {
            DialogKind::ReturnFeedback => &[InboxKind::Accept, InboxKind::Blocked],
            // Ein Kommentar passt in jedem Zustand; nur geben muss es das Ticket
            DialogKind::Comment => &[],
        };
        let detail = match self.ticket(ctx, query_id, &state, id, allowed).await? {
            Some(detail) => detail,
            None => return Ok(()),
        };
        let id = detail.ticket.id.clone();
        self.dialogs.open(ctx.chat_id, kind, &id);
        answer(ctx, query_id, None).await?;
        ctx.update(&dialog_message(kind, &id), None).await
    }

    /// Die Antwort auf einen Dialog. Eine Sprachnachricht kommt später als Transkript
    /// ebenfalls hier an
    pub(crate) async fn submit(
        &self,
        ctx: &BotContext,
        dialog: &Dialog,
        input: DialogInput,
    ) -> Result<()> {
        let text = input.text().trim();
        if text.is_empty() {
            return ctx.reply(&hint_message(EMPTY_ANSWER)).await;
        }
        let state = self.deps.state()?;
        let detail = match ticket_detail(&state, &dialog.ticket).await {
            Ok(detail) => detail,
            Err(error) => return report(ctx, &error).await,
        };
        let id = detail.ticket.id.clone();
        let comment = via_telegram(text);
        match dialog.kind {
            DialogKind::Comment => {
                let saved = state
                    .board
                    .run(|backend| {
                        let id = id.clone();
                        async move {
                            backend
                                .services()
                                .comments
                                .add(&id, &comment, &backend.actor(None), false)
                                .await
                        }
                    })
                    .await;
                match saved {
                    Ok(_) => {
                        self.reply_done(
                            ctx,
                            &state,
                            "Kommentar geschrieben",
                            Some(&format!("{id}: steht am Ticket.")),
                        )
                        .await
                    }
                    Err(error) => report(ctx, &error).await,
                }
            }
            DialogKind::ReturnFeedback => match kind_of(&detail) {
                // Vor der Abnahme: „Zurück an den Agenten“ wie `POST /api/dispatch/return/:id`
                Some(InboxKind::Accept) => {
                    match state.dispatch.return_to_agent(&id, &comment, None).await {
                        Ok(returned) => {
                            self.reply_done(
                                ctx,
                                &state,
                                "Zurück an den Agenten",
                                Some(&returned.result.text),
                            )
                            .await
                        }
                        Err(error) => report(ctx, &error).await,
                    }
                }
                // Blockiert: zurück nach Geplant, der Kommentar sagt dem Agenten warum
                Some(InboxKind::Blocked) => match self.to_planned(&state, &id, &comment).await {
                    Ok(()) => {
                        self.reply_done(
                            ctx,
                            &state,
                            "Zurück nach Geplant",
                            Some(&format!("{id} steht wieder in Geplant.")),
                        )
                        .await
                    }
                    Err(error) => report(ctx, &error).await,
                },
                _ => ctx.reply(&hint_message(&stale(&id, RETURN))).await,
            },
        }
    }

    /// Blockiert → Geplant samt Kommentar; Grund und Zuständiger fallen weg, damit der
    /// Dispatcher das Ticket wieder nimmt
    async fn to_planned(&self, state: &Arc<AppState>, id: &str, comment: &str) -> Result<()> {
        let id = id.to_string();
        let comment = comment.to_string();
        state
            .board
            .run(|backend| async move {
                backend
                    .services()
                    .comments
                    .add(&id, &comment, &backend.actor(None), false)
                    .await?;
                backend
                    .services()
                    .tickets
                    .update(
                        &id,
                        &TicketPatch {
                            status: Some(TicketStatus::Planned),
                            blocked_reason: Some(None),
                            assignee: Some(None),
                            ..TicketPatch::default()
                        },
                        &backend.actor(None),
                        &WriteOptions::default(),
                    )
                    .await?;
                Ok(())
            })
            .await
    }

    /// „Abbrechen“ unter der Sicherheitsfrage: nichts geschieht, die Knöpfe verschwinden
    async fn cancel(&self, ctx: &BotContext, query_id: &str) -> Result<()> {
        self.dialogs.cancel(ctx.chat_id);
        answer(ctx, query_id, None).await?;
        ctx.update(DIALOG_CANCELLED, None).await
    }

    // ---------------------------------------------------------------- Gemeinsames

    /// Das Ticket, wenn es den Knopf noch trägt. `None` heißt: der Zustand passt nicht mehr,
    /// der Hinweis ist schon raus und nichts wurde geändert. `allowed` leer = jeder Zustand
    async fn ticket(
        &self,
        ctx: &BotContext,
        query_id: &str,
        state: &Arc<AppState>,
        id: &str,
        allowed: &[InboxKind],
    ) -> Result<Option<TicketDetail>> {
        let detail = match ticket_detail(state, id).await {
            Ok(detail) => detail,
            Err(error) => {
                answer(ctx, query_id, None).await?;
                report(ctx, &error).await?;
                return Ok(None);
            }
        };
        let kind = kind_of(&detail);
        if !allowed.is_empty() && !kind.is_some_and(|kind| allowed.contains(&kind)) {
            let hint = stale(&detail.ticket.id, waiting_for(allowed));
            answer(ctx, query_id, Some(&hint)).await?;
            ctx.update(&hint_message(&hint), None).await?;
            return Ok(None);
        }
        Ok(Some(detail))
    }

    /// Die Notiz, wenn sie noch offen ist; sonst wie [`Self::ticket`] ohne Wirkung
    async fn note(
        &self,
        ctx: &BotContext,
        query_id: &str,
        state: &Arc<AppState>,
        note_id: i32,
    ) -> Result<Option<Note>> {
        let note = state
            .board
            .run(|backend| async move { backend.services().notes.get(note_id).await })
            .await;
        let note = match note {
            Ok(note) => note,
            Err(error) => {
                answer(ctx, query_id, None).await?;
                report(ctx, &error).await?;
                return Ok(None);
            }
        };
        if note.state != NoteState::Open {
            let hint = format!("Die Notiz ist nicht mehr offen{NOTHING_CHANGED}");
            answer(ctx, query_id, Some(&hint)).await?;
            ctx.update(&hint_message(&hint), None).await?;
            return Ok(None);
        }
        Ok(Some(note))
    }

    /// Ergebnis einer Aktion als neuer Text der Nachricht, ohne Knöpfe
    async fn done(
        &self,
        ctx: &BotContext,
        state: &Arc<AppState>,
        what: &str,
        detail: Option<&str>,
    ) -> Result<()> {
        let now = state.config.clock.now();
        ctx.update(&done_message(what, detail, now), None).await
    }

    /// Wie [`Self::done`], als neue Nachricht: die Antwort auf einen Dialog kam vom
    /// Benutzer, und dessen Nachricht lässt sich nicht ändern
    async fn reply_done(
        &self,
        ctx: &BotContext,
        state: &Arc<AppState>,
        what: &str,
        detail: Option<&str>,
    ) -> Result<()> {
        let now = state.config.clock.now();
        ctx.reply(&done_message(what, detail, now)).await
    }
}

#[async_trait]
impl CallbackHandler for Actions {
    async fn run(&self, ctx: &BotContext, action: &CallbackAction, query_id: &str) -> Result<bool> {
        match action {
            CallbackAction::Accept(id) => {
                self.ask(ctx, query_id, id, InboxKind::Accept, accept_question)
                    .await?
            }
            CallbackAction::AcceptConfirm(id) => self.accept(ctx, query_id, id).await?,
            CallbackAction::Merge(id) => {
                self.ask(ctx, query_id, id, InboxKind::Merge, merge_question)
                    .await?
            }
            CallbackAction::MergeConfirm(id) => self.merge(ctx, query_id, id).await?,
            CallbackAction::Diff(id) => self.diff(ctx, query_id, id).await?,
            CallbackAction::BlockedReason(id) => self.reason(ctx, query_id, id).await?,
            CallbackAction::Replan(id) => self.replan(ctx, query_id, id).await?,
            CallbackAction::NoteToTicket(note_id) => {
                self.take_note(ctx, query_id, *note_id).await?
            }
            CallbackAction::NoteDrop(note_id) => self.drop_note(ctx, query_id, *note_id).await?,
            CallbackAction::Return(id) => {
                self.open_dialog(ctx, query_id, id, DialogKind::ReturnFeedback)
                    .await?
            }
            CallbackAction::Comment(id) => {
                self.open_dialog(ctx, query_id, id, DialogKind::Comment)
                    .await?
            }
            CallbackAction::Cancel => self.cancel(ctx, query_id).await?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

#[async_trait]
impl MessageHandler for Actions {
    async fn run(&self, ctx: &BotContext, message: &Message) -> Result<bool> {
        // Sprachnachrichten nimmt ein späteres Ticket an; der Dialog bleibt bis dahin offen
        let Some(text) = message.text.as_deref() else {
            return Ok(false);
        };
        match self.dialogs.take(ctx.chat_id) {
            DialogAnswer::None => Ok(false),
            DialogAnswer::Expired(_) => {
                ctx.reply(DIALOG_EXPIRED).await?;
                Ok(true)
            }
            DialogAnswer::Open(dialog) => {
                self.submit(ctx, &dialog, DialogInput::Text(text.to_string()))
                    .await?;
                Ok(true)
            }
        }
    }
}

/// Der Befehl `/abbrechen`: beendet den offenen Dialog
#[derive(Debug)]
pub(crate) struct CancelCommand(pub(crate) Arc<Dialogs>);

#[async_trait]
impl CommandHandler for CancelCommand {
    async fn run(&self, ctx: &BotContext, _args: &str) -> Result<()> {
        match self.0.cancel(ctx.chat_id) {
            Some(_) => ctx.reply(DIALOG_CANCELLED).await,
            None => ctx.reply(NO_DIALOG).await,
        }
    }
}

/// Baut die Sicherheitsfrage zu einem Ticket samt ihren zwei Knöpfen
type Question = fn(&str) -> (String, InlineKeyboard);

/// Antwort auf eine Antwort ohne Inhalt
const EMPTY_ANSWER: &str = "Da war kein Text dabei – nichts gespeichert.";

/// Was ein Knopf erwartet, für den Hinweis „… wartet nicht mehr auf …“
const ACCEPT: &str = "eine Abnahme";
const MERGE: &str = "einen Merge";
const PLANNING: &str = "eine Planung";
const RETURN: &str = "eine Rückgabe";

/// Hinweis, dass der Knopf nicht mehr zum Zustand des Tickets passt
fn stale(id: &str, waiting: &str) -> String {
    format!("{id} wartet nicht mehr auf {waiting}{NOTHING_CHANGED}")
}

/// Worauf die Knöpfe dieser Arten warten
fn waiting_for(allowed: &[InboxKind]) -> &'static str {
    match allowed {
        [InboxKind::Accept] => ACCEPT,
        [InboxKind::Merge] => MERGE,
        [InboxKind::Planning] => PLANNING,
        _ => RETURN,
    }
}

/// Die Art aus „Meine Aufgaben“, die dieses Ticket gerade hat
fn kind_of(detail: &TicketDetail) -> Option<InboxKind> {
    InboxRanking::classify(
        detail.ticket.status,
        detail.ticket.blocked_reason.as_deref(),
    )
}

/// Das Ticket samt Kommentaren – derselbe Weg wie `GET /api/db/tickets/:id`
async fn ticket_detail(state: &Arc<AppState>, id: &str) -> Result<TicketDetail> {
    let id = id.to_string();
    state
        .board
        .run(|backend| async move { backend.services().tickets.get(&id).await })
        .await
}

/// Beantwortet den Knopfdruck, damit er beim Nutzer aufhört zu drehen; mit `hint` als
/// Hinweisfenster
async fn answer(ctx: &BotContext, query_id: &str, hint: Option<&str>) -> Result<()> {
    ctx.api
        .answer_callback_query(query_id, hint, hint.is_some())
        .await
        .map_err(nexus_core::Error::unexpected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_hinweis_nennt_das_ticket_und_was_der_knopf_erwartet() {
        assert_eq!(
            stale("NXS-0042", ACCEPT),
            "NXS-0042 wartet nicht mehr auf eine Abnahme – nichts geändert."
        );
    }

    #[test]
    fn jede_art_hat_ihren_eigenen_hinweis() {
        assert_eq!(waiting_for(&[InboxKind::Accept]), ACCEPT);
        assert_eq!(waiting_for(&[InboxKind::Merge]), MERGE);
        assert_eq!(waiting_for(&[InboxKind::Planning]), PLANNING);
        assert_eq!(
            waiting_for(&[InboxKind::Accept, InboxKind::Blocked]),
            RETURN,
            "„Zurück“ hängt an beiden Arten"
        );
    }
}
