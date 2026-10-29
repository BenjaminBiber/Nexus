//! `nexus board claim|release`: Tickets übernehmen und freigeben.
//! Portiert aus `packages/cli/src/commands/board/BoardClaimCommands.ts`.

use clap::Args;
use nexus_core::board::ticket_service::ClaimOptions;
use nexus_core::Result;

use crate::board::BoardOptionParser;
use crate::context::CliContext;
use crate::server::session_notice::ServerSessionNotice;

/// Beschreibung von `claim`
pub const CLAIM_ABOUT: &str = "Übernimmt ein Ticket für einen Agenten (Standard: In Arbeit). Bereits zugewiesen (auch an denselben Namen) → Exit 1, außer mit --force. Innerhalb einer Session wird sie mit dem Ticket verknüpft.";

/// Beispiel unter der Hilfe von `claim`
pub const CLAIM_EXAMPLE: &str = "\nBeispiel:\n  harness board claim ARC-0007 --agent claude";

/// Beschreibung von `release`
pub const RELEASE_ABOUT: &str = "Gibt ein Ticket frei; der Status ändert sich nur mit --status.";

/// Beispiel unter der Hilfe von `release`
pub const RELEASE_EXAMPLE: &str = "\nBeispiel:\n  harness board release ARC-0007 --status geplant";

/// Optionen von `board claim`
#[derive(Debug, Args)]
pub struct ClaimArgs {
    pub id: String,
    #[arg(
        long,
        required = true,
        value_name = "name",
        help = "Name des Agenten, z. B. claude"
    )]
    pub agent: String,
    #[arg(
        long,
        value_name = "status",
        help = "Zielstatus (Standard: in_progress)"
    )]
    pub status: Option<String>,
    #[arg(
        long,
        help = "bestehende Zuweisung und offene Voraussetzungen übergehen"
    )]
    pub force: bool,
}

/// Optionen von `board release`
#[derive(Debug, Args)]
pub struct ReleaseArgs {
    pub id: String,
    #[arg(long, value_name = "status", help = "neuer Status")]
    pub status: Option<String>,
    #[arg(long, value_name = "name", help = "wer freigibt")]
    pub by: Option<String>,
}

/// Führt `board claim` aus
pub fn claim(context: &CliContext, args: &ClaimArgs) -> Result<u8> {
    let session = context.session().map(ToString::to_string);
    let claimed = context.with_board(async |rt| {
        // Wie in TypeScript erst nach dem Öffnen des Boards
        let status = match &args.status {
            Some(status) => Some(BoardOptionParser.status(status)?),
            None => None,
        };
        rt.services
            .tickets
            .claim(
                &args.id,
                &args.agent,
                &rt.actor(Some(&args.agent)),
                &ClaimOptions {
                    status,
                    force: args.force,
                    session_id: session.clone(),
                    rev: None,
                },
            )
            .await
    })?;
    if let Some(session) = &session {
        ServerSessionNotice.tell(context.project_dir(), &claimed.ticket.id, session);
    }
    context.printer().result(
        &claimed,
        &[format!(
            "{} übernommen von {} ({})",
            claimed.ticket.id,
            claimed.ticket.assignee.as_deref().unwrap_or_default(),
            claimed.ticket.status
        )],
    );
    Ok(0)
}

/// Führt `board release` aus
pub fn release(context: &CliContext, args: &ReleaseArgs) -> Result<u8> {
    context.with_board(async |rt| {
        // Wie in TypeScript erst nach dem Öffnen des Boards
        let status = match &args.status {
            Some(status) => Some(BoardOptionParser.status(status)?),
            None => None,
        };
        let released = rt
            .services
            .tickets
            .release(&args.id, &rt.actor(args.by.as_deref()), status)
            .await?;
        context.printer().result(
            &released,
            &[format!(
                "{} freigegeben ({})",
                released.ticket.id, released.ticket.status
            )],
        );
        Ok(0)
    })
}
