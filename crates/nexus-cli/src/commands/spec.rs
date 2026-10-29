//! `nexus spec comments` und `nexus spec resolve`: offene Kommentare zu Spec- und
//! Plan-Dokumenten, Glossar und ADRs lesen und erledigen.
//! Portiert aus `packages/cli/src/commands/SpecCommentsCommand.ts` und `SpecResolveCommand.ts`.

use clap::{Args, Subcommand};
use nexus_core::docs::{DocCommentStore, OpenDocComment};
use nexus_core::Result;

use crate::context::CliContext;

/// Beschreibung der Gruppe `spec`
pub const SPEC_ABOUT: &str = "Kommentare zu Spec- und Plan-Dokumenten abarbeiten";

/// Beschreibung von `spec comments`
pub const COMMENTS_ABOUT: &str = "Listet offene Kommentare zu Spec- und Plan-Dokumenten, Glossar und ADRs (optional eines Dokuments oder Themas).";
/// Beispiele unter der Hilfe von `spec comments`
pub const COMMENTS_EXAMPLES: &str = "\nBeispiele:\n  harness spec comments\n  harness spec comments spec-docs-rework\n  harness spec comments docs/superpowers/specs/2026-10-22-spec-docs-rework-design.md";

/// Beschreibung von `spec resolve`
pub const RESOLVE_ABOUT: &str =
    "Markiert einen Kommentar als erledigt und speichert eine kurze Antwort.";
/// Beispiel unter der Hilfe von `spec resolve`
pub const RESOLVE_EXAMPLE: &str =
    "\nBeispiel:\n  harness spec resolve K-1a2b3c4d \"Ziel in Abschnitt 2 präzisiert\"";

/// Das Dokument oder Thema, auf das `spec comments` begrenzt wird
#[derive(Debug, Args)]
pub struct CommentsArgs {
    pub dokument: Option<String>,
}

/// Kommentar und Antwort von `spec resolve`
#[derive(Debug, Args)]
pub struct ResolveArgs {
    pub kommentar: String,
    pub antwort: String,
}

/// Die Unterbefehle von `spec`
#[derive(Debug, Subcommand)]
pub enum SpecCommands {
    #[command(
        name = "comments",
        about = COMMENTS_ABOUT,
        after_help = COMMENTS_EXAMPLES,
        disable_help_flag = true
    )]
    Comments(CommentsArgs),

    #[command(
        name = "resolve",
        about = RESOLVE_ABOUT,
        after_help = RESOLVE_EXAMPLE,
        disable_help_flag = true
    )]
    Resolve(ResolveArgs),

    #[command(
        name = "help",
        about = "Hilfe zu einem Befehl anzeigen",
        disable_help_flag = true
    )]
    Help { befehl: Option<String> },
}

/// Führt einen Unterbefehl von `spec` aus
pub fn run(context: &CliContext, command: &SpecCommands) -> Result<u8> {
    match command {
        SpecCommands::Comments(args) => comments(context, args.dokument.as_deref()),
        SpecCommands::Resolve(args) => resolve(context, &args.kommentar, &args.antwort),
        SpecCommands::Help { befehl } => crate::cli::group_help(&["spec"], befehl.as_deref()),
    }
}

/// Liest die offenen Kommentare und gibt sie als Text oder JSON aus
fn comments(context: &CliContext, dokument: Option<&str>) -> Result<u8> {
    let project = context.open_project()?;
    let open = DocCommentStore::new(&project.root).list_open(dokument)?;
    let lines: Vec<String> = if open.is_empty() {
        vec!["Keine offenen Kommentare.".to_string()]
    } else {
        open.iter().flat_map(format).collect()
    };
    context.printer().result(&open, &lines);
    Ok(0)
}

/// Setzt den Status und die Antwort
fn resolve(context: &CliContext, id: &str, antwort: &str) -> Result<u8> {
    let project = context.open_project()?;
    let done = DocCommentStore::new(&project.root).resolve(id, antwort)?;
    context
        .printer()
        .result(&done, &[format!("Kommentar {id} erledigt.")]);
    Ok(0)
}

/// Ein Kommentar als lesbarer Block
fn format(open: &OpenDocComment) -> Vec<String> {
    let section = match &open.comment.heading {
        Some(heading) => format!(" · Abschnitt „{heading}“"),
        None => String::new(),
    };
    let line = match open.comment.lines {
        Some(lines) => format!(" · Zeile {}", lines[0]),
        None => String::new(),
    };
    vec![
        format!("{} · {}{section}{line}", open.comment.id, open.path),
        format!("  Stelle: „{}“", open.comment.quote),
        format!("  Kommentar: {}", open.comment.text),
    ]
}
