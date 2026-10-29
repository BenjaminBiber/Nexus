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

/// Beschreibung von `customize` auf der Hauptseite der Hilfe
const CUSTOMIZE_ABOUT: &str =
    "Anpassungen des Clones (custom/) prüfen, kopieren, zurücksetzen und anwenden";

/// Zusatztext der Hilfe von `customize`: die Unterbefehle, die ihre Argumente selbst lesen
const CUSTOMIZE_AFTER: &str = "
Weitere Befehle:
  check                              alle Anpassungen prüfen, ohne etwas zu schreiben
  copy workflow|skill <name>|output-style <name> [--rebase]
                                     Standard als Ausgangspunkt nach custom/ kopieren
  reset workflow|skill <name>|output-style <name>
                                     Anpassung und Basis löschen

Beispiele:
  harness customize check
  harness customize copy skill harness-grill
  harness customize reset workflow";

/// Beschreibung von `init` (wortgleich zu `InitCommand.ts`)
const INIT_ABOUT: &str = "Richtet den Harness im aktuellen Git-Projekt ein (einmal pro Projekt, wiederholbar) und nennt das zugeordnete Board.";

/// Beschreibung von `install` (wortgleich zu `InstallCommand.ts`)
const INSTALL_ABOUT: &str = "Macht den Befehl harness im normalen Terminal verfügbar (Kopie unter ~/.harness, Eintrag im PATH des Benutzers).";

/// Beschreibung von `board` auf der Hilfeseite; wortgleich zum TypeScript-Befehl
const BOARD_ABOUT: &str = "Ohne Unterbefehl: startet bei Bedarf den Server und öffnet das Board. Mit Unterbefehl: Board-Datenbank bedienen.";

/// Hilfe der Option `--port`; die Zahl ist `DEFAULT_SERVER_PORT` aus dem Fachkern
const BOARD_PORT_HELP: &str = "Port des Servers (Standard 4710 oder HARNESS_PORT)";

/// Beschreibung von `serve` auf der Hilfeseite
const SERVE_ABOUT: &str = "Startet den Server im Vordergrund: Rust nimmt den Port an, der Node-Server läuft als Kindprozess dahinter.";

/// Beispiele unter der Hilfe von `serve`
const SERVE_EXAMPLES: &str = "\nBeispiele:\n  harness serve\n  harness serve --port 4800\n  harness serve --node-sidecar packages/server/dist/main.js\n\nBeendet wird mit Strg+C; der Node-Server geht dabei mit.";

/// Beispiele unter der Hilfe von `board`
const BOARD_EXAMPLES: &str = "\nBeispiele:\n  harness board\n  harness board --port 4800 --no-open";

/// Die Befehle des Programms. Die restlichen Fachbefehle folgen in P3-16 bis P3-18; `help` steht
/// wie bei commander in jedem Befehl, der selbst nichts tut und Unterbefehle hat.
// Die Varianten tragen die Optionen von clap; sie dürfen unterschiedlich groß sein
#[allow(clippy::large_enum_variant)]
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

    /// Ohne Unterbefehl: Server starten und das Board öffnen
    #[command(
        name = "board",
        about = BOARD_ABOUT,
        after_help = BOARD_EXAMPLES,
        disable_help_flag = true
    )]
    Board {
        #[arg(long, value_name = "nummer", help = BOARD_PORT_HELP)]
        port: Option<String>,
        #[arg(
            long = "no-open",
            help = "Browser nicht öffnen, nur die Adresse ausgeben"
        )]
        no_open: bool,
        /// Ohne Unterbefehl öffnet `board` das Board im Browser; ein Wort, das kein
        /// angemeldeter Unterbefehl ist, wird mit `command-unknown` abgewiesen.
        #[command(subcommand)]
        sub: Option<commands::board::BoardCommands>,
    },

    /// Server im Vordergrund starten (Rust vorn, Node-Server dahinter)
    #[command(
        name = "serve",
        about = SERVE_ABOUT,
        after_help = SERVE_EXAMPLES,
        disable_help_flag = true
    )]
    Serve {
        #[arg(long, value_name = "nummer", help = BOARD_PORT_HELP)]
        port: Option<String>,
        #[arg(
            long = "web-dir",
            value_name = "ordner",
            help = "Ordner der gebauten Oberfläche (Standard: web neben dem Programm oder HARNESS_WEB_DIR)"
        )]
        web_dir: Option<PathBuf>,
        #[arg(
            long = "node-sidecar",
            value_name = "pfad",
            help = "Programmdatei des Node-Servers (Standard: neben dem Programm oder NEXUS_NODE_SIDECAR)"
        )]
        node_sidecar: Option<String>,
        #[arg(
            long = "no-node",
            help = "Ohne Node-Server starten; nicht portierte Routen antworten dann mit 404"
        )]
        no_node: bool,
    },

    /// Kommentare zu Spec- und Plan-Dokumenten abarbeiten
    #[command(
        name = "spec",
        about = commands::spec::SPEC_ABOUT,
        disable_help_flag = true,
        disable_help_subcommand = true
    )]
    Spec {
        #[command(subcommand)]
        sub: commands::spec::SpecCommands,
    },

    /// Den laufenden Server beenden
    #[command(
        name = "stop",
        about = commands::stop::STOP_ABOUT,
        after_help = commands::stop::STOP_EXAMPLES,
        disable_help_flag = true
    )]
    Stop {
        #[arg(long, help = "Startet den Server auf demselben Port neu, wenn er lief")]
        restart: bool,
    },

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

    /// Hook-Ereignis von Claude Code verarbeiten
    #[command(
        name = "hook",
        about = commands::hook::about(),
        after_help = commands::hook::EXAMPLE,
        disable_help_flag = true
    )]
    Hook {
        #[arg(value_name = "ereignis")]
        ereignis: String,
    },

    /// Anpassungen des Clones prüfen, kopieren, zurücksetzen, anwenden und anzeigen
    #[command(
        name = "customize",
        about = CUSTOMIZE_ABOUT,
        after_help = CUSTOMIZE_AFTER,
        disable_help_flag = true,
        disable_help_subcommand = true
    )]
    Customize(commands::customize::CustomizeCli),

    /// Losgelöster Worker eines Laufs des Dispatchers; den startet nur der Server selbst,
    /// deshalb steht der Befehl nicht in der Hilfe.
    #[command(
        name = "dispatch-worker",
        about = "Führt einen Lauf des Dispatchers im eigenen Prozess aus",
        hide = true,
        disable_help_flag = true
    )]
    DispatchWorker {
        #[arg(value_name = "lauf", help = "ID des Laufs aus dispatch/runs")]
        lauf: String,
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
        Some(Commands::Board { port, no_open, sub }) => match sub {
            Some(sub) => commands::board::run(&context, port.as_deref(), &sub, root),
            None => commands::board::open::run(&context, port.as_deref(), !no_open),
        },
        Some(Commands::Serve {
            port,
            web_dir,
            node_sidecar,
            no_node,
        }) => commands::serve::run(
            &context,
            &commands::serve::ServeOptions {
                port,
                web_dir,
                node_sidecar,
                no_node,
            },
        ),
        Some(Commands::Spec { sub }) => commands::spec::run(&context, &sub),
        Some(Commands::Stop { restart }) => commands::stop::run(&context, restart),
        Some(Commands::Install { no_path }) => commands::install::run(&context, !no_path),
        Some(Commands::Hook { ereignis }) => commands::hook::run(&ereignis),
        Some(Commands::Customize(args)) => commands::customize::run(&context, args),
        Some(Commands::DispatchWorker { lauf }) => commands::dispatch_worker::run(&context, &lauf),
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

/// `harness <gruppe> help [befehl]`: Hilfe einer Gruppe oder eines ihrer Unterbefehle.
/// commander meldet diesen Befehl in jeder Gruppe selbst an; im clap-Baum steht er als Variante.
pub fn group_help(path: &[&str], name: Option<&str>) -> Result<u8> {
    let mut root = Cli::command();
    root.build();
    let mut cmd = &root;
    for part in path {
        cmd = cmd
            .find_subcommand(part)
            .unwrap_or_else(|| panic!("Befehl „{part}“ fehlt im Befehlsbaum"));
    }
    let aufruf = format!("{PROGRAM} {}", path.join(" "));
    let Some(name) = name else {
        print!("{}", help::render(&aufruf, cmd));
        return Ok(0);
    };
    match cmd.find_subcommand(name) {
        Some(sub) => {
            print!("{}", help::render(&format!("{aufruf} {name}"), sub));
            Ok(0)
        }
        None => {
            eprintln!("{}", errors::unknown_command(name, &subcommand_names(cmd)));
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
