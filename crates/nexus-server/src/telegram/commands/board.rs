//! `/board [name|id]` und der Knopf, der das aktive Board umschaltet.

use std::sync::Arc;

use async_trait::async_trait;
use nexus_core::board::board_service::BoardListFilter;
use nexus_core::board::config::BoardConfig;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::telegram::{escape, CallbackAction};
use nexus_core::Result;

use crate::state::AppState;
use crate::telegram::commands::{report, Deps};
use crate::telegram::router::{BotContext, CallbackHandler, CommandHandler};
use crate::telegram::views::{board_keyboard, board_message, BoardChoice, NO_ACTIVE_BOARD};

/// Antwort auf eine Angabe, zu der es kein eigenes Board gibt
const UNKNOWN_BOARD: &str = "So ein Board habe ich nicht";

/// Rückmeldung im Hinweisfenster des Knopfes
const SWITCHED: &str = "Aktives Board umgeschaltet";

/// Der Befehl `/board`
#[derive(Debug)]
pub(crate) struct BoardCommand(pub(crate) Deps);

#[async_trait]
impl CommandHandler for BoardCommand {
    async fn run(&self, ctx: &BotContext, args: &str) -> Result<()> {
        let state = self.0.state()?;
        let boards = match own_boards(&state).await {
            Ok(boards) => boards,
            Err(error) => return report(ctx, &error).await,
        };
        let wanted = args.trim();
        let chosen = match wanted.is_empty() {
            true => active_board(&state).filter(|id| boards.iter().any(|b| b.id == *id)),
            false => match find(&boards, wanted) {
                Some(id) => Some(id),
                // Eine Angabe, die zu keinem Board passt, ist keine Antwort „kein aktives
                // Board“: hier hat jemand etwas gesucht
                None => {
                    let text = format!("{UNKNOWN_BOARD}: {}", escape(&shortened(wanted)));
                    return match board_keyboard(&boards, None) {
                        Some(keyboard) => ctx.reply_with(&text, keyboard).await,
                        None => ctx.reply(&text).await,
                    };
                }
            },
        };
        let Some(board_id) = chosen else {
            return match board_keyboard(&boards, None) {
                Some(keyboard) => ctx.reply_with(NO_ACTIVE_BOARD, keyboard).await,
                None => ctx.reply("Es gibt noch kein Board.").await,
            };
        };
        show(ctx, &state, &boards, board_id).await
    }
}

/// Der Knopf, der das aktive Board umschaltet
#[derive(Debug)]
pub(crate) struct BoardButton(pub(crate) Deps);

#[async_trait]
impl CallbackHandler for BoardButton {
    async fn run(&self, ctx: &BotContext, action: &CallbackAction, query_id: &str) -> Result<bool> {
        let CallbackAction::Board(board_id) = action else {
            return Ok(false);
        };
        let board_id = *board_id;
        let state = self.0.state()?;
        let boards = match own_boards(&state).await {
            Ok(boards) => boards,
            Err(error) => {
                answer(ctx, query_id, None).await?;
                report(ctx, &error).await?;
                return Ok(true);
            }
        };
        // Ein Knopf aus einer alten Nachricht kann auf ein gelöschtes Board zeigen
        if !boards.iter().any(|board| board.id == board_id) {
            answer(ctx, query_id, Some(UNKNOWN_BOARD)).await?;
            return Ok(true);
        }
        if let Err(error) = activate(&state, board_id) {
            answer(ctx, query_id, None).await?;
            report(ctx, &error).await?;
            return Ok(true);
        }
        answer(ctx, query_id, Some(SWITCHED)).await?;
        show(ctx, &state, &boards, board_id).await?;
        Ok(true)
    }
}

/// Die Ansicht eines Boards samt Knöpfen in den Chat
async fn show(
    ctx: &BotContext,
    state: &Arc<AppState>,
    boards: &[BoardChoice],
    board_id: i32,
) -> Result<()> {
    let view = match state
        .board
        .run(|backend| async move { backend.services().views.get(board_id).await })
        .await
    {
        Ok(view) => view,
        Err(error) => return report(ctx, &error).await,
    };
    let name = boards
        .iter()
        .find(|board| board.id == board_id)
        .map(|board| board.name.as_str())
        .unwrap_or_default();
    let text = board_message(name, &view);
    match board_keyboard(boards, Some(board_id)) {
        Some(keyboard) => ctx.reply_with(&text, keyboard).await,
        None => ctx.reply(&text).await,
    }
}

/// Beantwortet den Knopfdruck; ohne Text dreht sich der Knopf sonst beim Nutzer weiter
async fn answer(ctx: &BotContext, query_id: &str, text: Option<&str>) -> Result<()> {
    ctx.api
        .answer_callback_query(query_id, text, false)
        .await
        .map_err(nexus_core::Error::unexpected)
}

/// Die eigenen Boards des Benutzers aus `board.yaml`, nach ID sortiert – damit die Knöpfe in
/// jeder Antwort an derselben Stelle stehen
pub(crate) async fn own_boards(state: &Arc<AppState>) -> Result<Vec<BoardChoice>> {
    let owner_id = BoardConfigStore::new(state.home.clone())
        .require()
        .map_err(crate::board_runtime::BoardRuntimeHolder::translate)?
        .user_id;
    let filter = BoardListFilter {
        owner_id: Some(owner_id),
        project_key: None,
    };
    let boards = state
        .board
        .run(|backend| async move { backend.services().boards.list(&filter).await })
        .await?;
    let mut choices: Vec<BoardChoice> = boards
        .into_iter()
        .map(|summary| BoardChoice {
            id: summary.board.id,
            name: summary.board.name,
        })
        .collect();
    choices.sort_by_key(|board| board.id);
    Ok(choices)
}

/// Das aktive Board aus `board.yaml`; eine unlesbare Datei gilt als „keins“
pub(crate) fn active_board(state: &Arc<AppState>) -> Option<i32> {
    BoardConfigStore::new(state.home.clone())
        .read()
        .ok()
        .flatten()
        .and_then(|config| config.active_board)
}

/// Schreibt das aktive Board in `board.yaml` – dieselbe Datei, die Oberfläche und CLI nutzen
fn activate(state: &Arc<AppState>, board_id: i32) -> Result<()> {
    let store = BoardConfigStore::new(state.home.clone());
    let config = store.require()?;
    store.write(&BoardConfig {
        active_board: Some(board_id),
        ..config
    })
}

/// Das Board zu einer Angabe: erst als ID, dann als Name ohne Rücksicht auf Groß- und
/// Kleinschreibung
fn find(boards: &[BoardChoice], wanted: &str) -> Option<i32> {
    if let Ok(id) = wanted.parse::<i32>() {
        if boards.iter().any(|board| board.id == id) {
            return Some(id);
        }
    }
    boards
        .iter()
        .find(|board| board.name.eq_ignore_ascii_case(wanted))
        .map(|board| board.id)
}

/// Eine Angabe des Nutzers, gekürzt – sie geht in die Antwort zurück
fn shortened(wanted: &str) -> String {
    nexus_core::text::utf16_prefix(wanted, 40).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zwei Boards zur Auswahl
    fn boards() -> Vec<BoardChoice> {
        vec![
            BoardChoice {
                id: 3,
                name: "Sprint".into(),
            },
            BoardChoice {
                id: 7,
                name: "Ideen".into(),
            },
        ]
    }

    #[test]
    fn findet_ein_board_an_id_und_name() {
        assert_eq!(find(&boards(), "7"), Some(7));
        assert_eq!(find(&boards(), "sprint"), Some(3));
        assert_eq!(find(&boards(), "Sprint"), Some(3));
    }

    #[test]
    fn findet_nichts_zu_einer_fremden_angabe() {
        assert_eq!(find(&boards(), "99"), None);
        assert_eq!(find(&boards(), "gibtesnicht"), None);
        assert_eq!(find(&boards(), ""), None);
    }
}
