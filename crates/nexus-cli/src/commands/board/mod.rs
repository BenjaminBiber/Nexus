//! Die Unterbefehle von `board`: das Board im Browser öffnen und die Board-Datenbank bedienen.

pub mod boards;
pub mod claim;
pub mod comment;
pub mod inbox;
pub mod note;
pub mod open;
pub mod planning;
pub mod project;
pub mod step;
pub mod tickets_read;
pub mod tickets_write;

use clap::Subcommand;
use nexus_core::{HarnessError, Result};

use crate::context::CliContext;

/// Die Unterbefehle von `board`, die die Board-Datenbank bedienen. Die übrigen (`start`, `done`,
/// `question`, `dispatch` …) kommen in den weiteren Tickets der Phase 3 dazu.
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

    #[command(
        name = "project",
        about = project::PROJECT_ABOUT,
        subcommand_required = true,
        arg_required_else_help = false,
        disable_help_flag = true,
        disable_help_subcommand = true
    )]
    Project {
        #[command(subcommand)]
        sub: project::ProjectCommands,
    },

    #[command(
        name = "boards",
        about = boards::BOARDS_ABOUT,
        subcommand_required = true,
        disable_help_flag = true,
        disable_help_subcommand = true
    )]
    Boards {
        #[command(subcommand)]
        sub: boards::BoardsCommands,
    },

    #[command(
        name = "step",
        about = step::STEP_ABOUT,
        after_help = step::STEP_EXAMPLES,
        disable_help_flag = true,
        disable_help_subcommand = true
    )]
    Step(step::StepArgs),

    #[command(
        name = "note",
        about = note::NOTE_ABOUT,
        subcommand_required = true,
        disable_help_flag = true,
        disable_help_subcommand = true
    )]
    Note {
        #[command(subcommand)]
        sub: note::NoteCommands,
    },

    #[command(
        name = "inbox",
        about = inbox::INBOX_ABOUT,
        after_help = inbox::INBOX_EXAMPLES,
        disable_help_flag = true
    )]
    Inbox(inbox::InboxArgs),

    #[command(
        name = "plan",
        about = planning::PLAN_ABOUT,
        after_help = planning::PLAN_EXAMPLE,
        disable_help_flag = true
    )]
    Plan(planning::PlanArgs),

    #[command(
        name = "plan-json",
        about = planning::PLAN_JSON_ABOUT,
        after_help = planning::PLAN_JSON_EXAMPLE,
        disable_help_flag = true
    )]
    PlanJson(planning::PlanJsonArgs),

    #[command(
        name = "next-planning",
        about = planning::NEXT_ABOUT,
        after_help = planning::NEXT_EXAMPLE,
        disable_help_flag = true
    )]
    NextPlanning(planning::NextPlanningArgs),

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
        BoardCommands::Project { sub } => project::run(context, sub),
        BoardCommands::Boards { sub } => boards::run(context, sub),
        BoardCommands::Step(args) => step::run(context, args),
        BoardCommands::Note { sub } => note::run(context, sub),
        BoardCommands::Inbox(args) => inbox::run(context, args),
        BoardCommands::Plan(args) => planning::plan(context, args),
        BoardCommands::PlanJson(args) => planning::plan_json(context, args),
        BoardCommands::NextPlanning(args) => planning::next_planning(context, args),
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
