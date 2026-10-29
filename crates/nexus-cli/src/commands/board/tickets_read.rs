//! `nexus board list` und `nexus board show`: Tickets lesen.
//! Portiert aus `packages/cli/src/commands/board/BoardTicketReadCommands.ts`.

use clap::Args;
use nexus_core::board::ticket_service::{TicketDetail, TicketListFilter};
use nexus_core::{HarnessError, Result};

use crate::board::{BoardOptionParser, BoardSelection, BoardTextFormatter, ShowTextOptions};
use crate::context::CliContext;

/// Felder, die `--field` roh ausgeben kann
const RAW_FIELDS: [&str; 8] = [
    "title",
    "description",
    "plan",
    "handbook",
    "diagram",
    "acceptance",
    "status",
    "blockedReason",
];

/// Beschreibung von `list`
pub const LIST_ABOUT: &str =
    "Listet Tickets je Spalte (Wartet ist abgeleitet). --md für Agenten, --json vollständig.";

/// Beispiele unter der Hilfe von `list`
pub const LIST_EXAMPLES: &str =
    "\nBeispiele:\n  harness board list --md\n  harness board list --status geplant,wartet --q login";

/// Beschreibung von `show`
pub const SHOW_ABOUT: &str = "Zeigt ein Ticket mit Texten, Voraussetzungen und Kommentaren (kompakt; --history, --all-comments für alles).";

/// Beispiele unter der Hilfe von `show`
pub const SHOW_EXAMPLES: &str = "\nBeispiele:\n  harness board show ARC-0007\n  harness board show ARC-0007 --all-comments --history\n  harness board show ARC-0007 --field plan";

/// Optionen von `board list`
#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        value_name = "id|all",
        help = "Board (Standard: Board dieses Repos, sonst aktives Board)"
    )]
    pub board: Option<String>,
    #[arg(
        long,
        value_name = "spalten",
        help = "nur diese Spalten, kommagetrennt (auch „wartet“)"
    )]
    pub status: Option<String>,
    #[arg(long = "needs-planning", help = "nur Tickets mit Planungsbedarf")]
    pub needs_planning: bool,
    #[arg(long, value_name = "name", help = "nur Tickets dieses Agenten")]
    pub assignee: Option<String>,
    #[arg(long, value_name = "label", help = "nur Tickets mit diesem Label")]
    pub label: Option<String>,
    #[arg(
        long,
        value_name = "text",
        help = "Suchtext in ID, Titel und Beschreibung"
    )]
    pub q: Option<String>,
    #[arg(long, help = "Ausgabe als Markdown")]
    pub md: bool,
}

/// Optionen von `board show`
#[derive(Debug, Args)]
pub struct ShowArgs {
    pub id: String,
    #[arg(long, value_name = "feld", help = FIELD_HELP)]
    pub field: Option<String>,
    #[arg(long, help = "Verlauf vollständig ausgeben")]
    pub history: bool,
    #[arg(
        long = "all-comments",
        help = "auch ältere Kommentare von Agenten ausgeben"
    )]
    pub all_comments: bool,
}

/// Hilfe der Option `--field`; die Liste ist [`RAW_FIELDS`]
const FIELD_HELP: &str = "nur ein Feld roh ausgeben (title, description, plan, handbook, diagram, acceptance, status, blockedReason)";

/// Führt `board list` aus
pub fn list(context: &CliContext, args: &ListArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let board_ids = BoardSelection
            .many(
                &rt.services,
                &rt.config,
                args.board.as_deref(),
                Some(context.project_dir()),
            )
            .await?;
        let columns = match &args.status {
            Some(status) => Some(BoardOptionParser.columns(status)?),
            None => None,
        };
        let tickets = rt
            .services
            .tickets
            .list(&TicketListFilter {
                board_ids: board_ids.clone(),
                columns,
                needs_planning: if args.needs_planning {
                    Some(true)
                } else {
                    None
                },
                assignee: args.assignee.clone(),
                label: args.label.clone(),
                q: args.q.clone(),
                without_texts: false,
            })
            .await?;
        let format = BoardTextFormatter::default();
        let heading = if args.board.as_deref() == Some("all") {
            "Alle Boards".to_string()
        } else {
            format!("Board {}", board_ids.first().copied().unwrap_or_default())
        };
        let lines = if args.md {
            format.list_markdown(&heading, &tickets)
        } else {
            format.list_text(&tickets)
        };
        context.printer().result(&tickets, &lines);
        Ok(0)
    })
}

/// Führt `board show` aus
pub fn show(context: &CliContext, args: &ShowArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let detail = rt.services.tickets.get(&args.id).await?;
        if let Some(field) = &args.field {
            return print_field(context, &detail, field);
        }
        let lines = BoardTextFormatter::default().show_text(
            &detail,
            ShowTextOptions {
                history: args.history,
                all_comments: args.all_comments,
            },
        );
        context.printer().result(&detail, &lines);
        Ok(0)
    })
}

/// Gibt ein einzelnes Feld ohne Formatierung aus
fn print_field(context: &CliContext, detail: &TicketDetail, field: &str) -> Result<u8> {
    if !RAW_FIELDS.contains(&field) {
        return Err(HarnessError::new(
            "field-invalid",
            format!(
                "Feld „{field}“ gibt es nicht. Erlaubt: {}.",
                RAW_FIELDS.join(", ")
            ),
        )
        .into());
    }
    let value = raw_field(detail, field);
    context.printer().result(
        &serde_json::json!({ field: value }),
        std::slice::from_ref(&value),
    );
    Ok(0)
}

/// Der Wert eines rohen Feldes; `blockedReason` ist das einzige, das leer sein darf
fn raw_field(detail: &TicketDetail, field: &str) -> String {
    let t = &detail.view.ticket;
    match field {
        "title" => t.title.clone(),
        "description" => t.description.clone(),
        "plan" => t.plan.clone(),
        "handbook" => t.handbook.clone(),
        "diagram" => t.diagram.clone(),
        "acceptance" => t.acceptance.clone(),
        "status" => t.status.as_str().to_string(),
        "blockedReason" => t.blocked_reason.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_hilfe_nennt_alle_rohen_felder() {
        assert_eq!(
            FIELD_HELP,
            format!("nur ein Feld roh ausgeben ({})", RAW_FIELDS.join(", "))
        );
    }

    #[test]
    fn ein_unbekanntes_feld_wird_mit_field_invalid_abgelehnt() {
        let context = CliContext::with_session(false, None, None);
        let detail = detail();
        let error = print_field(&context, &detail, "quatsch").unwrap_err();
        assert_eq!(error.code(), Some("field-invalid"));
        assert_eq!(
            error.to_string(),
            "Feld „quatsch“ gibt es nicht. Erlaubt: title, description, plan, handbook, diagram, acceptance, status, blockedReason."
        );
    }

    #[test]
    fn jedes_rohe_feld_hat_einen_wert() {
        let detail = detail();
        assert_eq!(raw_field(&detail, "title"), "Login bauen");
        assert_eq!(raw_field(&detail, "status"), "backlog");
        // Ein leerer Grund wird wie in TypeScript zur leeren Zeile
        assert_eq!(raw_field(&detail, "blockedReason"), "");
    }

    /// Ein Ticket, von dem nur Titel und Status zählen
    fn detail() -> TicketDetail {
        let ticket = nexus_core::testing::ticket_fixture("ARC-0001", 1, 1, 1);
        TicketDetail {
            view: nexus_core::board::ticket_service::TicketView {
                ticket: nexus_core::board::types::Ticket {
                    title: "Login bauen".into(),
                    ..ticket
                },
                column: nexus_core::board::ticket_ordering::BoardColumn::Backlog,
                waiting: false,
                depends_open: Vec::new(),
                depends_unmerged: Vec::new(),
                dependents: Vec::new(),
                unlocks: Vec::new(),
                waiting_on_questions: Vec::new(),
                steps_done: 0,
                steps_total: 0,
                sessions: Vec::new(),
            },
            comments: Vec::new(),
            history: Vec::new(),
            steps: Vec::new(),
        }
    }
}
