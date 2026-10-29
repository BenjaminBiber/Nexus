//! `nexus board step …`: Arbeitsschritte eines Tickets abhaken, anlegen, bearbeiten, löschen und
//! verschieben. Portiert aus `packages/cli/src/commands/board/BoardStepCommands.ts`.

use clap::{Args, Subcommand};
use nexus_core::board::ticket_service::{AddStepOptions, TicketDetail};
use nexus_core::board::ticket_step_edits::{StepChange, StepGuard};
use nexus_core::text::TextSource;
use nexus_core::{HarnessError, Result};

use crate::board::BoardOptionParser;
use crate::context::CliContext;

/// Hilfe der Option `--rev`
const REV_HELP: &str = "nur ändern, wenn das Ticket noch diese Version hat (sonst Konflikt)";

/// Hilfe der Option `--expect`
const EXPECT_HELP: &str =
    "nur ändern, wenn der Schritt an dieser Nummer noch diesen Text hat (sonst Konflikt)";

/// Hilfe der Option `--expect` von `step move`
const MOVE_EXPECT_HELP: &str =
    "nur verschieben, wenn der Schritt an Nummer <von> noch diesen Text hat (sonst Konflikt)";

/// Beschreibung von `step`
pub const STEP_ABOUT: &str = "Hakt einen Schritt des Tickets ab (Nummer ab 1, siehe „nexus board show“). Unterbefehle legen Schritte an, bearbeiten, löschen und verschieben sie.";

/// Beispiele unter der Hilfe von `step`
pub const STEP_EXAMPLES: &str = "\nBeispiele:\n  nexus board step ARC-0007 2 --by claude\n  nexus board step ARC-0007 2 --undo\n  nexus board step add ARC-0007 \"Test schreiben\" --detail \"Erwartet: rot\"\n  nexus board step done ARC-0007 2";

/// Beschreibung von `step done`
const DONE_ABOUT: &str = "Hakt einen Schritt ab (gleich wie „nexus board step <id> <nummer>“).";

/// Beispiele unter der Hilfe von `step done`
const DONE_EXAMPLES: &str = "\nBeispiele:\n  nexus board step done ARC-0007 2 --by claude\n  nexus board step done ARC-0007 2 --undo\n  nexus board step done ARC-0007 2 --expect \"Test schreiben\"";

/// Beschreibung von `step add`
const ADD_ABOUT: &str =
    "Hängt einen Schritt an (oder fügt ihn mit --at an dieser Nummer ein); optional mit Anleitung.";

/// Beispiele unter der Hilfe von `step add`
const ADD_EXAMPLES: &str = "\nBeispiele:\n  nexus board step add ARC-0007 \"Test schreiben\" --detail \"npm test – erwartet: rot\" --by claude\n  nexus board step add ARC-0007 \"Migration prüfen\" --at 2";

/// Beschreibung von `step edit`
const EDIT_ABOUT: &str = "Ändert Text und/oder Anleitung eines Schritts; der Haken bleibt. Leere Anleitung („“) entfernt sie.";

/// Beispiele unter der Hilfe von `step edit`
const EDIT_EXAMPLES: &str = "\nBeispiele:\n  nexus board step edit ARC-0007 2 --text \"Test für Login schreiben\"\n  nexus board step edit ARC-0007 2 --detail \"\"\n  nexus board step edit ARC-0007 2 --text \"Neu\" --expect \"Alt\" --rev 5";

/// Beschreibung von `step rm`
const RM_ABOUT: &str = "Löscht einen Schritt; die folgenden rücken eine Nummer nach vorn.";

/// Beispiele unter der Hilfe von `step rm`
const RM_EXAMPLES: &str = "\nBeispiele:\n  nexus board step rm ARC-0007 3\n  nexus board step rm ARC-0007 3 --expect \"Alter Schritt\"";

/// Beschreibung von `step move`
const MOVE_ABOUT: &str =
    "Verschiebt einen Schritt auf eine andere Nummer (samt Haken und Anleitung).";

/// Beispiele unter der Hilfe von `step move`
const MOVE_EXAMPLES: &str = "\nBeispiele:\n  nexus board step move ARC-0007 4 1\n  nexus board step move ARC-0007 4 1 --expect \"Committen\"";

/// `board step <id> <nummer>` samt seiner Unterbefehle
#[derive(Debug, Args)]
pub struct StepArgs {
    // Pflicht in der Hilfe (`<id> <nummer>`), beim Parsen freiwillig: an ihrer Stelle darf ein
    // Unterbefehl stehen. Siehe `cli::ohne_pflicht_neben_unterbefehlen`.
    pub id: Option<String>,
    pub nummer: Option<String>,
    #[arg(long, help = "nimmt den Haken zurück")]
    pub undo: bool,
    #[arg(long, value_name = "name", help = "wer abhakt")]
    pub by: Option<String>,
    #[arg(long, value_name = "n", help = REV_HELP)]
    pub rev: Option<String>,
    #[arg(long, value_name = "text", help = EXPECT_HELP)]
    pub expect: Option<String>,
    #[command(subcommand)]
    pub sub: Option<StepCommands>,
}

/// Die Unterbefehle von `board step`
#[derive(Debug, Subcommand)]
pub enum StepCommands {
    #[command(
        name = "done",
        about = DONE_ABOUT,
        after_help = DONE_EXAMPLES,
        disable_help_flag = true
    )]
    Done(DoneArgs),

    #[command(
        name = "add",
        about = ADD_ABOUT,
        after_help = ADD_EXAMPLES,
        disable_help_flag = true
    )]
    Add(AddArgs),

    #[command(
        name = "edit",
        about = EDIT_ABOUT,
        after_help = EDIT_EXAMPLES,
        disable_help_flag = true
    )]
    Edit(EditArgs),

    #[command(
        name = "rm",
        about = RM_ABOUT,
        after_help = RM_EXAMPLES,
        disable_help_flag = true
    )]
    Rm(RmArgs),

    #[command(
        name = "move",
        about = MOVE_ABOUT,
        after_help = MOVE_EXAMPLES,
        disable_help_flag = true
    )]
    Move(MoveArgs),
}

/// Optionen von `step done`
#[derive(Debug, Args)]
pub struct DoneArgs {
    pub id: String,
    pub nummer: String,
    #[arg(long, help = "nimmt den Haken zurück")]
    pub undo: bool,
    #[arg(long, value_name = "name", help = "wer abhakt")]
    pub by: Option<String>,
    #[arg(long, value_name = "n", help = REV_HELP)]
    pub rev: Option<String>,
    #[arg(long, value_name = "text", help = EXPECT_HELP)]
    pub expect: Option<String>,
}

/// Optionen von `step add`
#[derive(Debug, Args)]
pub struct AddArgs {
    pub id: String,
    pub text: String,
    #[arg(
        long,
        value_name = "text",
        help = "Anleitung zum Schritt (Markdown: Befehl, erwartetes Ergebnis)"
    )]
    pub detail: Option<String>,
    #[arg(
        long = "detail-file",
        value_name = "datei",
        help = "Anleitung aus Datei („-“ = Standardeingabe)"
    )]
    pub detail_file: Option<String>,
    #[arg(
        long,
        value_name = "n",
        help = "an dieser Nummer einfügen (ab 1; Standard: ans Ende)"
    )]
    pub at: Option<String>,
    #[arg(long, value_name = "name", help = "wer anlegt (z. B. claude)")]
    pub by: Option<String>,
}

/// Optionen von `step edit`
#[derive(Debug, Args)]
pub struct EditArgs {
    pub id: String,
    pub nummer: String,
    #[arg(long, value_name = "text", help = "neuer Text")]
    pub text: Option<String>,
    #[arg(long, value_name = "text", help = "neue Anleitung")]
    pub detail: Option<String>,
    #[arg(
        long = "detail-file",
        value_name = "datei",
        help = "Anleitung aus Datei („-“ = Standardeingabe)"
    )]
    pub detail_file: Option<String>,
    #[arg(long, value_name = "name", help = "wer ändert")]
    pub by: Option<String>,
    #[arg(long, value_name = "n", help = REV_HELP)]
    pub rev: Option<String>,
    #[arg(long, value_name = "text", help = EXPECT_HELP)]
    pub expect: Option<String>,
}

/// Optionen von `step rm`
#[derive(Debug, Args)]
pub struct RmArgs {
    pub id: String,
    pub nummer: String,
    #[arg(long, value_name = "name", help = "wer löscht")]
    pub by: Option<String>,
    #[arg(long, value_name = "n", help = REV_HELP)]
    pub rev: Option<String>,
    #[arg(long, value_name = "text", help = EXPECT_HELP)]
    pub expect: Option<String>,
}

/// Optionen von `step move`
#[derive(Debug, Args)]
pub struct MoveArgs {
    pub id: String,
    pub von: String,
    pub nach: String,
    #[arg(long, value_name = "name", help = "wer verschiebt")]
    pub by: Option<String>,
    #[arg(long, value_name = "n", help = REV_HELP)]
    pub rev: Option<String>,
    #[arg(long, value_name = "text", help = MOVE_EXPECT_HELP)]
    pub expect: Option<String>,
}

/// Führt `board step` mit oder ohne Unterbefehl aus
pub fn run(context: &CliContext, args: &StepArgs) -> Result<u8> {
    match &args.sub {
        // Ohne Unterbefehl sind `<id>` und `<nummer>` Pflicht; clap prüft das hier nicht mehr
        None => match (args.id.as_deref(), args.nummer.as_deref()) {
            (Some(id), Some(nummer)) => check(
                context,
                id,
                nummer,
                !args.undo,
                args.by.as_deref(),
                args.rev.as_deref(),
                args.expect.as_deref(),
            ),
            (None, _) => Ok(fehlendes_argument("id")),
            (_, None) => Ok(fehlendes_argument("nummer")),
        },
        Some(StepCommands::Done(o)) => check(
            context,
            &o.id,
            &o.nummer,
            !o.undo,
            o.by.as_deref(),
            o.rev.as_deref(),
            o.expect.as_deref(),
        ),
        Some(StepCommands::Add(o)) => add(context, o),
        Some(StepCommands::Edit(o)) => edit(context, o),
        Some(StepCommands::Rm(o)) => remove(context, o),
        Some(StepCommands::Move(o)) => move_step(context, o),
    }
}

/// Hakt ab oder nimmt den Haken zurück
#[allow(clippy::too_many_arguments)]
fn check(
    context: &CliContext,
    id: &str,
    nummer: &str,
    done: bool,
    by: Option<&str>,
    rev: Option<&str>,
    expect: Option<&str>,
) -> Result<u8> {
    context.with_board(async |rt| {
        let number = number(nummer)?;
        let detail = rt
            .services
            .tickets
            .set_step(id, number, done, &rt.actor(by), &guard(rev, expect)?)
            .await?;
        context.printer().result(
            &detail,
            &[format!(
                "{}: {}/{} Schritte erledigt.",
                detail.view.ticket.id, detail.view.steps_done, detail.view.steps_total
            )],
        );
        Ok(0)
    })
}

/// Führt `board step add` aus
fn add(context: &CliContext, args: &AddArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let at = match &args.at {
            Some(value) => Some(number(value)?),
            None => None,
        };
        let detail = rt
            .services
            .tickets
            .add_step(
                &args.id,
                &args.text,
                &rt.actor(args.by.as_deref()),
                &AddStepOptions {
                    detail: TextSource::read(args.detail.as_deref(), args.detail_file.as_deref())?,
                    at,
                    rev: None,
                    expect_text: None,
                },
            )
            .await?;
        let what = format!("Schritt {} angelegt", at.unwrap_or(detail.view.steps_total));
        print(context, &detail, &what);
        Ok(0)
    })
}

/// Führt `board step edit` aus
fn edit(context: &CliContext, args: &EditArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let number = number(&args.nummer)?;
        let detail = rt
            .services
            .tickets
            .edit_step(
                &args.id,
                number,
                &StepChange {
                    text: args.text.clone(),
                    // Ein leerer Text entfernt die Anleitung; das macht der Fachkern selbst
                    detail: TextSource::read(args.detail.as_deref(), args.detail_file.as_deref())?
                        .map(Some),
                },
                &rt.actor(args.by.as_deref()),
                &guard(args.rev.as_deref(), args.expect.as_deref())?,
            )
            .await?;
        print(context, &detail, &format!("Schritt {number} geändert"));
        Ok(0)
    })
}

/// Führt `board step rm` aus
fn remove(context: &CliContext, args: &RmArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let number = number(&args.nummer)?;
        let detail = rt
            .services
            .tickets
            .remove_step(
                &args.id,
                number,
                &rt.actor(args.by.as_deref()),
                &guard(args.rev.as_deref(), args.expect.as_deref())?,
            )
            .await?;
        print(context, &detail, &format!("Schritt {number} gelöscht"));
        Ok(0)
    })
}

/// Führt `board step move` aus
fn move_step(context: &CliContext, args: &MoveArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let from = number(&args.von)?;
        let to = number(&args.nach)?;
        let detail = rt
            .services
            .tickets
            .move_step(
                &args.id,
                from,
                to,
                &rt.actor(args.by.as_deref()),
                &guard(args.rev.as_deref(), args.expect.as_deref())?,
            )
            .await?;
        print(context, &detail, &format!("Schritt {from} → {to}"));
        Ok(0)
    })
}

/// Meldet ein fehlendes Pflichtargument wortgleich wie commander und endet mit Code 1
fn fehlendes_argument(name: &str) -> u8 {
    eprintln!("{}", crate::output::errors::missing_argument(name));
    1
}

/// Schutz aus `--rev`/`--expect`; ohne beide ein leerer Schutz (Verhalten wie bisher)
fn guard(rev: Option<&str>, expect: Option<&str>) -> Result<StepGuard> {
    Ok(StepGuard {
        rev: BoardOptionParser.rev(rev)?,
        expect_text: expect.map(ToString::to_string),
    })
}

/// Gibt das Ticket mit der nummerierten Schrittliste aus
fn print(context: &CliContext, detail: &TicketDetail, what: &str) {
    let mut lines = vec![format!(
        "{}: {what} ({}/{} erledigt).",
        detail.view.ticket.id, detail.view.steps_done, detail.view.steps_total
    )];
    for (index, step) in detail.steps.iter().enumerate() {
        lines.push(format!(
            "  {}. [{}] {}",
            index + 1,
            if step.done { "x" } else { " " },
            step.text
        ));
    }
    context.printer().result(detail, &lines);
}

/// Schrittnummer ab 1
fn number(value: &str) -> Result<i32> {
    match value.parse::<i32>() {
        Ok(number) if number >= 1 => Ok(number),
        _ => Err(HarnessError::new(
            "step-invalid",
            format!("„{value}“ ist keine Schrittnummer (ab 1)."),
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_schrittnummer_muss_eine_ganze_zahl_ab_eins_sein() {
        assert_eq!(number("3").unwrap(), 3);
        for raw in ["0", "-1", "1.5", "null", ""] {
            let error = number(raw).unwrap_err();
            assert_eq!(error.code(), Some("step-invalid"), "{raw}");
            assert_eq!(
                error.to_string(),
                format!("„{raw}“ ist keine Schrittnummer (ab 1).")
            );
        }
    }

    #[test]
    fn ohne_rev_und_expect_bleibt_der_schutz_leer() {
        assert_eq!(guard(None, None).unwrap(), StepGuard::default());
        assert_eq!(
            guard(Some("7"), Some("Alt")).unwrap(),
            StepGuard {
                rev: Some(7),
                expect_text: Some("Alt".into())
            }
        );
        assert_eq!(
            guard(Some("x"), None).unwrap_err().code(),
            Some("rev-invalid")
        );
    }
}
