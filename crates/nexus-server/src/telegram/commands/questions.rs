//! Die Knöpfe an einer offenen Frage: eine Option der neuesten Recherche wählen, per eigenem
//! Text entscheiden, eine Recherche starten oder die Frage mit Begründung verwerfen.
//!
//! Wie bei den Knöpfen der Tickets ([`super::actions`]) laufen alle Aktionen über dieselben
//! Dienste wie die Routen (`state.board`, `state.dispatch`); Handelnder ist der Mensch des
//! Servers. Beim Klick wird der Zustand neu gelesen: Ist die Frage nicht mehr offen oder
//! steht die Option nicht mehr zur Wahl, ändert sich nichts, und der Hinweis erscheint als
//! Fenster am Knopf und als neuer Text der Nachricht.
//!
//! Eine Option braucht zwei Klicks: der erste ersetzt die Knöpfe durch die Sicherheitsfrage,
//! erst „Ja“ entscheidet. Eigener Text und Verwerfen kommen als Nachricht; dafür hält
//! [`crate::telegram::dialog`] je Chat einen offenen Dialog. Verwerfen ohne Begründung ist
//! nicht möglich.

use std::sync::Arc;

use async_trait::async_trait;
use nexus_core::actors::AuthorKind;
use nexus_core::board::question_service::BoardQuestionWriteOptions;
use nexus_core::board::question_types::{
    BoardQuestionActor, BoardQuestionDetail, BoardQuestionOption,
};
use nexus_core::questions::{DecisionInput, QuestionState};
use nexus_core::telegram::CallbackAction;
use nexus_core::Result;

use crate::board_runtime::BoardBackend;
use crate::state::AppState;
use crate::telegram::action_views::{dialog_message, done_message, hint_message, via_telegram};
use crate::telegram::commands::actions::answer;
use crate::telegram::commands::{report, Deps};
use crate::telegram::dialog::{Dialog, DialogInput, DialogKind, Dialogs};
use crate::telegram::question_views::{
    not_open, option_question, options, unknown_option, DISCARD_WITHOUT_REASON, EMPTY_DECISION,
};
use crate::telegram::router::{BotContext, CallbackHandler};

/// Begründung, die der Bot an eine Entscheidung per Option schreibt
const DECIDED: &str = "Entschieden";

/// Die Knöpfe der Fragen und die Antworten auf ihre Dialoge
pub(crate) struct Questions {
    deps: Deps,
    dialogs: Arc<Dialogs>,
}

impl std::fmt::Debug for Questions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Questions")
            .field("dialogs", &self.dialogs)
            .finish_non_exhaustive()
    }
}

impl Questions {
    /// Knöpfe auf diesen Diensten mit diesen Dialogen
    pub(crate) fn new(deps: Deps, dialogs: Arc<Dialogs>) -> Self {
        Self { deps, dialogs }
    }

    /// Erster Klick auf eine Option: die Sicherheitsfrage ersetzt die Knöpfe der Frage
    async fn ask(&self, ctx: &BotContext, query_id: &str, id: &str, option: &str) -> Result<()> {
        let state = self.deps.state()?;
        let Some(detail) = self.open(ctx, query_id, &state, id).await? else {
            return Ok(());
        };
        let Some(option) = self.option(ctx, query_id, &detail, option).await? else {
            return Ok(());
        };
        answer(ctx, query_id, None).await?;
        let (text, keyboard) = option_question(&detail.view.question.id, &option);
        ctx.update(&text, Some(keyboard)).await
    }

    /// „Ja, O2“: Entscheidung für die Option wie `POST /api/db/questions/:id/decision`
    async fn decide_option(
        &self,
        ctx: &BotContext,
        query_id: &str,
        id: &str,
        option: &str,
    ) -> Result<()> {
        let state = self.deps.state()?;
        let Some(detail) = self.open(ctx, query_id, &state, id).await? else {
            return Ok(());
        };
        let Some(option) = self.option(ctx, query_id, &detail, option).await? else {
            return Ok(());
        };
        answer(ctx, query_id, None).await?;
        let input = DecisionInput::option(&option.id, &via_telegram(DECIDED));
        match decide(&state, &detail, &input).await {
            Ok(_) => {
                let detail = format!(
                    "{}: {} {}",
                    detail.view.question.id, option.id, option.title
                );
                self.done(ctx, &state, "Frage entschieden", Some(&detail), false)
                    .await
            }
            Err(error) => report(ctx, &error).await,
        }
    }

    /// „Recherche starten“: wie `POST /api/dispatch/research/:question_id`
    async fn research(&self, ctx: &BotContext, query_id: &str, id: &str) -> Result<()> {
        let state = self.deps.state()?;
        let Some(detail) = self.open(ctx, query_id, &state, id).await? else {
            return Ok(());
        };
        let id = detail.view.question.id.clone();
        answer(ctx, query_id, None).await?;
        match state.dispatch.start_research(&id).await {
            Ok(_) => {
                self.done(
                    ctx,
                    &state,
                    "Recherche gestartet",
                    Some(&format!("{id}: Der Agent recherchiert jetzt.")),
                    false,
                )
                .await
            }
            Err(error) => report(ctx, &error).await,
        }
    }

    /// „Eigener Text“ und „Verwerfen“: fragt nach dem Text; die nächste Nachricht des Chats
    /// ist die Antwort
    async fn open_dialog(
        &self,
        ctx: &BotContext,
        query_id: &str,
        id: &str,
        kind: DialogKind,
    ) -> Result<()> {
        let state = self.deps.state()?;
        let Some(detail) = self.open(ctx, query_id, &state, id).await? else {
            return Ok(());
        };
        let id = detail.view.question.id.clone();
        self.dialogs.open(ctx.chat_id, kind, &id);
        answer(ctx, query_id, None).await?;
        ctx.update(&dialog_message(kind, &id), None).await
    }

    /// Die Antwort auf einen Dialog zu einer Frage: der eigene Text als Entscheidung oder die
    /// Begründung zum Verwerfen. Eine Sprachnachricht kommt später als Transkript ebenfalls
    /// hier an
    pub(crate) async fn submit(
        &self,
        ctx: &BotContext,
        dialog: &Dialog,
        input: DialogInput,
    ) -> Result<()> {
        let text = input.text().trim().to_string();
        if text.is_empty() {
            let hint = match dialog.kind {
                DialogKind::QuestionDiscard => DISCARD_WITHOUT_REASON,
                _ => EMPTY_DECISION,
            };
            return ctx.reply(&hint_message(hint)).await;
        }
        let state = self.deps.state()?;
        let detail = match question_detail(&state, &dialog.target).await {
            Ok(detail) => detail,
            Err(error) => return report(ctx, &error).await,
        };
        let id = detail.view.question.id.clone();
        if detail.view.question.state != QuestionState::Open {
            return ctx.reply(&hint_message(&not_open(&id))).await;
        }
        let (what, done) = match dialog.kind {
            DialogKind::QuestionDiscard => (
                "Frage verworfen",
                discard(&state, &detail, &via_telegram(&text)).await,
            ),
            _ => (
                "Frage entschieden",
                decide(
                    &state,
                    &detail,
                    &DecisionInput::text(&text, &via_telegram(DECIDED)),
                )
                .await,
            ),
        };
        match done {
            Ok(_) => {
                self.done(ctx, &state, what, Some(&format!("{id}: {text}")), true)
                    .await
            }
            Err(error) => report(ctx, &error).await,
        }
    }

    /// Die Frage, wenn sie noch offen ist. `None` heißt: der Hinweis ist schon raus und nichts
    /// wurde geändert
    async fn open(
        &self,
        ctx: &BotContext,
        query_id: &str,
        state: &Arc<AppState>,
        id: &str,
    ) -> Result<Option<BoardQuestionDetail>> {
        let detail = match question_detail(state, id).await {
            Ok(detail) => detail,
            Err(error) => {
                answer(ctx, query_id, None).await?;
                report(ctx, &error).await?;
                return Ok(None);
            }
        };
        if detail.view.question.state != QuestionState::Open {
            self.stale(ctx, query_id, &not_open(&detail.view.question.id))
                .await?;
            return Ok(None);
        }
        Ok(Some(detail))
    }

    /// Die Option der neuesten Recherche; `None` wie in [`Self::open`]
    async fn option(
        &self,
        ctx: &BotContext,
        query_id: &str,
        detail: &BoardQuestionDetail,
        option: &str,
    ) -> Result<Option<BoardQuestionOption>> {
        match options(detail)
            .iter()
            .find(|known| known.id == option)
            .cloned()
        {
            Some(found) => Ok(Some(found)),
            None => {
                self.stale(ctx, query_id, &unknown_option(option)).await?;
                Ok(None)
            }
        }
    }

    /// Ein Knopf, der nicht mehr zum Zustand passt: Hinweisfenster und neuer Text, sonst
    /// nichts
    async fn stale(&self, ctx: &BotContext, query_id: &str, hint: &str) -> Result<()> {
        answer(ctx, query_id, Some(hint)).await?;
        ctx.update(&hint_message(hint), None).await
    }

    /// Ergebnis einer Aktion; `reply` schickt es als neue Nachricht, weil die Antwort auf
    /// einen Dialog vom Benutzer kam und dessen Nachricht sich nicht ändern lässt
    async fn done(
        &self,
        ctx: &BotContext,
        state: &Arc<AppState>,
        what: &str,
        detail: Option<&str>,
        reply: bool,
    ) -> Result<()> {
        let text = done_message(what, detail, state.config.clock.now());
        match reply {
            true => ctx.reply(&text).await,
            false => ctx.update(&text, None).await,
        }
    }
}

#[async_trait]
impl CallbackHandler for Questions {
    async fn run(&self, ctx: &BotContext, action: &CallbackAction, query_id: &str) -> Result<bool> {
        match action {
            CallbackAction::QuestionOption(id, option) => {
                self.ask(ctx, query_id, id, option).await?
            }
            CallbackAction::QuestionOptionConfirm(id, option) => {
                self.decide_option(ctx, query_id, id, option).await?
            }
            CallbackAction::QuestionFree(id) => {
                self.open_dialog(ctx, query_id, id, DialogKind::QuestionText)
                    .await?
            }
            CallbackAction::QuestionResearch(id) => self.research(ctx, query_id, id).await?,
            CallbackAction::QuestionDiscard(id) => {
                self.open_dialog(ctx, query_id, id, DialogKind::QuestionDiscard)
                    .await?
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// Die Frage samt Recherchen – derselbe Weg wie `GET /api/db/questions/:id`
pub(crate) async fn question_detail(
    state: &Arc<AppState>,
    id: &str,
) -> Result<BoardQuestionDetail> {
    let id = id.to_string();
    state
        .board
        .run(|backend| async move { backend.services().questions.get(&id).await })
        .await
}

/// Entscheidet die Frage auf der gelesenen Version wie `POST /api/db/questions/:id/decision`
async fn decide(
    state: &Arc<AppState>,
    detail: &BoardQuestionDetail,
    input: &DecisionInput,
) -> Result<BoardQuestionDetail> {
    let id = detail.view.question.id.clone();
    let options = BoardQuestionWriteOptions::rev(detail.view.question.rev);
    let input = input.clone();
    state
        .board
        .run(|backend| async move {
            backend
                .services()
                .questions
                .answer(&id, &input, &human(backend.as_ref()), &options)
                .await
        })
        .await
}

/// Verwirft die Frage mit Begründung wie `POST /api/db/questions/:id/discard`
async fn discard(
    state: &Arc<AppState>,
    detail: &BoardQuestionDetail,
    reason: &str,
) -> Result<BoardQuestionDetail> {
    let id = detail.view.question.id.clone();
    let options = BoardQuestionWriteOptions::rev(detail.view.question.rev);
    let reason = reason.to_string();
    state
        .board
        .run(|backend| async move {
            backend
                .services()
                .questions
                .discard(&id, Some(&reason), &human(backend.as_ref()), &options)
                .await
        })
        .await
}

/// Der Mensch des Servers; nur er darf entscheiden und verwerfen
fn human(backend: &dyn BoardBackend) -> BoardQuestionActor {
    BoardQuestionActor {
        actor: backend.actor(None),
        kind: AuthorKind::Human,
        session: None,
    }
}
