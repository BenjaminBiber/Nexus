//! `nexus board comment`: Kommentar an ein Ticket hängen.
//! Portiert aus `packages/cli/src/commands/board/BoardCommentCommand.ts`.

use clap::Args;
use nexus_core::text::TextSource;
use nexus_core::{HarnessError, Result};

use crate::context::CliContext;

/// Beschreibung von `comment`
pub const COMMENT_ABOUT: &str = "Hängt einen Kommentar an (Zwischenstand, Nachweis, Begründung).";

/// Beispiel unter der Hilfe von `comment`
pub const COMMENT_EXAMPLE: &str =
    "\nBeispiel:\n  harness board comment ARC-0007 \"Schritt 2 von 4 fertig\" --author claude";

/// Optionen von `board comment`
#[derive(Debug, Args)]
pub struct CommentArgs {
    pub id: String,
    pub text: Option<String>,
    #[arg(
        long = "body-file",
        value_name = "datei",
        help = "Text aus Datei („-“ = Standardeingabe)"
    )]
    pub body_file: Option<String>,
    #[arg(long, value_name = "name", help = "wer kommentiert (z. B. claude)")]
    pub author: Option<String>,
}

/// Führt `board comment` aus
pub fn comment(context: &CliContext, args: &CommentArgs) -> Result<u8> {
    // Aus einer Session geschrieben: gilt sofort als gelesen und wird nicht in die Session
    // zurückgespeist
    let read_by_agent = context.session().is_some();
    context.with_board(async |rt| {
        // Wie in TypeScript erst nach dem Öffnen des Boards
        let Some(body) = TextSource::read(args.text.as_deref(), args.body_file.as_deref())? else {
            return Err(HarnessError::new(
                "usage",
                "Kommentartext fehlt (Argument oder --body-file).",
            )
            .into());
        };
        let comment = rt
            .services
            .comments
            .add(
                &args.id,
                &body,
                &rt.actor(args.author.as_deref()),
                read_by_agent,
            )
            .await?;
        context.printer().result(
            &comment,
            &[format!("Kommentar an {} gespeichert.", comment.ticket_id)],
        );
        Ok(0)
    })
}
