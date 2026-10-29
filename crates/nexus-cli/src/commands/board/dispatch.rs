//! `nexus board dispatch …`, `board dispatch-runs backfill`, `board plan-start` und
//! `board question research-start`: den Dispatcher im lokalen Server steuern und seine Läufe
//! nachtragen.
//! Portiert aus `packages/cli/src/commands/board/BoardDispatchCommands.ts` (ohne `merge` und
//! `accept`, die mit P3-11 dazukommen).

use clap::{Args, Subcommand};
use nexus_core::{HarnessError, Result};

use crate::board::{BoardSelection, DispatchOptionParser};
use crate::context::CliContext;
use crate::output::DispatchStatusFormatter;
use crate::server::DispatchClient;

/// Umgebungsvariable mit dem Port des Servers
const PORT_ENV: &str = "HARNESS_PORT";

/// Beschreibung der Gruppe `dispatch`
pub const DISPATCH_ABOUT: &str = "Dispatcher: übernimmt bereite Tickets (Label auto) der eigenen Boards und lässt sie von claude -p umsetzen. Einstellungen in der Oberfläche oder ~/.harness/dispatch.yaml.";

/// Beschreibung von `dispatch status`
pub const STATUS_ABOUT: &str = "Zeigt Zustand, laufende Worker, Merges und warum Geplant-Tickets (noch) nicht bereit sind. Ändert nichts.";
/// Beispiel unter der Hilfe von `dispatch status`
pub const STATUS_EXAMPLE: &str = "\nBeispiel:\n  harness board dispatch status --board 3";

/// Beschreibung von `dispatch run`
pub const RUN_ABOUT: &str = "Führt sofort einen Durchlauf aus (sonst jede Minute).";
/// Beispiel unter der Hilfe von `dispatch run`
pub const RUN_EXAMPLE: &str = "\nBeispiel:\n  harness board dispatch run";

/// Beschreibung von `dispatch pause`
pub const PAUSE_ABOUT: &str =
    "Startet für eine Weile nichts Neues (laufende Worker laufen weiter).";
/// Beispiel unter der Hilfe von `dispatch pause`
pub const PAUSE_EXAMPLE: &str = "\nBeispiel:\n  harness board dispatch pause --minutes 30";

/// Beschreibung von `dispatch resume`
pub const RESUME_ABOUT: &str = "Hebt eine Pause auf.";
/// Beispiel unter der Hilfe von `dispatch resume`
pub const RESUME_EXAMPLE: &str = "\nBeispiel:\n  harness board dispatch resume";

/// Beschreibung von `dispatch enable`
pub const ENABLE_ABOUT: &str = "Schaltet den Dispatcher ein.";
/// Beispiel unter der Hilfe von `dispatch enable`
pub const ENABLE_EXAMPLE: &str = "\nBeispiel:\n  harness board dispatch enable";

/// Beschreibung von `dispatch disable`
pub const DISABLE_ABOUT: &str = "Schaltet den Dispatcher aus (laufende Worker laufen zu Ende).";
/// Beispiel unter der Hilfe von `dispatch disable`
pub const DISABLE_EXAMPLE: &str = "\nBeispiel:\n  harness board dispatch disable";

/// Beschreibung der Gruppe `dispatch-runs`
pub const RUNS_ABOUT: &str = "Läufe des Dispatchers für die Auswertung (Dashboard).";

/// Beschreibung von `dispatch-runs backfill`
pub const BACKFILL_ABOUT: &str = "Trägt Läufe aus den [dispatch]-Kommentaren nach (Profil, Modell-Alias, Effort, Versuch, Eskalation, Abbruchgrund; ohne Kosten). Schon nachgetragene Läufe bleiben unverändert – beliebig oft ausführbar.";
/// Beispiele unter der Hilfe von `dispatch-runs backfill`
pub const BACKFILL_EXAMPLES: &str = "\nBeispiele:\n  harness board dispatch-runs backfill --project ARC --dry-run\n  harness board dispatch-runs backfill";

/// Beschreibung von `plan-start`
pub const PLAN_START_ABOUT: &str = "Lässt ein Ticket eines eigenen Boards sofort von einer Claude-Session planen (claude -p, nur lesend, eigener losgelöster Worktree; wie „Planen“ im Ticket). Der Plan landet über plan-json am Ticket; fehlt eine Entscheidung, entsteht eine Frage. Läuft über den lokalen Server, unabhängig davon, ob der Dispatcher eingeschaltet ist.";
/// Beispiel unter der Hilfe von `plan-start`
pub const PLAN_START_EXAMPLE: &str = "\nBeispiel:\n  harness board plan-start ARC-0007";

/// Beschreibung der Gruppe `question` (wortgleich zu `BoardQuestionCommands.ts`)
pub const QUESTION_ABOUT: &str =
    "Fragen auf der Board-Datenbank: fragen, recherchieren, entscheiden (entscheiden nur Menschen).";

/// Beschreibung von `question research-start`
pub const RESEARCH_START_ABOUT: &str = "Lässt eine offene Frage sofort von einer Claude-Session (claude -p, nur lesend, eigener losgelöster Worktree) recherchieren; das Ergebnis landet als Recherche an der Frage. Läuft über den lokalen Server, unabhängig davon, ob der Dispatcher eingeschaltet ist.";
/// Beispiel unter der Hilfe von `question research-start`
pub const RESEARCH_START_EXAMPLE: &str =
    "\nBeispiel:\n  harness board question research-start ARC-F0007";

/// Option `--board` von `dispatch status`
#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(
        long,
        value_name = "id",
        help = "Board für die Bereitschaft (Standard: aktives Board)"
    )]
    pub board: Option<String>,
}

/// Option `--board` von `dispatch run`
#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(
        long,
        value_name = "id",
        help = "Board für die anschließende Übersicht"
    )]
    pub board: Option<String>,
}

/// Option `--minutes` von `dispatch pause`
#[derive(Debug, Args)]
pub struct PauseArgs {
    #[arg(
        long,
        value_name = "n",
        default_value = "60",
        help = "Dauer in Minuten (1 bis 1440)"
    )]
    pub minutes: String,
}

/// Optionen von `dispatch-runs backfill`
#[derive(Debug, Args)]
pub struct BackfillArgs {
    #[arg(
        long,
        value_name = "key",
        help = "Projekt (Standard: Projekt des Boards dieses Repos, sonst des aktiven Boards)"
    )]
    pub project: Option<String>,
    #[arg(long = "dry-run", help = "nur zählen, nichts schreiben")]
    pub dry_run: bool,
}

/// Das Ticket bzw. die Frage, die gestartet werden soll
#[derive(Debug, Args)]
pub struct IdArgs {
    pub id: String,
}

/// Die Unterbefehle von `board dispatch`
#[derive(Debug, Subcommand)]
pub enum DispatchCommands {
    #[command(
        name = "status",
        about = STATUS_ABOUT,
        after_help = STATUS_EXAMPLE,
        disable_help_flag = true
    )]
    Status(StatusArgs),

    #[command(
        name = "run",
        about = RUN_ABOUT,
        after_help = RUN_EXAMPLE,
        disable_help_flag = true
    )]
    Run(RunArgs),

    #[command(
        name = "pause",
        about = PAUSE_ABOUT,
        after_help = PAUSE_EXAMPLE,
        disable_help_flag = true
    )]
    Pause(PauseArgs),

    #[command(
        name = "resume",
        about = RESUME_ABOUT,
        after_help = RESUME_EXAMPLE,
        disable_help_flag = true
    )]
    Resume,

    #[command(
        name = "enable",
        about = ENABLE_ABOUT,
        after_help = ENABLE_EXAMPLE,
        disable_help_flag = true
    )]
    Enable,

    #[command(
        name = "disable",
        about = DISABLE_ABOUT,
        after_help = DISABLE_EXAMPLE,
        disable_help_flag = true
    )]
    Disable,

    #[command(
        name = "help",
        about = "Hilfe zu einem Befehl anzeigen",
        disable_help_flag = true
    )]
    Help { befehl: Option<String> },
}

/// Die Unterbefehle von `board dispatch-runs`
#[derive(Debug, Subcommand)]
pub enum RunsCommands {
    #[command(
        name = "backfill",
        about = BACKFILL_ABOUT,
        after_help = BACKFILL_EXAMPLES,
        disable_help_flag = true
    )]
    Backfill(BackfillArgs),

    #[command(
        name = "help",
        about = "Hilfe zu einem Befehl anzeigen",
        disable_help_flag = true
    )]
    Help { befehl: Option<String> },
}

/// Die Unterbefehle von `board question`; die übrigen kommen mit P3-14 dazu
#[derive(Debug, Subcommand)]
pub enum QuestionCommands {
    #[command(
        name = "research-start",
        about = RESEARCH_START_ABOUT,
        after_help = RESEARCH_START_EXAMPLE,
        disable_help_flag = true
    )]
    ResearchStart(IdArgs),

    #[command(
        name = "help",
        about = "Hilfe zu einem Befehl anzeigen",
        disable_help_flag = true
    )]
    Help { befehl: Option<String> },
}

/// Führt einen Unterbefehl von `board dispatch` aus
pub fn dispatch(
    context: &CliContext,
    port: Option<&str>,
    command: &DispatchCommands,
) -> Result<u8> {
    let parser = DispatchOptionParser;
    match command {
        DispatchCommands::Status(args) => {
            let board = parser.board_id(args.board.as_deref())?;
            show(context, port, |client| client.status(board))
        }
        DispatchCommands::Run(args) => {
            let board = parser.board_id(args.board.as_deref())?;
            show(context, port, |client| client.run(board))
        }
        DispatchCommands::Pause(args) => {
            let minutes = parser.minutes(&args.minutes)?;
            show(context, port, |client| client.pause(minutes))
        }
        DispatchCommands::Resume => show(context, port, DispatchClient::resume),
        DispatchCommands::Enable => show(context, port, |client| client.set_enabled(true)),
        DispatchCommands::Disable => show(context, port, |client| client.set_enabled(false)),
        DispatchCommands::Help { befehl } => {
            crate::cli::group_help(&["board", "dispatch"], befehl.as_deref())
        }
    }
}

/// Führt einen Unterbefehl von `board dispatch-runs` aus
pub fn runs(context: &CliContext, command: &RunsCommands) -> Result<u8> {
    match command {
        RunsCommands::Backfill(args) => backfill(context, args),
        RunsCommands::Help { befehl } => {
            crate::cli::group_help(&["board", "dispatch-runs"], befehl.as_deref())
        }
    }
}

/// Führt einen Unterbefehl von `board question` aus
pub fn question(
    context: &CliContext,
    port: Option<&str>,
    command: &QuestionCommands,
) -> Result<u8> {
    match command {
        QuestionCommands::ResearchStart(args) => research_start(context, port, &args.id),
        QuestionCommands::Help { befehl } => {
            crate::cli::group_help(&["board", "question"], befehl.as_deref())
        }
    }
}

/// `board plan-start <id>`: Planung über den Server starten
pub fn plan_start(context: &CliContext, port: Option<&str>, id: &str) -> Result<u8> {
    let view = client(port)?.start_planning(id)?;
    let lines = DispatchStatusFormatter.plan_lines(&DispatchStatusFormatter::view(&view));
    context.printer().result(&view, &lines);
    Ok(0)
}

/// `board question research-start <id>`: Recherche über den Server starten
fn research_start(context: &CliContext, port: Option<&str>, id: &str) -> Result<u8> {
    let view = client(port)?.start_research(id)?;
    let lines = DispatchStatusFormatter.research_lines(&DispatchStatusFormatter::view(&view));
    context.printer().result(&view, &lines);
    Ok(0)
}

/// `board dispatch-runs backfill`: Läufe aus Kommentaren direkt in der Board-Datenbank nachtragen
fn backfill(context: &CliContext, args: &BackfillArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let projects = rt.services.projects.list().await?;
        let project_id = match &args.project {
            Some(key) => {
                projects
                    .iter()
                    .find(|project| project.key.to_uppercase() == key.to_uppercase())
                    .ok_or_else(|| {
                        HarnessError::new(
                            "project-not-found",
                            format!("Projekt {key} gibt es nicht."),
                        )
                    })?
                    .id
            }
            None => {
                let board = BoardSelection
                    .one(&rt.services, &rt.config, None, Some(context.project_dir()))
                    .await?;
                rt.services.boards.require(board).await?.board.project_id
            }
        };
        let key = projects
            .iter()
            .find(|project| project.id == project_id)
            .map(|project| project.key.clone())
            .unwrap_or_else(|| project_id.to_string());
        let result = rt
            .services
            .dispatch_runs
            .backfill(project_id, args.dry_run)
            .await?;
        let mut lines = vec![format!(
            "Projekt {key}: {} Läufe in den [dispatch]-Kommentaren gefunden, {} {}.",
            result.found,
            result.created.len(),
            if args.dry_run {
                "würden neu angelegt (Trockenlauf)"
            } else {
                "neu angelegt"
            }
        )];
        for run in &result.created {
            let attempt = match run.attempt {
                Some(attempt) => attempt.to_string(),
                None => "-".to_string(),
            };
            let escalated = match &run.escalated_from {
                Some(from) => format!(" (eskaliert von {from})"),
                None => String::new(),
            };
            let category = match &run.abort_category {
                Some(category) => format!(" ({category})"),
                None => String::new(),
            };
            lines.push(format!(
                "  {}  Versuch {attempt}  {}{escalated}  {}{category}",
                run.subject_id, run.profile, run.outcome
            ));
        }
        context.printer().result(&result, &lines);
        Ok(0)
    })
}

/// Führt den Aufruf aus und gibt die Übersicht aus
fn show<F>(context: &CliContext, port: Option<&str>, call: F) -> Result<u8>
where
    F: FnOnce(&DispatchClient) -> Result<serde_json::Value>,
{
    let status = call(&client(port)?)?;
    let lines = DispatchStatusFormatter.lines(&DispatchStatusFormatter::view(&status));
    context.printer().result(&status, &lines);
    Ok(0)
}

/// Client für den Port aus `--port` von `board`, sonst `HARNESS_PORT`, sonst Standard
fn client(port: Option<&str>) -> Result<DispatchClient> {
    let env = std::env::var(PORT_ENV).ok().filter(|raw| !raw.is_empty());
    Ok(DispatchClient::new(
        DispatchOptionParser.port(port, env.as_deref())?,
    ))
}
