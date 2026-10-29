//! `nexus board boards list|create|rename|use|repo|delete`: Boards verwalten.
//! Portiert aus `packages/cli/src/commands/board/BoardBoardsCommands.ts`.

use clap::{Args, Subcommand};
use nexus_core::board::board_service::BoardListFilter;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::{Error, HarnessError, Result};

use crate::board::{BoardSelection, BoardTextFormatter};
use crate::context::CliContext;

/// Beschreibung der Gruppe `boards`
pub const BOARDS_ABOUT: &str = "Boards anlegen, auflisten, umschalten und löschen.";

/// Beschreibung von `boards list`
const LIST_ABOUT: &str =
    "Listet Boards mit Projekt, Besitzer, offenen Tickets und deinem Repo-Pfad; * = aktives Board.";

/// Beispiele unter der Hilfe von `boards list`
const LIST_EXAMPLES: &str =
    "\nBeispiele:\n  harness board boards list\n  harness board boards list --project ARC --mine";

/// Beschreibung von `boards create`
const CREATE_ABOUT: &str = "Legt ein eigenes Board im Projekt an.";

/// Beispiel unter der Hilfe von `boards create`
const CREATE_EXAMPLE: &str = "\nBeispiel:\n  harness board boards create ARC \"Sprint 12\" --use";

/// Beschreibung von `boards rename`
const RENAME_ABOUT: &str = "Benennt ein Board um.";

/// Beispiel unter der Hilfe von `boards rename`
const RENAME_EXAMPLE: &str = "\nBeispiel:\n  harness board boards rename 7 \"Sprint 13\"";

/// Beschreibung von `boards use`
const USE_ABOUT: &str = "Setzt das aktive Board dieses Rechners (Standard für alle Ticketbefehle).";

/// Beispiel unter der Hilfe von `boards use`
const USE_EXAMPLE: &str = "\nBeispiel:\n  harness board boards use 7";

/// Beschreibung von `boards repo`
const REPO_ABOUT: &str = "Setzt deinen Repo-Pfad des Boards – nur für dich auf diesem Rechner (~/.harness/board-repos.yaml), nicht in der gemeinsamen DB; ohne Pfad zeigt er ihn.";

/// Beispiele unter der Hilfe von `boards repo`
const REPO_EXAMPLES: &str = "\nBeispiele:\n  harness board boards repo 7 C:\\Projects\\Arches\\Shopsystem\n  harness board boards repo 7\n  harness board boards repo 7 --clear";

/// Beschreibung von `boards delete`
const DELETE_ABOUT: &str =
    "Löscht ein Board. Mit offenen Tickets nur mit --delete-open (löscht dann alles).";

/// Beispiele unter der Hilfe von `boards delete`
const DELETE_EXAMPLES: &str =
    "\nBeispiele:\n  harness board boards delete 7\n  harness board boards delete 7 --delete-open";

/// Die Unterbefehle von `board boards`
#[derive(Debug, Subcommand)]
pub enum BoardsCommands {
    #[command(
        name = "list",
        about = LIST_ABOUT,
        after_help = LIST_EXAMPLES,
        disable_help_flag = true
    )]
    List(ListArgs),

    #[command(
        name = "create",
        about = CREATE_ABOUT,
        after_help = CREATE_EXAMPLE,
        disable_help_flag = true
    )]
    Create(CreateArgs),

    #[command(
        name = "rename",
        about = RENAME_ABOUT,
        after_help = RENAME_EXAMPLE,
        disable_help_flag = true
    )]
    Rename(RenameArgs),

    #[command(
        name = "use",
        about = USE_ABOUT,
        after_help = USE_EXAMPLE,
        disable_help_flag = true
    )]
    Use(UseArgs),

    #[command(
        name = "repo",
        about = REPO_ABOUT,
        after_help = REPO_EXAMPLES,
        disable_help_flag = true
    )]
    Repo(RepoArgs),

    #[command(
        name = "delete",
        about = DELETE_ABOUT,
        after_help = DELETE_EXAMPLES,
        disable_help_flag = true
    )]
    Delete(DeleteArgs),

    /// Den `help`-Befehl legt commander in jeder Gruppe selbst an; hier steht er im Modell
    #[command(
        name = "help",
        about = "Hilfe zu einem Befehl anzeigen",
        disable_help_flag = true
    )]
    Help { befehl: Option<String> },
}

/// Optionen von `boards list`
#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, value_name = "key", help = "nur Boards dieses Projekts")]
    pub project: Option<String>,
    #[arg(long, help = "nur eigene Boards")]
    pub mine: bool,
}

/// Optionen von `boards create`
#[derive(Debug, Args)]
pub struct CreateArgs {
    pub projekt: String,
    pub name: String,
    #[arg(long = "use", help = "danach als aktives Board setzen")]
    pub use_it: bool,
}

/// Optionen von `boards rename`
#[derive(Debug, Args)]
pub struct RenameArgs {
    pub id: String,
    pub name: String,
}

/// Optionen von `boards use`
#[derive(Debug, Args)]
pub struct UseArgs {
    pub id: String,
}

/// Optionen von `boards repo`
#[derive(Debug, Args)]
pub struct RepoArgs {
    pub id: String,
    pub pfad: Option<String>,
    #[arg(long, help = "eigenen Repo-Pfad entfernen")]
    pub clear: bool,
}

/// Optionen von `boards delete`
#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub id: String,
    #[arg(long = "delete-open", help = "auch offene Tickets endgültig löschen")]
    pub delete_open: bool,
    #[arg(long, value_name = "name", help = "Name im Löschprotokoll")]
    pub by: Option<String>,
}

/// Führt einen Unterbefehl von `board boards` aus
pub fn run(context: &CliContext, command: &BoardsCommands) -> Result<u8> {
    match command {
        BoardsCommands::List(args) => list(context, args),
        BoardsCommands::Create(args) => create(context, args),
        BoardsCommands::Rename(args) => rename(context, args),
        BoardsCommands::Use(args) => use_board(context, args),
        BoardsCommands::Repo(args) => repo(context, args),
        BoardsCommands::Delete(args) => delete(context, args),
        BoardsCommands::Help { befehl } => {
            crate::cli::group_help(&["board", "boards"], befehl.as_deref())
        }
    }
}

/// Führt `board boards list` aus
fn list(context: &CliContext, args: &ListArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let list = rt
            .services
            .boards
            .list(&BoardListFilter {
                project_key: args.project.clone(),
                owner_id: args.mine.then(|| rt.config.user_id.clone()),
            })
            .await?;
        let lines = BoardTextFormatter::default().boards_text(&list, rt.config.active_board);
        context.printer().result(&list, &lines);
        Ok(0)
    })
}

/// Führt `board boards create` aus
fn create(context: &CliContext, args: &CreateArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let created = rt
            .services
            .boards
            .create(&args.projekt, &args.name, &rt.actor(None))
            .await?;
        if args.use_it {
            activate(context, Some(created.board.id))?;
        }
        context.printer().result(
            &created,
            &[format!(
                "Board {} angelegt: {}{}",
                created.board.id,
                created.board.name,
                if args.use_it { " (aktiv)" } else { "" }
            )],
        );
        Ok(0)
    })
}

/// Führt `board boards rename` aus
fn rename(context: &CliContext, args: &RenameArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let renamed = rt
            .services
            .boards
            .rename(BoardSelection.parse_id(&args.id)?, &args.name)
            .await?;
        context.printer().result(
            &renamed,
            &[format!(
                "Board {} heißt jetzt {}",
                renamed.board.id, renamed.board.name
            )],
        );
        Ok(0)
    })
}

/// Führt `board boards use` aus
fn use_board(context: &CliContext, args: &UseArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let board_id = BoardSelection.parse_id(&args.id)?;
        let found = rt.services.boards.require(board_id).await?;
        activate(context, Some(found.board.id))?;
        // Wie in TypeScript kommt hier das rohe Board aus der Datenbank, ohne Repo-Herkunft
        context.printer().result(
            &found.board,
            &[format!(
                "Aktives Board: {} {}",
                found.board.id, found.board.name
            )],
        );
        Ok(0)
    })
}

/// Führt `board boards repo` aus
fn repo(context: &CliContext, args: &RepoArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let board_id = BoardSelection.parse_id(&args.id)?;
        if args.pfad.is_some() && args.clear {
            return Err(HarnessError::new(
                "usage",
                "Entweder einen Pfad angeben oder --clear, nicht beides.",
            )
            .into());
        }
        // Der Wert der Ausgabe unterscheidet sich wie in TypeScript: gesetzt wird ein Board mit
        // Repo-Herkunft, gezeigt wird der Eintrag der Liste (mit Projekt und offenen Tickets)
        let (value, path, source) = if args.pfad.is_some() || args.clear {
            let board = rt
                .services
                .boards
                .set_repo_path(
                    board_id,
                    if args.clear {
                        None
                    } else {
                        args.pfad.as_deref()
                    },
                )
                .await?;
            let text = (board.board.repo_path.clone(), board.repo_source);
            (
                serde_json::to_value(&board).map_err(Error::unexpected)?,
                text.0,
                text.1,
            )
        } else {
            match rt
                .services
                .boards
                .list(&BoardListFilter::default())
                .await?
                .into_iter()
                .find(|summary| summary.board.id == board_id)
            {
                Some(summary) => {
                    let text = (summary.board.repo_path.clone(), summary.repo_source);
                    (
                        serde_json::to_value(&summary).map_err(Error::unexpected)?,
                        text.0,
                        text.1,
                    )
                }
                None => {
                    let board = rt.services.boards.require(board_id).await?;
                    let text = (board.board.repo_path.clone(), board.repo_source);
                    (
                        serde_json::to_value(&board).map_err(Error::unexpected)?,
                        text.0,
                        text.1,
                    )
                }
            }
        };
        let text = BoardTextFormatter::default().repo_path_text(path.as_deref(), source);
        context.printer().result(
            &value,
            &[format!("Mein Repo-Pfad von Board {board_id}: {text}")],
        );
        Ok(0)
    })
}

/// Führt `board boards delete` aus
fn delete(context: &CliContext, args: &DeleteArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let board_id = BoardSelection.parse_id(&args.id)?;
        let result = rt
            .services
            .boards
            .delete(board_id, args.delete_open, &rt.actor(args.by.as_deref()))
            .await?;
        if rt.config.active_board == Some(board_id) {
            activate(context, None)?;
        }
        context.printer().result(
            &result,
            &[format!(
                "Board {board_id} gelöscht ({} Tickets).",
                result.deleted_tickets.len()
            )],
        );
        Ok(0)
    })
}

/// Schreibt das aktive Board in die Konfig
fn activate(context: &CliContext, id: Option<i32>) -> Result<()> {
    let store = BoardConfigStore::new(context.home());
    let config = store.require()?;
    store.write(&nexus_core::board::config::BoardConfig {
        active_board: id,
        ..config
    })
}
