//! Deutsche Meldungen für Fehler beim Parsen der Argumente – wortgleich zu den Texten, die
//! `packages/cli/src/output/GermanErrors.ts` heute aus den commander-Meldungen macht.

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::Command;

/// Größter Abstand, bei dem ein Vorschlag noch gemacht wird (wie `maxDistance` in commander)
const MAX_DISTANCE: usize = 3;
/// Mindestähnlichkeit eines Vorschlags (wie `minSimilarity` in commander)
const MIN_SIMILARITY: f64 = 0.4;

/// Übersetzt einen Fehler von clap in die Meldung, die commander heute ausgibt.
/// `cmd` ist der Befehl, in dem der Fehler aufgetreten ist – daraus kommen die Vorschläge.
pub fn translate(error: &clap::Error, cmd: &Command) -> String {
    match error.kind() {
        ErrorKind::InvalidSubcommand => match context(error, ContextKind::InvalidSubcommand) {
            Some(name) => unknown_command(&name, &command_names(cmd)),
            None => fallback(error),
        },
        ErrorKind::UnknownArgument => match context(error, ContextKind::InvalidArg) {
            Some(flag) if flag.starts_with('-') => unknown_option(&flag, &option_flags(cmd)),
            Some(_) => too_many_arguments(),
            None => fallback(error),
        },
        ErrorKind::MissingRequiredArgument => match context(error, ContextKind::InvalidArg) {
            Some(term) if term.starts_with('-') => missing_option(&term),
            Some(name) => missing_argument(&argument_name(cmd, &name)),
            None => fallback(error),
        },
        ErrorKind::InvalidValue
            if context(error, ContextKind::InvalidValue).as_deref() == Some("") =>
        {
            match context(error, ContextKind::InvalidArg) {
                Some(term) => missing_value(&term),
                None => fallback(error),
            }
        }
        ErrorKind::TooManyValues | ErrorKind::WrongNumberOfValues => too_many_arguments(),
        _ => fallback(error),
    }
}

/// `Fehler: Unbekannter Befehl '<name>'.`, dazu ein Vorschlag, wenn einer nahe liegt
pub fn unknown_command(name: &str, candidates: &[String]) -> String {
    format!(
        "Fehler: Unbekannter Befehl '{name}'.{}",
        suggest_similar(name, candidates)
    )
}

/// `Fehler: Unbekannte Option '<flag>'.`, dazu ein Vorschlag, wenn einer nahe liegt
pub fn unknown_option(flag: &str, candidates: &[String]) -> String {
    format!(
        "Fehler: Unbekannte Option '{flag}'.{}",
        suggest_similar(flag, candidates)
    )
}

/// `Fehler: Das Pflichtargument '<name>' fehlt.`
pub fn missing_argument(name: &str) -> String {
    format!("Fehler: Das Pflichtargument '{name}' fehlt.")
}

/// `Fehler: required option '--agent <name>' not specified` – für eine fehlende Pflichtoption hat
/// `GermanErrors.ts` keine Regel, deshalb bleibt die Meldung von commander englisch stehen. Der
/// Text ist Vertrag und wird wortgleich übernommen.
pub fn missing_option(term: &str) -> String {
    format!("Fehler: required option '{term}' not specified")
}

/// clap meldet den Platzhalter eines Arguments (`<CODE>`), commander seinen angemeldeten Namen
/// (`code`): deshalb den Platzhalter im Befehl nachschlagen. clap schreibt den Namen eines
/// Arguments ohne eigenen Platzhalter groß, deshalb wird ohne Groß-/Kleinschreibung verglichen.
fn argument_name(cmd: &Command, rendered: &str) -> String {
    let bare = rendered.trim_matches(|c| matches!(c, '<' | '>' | '[' | ']'));
    cmd.get_positionals()
        .find(|arg| placeholder(arg).eq_ignore_ascii_case(bare))
        .map(|arg| arg.get_id().to_string())
        .unwrap_or_else(|| bare.to_string())
}

/// Platzhalter, den clap für ein Argument ausgibt
fn placeholder(arg: &clap::Arg) -> String {
    arg.get_value_names()
        .and_then(|names| names.first())
        .map(ToString::to_string)
        .unwrap_or_else(|| arg.get_id().to_string())
}

/// `Fehler: Zur Option '--title <text>' fehlt ein Wert.`
pub fn missing_value(term: &str) -> String {
    format!("Fehler: Zur Option '{term}' fehlt ein Wert.")
}

/// `Fehler: Zu viele Argumente.`
pub fn too_many_arguments() -> String {
    "Fehler: Zu viele Argumente.".to_string()
}

/// Namen der sichtbaren Unterbefehle samt Kurzformen
fn command_names(cmd: &Command) -> Vec<String> {
    cmd.get_subcommands()
        .filter(|sub| !sub.is_hide_set())
        .flat_map(|sub| {
            std::iter::once(sub.get_name().to_string())
                .chain(sub.get_visible_aliases().map(ToString::to_string))
        })
        .collect()
}

/// Lange Schreibweisen der sichtbaren Optionen, dazu die Hilfe-Option
fn option_flags(cmd: &Command) -> Vec<String> {
    cmd.get_arguments()
        .filter(|arg| !arg.is_positional() && !arg.is_hide_set())
        .filter_map(|arg| arg.get_long().map(|long| format!("--{long}")))
        .chain(["--help".to_string()])
        .collect()
}

/// Der Text eines Zusammenhangs des clap-Fehlers (erster Eintrag, falls es mehrere sind)
fn context(error: &clap::Error, kind: ContextKind) -> Option<String> {
    match error.get(kind)? {
        ContextValue::String(value) => Some(value.clone()),
        ContextValue::Strings(values) => values.first().cloned(),
        _ => None,
    }
}

/// Fehler, für die es keine eigene deutsche Meldung gibt: nur das englische `error:` ersetzen
fn fallback(error: &clap::Error) -> String {
    let rendered = error.render().to_string();
    let first = rendered
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    match first.strip_prefix("error:") {
        Some(rest) => format!("Fehler:{rest}"),
        None => format!("Fehler: {first}"),
    }
}

/// Vorschlag „(Meinten Sie …?)“ nach den Regeln von commander (`suggestSimilar`)
fn suggest_similar(word: &str, candidates: &[String]) -> String {
    if candidates.is_empty() {
        return String::new();
    }
    let options = word.starts_with("--");
    let needle: String = if options {
        word[2..].to_string()
    } else {
        word.to_string()
    };
    let mut seen: Vec<String> = Vec::new();
    for candidate in candidates {
        let bare = if options {
            candidate.trim_start_matches("--").to_string()
        } else {
            candidate.clone()
        };
        if !seen.contains(&bare) {
            seen.push(bare);
        }
    }

    let mut best = MAX_DISTANCE;
    let mut similar: Vec<String> = Vec::new();
    for candidate in &seen {
        if candidate.chars().count() <= 1 {
            continue;
        }
        let distance = edit_distance(&needle, candidate);
        let length = needle.chars().count().max(candidate.chars().count());
        let similarity = (length - distance.min(length)) as f64 / length as f64;
        if similarity <= MIN_SIMILARITY {
            continue;
        }
        if distance < best {
            best = distance;
            similar = vec![candidate.clone()];
        } else if distance == best {
            similar.push(candidate.clone());
        }
    }
    similar.sort();
    if options {
        similar = similar
            .into_iter()
            .map(|name| format!("--{name}"))
            .collect();
    }
    match similar.len() {
        0 => String::new(),
        1 => format!("\n(Meinten Sie {}?)", similar[0]),
        // Bei mehreren Vorschlägen sagt commander „Did you mean one of a, b?“; die Übersetzung in
        // GermanErrors.ts greift nur „Did you mean“ auf, deshalb bleibt „one of“ heute stehen.
        // Der Text ist Vertrag und wird wortgleich übernommen.
        _ => format!("\n(Meinten Sie one of {}?)", similar.join(", ")),
    }
}

/// Abstand nach Damerau-Levenshtein, wie `editDistance` in commander
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > MAX_DISTANCE {
        return a.len().max(b.len());
    }
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for j in 1..=b.len() {
        for i in 1..=a.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn befehle() -> Vec<String> {
        ["init", "board", "install", "stop", "hook", "spec", "help"]
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn unbekannter_befehl_ohne_ahnlichen_namen_bleibt_ohne_vorschlag() {
        assert_eq!(
            unknown_command("quatsch", &befehle()),
            "Fehler: Unbekannter Befehl 'quatsch'."
        );
    }

    #[test]
    fn unbekannter_befehl_mit_ahnlichem_namen_schlagt_ihn_vor() {
        assert_eq!(
            unknown_command("bord", &befehle()),
            "Fehler: Unbekannter Befehl 'bord'.\n(Meinten Sie board?)"
        );
    }

    #[test]
    fn unbekannte_option_nennt_die_option() {
        assert_eq!(
            unknown_option(
                "--quatsch",
                &[
                    "--json".to_string(),
                    "--dir".to_string(),
                    "--help".to_string()
                ]
            ),
            "Fehler: Unbekannte Option '--quatsch'."
        );
    }

    #[test]
    fn unbekannte_option_schlagt_eine_ahnliche_vor() {
        assert_eq!(
            unknown_option("--jsn", &["--json".to_string(), "--dir".to_string()]),
            "Fehler: Unbekannte Option '--jsn'.\n(Meinten Sie --json?)"
        );
    }

    #[test]
    fn pflichtargument_wird_mit_seinem_namen_genannt() {
        assert_eq!(
            missing_argument("antwort"),
            "Fehler: Das Pflichtargument 'antwort' fehlt."
        );
    }

    #[test]
    fn eine_fehlende_pflichtoption_behalt_den_text_von_commander() {
        let cmd = clap::Command::new("claim").arg(
            clap::Arg::new("agent")
                .long("agent")
                .value_name("name")
                .required(true),
        );
        let error = cmd.try_get_matches_from(["claim"]).unwrap_err();
        assert_eq!(
            translate(&error, &clap::Command::new("claim")),
            "Fehler: required option '--agent <name>' not specified"
        );
    }

    #[test]
    fn der_platzhalter_von_clap_wird_auf_den_angemeldeten_namen_zuruckgefuhrt() {
        let cmd = clap::Command::new("resolve")
            .arg(clap::Arg::new("antwort").required(true))
            .arg(clap::Arg::new("datei").required(true).value_name("pfad"));
        assert_eq!(argument_name(&cmd, "<ANTWORT>"), "antwort");
        assert_eq!(argument_name(&cmd, "<pfad>"), "datei");
        assert_eq!(argument_name(&cmd, "<unbekannt>"), "unbekannt");
    }

    #[test]
    fn fehlender_wert_nennt_die_ganze_option() {
        assert_eq!(
            missing_value("--title <text>"),
            "Fehler: Zur Option '--title <text>' fehlt ein Wert."
        );
    }

    #[test]
    fn abstand_zahlt_auch_vertauschte_zeichen() {
        assert_eq!(edit_distance("bord", "board"), 1);
        assert_eq!(edit_distance("baord", "board"), 1);
        assert_eq!(edit_distance("quatsch", "board"), 6);
        // Weit auseinanderliegende Längen rechnet commander gar nicht erst aus
        assert_eq!(edit_distance("a", "abcdefg"), 7);
    }
}
