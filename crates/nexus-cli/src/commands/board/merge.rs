//! `nexus board merge|accept`: Merge eines erledigten Tickets erneut anstoßen und ein mit
//! „Human Interaction: …“ blockiertes Ticket abnehmen – beides über den laufenden Server.
//! Portiert aus `packages/cli/src/commands/board/BoardDispatchCommands.ts`
//! (Methoden `mergeTicket` und `acceptTicket`).

use clap::Args;
use nexus_core::Result;

use crate::context::CliContext;
use crate::output::DispatchStatusFormatter;

/// Beschreibung von `merge`
pub const MERGE_ABOUT: &str = "Stößt den Merge eines erledigten Tickets eines eigenen Boards erneut an (nach „Blockiert: Merge: …“), erlaubt dabei wieder eine Merge-Session und nennt das Ergebnis (gemergt, blockiert mit Grund, wartet, Merge-Session).";

/// Beispiel unter der Hilfe von `merge`
pub const MERGE_EXAMPLE: &str = "\nBeispiel:\n  harness board merge ARC-0009";

/// Beschreibung von `accept`
pub const ACCEPT_ABOUT: &str = "Nimmt ein Ticket ab, das mit Grund „Human Interaction: …“ blockiert ist (nur ein Mensch, nie ein Agent): setzt Erledigt (Abnahme-Text nötig), leert den Grund, vermerkt es im Verlauf und stößt auf einem eigenen Board den Merge an wie „harness board merge“. Nennt das Ergebnis.";

/// Beispiel unter der Hilfe von `accept`
pub const ACCEPT_EXAMPLE: &str = "\nBeispiel:\n  harness board accept ARC-0009 --by Florian";

/// Das Ticket, dessen Merge erneut angestoßen wird
#[derive(Debug, Args)]
pub struct MergeArgs {
    pub id: String,
}

/// Optionen von `board accept`
#[derive(Debug, Args)]
pub struct AcceptArgs {
    pub id: String,
    #[arg(
        long,
        value_name = "name",
        help = "wer abnimmt (Standard: Git-Benutzername)"
    )]
    pub by: Option<String>,
}

/// `board merge <id>`: „Erneut mergen“ und das Ergebnis für dieses Ticket ausgeben
pub fn merge(context: &CliContext, port: Option<&str>, args: &MergeArgs) -> Result<u8> {
    let view = super::dispatch::client(port)?.merge(&args.id)?;
    let lines = DispatchStatusFormatter.merge_lines(&DispatchStatusFormatter::view(&view));
    context.printer().result(&view, &lines);
    Ok(0)
}

/// `board accept <id>`: Abnehmen über den Server (nur Mensch, nicht aus einer Session)
pub fn accept(context: &CliContext, port: Option<&str>, args: &AcceptArgs) -> Result<u8> {
    let actor = context.human_actor(None, args.by.as_deref())?;
    let view = super::dispatch::client(port)?.accept(&args.id, &actor.author)?;
    let lines = DispatchStatusFormatter.merge_lines(&DispatchStatusFormatter::view(&view));
    context.printer().result(&view, &lines);
    Ok(0)
}
