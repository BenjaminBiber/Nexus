//! `nexus board inbox`: „Meine Aufgaben“ – was auf den eigenen Boards gerade auf mich wartet, in
//! meiner Reihenfolge. Portiert aus `packages/cli/src/commands/board/BoardInboxCommands.ts`.

use std::sync::LazyLock;

use clap::Args;
use nexus_core::board::inbox::{InboxFilter, InboxItem, InboxKind, InboxOrder, InboxOrderFile};
use nexus_core::{HarnessError, Result};

use crate::board::BoardSelection;
use crate::context::CliContext;

/// Die Arten, kommagetrennt – für Hilfe und Fehlermeldung
static KIND_NAMES: LazyLock<String> = LazyLock::new(|| {
    InboxKind::ALL
        .iter()
        .map(InboxKind::as_str)
        .collect::<Vec<_>>()
        .join(", ")
});

/// Beschreibung von `inbox`
pub const INBOX_ABOUT: &str =
    "Meine Aufgaben: wartende Tickets, offene Fragen und Notizen der eigenen Boards.";

/// Beispiele unter der Hilfe von `inbox`
pub const INBOX_EXAMPLES: &str =
    "\nBeispiele:\n  nexus board inbox --md\n  nexus board inbox --kind merge,accept";

/// Hilfe der Option `--kind`
pub fn kind_help() -> String {
    format!("nur diese Arten, kommagetrennt ({})", *KIND_NAMES)
}

/// Optionen von `board inbox`
#[derive(Debug, Args)]
pub struct InboxArgs {
    #[arg(
        long,
        value_name = "id",
        help = "nur dieses eigene Board (Standard: alle eigenen Boards)"
    )]
    pub board: Option<String>,
    #[arg(long, value_name = "arten", help = kind_help())]
    pub kind: Option<String>,
    #[arg(long, help = "Ausgabe als Markdown")]
    pub md: bool,
}

/// Führt `board inbox` aus
pub fn run(context: &CliContext, args: &InboxArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let kinds = match &args.kind {
            Some(raw) => Some(parse_kinds(raw)?),
            None => None,
        };
        let board_id = match &args.board {
            Some(raw) => Some(BoardSelection.parse_id(raw)?),
            None => None,
        };
        let listed = rt
            .services
            .inbox
            .list(&rt.config.user_id, &InboxFilter { board_id })
            .await?;
        let ordered = InboxOrder::apply(listed, &InboxOrderFile::new(context.home()).read());
        let items: Vec<InboxItem> = match &kinds {
            Some(kinds) => ordered
                .into_iter()
                .filter(|item| kinds.contains(&item.kind))
                .collect(),
            None => ordered,
        };
        let lines = if args.md {
            markdown(&items)
        } else {
            text(&items)
        };
        context.printer().result(&items, &lines);
        Ok(0)
    })
}

/// Kommagetrennte Arten oder `None` (alle)
fn parse_kinds(raw: &str) -> Result<Vec<InboxKind>> {
    let names: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if let Some(name) = names.iter().find(|name| kind_of(name).is_none()) {
        return Err(invalid(name));
    }
    if names.is_empty() {
        return Err(invalid(raw));
    }
    Ok(names.iter().filter_map(|name| kind_of(name)).collect())
}

/// Art zu ihrer Schreibweise
fn kind_of(name: &str) -> Option<InboxKind> {
    InboxKind::ALL
        .into_iter()
        .find(|kind| kind.as_str() == name)
}

/// Meldung über eine unbekannte Art
fn invalid(name: &str) -> nexus_core::Error {
    HarnessError::new(
        "input-invalid",
        format!("„{name}“ ist keine Art. Erlaubt: {}.", *KIND_NAMES),
    )
    .into()
}

/// Die Einträge als Text
fn text(items: &[InboxItem]) -> Vec<String> {
    if items.is_empty() {
        return vec!["Nichts wartet auf dich.".to_string()];
    }
    let mut lines = Vec::new();
    for (index, item) in items.iter().enumerate() {
        lines.push(format!(
            "{}. [{}] {}  {}  ({})",
            index + 1,
            item.kind.label(),
            id(item),
            item.title,
            where_of(item)
        ));
        if let Some(detail) = &item.detail {
            lines.push(format!("   {detail}"));
        }
        lines.push(format!("   → {}", action(item)));
    }
    lines
}

/// Die Einträge als Markdown
fn markdown(items: &[InboxItem]) -> Vec<String> {
    if items.is_empty() {
        return vec!["_Nichts wartet auf dich._".to_string()];
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let detail = match &item.detail {
                Some(detail) => format!(" – {}", collapse_spaces(detail)),
                None => String::new(),
            };
            format!(
                "{}. **{}** `{}` {} ({}){detail} → `{}`",
                index + 1,
                item.kind.label(),
                id(item),
                item.title,
                where_of(item),
                action(item)
            )
        })
        .collect()
}

/// Alle Folgen von Leerraum zu einem Leerzeichen (wie `/\s+/g` in TypeScript)
fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Was in der Spalte der Kennung steht
fn id(item: &InboxItem) -> String {
    item.question_id
        .clone()
        .or_else(|| item.ticket_id.clone())
        .unwrap_or_else(|| format!("Notiz {}", item.note_id.unwrap_or_default()))
}

/// Projekt, Board und – bei Fragen – die Zahl der wartenden Tickets
fn where_of(item: &InboxItem) -> String {
    let waiting = if item.kind == InboxKind::Question {
        format!(", {} Ticket(s) warten", item.waiting_tickets)
    } else {
        String::new()
    };
    format!(
        "{}{}{waiting}",
        item.project_key,
        match &item.board_name {
            Some(name) => format!(" · {name}"),
            None => String::new(),
        }
    )
}

/// Befehl, mit dem es weitergeht
fn action(item: &InboxItem) -> String {
    let ticket = item.ticket_id.clone().unwrap_or_default();
    match item.kind {
        InboxKind::Merge => format!("nexus board merge {ticket}"),
        InboxKind::Accept => format!("nexus board accept {ticket}"),
        InboxKind::Question => format!(
            "nexus board question show {}",
            item.question_id.clone().unwrap_or_default()
        ),
        InboxKind::Note => format!("nexus board note take {}", item.note_id.unwrap_or_default()),
        _ => format!("nexus board show {ticket}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_hilfe_nennt_alle_arten() {
        assert_eq!(
            kind_help(),
            "nur diese Arten, kommagetrennt (merge, setup, question, accept, blocked, planning, note)"
        );
    }

    #[test]
    fn kommagetrennte_arten_werden_gelesen() {
        assert_eq!(
            parse_kinds("merge, accept").unwrap(),
            [InboxKind::Merge, InboxKind::Accept]
        );
    }

    #[test]
    fn eine_unbekannte_oder_leere_art_wird_mit_input_invalid_abgelehnt() {
        let error = parse_kinds("quatsch").unwrap_err();
        assert_eq!(error.code(), Some("input-invalid"));
        assert_eq!(
            error.to_string(),
            "„quatsch“ ist keine Art. Erlaubt: merge, setup, question, accept, blocked, planning, note."
        );
        assert_eq!(
            parse_kinds(",").unwrap_err().to_string(),
            "„,“ ist keine Art. Erlaubt: merge, setup, question, accept, blocked, planning, note."
        );
    }

    #[test]
    fn ohne_eintrage_meldet_die_ausgabe_dass_nichts_wartet() {
        assert_eq!(text(&[]), ["Nichts wartet auf dich."]);
        assert_eq!(markdown(&[]), ["_Nichts wartet auf dich._"]);
    }
}
