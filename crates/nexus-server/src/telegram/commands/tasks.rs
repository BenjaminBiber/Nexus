//! `/aufgaben`: „Meine Aufgaben“ als eine Nachricht je Eintrag, höchstens
//! [`INBOX_PAGE_SIZE`] auf einmal, dann je offener Art ein Knopf „Mehr“.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nexus_core::board::inbox::order::InboxOrder;
use nexus_core::board::inbox::order_file::InboxOrderFile;
use nexus_core::board::inbox::service::{InboxFilter, InboxService};
use nexus_core::board::inbox::types::{InboxItem, InboxKind, InboxList};
use nexus_core::telegram::CallbackAction;
use nexus_core::Result;

use crate::board_db::ServerBoardDatabase;
use crate::state::AppState;
use crate::telegram::api::{InlineButton, InlineKeyboard};
use crate::telegram::commands::questions::question_detail;
use crate::telegram::commands::{report, Deps};
use crate::telegram::question_views::question_message;
use crate::telegram::router::{BotContext, CallbackHandler, CommandHandler};
use crate::telegram::views::{
    inbox_item_message, more_keyboard, more_page_title, no_more_items, remaining_per_kind,
    INBOX_EMPTY, INBOX_PAGE_SIZE,
};

/// Der Befehl `/aufgaben`
#[derive(Debug)]
pub(crate) struct TasksCommand(pub(crate) Deps);

#[async_trait]
impl CommandHandler for TasksCommand {
    async fn run(&self, ctx: &BotContext, _args: &str) -> Result<()> {
        let state = self.0.state()?;
        let list = match inbox_list(&state).await {
            Ok(list) => list,
            Err(error) => return report(ctx, &error).await,
        };
        if list.items.is_empty() {
            return ctx.reply(INBOX_EMPTY).await;
        }
        let shown: Vec<InboxItem> = list.items.iter().take(INBOX_PAGE_SIZE).cloned().collect();
        send_page(
            ctx,
            &state,
            None,
            &shown,
            &remaining_per_kind(&shown, &list),
            2,
        )
        .await
    }
}

/// Der Knopf „Mehr“ unter `/aufgaben`: die nächste Seite einer Art
#[derive(Debug)]
pub(crate) struct MoreButton(pub(crate) Deps);

#[async_trait]
impl CallbackHandler for MoreButton {
    async fn run(&self, ctx: &BotContext, action: &CallbackAction, query_id: &str) -> Result<bool> {
        let CallbackAction::More(kind, page) = action else {
            return Ok(false);
        };
        let (kind, page) = (*kind, (*page).max(2));
        let state = self.0.state()?;
        answer(ctx, query_id).await?;
        let list = match inbox_list(&state).await {
            Ok(list) => list,
            Err(error) => {
                report(ctx, &error).await?;
                return Ok(true);
            }
        };
        // Seite 1 steckt in der Antwort auf `/aufgaben` – die mischt die Arten, deshalb zählt
        // hier nur, was von dieser Art schon gezeigt wurde
        let of_kind: Vec<InboxItem> = list
            .items
            .iter()
            .filter(|item| item.kind == kind)
            .cloned()
            .collect();
        let skip = shown_before(&list, kind, page);
        let page_items: Vec<InboxItem> = of_kind
            .iter()
            .skip(skip)
            .take(INBOX_PAGE_SIZE)
            .cloned()
            .collect();
        if page_items.is_empty() {
            ctx.reply(&no_more_items(kind)).await?;
            return Ok(true);
        }
        let rest = of_kind.len() as i32 - (skip + page_items.len()) as i32;
        send_page(
            ctx,
            &state,
            Some(more_page_title(kind, page)),
            &page_items,
            &[(kind, rest)],
            page + 1,
        )
        .await?;
        Ok(true)
    }
}

/// Wie viele Einträge dieser Art vor `page` schon gezeigt wurden: auf Seite 1 die der
/// gemischten ersten Seite, danach je Seite [`INBOX_PAGE_SIZE`]
fn shown_before(list: &InboxList, kind: InboxKind, page: u32) -> usize {
    let on_first = list
        .items
        .iter()
        .take(INBOX_PAGE_SIZE)
        .filter(|item| item.kind == kind)
        .count();
    on_first + (page as usize - 2) * INBOX_PAGE_SIZE
}

/// Schickt die Einträge einer Seite, je Eintrag eine Nachricht; die Knöpfe „Mehr“ hängen
/// unter der letzten
async fn send_page(
    ctx: &BotContext,
    state: &Arc<AppState>,
    title: Option<String>,
    items: &[InboxItem],
    remaining: &[(InboxKind, i32)],
    next_page: u32,
) -> Result<()> {
    if let Some(title) = title {
        ctx.reply(&title).await?;
    }
    let now = state.config.clock.now();
    let keyboard = more_keyboard(remaining, next_page);
    let last = items.len().saturating_sub(1);
    for (index, item) in items.iter().enumerate() {
        let (text, own) = match question_message_of(state, item, now).await {
            Some(question) => question,
            None => inbox_item_message(item, now),
        };
        // „Mehr“ hängt unter dem letzten Eintrag, unter dessen eigenen Knöpfen
        let below = match index == last {
            true => keyboard.clone(),
            false => None,
        };
        match stacked(own, below) {
            Some(keyboard) => ctx.reply_with(&text, keyboard).await?,
            None => ctx.reply(&text).await?,
        }
    }
    Ok(())
}

/// Die Nachricht einer offenen Frage samt ihren Knöpfen (Optionen der neuesten Recherche,
/// Freitext, Recherche, Verwerfen). `None` bei jedem anderen Eintrag und auch dann, wenn die
/// Frage gerade nicht lesbar ist – dann steht der Eintrag wie bisher ohne Knöpfe da
async fn question_message_of(
    state: &Arc<AppState>,
    item: &InboxItem,
    now: DateTime<Utc>,
) -> Option<(String, Option<InlineKeyboard>)> {
    if item.kind != InboxKind::Question {
        return None;
    }
    let detail = question_detail(state, item.question_id.as_deref()?)
        .await
        .ok()?;
    let (text, keyboard) = question_message(&detail, now);
    Some((text, Some(keyboard)))
}

/// Legt zwei Tastaturen untereinander; ohne beide gibt es keine
fn stacked(own: Option<InlineKeyboard>, below: Option<InlineKeyboard>) -> Option<InlineKeyboard> {
    let rows: Vec<Vec<InlineButton>> = own
        .into_iter()
        .chain(below)
        .flat_map(|keyboard| keyboard.inline_keyboard)
        .collect();
    match rows.is_empty() {
        true => None,
        false => Some(InlineKeyboard::new(rows)),
    }
}

/// Beantwortet den Knopfdruck, damit er beim Nutzer aufhört zu drehen
async fn answer(ctx: &BotContext, query_id: &str) -> Result<()> {
    ctx.api
        .answer_callback_query(query_id, None, false)
        .await
        .map_err(nexus_core::Error::unexpected)
}

/// „Meine Aufgaben“ über alle eigenen Boards, in der gespeicherten Reihenfolge – derselbe
/// Weg wie `GET /api/db/inbox`
pub(crate) async fn inbox_list(state: &Arc<AppState>) -> Result<InboxList> {
    let keys = InboxOrderFile::new(state.home.clone()).read();
    state
        .board
        .run(|backend| async move {
            let items = backend
                .services()
                .inbox
                .list(&backend.actor(None).user_id, &InboxFilter::default())
                .await?;
            Ok(InboxService::<ServerBoardDatabase>::summarize(
                InboxOrder::apply(items, &keys),
            ))
        })
        .await
}

#[cfg(test)]
mod tests {
    use nexus_core::board::inbox::types::InboxCounts;

    use super::*;

    /// Eine Liste mit so vielen Einträgen dieser Art am Anfang
    fn list(kinds: &[InboxKind]) -> InboxList {
        let items: Vec<InboxItem> = kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| InboxItem {
                key: format!("ticket:NXS-{index:04}"),
                kind: *kind,
                title: format!("Eintrag {index}"),
                detail: None,
                board_id: Some(3),
                board_name: Some("Sprint".into()),
                project_key: "NXS".into(),
                ticket_id: Some(format!("NXS-{index:04}")),
                question_id: None,
                note_id: None,
                waiting_tickets: 0,
                priority: None,
                since: "2026-11-03T10:00:00Z".into(),
                rank: index as i32,
            })
            .collect();
        InboxService::<ServerBoardDatabase>::summarize(items)
    }

    #[test]
    fn zahlt_fuer_seite_zwei_die_eintraege_der_gemischten_ersten_seite() {
        // Zwölf Fragen: zehn stehen auf der gemischten Seite 1
        let liste = list(&[InboxKind::Question; 12]);

        assert_eq!(shown_before(&liste, InboxKind::Question, 2), 10);
        assert_eq!(shown_before(&liste, InboxKind::Question, 3), 20);
    }

    #[test]
    fn zahlt_nur_die_eintraege_der_eigenen_art() {
        let mut kinds = [InboxKind::Note; 12];
        kinds[0] = InboxKind::Question;

        let liste = list(&kinds);

        assert_eq!(
            shown_before(&liste, InboxKind::Note, 2),
            9,
            "auf Seite 1 standen neun Notizen und eine Frage"
        );
    }

    #[test]
    fn fasst_die_zaehler_wie_die_route_zusammen() {
        let liste = list(&[InboxKind::Merge, InboxKind::Note, InboxKind::Note]);

        assert_eq!(liste.total, 3);
        assert_eq!(
            liste.counts,
            InboxCounts {
                merge: 1,
                setup: 0,
                question: 0,
                accept: 0,
                blocked: 0,
                planning: 0,
                note: 2,
            }
        );
    }
}
