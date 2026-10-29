//! `nexus board project list|create|rename`: Projekte verwalten.
//! Portiert aus `packages/cli/src/commands/board/BoardProjectCommands.ts`.

use clap::{Args, Subcommand};
use nexus_core::Result;

use crate::context::CliContext;

/// Beschreibung der Gruppe `project`
pub const PROJECT_ABOUT: &str = "Projekte des Boards verwalten (Projekt → Boards → Tickets).";

/// Beschreibung von `project list`
const LIST_ABOUT: &str = "Listet alle Projekte mit Key und Namen.";

/// Beispiel unter der Hilfe von `project list`
const LIST_EXAMPLE: &str = "\nBeispiel:\n  harness board project list";

/// Beschreibung von `project create`
const CREATE_ABOUT: &str = "Legt ein Projekt an. Der Key (2–6 Zeichen) wird Präfix der Ticket-IDs.";

/// Beispiel unter der Hilfe von `project create`
const CREATE_EXAMPLE: &str = "\nBeispiel:\n  harness board project create ARC \"Arches\"";

/// Beschreibung von `project rename`
const RENAME_ABOUT: &str = "Benennt ein Projekt um (der Key bleibt).";

/// Beispiel unter der Hilfe von `project rename`
const RENAME_EXAMPLE: &str = "\nBeispiel:\n  harness board project rename ARC \"Arches Shop\"";

/// Die Unterbefehle von `board project`
#[derive(Debug, Subcommand)]
pub enum ProjectCommands {
    #[command(
        name = "list",
        about = LIST_ABOUT,
        after_help = LIST_EXAMPLE,
        disable_help_flag = true
    )]
    List,

    #[command(
        name = "create",
        about = CREATE_ABOUT,
        after_help = CREATE_EXAMPLE,
        disable_help_flag = true
    )]
    Create(KeyNameArgs),

    #[command(
        name = "rename",
        about = RENAME_ABOUT,
        after_help = RENAME_EXAMPLE,
        disable_help_flag = true
    )]
    Rename(KeyNameArgs),

    /// Den `help`-Befehl legt commander in jeder Gruppe selbst an; hier steht er im Modell
    #[command(
        name = "help",
        about = "Hilfe zu einem Befehl anzeigen",
        disable_help_flag = true
    )]
    Help { befehl: Option<String> },
}

/// Key und Name eines Projekts
#[derive(Debug, Args)]
pub struct KeyNameArgs {
    pub key: String,
    pub name: String,
}

/// Führt einen Unterbefehl von `board project` aus
pub fn run(context: &CliContext, command: &ProjectCommands) -> Result<u8> {
    match command {
        ProjectCommands::List => list(context),
        ProjectCommands::Create(args) => create(context, args),
        ProjectCommands::Rename(args) => rename(context, args),
        ProjectCommands::Help { befehl } => {
            crate::cli::group_help(&["board", "project"], befehl.as_deref())
        }
    }
}

/// Führt `board project list` aus
fn list(context: &CliContext) -> Result<u8> {
    context.with_board(async |rt| {
        let projects = rt.services.projects.list().await?;
        let lines: Vec<String> = if projects.is_empty() {
            vec!["Keine Projekte.".to_string()]
        } else {
            projects
                .iter()
                .map(|p| format!("{}  {}", p.key, p.name))
                .collect()
        };
        context.printer().result(&projects, &lines);
        Ok(0)
    })
}

/// Führt `board project create` aus
fn create(context: &CliContext, args: &KeyNameArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let created = rt
            .services
            .projects
            .create(&args.key, &args.name, &rt.actor(None))
            .await?;
        context.printer().result(
            &created,
            &[format!(
                "Projekt {} angelegt: {}",
                created.key, created.name
            )],
        );
        Ok(0)
    })
}

/// Führt `board project rename` aus
fn rename(context: &CliContext, args: &KeyNameArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let renamed = rt.services.projects.rename(&args.key, &args.name).await?;
        context.printer().result(
            &renamed,
            &[format!(
                "Projekt {} heißt jetzt {}",
                renamed.key, renamed.name
            )],
        );
        Ok(0)
    })
}
