//! Befehlsbaum des Binaries `nexus`: globale Optionen, deutsche Hilfe und deutsche Parserfehler.
//! Portiert aus `packages/cli/src/main.ts` und `output/HarnessCommand.ts`.

use std::path::PathBuf;

use clap::{Command, CommandFactory, FromArgMatches, Parser, Subcommand};
use nexus_core::Result;

use crate::commands;
use crate::context::CliContext;
use crate::output::{errors, help};

/// Name in Hilfe, Beispielen und Fehlermeldungen; bis P3-16 bleibt es „harness“
pub const PROGRAM: &str = "harness";

/// Beschreibung des Programms auf der Hauptseite der Hilfe
const ABOUT: &str =
    "AI Harness: Board, Specs, Worktrees und Sessions für parallele Claude-Sessions";

/// Globale Optionen und der Befehl, der ausgeführt werden soll.
///
/// Hilfe und Fehler rendert die CLI selbst im Layout von commander, deshalb sind die
/// clap-eigene Hilfe-Option, der Hilfe-Unterbefehl und die Version abgeschaltet.
#[derive(Debug, Parser)]
#[command(
    name = PROGRAM,
    about = ABOUT,
    disable_help_flag = true,
    disable_help_subcommand = true,
    disable_version_flag = true
)]
pub struct Cli {
    #[arg(long, help = "Ausgabe als JSON")]
    pub json: bool,
    #[arg(
        long,
        value_name = "pfad",
        help = "Projektordner (Standard: aktuelles Verzeichnis)"
    )]
    pub dir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Beschreibung von `init` (wortgleich zu `InitCommand.ts`)
const INIT_ABOUT: &str = "Richtet den Harness im aktuellen Git-Projekt ein (einmal pro Projekt, wiederholbar) und nennt das zugeordnete Board.";

/// Beschreibung von `install` (wortgleich zu `InstallCommand.ts`)
const INSTALL_ABOUT: &str = "Macht den Befehl harness im normalen Terminal verfügbar (Kopie unter ~/.harness, Eintrag im PATH des Benutzers).";

/// Die Befehle des Programms. Die restlichen Fachbefehle folgen in P3-15 bis P3-18; `help` steht
/// wie bei commander in jedem Befehl, der selbst nichts tut und Unterbefehle hat.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Projekt einrichten
    #[command(
        name = "init",
        about = INIT_ABOUT,
        after_help = "\nBeispiel:\n  harness init",
        disable_help_flag = true
    )]
    Init,

    /// `harness` im normalen Terminal verfügbar machen
    #[command(
        name = "install",
        about = INSTALL_ABOUT,
        after_help = "\nBeispiel:\n  harness install\n\nWiederholbar; aktualisiert die Kopie nach einem Plugin-Update.",
        disable_help_flag = true
    )]
    Install {
        #[arg(long = "no-path", help = "PATH nicht ändern, nur kopieren")]
        no_path: bool,
    },

    /// Hilfe zu einem Befehl anzeigen
    #[command(
        name = "help",
        about = "Hilfe zu einem Befehl anzeigen",
        disable_help_flag = true
    )]
    Help { befehl: Option<String> },

    /// Nur in Debug-Builds: wirft einen erwartbaren Fehler, damit sich Rückgabecodes und das
    /// JSON-Format eines Fehlers von außen prüfen lassen.
    #[cfg(debug_assertions)]
    #[command(name = "selbsttest-fehler", hide = true, disable_help_flag = true)]
    SelbsttestFehler { code: String, meldung: String },
}

/// Führt das Programm aus und liefert den Rückgabecode für alles, was schon ausgegeben wurde;
/// erwartbare und unerwartete Fehler reicht sie an `main` weiter.
pub fn run(args: &[String]) -> Result<u8> {
    let mut root = Cli::command();
    root.build();
    let walk = locate(&root, args);

    if walk.help {
        print!("{}", help::render(&walk.path.join(" "), walk.cmd));
        return Ok(0);
    }

    let matches = match root
        .clone()
        .try_get_matches_from(std::iter::once(PROGRAM.to_string()).chain(args.iter().cloned()))
    {
        Ok(matches) => matches,
        Err(error) => {
            eprintln!("{}", errors::translate(&error, walk.cmd));
            return Ok(1);
        }
    };
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(error) => {
            eprintln!("{}", errors::translate(&error, walk.cmd));
            return Ok(1);
        }
    };
    dispatch(cli, &root)
}

/// Führt den gewählten Befehl aus; ohne Befehl zeigt das Programm wie commander seine Hilfe
/// auf stderr und endet mit Code 1.
fn dispatch(cli: Cli, root: &Command) -> Result<u8> {
    let context = CliContext::new(cli.json, cli.dir);
    match cli.command {
        None => {
            eprint!("{}", help::render(PROGRAM, root));
            Ok(1)
        }
        Some(Commands::Init) => commands::init::run(&context),
        Some(Commands::Install { no_path }) => commands::install::run(&context, !no_path),
        Some(Commands::Help { befehl }) => help_command(root, befehl.as_deref()),
        #[cfg(debug_assertions)]
        Some(Commands::SelbsttestFehler { code, meldung }) => {
            commands::selbsttest::fehler(&context, &code, &meldung)
        }
    }
}

/// `harness help [befehl]`: Hilfe des Programms oder eines Unterbefehls
fn help_command(root: &Command, name: Option<&str>) -> Result<u8> {
    let Some(name) = name else {
        print!("{}", help::render(PROGRAM, root));
        return Ok(0);
    };
    match root.find_subcommand(name) {
        Some(sub) => {
            print!("{}", help::render(&format!("{PROGRAM} {name}"), sub));
            Ok(0)
        }
        None => {
            eprintln!("{}", errors::unknown_command(name, &subcommand_names(root)));
            Ok(1)
        }
    }
}

/// Namen der sichtbaren Unterbefehle (für Vorschläge)
fn subcommand_names(cmd: &Command) -> Vec<String> {
    cmd.get_subcommands()
        .filter(|sub| !sub.is_hide_set())
        .map(|sub| sub.get_name().to_string())
        .collect()
}

/// Was die Vorabsuche in den Argumenten ergeben hat: der erreichte Befehl, sein Aufrufpfad und
/// ob die Hilfe gemeint war.
struct Walk<'a> {
    path: Vec<String>,
    cmd: &'a Command,
    help: bool,
}

/// Sucht `-h`/`--help` in den rohen Argumenten und merkt sich dabei, in welchem Befehl sie stehen.
/// clap kennt die Option nicht, weil die CLI die Hilfe selbst rendert.
fn locate<'a>(root: &'a Command, args: &[String]) -> Walk<'a> {
    let mut cmd = root;
    let mut path = vec![PROGRAM.to_string()];
    let mut descending = true;
    let mut index = 0;
    while index < args.len() {
        let token = args[index].as_str();
        index += 1;
        if token == "-h" || token == "--help" {
            return Walk {
                path,
                cmd,
                help: true,
            };
        }
        if token == "--" {
            break;
        }
        if token.starts_with('-') && token.len() > 1 {
            if needs_value(cmd, token) {
                index += 1;
            }
            continue;
        }
        if !descending {
            continue;
        }
        match cmd.find_subcommand(token) {
            Some(sub) => {
                path.push(token.to_string());
                cmd = sub;
            }
            // Ein unbekannter Befehl ist ein Fehler, kein Argument: dann endet auch die Suche
            // nach der Hilfe, damit `harness gibtesnicht --help` wie bei commander scheitert.
            None if cmd.get_subcommands().next().is_some()
                && cmd.get_positionals().next().is_none() =>
            {
                return Walk {
                    path,
                    cmd,
                    help: false,
                };
            }
            None => descending = false,
        }
    }
    Walk {
        path,
        cmd,
        help: false,
    }
}

/// Braucht die Option hinter diesem Zeichen noch einen eigenen Wert?
fn needs_value(cmd: &Command, token: &str) -> bool {
    if token.contains('=') {
        return false;
    }
    cmd.get_arguments()
        .filter(|arg| !arg.is_positional())
        .find(|arg| match token.strip_prefix("--") {
            Some(long) => arg.get_long() == Some(long),
            None => token
                .chars()
                .nth(1)
                .is_some_and(|short| arg.get_short() == Some(short)),
        })
        .and_then(|arg| arg.get_num_args())
        .is_some_and(|range| range.takes_values())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baum() -> Command {
        let mut root = Cli::command();
        root.build();
        root
    }

    fn argumente(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn findet_die_hilfe_der_hauptseite() {
        let root = baum();
        let walk = locate(&root, &argumente(&["--help"]));
        assert!(walk.help);
        assert_eq!(walk.path, vec!["harness".to_string()]);
    }

    #[test]
    fn findet_die_hilfe_eines_unterbefehls() {
        let root = baum();
        let walk = locate(&root, &argumente(&["help", "-h"]));
        assert!(walk.help);
        assert_eq!(walk.path, vec!["harness".to_string(), "help".to_string()]);
    }

    #[test]
    fn uberspringt_den_wert_einer_option() {
        let root = baum();
        assert!(!locate(&root, &argumente(&["--dir", "--help"])).help);
        assert!(locate(&root, &argumente(&["--dir=x", "--help"])).help);
        assert!(locate(&root, &argumente(&["--json", "--help"])).help);
    }

    #[test]
    fn ein_unbekannter_befehl_beendet_die_suche_nach_der_hilfe() {
        let root = baum();
        assert!(!locate(&root, &argumente(&["gibtesnicht", "--help"])).help);
    }

    #[test]
    fn der_befehlsbaum_ist_gultig() {
        Cli::command().debug_assert();
    }
}
