//! Die Unterbefehle von `board`: das Board im Browser öffnen und die Board-Datenbank bedienen.

pub mod claim;
pub mod comment;
pub mod open;
pub mod tickets_read;
pub mod tickets_write;

use clap::Subcommand;
use nexus_core::{HarnessError, Result};

use crate::context::CliContext;

/// Die Unterbefehle von `board`, die die Board-Datenbank bedienen. Die übrigen (`project`,
/// `boards`, `start`, `done`, `step`, `note`, `question`, `dispatch` …) kommen in den weiteren
/// Tickets der Phase 3 dazu.
// Die Varianten tragen die Optionen von clap; sie dürfen unterschiedlich groß sein
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum BoardCommands {
    #[command(
        name = "list",
        about = tickets_read::LIST_ABOUT,
        after_help = tickets_read::LIST_EXAMPLES,
        disable_help_flag = true
    )]
    List(tickets_read::ListArgs),

    #[command(
        name = "show",
        about = tickets_read::SHOW_ABOUT,
        after_help = tickets_read::SHOW_EXAMPLES,
        disable_help_flag = true
    )]
    Show(tickets_read::ShowArgs),

    #[command(
        name = "create",
        about = tickets_write::CREATE_ABOUT,
        after_help = tickets_write::CREATE_EXAMPLES,
        disable_help_flag = true
    )]
    Create(tickets_write::CreateArgs),

    #[command(
        name = "set",
        about = tickets_write::SET_ABOUT,
        after_help = tickets_write::SET_EXAMPLES,
        disable_help_flag = true
    )]
    Set(tickets_write::SetArgs),

    #[command(
        name = "move",
        about = tickets_write::MOVE_ABOUT,
        after_help = tickets_write::MOVE_EXAMPLES,
        disable_help_flag = true
    )]
    Move(tickets_write::MoveArgs),

    #[command(
        name = "delete",
        about = tickets_write::DELETE_ABOUT,
        after_help = tickets_write::DELETE_EXAMPLE,
        disable_help_flag = true
    )]
    Delete(tickets_write::DeleteArgs),

    #[command(
        name = "claim",
        about = claim::CLAIM_ABOUT,
        after_help = claim::CLAIM_EXAMPLE,
        disable_help_flag = true
    )]
    Claim(claim::ClaimArgs),

    #[command(
        name = "release",
        about = claim::RELEASE_ABOUT,
        after_help = claim::RELEASE_EXAMPLE,
        disable_help_flag = true
    )]
    Release(claim::ReleaseArgs),

    #[command(
        name = "comment",
        about = comment::COMMENT_ABOUT,
        after_help = comment::COMMENT_EXAMPLE,
        disable_help_flag = true
    )]
    Comment(comment::CommentArgs),

    /// Alles, was kein angemeldeter Unterbefehl ist; wird mit `command-unknown` abgewiesen
    #[command(external_subcommand)]
    Unbekannt(Vec<String>),
}

/// Führt einen Unterbefehl von `board` aus
pub fn run(context: &CliContext, command: &BoardCommands) -> Result<u8> {
    match command {
        BoardCommands::List(args) => tickets_read::list(context, args),
        BoardCommands::Show(args) => tickets_read::show(context, args),
        BoardCommands::Create(args) => tickets_write::create(context, args),
        BoardCommands::Set(args) => tickets_write::set(context, args),
        BoardCommands::Move(args) => tickets_write::move_to(context, args),
        BoardCommands::Delete(args) => tickets_write::delete(context, args),
        BoardCommands::Claim(args) => claim::claim(context, args),
        BoardCommands::Release(args) => claim::release(context, args),
        BoardCommands::Comment(args) => comment::comment(context, args),
        BoardCommands::Unbekannt(words) => Err(unknown(words)),
    }
}

/// Ein unbekannter Unterbefehl wird ohne Datenbankzugriff abgewiesen
pub fn unknown(words: &[String]) -> nexus_core::Error {
    HarnessError::new(
        "command-unknown",
        format!(
            "Unbekannter Befehl „board {}“. Hilfe: harness board --help",
            words.join(" ")
        ),
    )
    .into()
}
