//! `nexus board note …`: Notizen als Vorrat für spätere Tickets.
//! Portiert aus `packages/cli/src/commands/board/BoardNoteCommands.ts`.

use clap::{ArgAction, Args, Subcommand};
use nexus_core::board::basics::NoteState;
use nexus_core::board::note_service::{AddNoteOptions, NewNoteText, TakeNoteOptions};
use nexus_core::board::ticket_ids::TicketIds;
use nexus_core::{HarnessError, Result};

use crate::board::{BoardOptionParser, BoardSelection, BoardTextFormatter};
use crate::context::CliContext;

/// Beschreibung der Gruppe `note`
pub const NOTE_ABOUT: &str = "Notizen: Befunde über etwas anderes als das laufende Ticket.";

/// Beschreibung von `note add`
const ADD_ABOUT: &str = "Hält einen Befund fest, der nicht zum laufenden Ticket gehört. Gibt es ihn schon, wird abgelehnt.";

/// Zusatztext unter der Hilfe von `note add`
const ADD_AFTER: &str = "\nDer Titel ist ein Satz, der ohne Kontext verständlich ist: was ist kaputt oder fehlt – ohne Ticket-ID.\nDie Herkunft steht in --from; „unabhängig von <Ticket>“ gehört nicht in den Text.\n\nBeispiel:\n  harness board note add \"Cache der Board-Ansicht läuft nie ab\" --details \"BoardViewService.ts: ohne TTL; nach Ticketwechsel bleibt die alte Spalte stehen. Vorschlag: changeMarker prüfen.\" --from ARC-0009 --label cache --author claude";

/// Beschreibung von `note seen`
const SEEN_ABOUT: &str =
    "Derselbe Befund ist erneut aufgefallen: zählt die offene Notiz hoch statt einer neuen.";

/// Beispiel unter der Hilfe von `note seen`
const SEEN_EXAMPLE: &str = "\nBeispiel:\n  harness board note seen 12 --from ARC-0014";

/// Beschreibung von `note list`
const LIST_ABOUT: &str = "Listet Notizen; ohne --state nur offene.";

/// Beispiele unter der Hilfe von `note list`
const LIST_EXAMPLES: &str =
    "\nBeispiele:\n  harness board note list --md\n  harness board note list --state alle";

/// Beschreibung von `note show`
const SHOW_ABOUT: &str = "Zeigt eine Notiz vollständig.";

/// Beispiel unter der Hilfe von `note show`
const SHOW_EXAMPLE: &str = "\nBeispiel:\n  harness board note show 12";

/// Beschreibung von `note take`
const TAKE_ABOUT: &str = "Macht aus einer offenen Notiz ein Ticket (Titel der Notiz = Titel, Details = Beschreibung, Labels gehen mit).";

/// Beispiel unter der Hilfe von `note take`
const TAKE_EXAMPLE: &str = "\nBeispiel:\n  harness board note take 12 --priority high";

/// Beschreibung von `note drop`
const DROP_ABOUT: &str = "Verwirft eine Notiz mit Grund (wird nie gelöscht).";

/// Beispiel unter der Hilfe von `note drop`
const DROP_EXAMPLE: &str =
    "\nBeispiel:\n  harness board note drop 12 --reason \"erledigt sich mit ARC-0020\"";

/// Die Unterbefehle von `board note`
#[derive(Debug, Subcommand)]
pub enum NoteCommands {
    #[command(
        name = "add",
        about = ADD_ABOUT,
        after_help = ADD_AFTER,
        disable_help_flag = true
    )]
    Add(AddArgs),

    #[command(
        name = "seen",
        about = SEEN_ABOUT,
        after_help = SEEN_EXAMPLE,
        disable_help_flag = true
    )]
    Seen(SeenArgs),

    #[command(
        name = "list",
        about = LIST_ABOUT,
        after_help = LIST_EXAMPLES,
        disable_help_flag = true
    )]
    List(ListArgs),

    #[command(
        name = "show",
        about = SHOW_ABOUT,
        after_help = SHOW_EXAMPLE,
        disable_help_flag = true
    )]
    Show(ShowArgs),

    #[command(
        name = "take",
        about = TAKE_ABOUT,
        after_help = TAKE_EXAMPLE,
        disable_help_flag = true
    )]
    Take(TakeArgs),

    #[command(
        name = "drop",
        about = DROP_ABOUT,
        after_help = DROP_EXAMPLE,
        disable_help_flag = true
    )]
    Drop(DropArgs),
}

/// Optionen von `note add`
#[derive(Debug, Args)]
pub struct AddArgs {
    pub titel: String,
    #[arg(
        long,
        value_name = "text",
        help = "wo, woran man es sieht, vermutete Ursache, Vorschlag"
    )]
    pub details: Option<String>,
    #[arg(long, value_name = "id", help = "Ticket, bei dem es aufgefallen ist")]
    pub from: Option<String>,
    #[arg(long, value_name = "label", action = ArgAction::Append, help = "Label (wiederholbar)")]
    pub label: Vec<String>,
    #[arg(long, value_name = "name", help = "wer notiert (z. B. claude)")]
    pub author: Option<String>,
    #[arg(
        long,
        value_name = "id",
        help = "Board (Standard: Board des --from-Tickets, sonst Board dieses Repos, sonst aktives Board)"
    )]
    pub board: Option<String>,
    #[arg(long, help = "auch anlegen, wenn eine ähnliche Notiz existiert")]
    pub force: bool,
}

/// Optionen von `note seen`
#[derive(Debug, Args)]
pub struct SeenArgs {
    pub id: String,
    #[arg(
        long,
        value_name = "id",
        help = "Ticket, bei dem es erneut aufgefallen ist"
    )]
    pub from: Option<String>,
}

/// Optionen von `note list`
#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        value_name = "stand",
        help = "offen, uebernommen, verworfen oder alle"
    )]
    pub state: Option<String>,
    #[arg(long, value_name = "id", help = "nur Notizen aus diesem Ticket")]
    pub from: Option<String>,
    #[arg(long, help = "Ausgabe als Markdown")]
    pub md: bool,
    #[arg(
        long,
        value_name = "id",
        help = "Board (Standard: Board dieses Repos, sonst aktives Board)"
    )]
    pub board: Option<String>,
}

/// Optionen von `note show`
#[derive(Debug, Args)]
pub struct ShowArgs {
    pub id: String,
}

/// Optionen von `note take`
#[derive(Debug, Args)]
pub struct TakeArgs {
    pub id: String,
    #[arg(long, value_name = "text", help = "anderer Titel")]
    pub title: Option<String>,
    #[arg(long, value_name = "status", help = "Startspalte (Standard: Backlog)")]
    pub status: Option<String>,
    #[arg(long, value_name = "prio", help = "low, normal, high oder urgent")]
    pub priority: Option<String>,
    #[arg(long, value_name = "label", action = ArgAction::Append, help = "Labels ersetzen (wiederholbar)")]
    pub label: Vec<String>,
    #[arg(long, value_name = "name", help = "wer übernimmt")]
    pub by: Option<String>,
}

/// Optionen von `note drop`
#[derive(Debug, Args)]
pub struct DropArgs {
    pub id: String,
    #[arg(long, value_name = "text", help = "warum daraus nichts wird (Pflicht)")]
    pub reason: Option<String>,
}

/// Führt einen Unterbefehl von `board note` aus
pub fn run(context: &CliContext, command: &NoteCommands) -> Result<u8> {
    match command {
        NoteCommands::Add(args) => add(context, args),
        NoteCommands::Seen(args) => seen(context, args),
        NoteCommands::List(args) => list(context, args),
        NoteCommands::Show(args) => show(context, args),
        NoteCommands::Take(args) => take(context, args),
        NoteCommands::Drop(args) => drop_note(context, args),
    }
}

/// Führt `board note add` aus
fn add(context: &CliContext, args: &AddArgs) -> Result<u8> {
    context.with_board(async |rt| {
        // Ohne --board zählt das Board des Herkunftstickets
        let board_id = match (&args.board, &args.from) {
            (None, Some(from)) => rt.services.tickets.get(from).await?.view.ticket.board_id,
            _ => {
                BoardSelection
                    .one(
                        &rt.services,
                        &rt.config,
                        args.board.as_deref(),
                        Some(context.project_dir()),
                    )
                    .await?
            }
        };
        let created = rt
            .services
            .notes
            .add(
                board_id,
                &NewNoteText {
                    title: args.titel.clone(),
                    body: args.details.clone(),
                },
                &rt.actor(args.author.as_deref()),
                &AddNoteOptions {
                    source: args.from.clone(),
                    labels: Some(args.label.clone()),
                    force: args.force,
                },
            )
            .await?;
        context
            .printer()
            .result(&created, &[format!("Notiz {} gespeichert.", created.id)]);
        Ok(0)
    })
}

/// Führt `board note seen` aus
fn seen(context: &CliContext, args: &SeenArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let updated = rt
            .services
            .notes
            .seen(note_id(&args.id)?, args.from.as_deref())
            .await?;
        context.printer().result(
            &updated,
            &[format!(
                "Notiz {}: {}× aufgefallen.",
                updated.id, updated.seen_count
            )],
        );
        Ok(0)
    })
}

/// Führt `board note list` aus
fn list(context: &CliContext, args: &ListArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let states = match &args.state {
            Some(state) => Some(parse_states(state)?),
            None => None,
        };
        let board_id = BoardSelection
            .one(
                &rt.services,
                &rt.config,
                args.board.as_deref(),
                Some(context.project_dir()),
            )
            .await?;
        let source = match args.from.as_deref() {
            Some(id) => Some(TicketIds::canonical(id)?),
            None => None,
        };
        let notes = rt
            .services
            .notes
            .list(board_id, states.as_deref(), source.as_deref())
            .await?;
        let format = BoardTextFormatter::default();
        let lines = if args.md {
            format.notes_markdown(&notes)
        } else {
            format.notes_text(&notes)
        };
        context.printer().result(&notes, &lines);
        Ok(0)
    })
}

/// Führt `board note show` aus
fn show(context: &CliContext, args: &ShowArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let found = rt.services.notes.get(note_id(&args.id)?).await?;
        let lines = BoardTextFormatter::default().note_text(&found);
        context.printer().result(&found, &lines);
        Ok(0)
    })
}

/// Führt `board note take` aus
fn take(context: &CliContext, args: &TakeArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let result = rt
            .services
            .notes
            .take(
                note_id(&args.id)?,
                &rt.actor(args.by.as_deref()),
                &TakeNoteOptions {
                    title: args.title.clone(),
                    status: match &args.status {
                        Some(status) => Some(BoardOptionParser.status(status)?),
                        None => None,
                    },
                    priority: match &args.priority {
                        Some(priority) => Some(priority.parse()?),
                        None => None,
                    },
                    labels: if args.label.is_empty() {
                        None
                    } else {
                        Some(args.label.clone())
                    },
                },
            )
            .await?;
        context.printer().result(
            &result,
            &[format!(
                "Notiz {} → {} {}",
                result.note.id, result.ticket.ticket.id, result.ticket.ticket.title
            )],
        );
        Ok(0)
    })
}

/// Führt `board note drop` aus
fn drop_note(context: &CliContext, args: &DropArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let dropped = rt
            .services
            .notes
            .drop_note(note_id(&args.id)?, args.reason.as_deref().unwrap_or(""))
            .await?;
        context
            .printer()
            .result(&dropped, &[format!("Notiz {} verworfen.", dropped.id)]);
        Ok(0)
    })
}

/// Stände hinter `--state` (deutsche und englische Schreibweisen)
fn parse_states(value: &str) -> Result<Vec<NoteState>> {
    match value.to_lowercase().as_str() {
        "offen" | "open" => Ok(vec![NoteState::Open]),
        "uebernommen" | "übernommen" | "taken" => Ok(vec![NoteState::Taken]),
        "verworfen" | "dropped" => Ok(vec![NoteState::Dropped]),
        "alle" | "all" => Ok(NoteState::ALL.to_vec()),
        _ => Err(HarnessError::new(
            "state-invalid",
            format!("Stand „{value}“ ist ungültig. Erlaubt: offen, uebernommen, verworfen, alle."),
        )
        .into()),
    }
}

/// Notiz-Nummer aus Text
fn note_id(raw: &str) -> Result<i32> {
    match raw.parse::<i32>() {
        Ok(id) if id >= 1 => Ok(id),
        _ => Err(HarnessError::new(
            "note-id-invalid",
            format!("„{raw}“ ist keine Notiz-Nummer."),
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_notiz_nummer_muss_eine_ganze_zahl_ab_eins_sein() {
        assert_eq!(note_id("12").unwrap(), 12);
        for raw in ["0", "-1", "null", ""] {
            let error = note_id(raw).unwrap_err();
            assert_eq!(error.code(), Some("note-id-invalid"), "{raw}");
            assert_eq!(
                error.to_string(),
                format!("„{raw}“ ist keine Notiz-Nummer.")
            );
        }
    }

    #[test]
    fn die_stande_kennen_deutsche_und_englische_schreibweisen() {
        assert_eq!(parse_states("Offen").unwrap(), [NoteState::Open]);
        assert_eq!(parse_states("übernommen").unwrap(), [NoteState::Taken]);
        assert_eq!(parse_states("uebernommen").unwrap(), [NoteState::Taken]);
        assert_eq!(parse_states("verworfen").unwrap(), [NoteState::Dropped]);
        assert_eq!(parse_states("alle").unwrap(), NoteState::ALL);
    }

    #[test]
    fn ein_unbekannter_stand_wird_mit_state_invalid_abgelehnt() {
        let error = parse_states("quatsch").unwrap_err();
        assert_eq!(error.code(), Some("state-invalid"));
        assert_eq!(
            error.to_string(),
            "Stand „quatsch“ ist ungültig. Erlaubt: offen, uebernommen, verworfen, alle."
        );
    }
}
