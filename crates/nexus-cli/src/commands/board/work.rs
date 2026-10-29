//! `nexus board start|done`: Ticket im eigenen Worktree beginnen und abschließen.
//! Portiert aus `packages/cli/src/commands/board/BoardWorkCommands.ts`.

use clap::Args;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::docs::SpecCommitCheck;
use nexus_core::text::TextSource;
use nexus_core::worktrees::{
    FinishOptions, RepoBoard, StartOptions, WorktreeManager, WorktreeOptions,
};
use nexus_core::Result;

use crate::context::CliContext;

/// Beschreibung von `start`
pub const START_ABOUT: &str = "Startet ein Ticket: eigener Branch ticket/<ID>-<Kurztitel> (vom Zielbranch des Tickets, sonst vom ausgecheckten Branch) und Worktree neben dem Projekt, Status In Arbeit (Sperren gelten). Mit --join tritt es stattdessen einem offenen Worktree derselben Spec bei, wenn sich die Bereiche nicht überschneiden. Nur für Tickets des Boards, das diesem Repo zugeordnet ist. Warnt, wenn das Spec-Dokument des Tickets nicht (vollständig) committet ist.";

/// Beispiele unter der Hilfe von `start`
pub const START_EXAMPLES: &str = "\nBeispiele:\n  nexus board start ARC-0007 --by claude\n  nexus board start ARC-0008 --in ARC-0007\n  nexus board start ARC-0009 --join";

/// Beschreibung von `done`
pub const DONE_ABOUT: &str = "Setzt das Ticket auf Erledigt (Abnahme nötig) und meldet, ob der Worktree zum Merge bereit ist. Es wird nichts gemergt. Nur für Tickets des Boards dieses Repos.";

/// Beispiel unter der Hilfe von `done`
pub const DONE_EXAMPLE: &str = "\nBeispiel:\n  nexus board done ARC-0007 --acceptance \"Tests grün, Review erledigt\" --by claude";

/// Optionen von `board start`
#[derive(Debug, Args)]
pub struct StartArgs {
    pub id: String,
    #[arg(
        long = "in",
        value_name = "id",
        help = "dem Worktree dieses Tickets beitreten (bei Überschneidung entsteht ein eigener)"
    )]
    pub in_ticket: Option<String>,
    #[arg(
        long,
        help = "einem offenen Worktree derselben Spec ohne Überschneidung beitreten (früheres Verhalten)"
    )]
    pub join: bool,
    #[arg(
        long,
        help = "offene Voraussetzungen und Fragen übergehen (nur auf ausdrückliche Anweisung)"
    )]
    pub force: bool,
    #[arg(long, value_name = "name", help = "wer startet (z. B. claude)")]
    pub by: Option<String>,
}

/// Optionen von `board done`
#[derive(Debug, Args)]
pub struct DoneArgs {
    pub id: String,
    #[arg(
        long,
        value_name = "text",
        help = "Abnahme: woran man sieht, dass es fertig ist (falls noch keine eingetragen ist)"
    )]
    pub acceptance: Option<String>,
    #[arg(
        long = "acceptance-file",
        value_name = "datei",
        help = "Abnahme aus Datei („-“ = Standardeingabe)"
    )]
    pub acceptance_file: Option<String>,
    #[arg(
        long,
        help = "offene Voraussetzungen und Fragen übergehen (nur auf ausdrückliche Anweisung)"
    )]
    pub force: bool,
    #[arg(long, value_name = "name", help = "wer abschließt")]
    pub by: Option<String>,
}

/// Führt `board start` aus
pub fn start(context: &CliContext, args: &StartArgs) -> Result<u8> {
    let session = context.session().map(ToString::to_string);
    let join_worktrees = context.join_worktrees();
    context.with_board(async |rt| {
        let project = context.open_project()?;
        let root = project.root.clone();
        let manager = WorktreeManager::new(
            project.layout.clone(),
            rt.services.duplicate(),
            root.clone(),
            WorktreeOptions {
                join_worktrees,
                ..WorktreeOptions::default()
            },
        );
        let repo_board = repo_board(rt, &root).await?;
        let result = manager
            .start(
                &args.id,
                &StartOptions {
                    in_ticket: args.in_ticket.clone(),
                    join: args.join,
                    force: args.force,
                    session_id: session.clone(),
                    repo_board: Some(repo_board),
                    join_only_same_spec: false,
                },
                context.project_dir(),
                &rt.actor(args.by.as_deref()),
            )
            .await?;
        let printer = context.printer();
        if let Some(warning) = &result.warning {
            printer.warn(warning);
        }
        // Worktrees entstehen aus dem letzten Commit: ein nicht committetes Spec-Dokument
        // fehlt dort (nur Warnung)
        let spec = rt
            .services
            .tickets
            .get(&result.id)
            .await?
            .view
            .ticket
            .spec
            .clone();
        if let Some(spec) = spec {
            if let Some(warning) =
                SpecCommitCheck::new().warning_for_doc(std::path::Path::new(&root), &spec)
            {
                printer.warn(&warning);
            }
        }
        let mut lines = vec![format!(
            "{} {} auf Branch {} im Worktree {}",
            result.id,
            if result.joined {
                "arbeitet jetzt"
            } else {
                "läuft jetzt"
            },
            result.branch,
            result.path
        )];
        if let Some(target) = &result.target_created {
            lines.push(target.clone());
        }
        for path in &result.removed {
            lines.push(format!(
                "Worktree {path} aufgeräumt (der Branch bleibt erhalten)."
            ));
        }
        printer.result(&result, &lines);
        Ok(0)
    })
}

/// Führt `board done` aus
pub fn done(context: &CliContext, args: &DoneArgs) -> Result<u8> {
    let join_worktrees = context.join_worktrees();
    context.with_board(async |rt| {
        let acceptance =
            TextSource::read(args.acceptance.as_deref(), args.acceptance_file.as_deref())?;
        let project = context.open_project()?;
        let root = project.root.clone();
        let manager = WorktreeManager::new(
            project.layout.clone(),
            rt.services.duplicate(),
            root.clone(),
            WorktreeOptions {
                join_worktrees,
                ..WorktreeOptions::default()
            },
        );
        let repo_board = repo_board(rt, &root).await?;
        let result = manager
            .finish(
                &args.id,
                &FinishOptions {
                    acceptance,
                    force: args.force,
                    repo_board: Some(repo_board),
                },
                context.project_dir(),
                &rt.actor(args.by.as_deref()),
            )
            .await?;
        let printer = context.printer();
        for warning in &result.warnings {
            printer.warn(warning);
        }
        let mut lines = vec![format!("{} ist erledigt.", result.id)];
        if result.ready_to_merge {
            if let Some(branch) = &result.branch {
                lines.push(format!(
                    "Der Branch {branch} ist bereit zum Merge (es wurde nichts gemergt)."
                ));
            }
        }
        for path in &result.removed {
            lines.push(format!(
                "Worktree {path} aufgeräumt (der Branch bleibt erhalten)."
            ));
        }
        printer.result(&result, &lines);
        Ok(0)
    })
}

/// Board, das dem Repo zugeordnet ist (ohne Zuordnung ein Fehler mit Hinweis)
async fn repo_board(rt: &nexus_db::BoardRuntime, root: &str) -> Result<RepoBoard> {
    let me = RepoBoardOwner {
        user_id: rt.config.user_id.clone(),
        active_board: rt.config.active_board,
    };
    let board = rt.services.boards.require_for_repo(root, &me).await?;
    Ok(RepoBoard {
        id: board.board.id,
        name: board.board.name.clone(),
    })
}
