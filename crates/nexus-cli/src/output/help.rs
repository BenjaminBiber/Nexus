//! Deutsche Hilfe im Layout der heutigen commander-Ausgabe, gerendert aus dem clap-Befehlsmodell.
//! Portiert aus `packages/cli/src/output/GermanHelp.ts` und dem Standardlayout von commander.

use clap::{Arg, Command};

use super::wrap::wrap;

/// Breite der Hilfe in Spalten (commander: `helpWidth`)
const HELP_WIDTH: usize = 80;
/// Einrückung der Listeneinträge
const ITEM_INDENT: usize = 2;
/// Abstand zwischen Begriff und Beschreibung
const ITEM_SEPARATOR: usize = 2;

/// Die Hilfe-Option steht in jedem Befehl und immer als letzte (commander legt sie selbst an)
pub const HELP_TERM: &str = "-h, --help";
/// Beschreibung der Hilfe-Option
pub const HELP_DESCRIPTION: &str = "Hilfe anzeigen";

/// Ein Eintrag einer Liste: Begriff und Beschreibung
type Item = (String, String);

/// Rendert die Hilfe eines Befehls; `path` ist der Aufruf bis hierher, z. B. „nexus board create“
pub fn render(path: &str, cmd: &Command) -> String {
    let sections = [
        ("Argumente:", argument_items(cmd)),
        ("Optionen:", option_items(cmd)),
        ("Befehle:", command_items(cmd)),
    ];
    let width = sections
        .iter()
        .flat_map(|(_, items)| items.iter())
        .map(|(term, _)| term.chars().count())
        .max()
        .unwrap_or(0);

    let mut out = vec![format!("Aufruf: {}", usage(path, cmd)), String::new()];
    let about = text_of(cmd.get_about());
    if !about.is_empty() {
        out.push(wrap(&about, HELP_WIDTH, 0));
        out.push(String::new());
    }
    for (title, items) in &sections {
        if items.is_empty() {
            continue;
        }
        out.push((*title).to_string());
        out.push(format_list(items, width));
        out.push(String::new());
    }

    let mut help = out.join("\n");
    // Der Zusatztext (`Beispiel:`/`Beispiele:`) bringt seine eigene Leerzeile mit, wie in commander
    let after = text_of(cmd.get_after_help());
    if !after.is_empty() {
        help.push_str(&after);
        help.push('\n');
    }
    help
}

/// Aufrufzeile: `[options]`, bei Unterbefehlen `[command]`, danach die Argumente
fn usage(path: &str, cmd: &Command) -> String {
    let mut parts = vec!["[options]".to_string()];
    if cmd.get_subcommands().next().is_some() {
        parts.push("[command]".to_string());
    }
    parts.extend(positionals(cmd).map(argument_term));
    format!("{path} {}", parts.join(" "))
}

/// Sichtbare Argumente eines Befehls; versteckte zählen weder in der Aufrufzeile noch in der Liste
fn positionals(cmd: &Command) -> impl Iterator<Item = &Arg> {
    cmd.get_positionals().filter(|arg| !arg.is_hide_set())
}

/// Argumente nur dann auflisten, wenn mindestens eines beschrieben ist (wie commander)
fn argument_items(cmd: &Command) -> Vec<Item> {
    let items: Vec<Item> = positionals(cmd)
        .map(|arg| (argument_term(arg), text_of(arg.get_help())))
        .collect();
    if items.iter().any(|(_, help)| !help.is_empty()) {
        items
    } else {
        Vec::new()
    }
}

/// Optionen in der Reihenfolge der Anmeldung, die Hilfe-Option zuletzt.
/// Die von clap selbst angelegten Optionen `--help` und `--version` bleiben außen vor: die
/// Hilfe-Option hängt der Renderer mit deutschem Text selbst an, eine Version gibt es nicht.
fn option_items(cmd: &Command) -> Vec<Item> {
    cmd.get_arguments()
        .filter(|arg| !arg.is_positional() && !arg.is_hide_set())
        .filter(|arg| !matches!(arg.get_id().as_str(), "help" | "version"))
        .map(|arg| (option_term(arg), option_description(arg)))
        .chain([(HELP_TERM.to_string(), HELP_DESCRIPTION.to_string())])
        .collect()
}

/// Beschreibung einer Option; einen Standardwert hängt commander wie in JavaScript an.
/// Schalter zählen nicht: clap gibt ihnen „false“ als Standard, commander nennt dafür nichts.
fn option_description(arg: &Arg) -> String {
    let help = text_of(arg.get_help());
    if !takes_value(arg) {
        return help;
    }
    match arg.get_default_values().first() {
        Some(value) => format!("{help} (default: \"{}\")", value.to_string_lossy()),
        None => help,
    }
}

/// Nimmt die Option einen eigenen Wert an?
fn takes_value(arg: &Arg) -> bool {
    arg.get_num_args().is_some_and(|range| range.takes_values())
}

/// Unterbefehle, die nicht versteckt sind
fn command_items(cmd: &Command) -> Vec<Item> {
    cmd.get_subcommands()
        .filter(|sub| !sub.is_hide_set())
        .map(|sub| (command_term(sub), text_of(sub.get_about())))
        .collect()
}

/// Begriff eines Arguments: `<name>` wenn Pflicht, sonst `[name]`; mehrfach mit „…“
fn argument_term(arg: &Arg) -> String {
    let mut name = arg.get_id().to_string();
    if is_variadic(arg) {
        name.push_str("...");
    }
    if arg.is_required_set() {
        format!("<{name}>")
    } else {
        format!("[{name}]")
    }
}

/// Begriff einer Option: `--json`, `--dir <pfad>` oder `-h, --help`
fn option_term(arg: &Arg) -> String {
    let mut term = match (arg.get_short(), arg.get_long()) {
        (Some(short), Some(long)) => format!("-{short}, --{long}"),
        (None, Some(long)) => format!("--{long}"),
        (Some(short), None) => format!("-{short}"),
        (None, None) => arg.get_id().to_string(),
    };
    if let Some(value) = value_placeholder(arg) {
        term.push(' ');
        term.push_str(&value);
    }
    term
}

/// Begriff eines Unterbefehls: Name, `[options]` wenn er eigene Optionen hat, dann seine Argumente
fn command_term(cmd: &Command) -> String {
    let mut term = cmd.get_name().to_string();
    if let Some(alias) = cmd.get_visible_aliases().next() {
        term.push('|');
        term.push_str(alias);
    }
    if cmd.get_arguments().any(|arg| !arg.is_positional()) {
        term.push_str(" [options]");
    }
    let args: Vec<String> = positionals(cmd).map(argument_term).collect();
    if !args.is_empty() {
        term.push(' ');
        term.push_str(&args.join(" "));
    }
    term
}

/// Platzhalter für den Wert einer Option, falls sie einen braucht
fn value_placeholder(arg: &Arg) -> Option<String> {
    let range = arg.get_num_args().filter(|range| range.takes_values())?;
    let name = arg
        .get_value_names()
        .and_then(|names| names.first())
        .map(|name| name.to_string())
        .unwrap_or_else(|| arg.get_id().to_string());
    Some(if range.min_values() == 0 {
        format!("[{name}]")
    } else {
        format!("<{name}>")
    })
}

/// Nimmt eine Option mehrere Werte auf einmal an?
fn is_variadic(arg: &Arg) -> bool {
    arg.get_num_args()
        .is_some_and(|range| range.max_values() > 1)
}

/// Eine Liste von Einträgen, um zwei Spalten eingerückt
fn format_list(items: &[Item], width: usize) -> String {
    let text = items
        .iter()
        .map(|(term, description)| format_item(term, description, width))
        .collect::<Vec<_>>()
        .join("\n");
    text.split('\n')
        .map(|line| format!("{}{line}", " ".repeat(ITEM_INDENT)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ein Eintrag: Begriff, auf die Spaltenbreite aufgefüllt, danach die umbrochene Beschreibung
fn format_item(term: &str, description: &str, width: usize) -> String {
    if description.is_empty() {
        return term.to_string();
    }
    let column = width + ITEM_SEPARATOR;
    let filler = " ".repeat(column.saturating_sub(term.chars().count()));
    wrap(
        &format!("{term}{filler}{description}"),
        HELP_WIDTH - ITEM_INDENT,
        column,
    )
}

/// Text eines clap-Feldes, leer wenn es keines gibt
fn text_of(value: Option<&clap::builder::StyledStr>) -> String {
    value.map(ToString::to_string).unwrap_or_default()
}
