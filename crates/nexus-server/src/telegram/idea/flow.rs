//! Der Ideen-Dialog: `/idee`, freier Text und Sprachnachrichten werden zu einem Entwurf,
//! den eine Bestätigungsnachricht samt Knöpfen zeigt – angelegt wird erst auf Knopfdruck.
//!
//! Der Weg ist immer derselbe: Ansage „🤔 Ich schaue mir das Board an …“, dann der
//! [`IdeaInterpreter`], dann ersetzt der Entwurf die Ansage. Die Knöpfe schalten das Ziel
//! (Notiz/Ticket) und das Board um, starten eine Korrekturrunde oder verwerfen.
//!
//! Hat der Entwurf Rückfragen, kommen sie vorher einzeln ([`Questionnaire`]): je Frage ihre
//! Antwortmöglichkeiten als Knöpfe, dazu Überspringen, Zurück und Abbrechen; eine Nachricht –
//! Text oder Sprache – ist die freie Antwort. Erst nach der letzten Frage läuft eine
//! Korrekturrunde mit allen Antworten (`previous`) durch den Interpreter. Ab
//! `ideas.maxRounds` bleiben nur noch Anlegen und Verwerfen.
//!
//! Angelegt wird über dieselben Dienste wie die Routen (`state.board`): die Notiz über den
//! Notiz-Dienst, das Ticket im Backlog samt Kommentar „Aus Telegram-Sprachnotiz“, der
//! Kommentar am genannten Ticket. Danach ist der Entwurf weg.
//!
//! Ob aus einer Idee eine Notiz oder ein Ticket wird, entscheidet nicht Claude, sondern die
//! Einstellung `ideas.defaultTarget` und der Knopf darunter; nur den Kommentar schlägt
//! Claude vor, weil er ein genanntes Ticket braucht.

use std::sync::Arc;

use async_trait::async_trait;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::note_service::{AddNoteOptions, NewNoteText};
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use nexus_core::telegram::idea::{IdeaDraft, IdeaKind};
use nexus_core::telegram::{
    AskError, AskStep, CallbackAction, IdeaTarget, Questionnaire, TelegramConfigStore,
};
use nexus_core::Result;

use crate::state::AppState;
use crate::telegram::action_views::{hint_message, via_telegram};
use crate::telegram::api::{InlineKeyboard, Message};
use crate::telegram::commands::{active_board, answer, own_boards, report, trouble, Deps};
use crate::telegram::dialog::{Dialog, DialogInput, DialogKind, Dialogs};
use crate::telegram::idea::store::IdeaEntry;
use crate::telegram::idea::views::{
    ask_message, asks_done_message, board_list, correction_message, created_message,
    draft_keyboard, draft_message, dropped_message, replace_question, retry_keyboard, ASK_STALE,
    DROPPED, EMPTY_ANSWER, EMPTY_CORRECTION, FROM_TELEGRAM_VOICE, IDEA_WITHOUT_TEXT,
    NOTHING_TICKED, NO_DRAFT, ROUNDS_SPENT, THINKING, TRANSCRIPT, UNKNOWN_ANSWER,
};
use crate::telegram::idea::IdeaRequest;
use crate::telegram::router::{BotContext, CallbackHandler, CommandHandler, MessageHandler};
use crate::telegram::views::BoardChoice;

/// Der Ideen-Dialog: Befehl, freier Text, Knöpfe und die Antworten seiner Dialoge
pub(crate) struct IdeaFlow {
    deps: Deps,
    dialogs: Arc<Dialogs>,
}

impl std::fmt::Debug for IdeaFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdeaFlow")
            .field("dialogs", &self.dialogs)
            .finish_non_exhaustive()
    }
}

impl IdeaFlow {
    /// Der Dialog auf diesen Diensten mit diesen Dialogen
    pub(crate) fn new(deps: Deps, dialogs: Arc<Dialogs>) -> Self {
        Self { deps, dialogs }
    }

    // ---------------------------------------------------------------- Einstieg

    /// Eine neue Idee. Liegt noch ein Entwurf im Chat, wartet sie, bis entschieden ist, was
    /// mit dem alten wird
    async fn start(&self, ctx: &BotContext, input: &str) -> Result<()> {
        let state = self.deps.state()?;
        if let Some(open) = state.idea_drafts.get(ctx.chat_id) {
            state.idea_drafts.hold(ctx.chat_id, input)?;
            let (text, keyboard) = replace_question(&open.id, &open.draft);
            return ctx.reply_with(&text, keyboard).await;
        }
        self.run(ctx, &state, input, None).await
    }

    /// Ein Durchgang des Interpreters: Ansage, Auswertung, Bestätigung. `previous` macht
    /// daraus eine Korrekturrunde am schon offenen Entwurf
    async fn run(
        &self,
        ctx: &BotContext,
        state: &Arc<AppState>,
        input: &str,
        previous: Option<IdeaEntry>,
    ) -> Result<()> {
        let thinking = ctx
            .api
            .send_message(ctx.chat_id, THINKING, None, None)
            .await
            .map_err(nexus_core::Error::unexpected)?;
        let request = IdeaRequest {
            chat_id: ctx.chat_id,
            input: input.to_string(),
            active_board_id: active_board(state),
            previous: previous.as_ref().map(|entry| entry.draft.clone()),
        };
        let made = state
            .idea
            .interpret_checked(state.claude.idea.as_ref(), &state.board, &request)
            .await;
        let check = match made {
            Ok(check) => check,
            Err(error) => {
                // Nach einem Fragebogen bleiben die Antworten liegen: „Nochmal“ rechnet neu
                let retry = previous.as_ref().and_then(|entry| {
                    entry
                        .questionnaire
                        .as_ref()
                        .filter(|questionnaire| questionnaire.is_done())
                        .map(|questionnaire| retry_keyboard(&entry.id, questionnaire.answers.len()))
                });
                return self.edit(ctx, thinking, &trouble(&error), retry).await;
            }
        };
        let config = TelegramConfigStore::new(state.home.clone())
            .read()
            .unwrap_or_default();
        let mut draft = check.draft;
        with_target(
            &mut draft,
            config.ideas.default_target,
            previous.as_ref().map(|entry| &entry.draft),
        );
        let entry = match &previous {
            Some(old) => {
                // Der Entwurf kann in der Zwischenzeit verworfen oder abgelaufen sein
                if !state
                    .idea_drafts
                    .replace(ctx.chat_id, &old.id, draft.clone())?
                {
                    return self
                        .edit(ctx, thinking, &hint_message(NO_DRAFT), None)
                        .await;
                }
                IdeaEntry {
                    draft,
                    ..old.clone()
                }
            }
            None => state.idea_drafts.put(ctx.chat_id, draft)?,
        };
        let rounds_left = entry.draft.rounds < config.ideas.max_rounds;
        // Offene Rückfragen kommen einzeln, solange noch eine Runde übrig ist, sie einzuarbeiten;
        // der Entwurf selbst erscheint erst danach
        let questionnaire = (!entry.draft.questions.is_empty() && rounds_left)
            .then(|| Questionnaire::new(entry.draft.questions.len()));
        if !state
            .idea_drafts
            .set_questionnaire(ctx.chat_id, &entry.id, questionnaire.clone())?
        {
            return self
                .edit(ctx, thinking, &hint_message(NO_DRAFT), None)
                .await;
        }
        let (text, keyboard) = match &questionnaire {
            Some(questionnaire) => ask_message(&entry.id, &entry.draft, questionnaire),
            None => self.confirmation(state, &entry, rounds_left).await,
        };
        let text = match dropped_message(&check.dropped) {
            Some(hints) => format!("{text}\n\n{hints}"),
            None => text,
        };
        self.edit(ctx, thinking, &text, Some(keyboard)).await?;
        // Die aktuelle Rückfrage wartet gleich auf eine freie Antwort per Text oder Sprache
        if questionnaire.is_some() {
            self.dialogs
                .open(ctx.chat_id, DialogKind::IdeaAnswer, &entry.id);
        }
        Ok(())
    }

    // ---------------------------------------------------------------- Rückfragen

    /// Ein Knopf an einer Rückfrage: antworten, ankreuzen, überspringen oder zurück. Ist danach
    /// alles beantwortet, beginnt die Verfeinerungsrunde
    async fn ask(&self, ctx: &BotContext, query_id: &str, draft_id: &str, op: AskOp) -> Result<()> {
        let state = self.deps.state()?;
        let Some(entry) = self.open(ctx, query_id, &state, draft_id).await? else {
            return Ok(());
        };
        let Some(mut questionnaire) = entry.questionnaire.clone() else {
            return answer(ctx, query_id, Some(ASK_STALE)).await;
        };
        // „Nochmal“ nach einer gescheiterten Runde: dieselben Antworten neu einarbeiten
        if questionnaire.is_done() {
            if op == AskOp::Confirm(questionnaire.answers.len()) {
                answer(ctx, query_id, None).await?;
                ctx.update(&asks_done_message(&entry.draft, &questionnaire), None)
                    .await?;
                return self.finish(ctx, &state, entry, questionnaire).await;
            }
            return answer(ctx, query_id, Some(ASK_STALE)).await;
        }
        let asks = &entry.draft.questions;
        let step = match op {
            AskOp::Choose(question, option) => questionnaire.choose(asks, question, option),
            AskOp::Toggle(question, option) => questionnaire
                .toggle(asks, question, option)
                .map(|()| AskStep::Next),
            AskOp::Confirm(question) => questionnaire.confirm(asks, question),
            AskOp::Skip(question) => questionnaire.skip(asks, question),
            AskOp::Back(question) => questionnaire.back(asks, question).map(|()| AskStep::Next),
        };
        let step = match step {
            Ok(step) => step,
            Err(error) => return answer(ctx, query_id, Some(ask_hint(error))).await,
        };
        if !state.idea_drafts.set_questionnaire(
            ctx.chat_id,
            &entry.id,
            Some(questionnaire.clone()),
        )? {
            return self.stale(ctx, query_id, NO_DRAFT).await;
        }
        answer(ctx, query_id, None).await?;
        match step {
            AskStep::Next => {
                let (text, keyboard) = ask_message(&entry.id, &entry.draft, &questionnaire);
                self.dialogs
                    .open(ctx.chat_id, DialogKind::IdeaAnswer, &entry.id);
                ctx.update(&text, Some(keyboard)).await
            }
            AskStep::Done => {
                ctx.update(&asks_done_message(&entry.draft, &questionnaire), None)
                    .await?;
                self.finish(ctx, &state, entry, questionnaire).await
            }
        }
    }

    /// Eine freie Antwort (Text oder Sprache) auf die aktuelle Rückfrage; die nächste Frage
    /// kommt als neue Nachricht
    async fn ask_free(
        &self,
        ctx: &BotContext,
        state: &Arc<AppState>,
        entry: IdeaEntry,
        mut questionnaire: Questionnaire,
        text: &str,
    ) -> Result<()> {
        let step = match questionnaire.free(&entry.draft.questions, text) {
            Ok(step) => step,
            Err(error) => return ctx.reply(&hint_message(ask_hint(error))).await,
        };
        if !state.idea_drafts.set_questionnaire(
            ctx.chat_id,
            &entry.id,
            Some(questionnaire.clone()),
        )? {
            return ctx.reply(&hint_message(NO_DRAFT)).await;
        }
        match step {
            AskStep::Next => {
                let (text, keyboard) = ask_message(&entry.id, &entry.draft, &questionnaire);
                self.dialogs
                    .open(ctx.chat_id, DialogKind::IdeaAnswer, &entry.id);
                ctx.reply_with(&text, keyboard).await
            }
            AskStep::Done => self.finish(ctx, state, entry, questionnaire).await,
        }
    }

    /// Alle Rückfragen beantwortet: eine Verfeinerungsrunde mit allen Antworten
    async fn finish(
        &self,
        ctx: &BotContext,
        state: &Arc<AppState>,
        entry: IdeaEntry,
        questionnaire: Questionnaire,
    ) -> Result<()> {
        let answers = questionnaire.answers_text(&entry.draft.questions);
        let entry = IdeaEntry {
            questionnaire: Some(questionnaire),
            ..entry
        };
        self.run(ctx, state, &answers, Some(entry)).await
    }

    // ---------------------------------------------------------------- Knöpfe am Entwurf

    /// „Als Notiz“ bzw. „Als Ticket“: Ziel umschalten, die Bestätigung neu schreiben
    async fn switch_target(
        &self,
        ctx: &BotContext,
        query_id: &str,
        draft_id: &str,
        target: IdeaTarget,
    ) -> Result<()> {
        let state = self.deps.state()?;
        let Some(entry) = self.open(ctx, query_id, &state, draft_id).await? else {
            return Ok(());
        };
        let mut draft = entry.draft.clone();
        draft.target.kind = kind_of(target);
        draft.target.ticket_id = None;
        self.changed(ctx, query_id, &state, &entry, draft).await
    }

    /// „Anderes Board“: die eigenen Boards zur Wahl; der Entwurf bleibt, wie er ist
    async fn board_choice(&self, ctx: &BotContext, query_id: &str, draft_id: &str) -> Result<()> {
        let state = self.deps.state()?;
        let Some(entry) = self.open(ctx, query_id, &state, draft_id).await? else {
            return Ok(());
        };
        let boards = match own_boards(&state).await {
            Ok(boards) => boards,
            Err(error) => {
                answer(ctx, query_id, None).await?;
                return report(ctx, &error).await;
            }
        };
        answer(ctx, query_id, None).await?;
        let (text, keyboard) = board_list(&entry.id, &boards, entry.draft.target.board_id);
        ctx.update(&text, Some(keyboard)).await
    }

    /// Ein gewähltes Board: der Entwurf zieht um, die Bestätigung kommt wieder
    async fn set_board(
        &self,
        ctx: &BotContext,
        query_id: &str,
        draft_id: &str,
        board_id: i32,
    ) -> Result<()> {
        let state = self.deps.state()?;
        let Some(entry) = self.open(ctx, query_id, &state, draft_id).await? else {
            return Ok(());
        };
        // Ein Knopf aus einer alten Nachricht kann auf ein gelöschtes Board zeigen
        let boards = own_boards(&state).await.unwrap_or_default();
        if !boards.iter().any(|board| board.id == board_id) {
            return self.stale(ctx, query_id, UNKNOWN_BOARD).await;
        }
        let mut draft = entry.draft.clone();
        draft.target.board_id = board_id;
        self.changed(ctx, query_id, &state, &entry, draft).await
    }

    /// „Korrigieren“: der Dialog wartet auf Text oder Sprache; die Bestätigung bleibt stehen,
    /// damit noch zu lesen ist, worum es geht
    async fn correct(&self, ctx: &BotContext, query_id: &str, draft_id: &str) -> Result<()> {
        let state = self.deps.state()?;
        let Some(entry) = self.open(ctx, query_id, &state, draft_id).await? else {
            return Ok(());
        };
        if !self.rounds_left(&state, &entry.draft) {
            return self.stale(ctx, query_id, ROUNDS_SPENT).await;
        }
        self.dialogs
            .open(ctx.chat_id, DialogKind::IdeaAnswer, &entry.id);
        answer(ctx, query_id, None).await?;
        ctx.reply(&correction_message(entry.draft.questions.len()))
            .await
    }

    /// „Verwerfen“: der Entwurf ist weg, angelegt wurde nichts
    async fn drop_draft(&self, ctx: &BotContext, query_id: &str, draft_id: &str) -> Result<()> {
        let state = self.deps.state()?;
        if self.open(ctx, query_id, &state, draft_id).await?.is_none() {
            return Ok(());
        }
        self.dialogs.cancel(ctx.chat_id);
        state.idea_drafts.remove(ctx.chat_id)?;
        answer(ctx, query_id, None).await?;
        ctx.update(DROPPED, None).await
    }

    /// „Alten verwerfen“: der wartende Text wird zur neuen Idee
    async fn replace(&self, ctx: &BotContext, query_id: &str, draft_id: &str) -> Result<()> {
        let state = self.deps.state()?;
        if self.open(ctx, query_id, &state, draft_id).await?.is_none() {
            return Ok(());
        }
        let Some(waiting) = state.idea_drafts.take_pending(ctx.chat_id)? else {
            return self.stale(ctx, query_id, NO_PENDING).await;
        };
        self.dialogs.cancel(ctx.chat_id);
        state.idea_drafts.remove(ctx.chat_id)?;
        answer(ctx, query_id, None).await?;
        ctx.update(DROPPED, None).await?;
        self.run(ctx, &state, &waiting, None).await
    }

    // ---------------------------------------------------------------- Anlegen

    /// „Anlegen“: Notiz, Ticket im Backlog oder Kommentar am genannten Ticket
    async fn create(&self, ctx: &BotContext, query_id: &str, draft_id: &str) -> Result<()> {
        let state = self.deps.state()?;
        let Some(entry) = self.open(ctx, query_id, &state, draft_id).await? else {
            return Ok(());
        };
        answer(ctx, query_id, None).await?;
        let draft = entry.draft;
        let made = match draft.target.kind {
            IdeaKind::Note => add_note(&state, &draft).await,
            IdeaKind::Ticket => add_ticket(&state, &draft).await,
            IdeaKind::Comment => add_comment(&state, &draft).await,
        };
        match made {
            Ok((what, id)) => {
                self.dialogs.cancel(ctx.chat_id);
                state.idea_drafts.remove(ctx.chat_id)?;
                ctx.update(&created_message(what, &id, &draft.title), None)
                    .await
            }
            Err(error) => report(ctx, &error).await,
        }
    }

    // ---------------------------------------------------------------- Dialoge

    /// Die Antwort auf eine Rückfrage oder eine Korrektur: eine weitere Runde am selben
    /// Entwurf. Eine Sprachnachricht kommt als ihr Transkript hier an
    pub(crate) async fn submit(
        &self,
        ctx: &BotContext,
        dialog: &Dialog,
        input: DialogInput,
    ) -> Result<()> {
        let text = input.text().trim().to_string();
        let state = self.deps.state()?;
        let Some(entry) = state.idea_drafts.with_id(ctx.chat_id, &dialog.target) else {
            return ctx.reply(&hint_message(NO_DRAFT)).await;
        };
        // Eine offene Rückfrage: die Nachricht ist ihre Antwort
        if let Some(questionnaire) = entry.questionnaire.clone().filter(|q| !q.is_done()) {
            if text.is_empty() {
                // Die Frage wartet weiter
                self.dialogs
                    .open(ctx.chat_id, DialogKind::IdeaAnswer, &entry.id);
                return ctx.reply(&hint_message(EMPTY_ANSWER)).await;
            }
            return self
                .ask_free(ctx, &state, entry, questionnaire, &text)
                .await;
        }
        if text.is_empty() {
            return ctx.reply(&hint_message(EMPTY_CORRECTION)).await;
        }
        if !self.rounds_left(&state, &entry.draft) {
            return ctx.reply(&hint_message(ROUNDS_SPENT)).await;
        }
        // Nach einer gescheiterten Runde gehören die Antworten auf die Rückfragen mit dazu
        let text = match entry.questionnaire.as_ref() {
            Some(done) => format!("{}\n{text}", done.answers_text(&entry.draft.questions)),
            None => text,
        };
        self.run(ctx, &state, &text, Some(entry)).await
    }

    // ---------------------------------------------------------------- Gemeinsames

    /// Der Entwurf, wenn der Chat ihn noch unter dieser ID hält. `None` heißt: der Hinweis
    /// ist schon raus und nichts wurde geändert
    async fn open(
        &self,
        ctx: &BotContext,
        query_id: &str,
        state: &Arc<AppState>,
        draft_id: &str,
    ) -> Result<Option<IdeaEntry>> {
        match state.idea_drafts.with_id(ctx.chat_id, draft_id) {
            Some(entry) => Ok(Some(entry)),
            None => {
                self.stale(ctx, query_id, NO_DRAFT).await?;
                Ok(None)
            }
        }
    }

    /// Ein geänderter Entwurf: speichern und die Bestätigung neu schreiben
    async fn changed(
        &self,
        ctx: &BotContext,
        query_id: &str,
        state: &Arc<AppState>,
        entry: &IdeaEntry,
        draft: IdeaDraft,
    ) -> Result<()> {
        if !state
            .idea_drafts
            .replace(ctx.chat_id, &entry.id, draft.clone())?
        {
            return self.stale(ctx, query_id, NO_DRAFT).await;
        }
        let entry = IdeaEntry {
            draft,
            ..entry.clone()
        };
        let rounds_left = self.rounds_left(state, &entry.draft);
        let (text, keyboard) = self.confirmation(state, &entry, rounds_left).await;
        answer(ctx, query_id, None).await?;
        ctx.update(&text, Some(keyboard)).await
    }

    /// Text und Knöpfe der Bestätigung; der Name des Ziel-Boards kommt aus den eigenen Boards
    async fn confirmation(
        &self,
        state: &Arc<AppState>,
        entry: &IdeaEntry,
        rounds_left: bool,
    ) -> (String, InlineKeyboard) {
        let boards: Vec<BoardChoice> = own_boards(state).await.unwrap_or_default();
        let name = boards
            .iter()
            .find(|board| board.id == entry.draft.target.board_id)
            .map(|board| board.name.as_str());
        (
            draft_message(&entry.draft, name),
            draft_keyboard(&entry.id, &entry.draft, boards.len(), rounds_left),
        )
    }

    /// Sind noch Korrekturrunden übrig? Ohne lesbare Einstellung gilt die Vorgabe
    fn rounds_left(&self, state: &Arc<AppState>, draft: &IdeaDraft) -> bool {
        let max = TelegramConfigStore::new(state.home.clone())
            .read()
            .unwrap_or_default()
            .ideas
            .max_rounds;
        draft.rounds < max
    }

    /// Ein Knopf, der nicht mehr zum Zustand passt: Hinweisfenster und neuer Text, sonst
    /// nichts
    async fn stale(&self, ctx: &BotContext, query_id: &str, hint: &str) -> Result<()> {
        answer(ctx, query_id, Some(hint)).await?;
        ctx.update(&hint_message(hint), None).await
    }

    /// Ersetzt die Ansage durch das Ergebnis
    async fn edit(
        &self,
        ctx: &BotContext,
        message_id: i64,
        html: &str,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<()> {
        ctx.api
            .edit_message_text(ctx.chat_id, message_id, html, keyboard)
            .await
            .map_err(nexus_core::Error::unexpected)
    }
}

/// Was ein Knopf an einer Rückfrage auslöst (Nummer der Frage, ggf. der Möglichkeit)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AskOp {
    Choose(usize, usize),
    Toggle(usize, usize),
    Confirm(usize),
    Skip(usize),
    Back(usize),
}

/// Der Hinweis zu einem Schritt im Fragebogen, der nichts geändert hat
fn ask_hint(error: AskError) -> &'static str {
    match error {
        AskError::Stale => ASK_STALE,
        AskError::UnknownOption => UNKNOWN_ANSWER,
        AskError::NothingTicked => NOTHING_TICKED,
        AskError::Empty => EMPTY_ANSWER,
    }
}

/// Antwort auf einen Knopf, dessen Board es nicht mehr gibt
const UNKNOWN_BOARD: &str = "So ein Board habe ich nicht";

/// Antwort auf „Alten verwerfen“, wenn keine neue Idee mehr wartet
const NO_PENDING: &str = "Da wartet keine neue Idee mehr";

#[async_trait]
impl CommandHandler for IdeaFlow {
    async fn run(&self, ctx: &BotContext, args: &str) -> Result<()> {
        let input = args.trim();
        if input.is_empty() {
            return ctx.reply(IDEA_WITHOUT_TEXT).await;
        }
        self.start(ctx, input).await
    }
}

#[async_trait]
impl MessageHandler for IdeaFlow {
    async fn run(&self, ctx: &BotContext, message: &Message) -> Result<bool> {
        // Eine Sprachnachricht kommt als ihr Transkript an (der Router erkennt sie vorher);
        // jeder andere Anhang ist keine Idee
        let Some(text) = message.text.as_deref().map(str::trim) else {
            return Ok(false);
        };
        if text.is_empty() {
            return Ok(false);
        }
        self.start(ctx, text).await?;
        Ok(true)
    }
}

#[async_trait]
impl CallbackHandler for IdeaFlow {
    async fn run(&self, ctx: &BotContext, action: &CallbackAction, query_id: &str) -> Result<bool> {
        match action {
            CallbackAction::IdeaCreate(draft) => self.create(ctx, query_id, draft).await?,
            CallbackAction::IdeaTarget(draft, target) => {
                self.switch_target(ctx, query_id, draft, *target).await?
            }
            CallbackAction::IdeaBoardList(draft) => self.board_choice(ctx, query_id, draft).await?,
            CallbackAction::IdeaBoard(draft, board_id) => {
                self.set_board(ctx, query_id, draft, *board_id).await?
            }
            CallbackAction::IdeaEdit(draft) => self.correct(ctx, query_id, draft).await?,
            CallbackAction::IdeaDrop(draft) => self.drop_draft(ctx, query_id, draft).await?,
            CallbackAction::IdeaReplace(draft) => self.replace(ctx, query_id, draft).await?,
            CallbackAction::IdeaAskChoose(draft, question, option) => {
                self.ask(ctx, query_id, draft, AskOp::Choose(*question, *option))
                    .await?
            }
            CallbackAction::IdeaAskToggle(draft, question, option) => {
                self.ask(ctx, query_id, draft, AskOp::Toggle(*question, *option))
                    .await?
            }
            CallbackAction::IdeaAskDone(draft, question) => {
                self.ask(ctx, query_id, draft, AskOp::Confirm(*question))
                    .await?
            }
            CallbackAction::IdeaAskSkip(draft, question) => {
                self.ask(ctx, query_id, draft, AskOp::Skip(*question))
                    .await?
            }
            CallbackAction::IdeaAskBack(draft, question) => {
                self.ask(ctx, query_id, draft, AskOp::Back(*question))
                    .await?
            }
            CallbackAction::IdeaAskCancel(draft) => self.drop_draft(ctx, query_id, draft).await?,
            // „Abbrechen“ beantwortet [`crate::telegram::commands`]; hier fällt nur die
            // wartende Idee weg, damit sie nicht später noch angelegt wird
            CallbackAction::Cancel => {
                if let Ok(state) = self.deps.state() {
                    let _ = state.idea_drafts.take_pending(ctx.chat_id);
                }
                return Ok(false);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// Setzt das Ziel des Entwurfs: einen Kommentar schlägt Claude vor, Notiz und Ticket
/// entscheidet der Nutzer – in der ersten Runde über `ideas.defaultTarget`, danach bleibt,
/// was er am offenen Entwurf gewählt hat
fn with_target(draft: &mut IdeaDraft, default: IdeaTarget, previous: Option<&IdeaDraft>) {
    if draft.target.kind == IdeaKind::Comment {
        return;
    }
    let chosen = previous
        .map(|previous| previous.target.kind)
        .filter(|kind| *kind != IdeaKind::Comment)
        .unwrap_or_else(|| kind_of(default));
    draft.target.kind = chosen;
    draft.target.ticket_id = None;
}

/// Das Ziel der Einstellung als Art eines Entwurfs
fn kind_of(target: IdeaTarget) -> IdeaKind {
    match target {
        IdeaTarget::Note => IdeaKind::Note,
        IdeaTarget::Ticket => IdeaKind::Ticket,
    }
}

/// Die Notiz über den Notiz-Dienst; die Eingabe steht als Transkript in den Details.
/// `force`, weil die möglichen Dubletten in der Bestätigung standen und der Nutzer trotzdem
/// anlegen wollte
async fn add_note(state: &Arc<AppState>, draft: &IdeaDraft) -> Result<(&'static str, String)> {
    let board_id = draft.target.board_id;
    let text = NewNoteText {
        title: one_line(&draft.title),
        body: Some(note_body(draft)),
    };
    let note = state
        .board
        .run(|backend| async move {
            backend
                .services()
                .notes
                .add(
                    board_id,
                    &text,
                    &backend.actor(None),
                    &AddNoteOptions {
                        force: true,
                        ..AddNoteOptions::default()
                    },
                )
                .await
        })
        .await?;
    Ok(("Notiz angelegt:", note.id.to_string()))
}

/// Das Ticket im Backlog samt Kommentar „Aus Telegram-Sprachnotiz“ mit dem Transkript
async fn add_ticket(state: &Arc<AppState>, draft: &IdeaDraft) -> Result<(&'static str, String)> {
    let board_id = draft.target.board_id;
    let input = NewTicketInput {
        description: Some(draft.details.trim().to_string()),
        status: Some(TicketStatus::Backlog),
        ..NewTicketInput::titled(one_line(&draft.title))
    };
    let comment = ticket_comment(draft);
    let ticket = state
        .board
        .run(|backend| async move {
            let ticket = backend
                .services()
                .tickets
                .create(
                    board_id,
                    &input,
                    &backend.actor(None),
                    // Mögliche Dubletten standen in der Bestätigung; der Nutzer wollte es
                    &WriteOptions {
                        allow_duplicate: true,
                        ..WriteOptions::default()
                    },
                )
                .await?;
            backend
                .services()
                .comments
                .add(&ticket.ticket.id, &comment, &backend.actor(None), false)
                .await?;
            Ok(ticket)
        })
        .await?;
    Ok(("Ticket angelegt:", ticket.ticket.id))
}

/// Der Kommentar am genannten Ticket, mit dem Zusatz „(über Telegram)“
async fn add_comment(state: &Arc<AppState>, draft: &IdeaDraft) -> Result<(&'static str, String)> {
    let ticket_id = draft
        .target
        .ticket_id
        .clone()
        .unwrap_or_else(|| draft.title.clone());
    let body = via_telegram(&comment_body(draft));
    let id = ticket_id.clone();
    state
        .board
        .run(|backend| async move {
            backend
                .services()
                .comments
                .add(&id, &body, &backend.actor(None), false)
                .await
        })
        .await?;
    Ok(("Kommentar geschrieben an", ticket_id))
}

/// Die Details einer Notiz: der Text des Entwurfs und darunter die Eingabe als Transkript
fn note_body(draft: &IdeaDraft) -> String {
    let mut out = draft.details.trim().to_string();
    if let Some(transcript) = transcript(draft) {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&format!("{TRANSCRIPT} {transcript}"));
    }
    out
}

/// Der Kommentar, der am neuen Ticket festhält, woher es kommt
fn ticket_comment(draft: &IdeaDraft) -> String {
    let mut out = via_telegram(FROM_TELEGRAM_VOICE);
    if let Some(transcript) = transcript(draft) {
        out.push_str(&format!("\n{TRANSCRIPT} {transcript}"));
    }
    out
}

/// Der Text eines Kommentars an einem vorhandenen Ticket: die Details, sonst der Titel
fn comment_body(draft: &IdeaDraft) -> String {
    match draft.details.trim() {
        "" => draft.title.trim().to_string(),
        details => details.to_string(),
    }
}

/// Die Eingabe, soweit sie nicht schon in den Details steht
fn transcript(draft: &IdeaDraft) -> Option<String> {
    let transcript = draft.transcript.trim();
    if transcript.is_empty() || draft.details.trim() == transcript {
        return None;
    }
    Some(transcript.replace('\n', " "))
}

/// Ein Titel ist eine Zeile – der Notiz-Dienst nimmt keinen mehrzeiligen an
fn one_line(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use nexus_core::telegram::idea::IdeaTargetRef;

    use super::*;

    /// Ein Entwurf als Notiz
    fn draft() -> IdeaDraft {
        IdeaDraft {
            summary: "Der Cache läuft nie ab.".into(),
            target: IdeaTargetRef {
                kind: IdeaKind::Note,
                board_id: 3,
                ticket_id: None,
            },
            title: "Cache der Board-Ansicht läuft nie ab".into(),
            details: "Gesehen in der Spaltenansicht.".into(),
            related: Vec::new(),
            duplicates: Vec::new(),
            questions: Vec::new(),
            confidence: 0.8,
            transcript: "Der Cache läuft nie ab, hab ich eben gesehen.".into(),
            rounds: 1,
        }
    }

    #[test]
    fn die_einstellung_gibt_das_ziel_der_ersten_runde_vor() {
        let mut as_note = IdeaDraft {
            target: IdeaTargetRef {
                kind: IdeaKind::Ticket,
                board_id: 3,
                ticket_id: None,
            },
            ..draft()
        };
        with_target(&mut as_note, IdeaTarget::Note, None);
        assert_eq!(as_note.target.kind, IdeaKind::Note);

        let mut as_ticket = draft();
        with_target(&mut as_ticket, IdeaTarget::Ticket, None);
        assert_eq!(as_ticket.target.kind, IdeaKind::Ticket);
    }

    #[test]
    fn in_der_korrekturrunde_bleibt_die_wahl_des_nutzers() {
        let chosen = IdeaDraft {
            target: IdeaTargetRef {
                kind: IdeaKind::Ticket,
                board_id: 3,
                ticket_id: None,
            },
            ..draft()
        };
        let mut next = draft();

        with_target(&mut next, IdeaTarget::Note, Some(&chosen));

        assert_eq!(
            next.target.kind,
            IdeaKind::Ticket,
            "der Knopf gilt weiter, nicht die Vorgabe"
        );
    }

    #[test]
    fn ein_kommentar_bleibt_an_seinem_ticket() {
        let mut comment = IdeaDraft {
            target: IdeaTargetRef {
                kind: IdeaKind::Comment,
                board_id: 3,
                ticket_id: Some("NXS-0042".into()),
            },
            ..draft()
        };

        with_target(&mut comment, IdeaTarget::Ticket, None);

        assert_eq!(comment.target.kind, IdeaKind::Comment);
        assert_eq!(comment.target.ticket_id.as_deref(), Some("NXS-0042"));
    }

    #[test]
    fn das_transkript_steht_in_den_details_der_notiz() {
        let body = note_body(&draft());

        assert_eq!(
            body,
            "Gesehen in der Spaltenansicht.\n\nTranskript: Der Cache läuft nie ab, hab ich eben gesehen."
        );
    }

    #[test]
    fn das_transkript_kommt_nicht_doppelt() {
        let same = IdeaDraft {
            details: "Der Cache läuft nie ab.".into(),
            transcript: "Der Cache läuft nie ab.".into(),
            ..draft()
        };

        assert_eq!(note_body(&same), "Der Cache läuft nie ab.");
        assert_eq!(
            ticket_comment(&same),
            "Aus Telegram-Sprachnotiz (über Telegram)"
        );
    }

    #[test]
    fn das_ticket_haelt_seine_herkunft_im_kommentar_fest() {
        assert_eq!(
            ticket_comment(&draft()),
            "Aus Telegram-Sprachnotiz (über Telegram)\nTranskript: Der Cache läuft nie ab, hab ich eben gesehen."
        );
    }

    #[test]
    fn der_kommentar_nimmt_die_details_und_sonst_den_titel() {
        assert_eq!(comment_body(&draft()), "Gesehen in der Spaltenansicht.");
        let ohne = IdeaDraft {
            details: "  ".into(),
            ..draft()
        };
        assert_eq!(comment_body(&ohne), "Cache der Board-Ansicht läuft nie ab");
    }

    #[test]
    fn der_titel_bleibt_eine_zeile() {
        assert_eq!(one_line("Cache läuft\nnie   ab"), "Cache läuft nie ab");
    }
}
