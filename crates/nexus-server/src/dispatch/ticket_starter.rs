//! Übernimmt ein bereites Ticket für den Junior-Agent, richtet den Worktree ein und startet
//! den Worker; portiert aus `packages/server/src/dispatch/TicketStarter.ts`.

use std::path::Path;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::{ClaimOptions, MoveOptions, TicketDetail, TicketView};
use nexus_core::dispatch::{
    markers, ProfileTicket, ReadinessBoard, ReadinessTicket, RunKind, RunRecord, RunSettings,
    WorktreeSetupStep,
};
use nexus_core::git::runner::{GitCommands, GitRunner};
use nexus_core::worktrees::manager::{ResumeResult, StartOptions};
use nexus_core::worktrees::registry::SetupState;
use nexus_core::Result;

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::context::{DispatchContext, LaunchInput, RepoWork};
use crate::dispatch::prompt_values::PromptValues;
use crate::dispatch::result_handler::ResultHandler;
use crate::dispatch::run_recorder::{RunRecorder, StartFailed};

/// Ausgang eines Startversuchs; der Lauf liegt im Kasten, weil er die beiden anderen
/// Varianten um ein Vielfaches überwiegt
#[derive(Debug, Clone)]
pub enum StartOutcome {
    /// Der Lauf ist gestartet
    Started(Box<RunRecord>),
    /// Das Ticket bleibt liegen; der Text nennt den Grund
    Skipped(String),
    /// Der Start ist nach der Übernahme gescheitert; der Text nennt den Fehler
    Error(String),
}

/// Gestarteter Lauf; `target_created` meldet einen beim Start neu angelegten Zielbranch
#[derive(Debug, Clone)]
struct Launched {
    record: RunRecord,
    branch: String,
    cwd: String,
    warning: Option<String>,
    target_created: Option<String>,
}

/// Startet bereite Tickets.
#[derive(Debug)]
pub struct TicketStarter<'a> {
    ctx: &'a DispatchContext,
    results: &'a ResultHandler<'a>,
    git: GitRunner,
}

impl<'a> TicketStarter<'a> {
    /// Starter auf diesem Kontext
    pub fn new(ctx: &'a DispatchContext, results: &'a ResultHandler<'a>) -> Self {
        Self {
            ctx,
            results,
            git: GitRunner::new(),
        }
    }

    /// Frisch lesen und erneut prüfen (auch: Worktree frei) → claim (nur unveränderte Version)
    /// → Worktree (neu oder vorhanden) → Lauf. Scheitert der Start nach dem claim, zählt das
    /// als Abbruch-Versuch; nach dem Start nie mehr zurück.
    pub async fn start(&self, view: &TicketView, work: &RepoWork) -> Result<StartOutcome> {
        let ctx = self.ctx;
        let detail = ctx.services().tickets.get(&view.ticket.id).await?;
        let board = ReadinessBoard {
            owner_id: work.board.owner_id.clone(),
            repo_path: work.board.repo_path.clone(),
        };
        let exists = |path: &str| Path::new(path).exists();
        if let Some(not_ready) = ctx.readiness().reason(
            &ReadinessTicket::from(&detail.view),
            &board,
            &ctx.me.user_id,
            &exists,
        ) {
            return Ok(StartOutcome::Skipped(not_ready));
        }
        // Gehört der eingetragene Worktree nicht mehr zum Ticket, startet es neu (eigener
        // Branch) statt ihn wiederherzustellen
        let stale = work
            .manager
            .stale_reason(&detail.ticket.id, detail.ticket.branch.as_deref())?;
        if let Some(branch) = detail.ticket.branch.as_deref() {
            if stale.is_none()
                && !self
                    .git
                    .try_run(
                        Path::new(&work.root),
                        &[
                            "rev-parse",
                            "--verify",
                            "--quiet",
                            &format!("refs/heads/{branch}"),
                        ],
                    )
                    .ok
            {
                return Ok(StartOutcome::Skipped(format!(
                    "Branch {branch} gibt es auf diesem Rechner nicht"
                )));
            }
        }
        // Läuft im (künftigen) Worktree schon ein Worker, wartet das Ticket ungeclaimt auf den
        // nächsten Durchlauf
        if let Some(busy) = ctx
            .worktree_busy(&detail.ticket.id, detail.ticket.branch.as_deref(), work)
            .await?
        {
            return Ok(StartOutcome::Skipped(busy));
        }
        let attempt = ctx.attempts().failed(&detail.comments) as i64 + 1;
        let run = ctx
            .chooser()
            .choose(&ProfileTicket::from(&detail.ticket), attempt);
        let agent = ctx.agent_name(&view.ticket.id);
        let claimed = ctx
            .services()
            .tickets
            .claim(
                &view.ticket.id,
                &agent,
                ctx.actor(),
                &ClaimOptions {
                    status: Some(TicketStatus::Planned),
                    rev: Some(detail.ticket.rev),
                    ..ClaimOptions::default()
                },
            )
            .await;
        if let Err(error) = claimed {
            // Nur fachliche Fehler (Version geändert, schon vergeben) überspringen das Ticket;
            // ein Verbindungsfehler bricht den Durchlauf ab
            if BoardRuntimeHolder::is_connection_error(&error) {
                return Err(error);
            }
            return Ok(StartOutcome::Skipped(error.to_string()));
        }
        let launched = match self
            .launch(view, work, &detail, &agent, &run, attempt)
            .await
        {
            Ok(launched) => launched,
            Err(error) => {
                return self
                    .failed(
                        &detail,
                        &error.to_string(),
                        StartFailed {
                            ticket_id: detail.ticket.id.clone(),
                            project_id: work.board.project_id,
                            board_id: work.board.id,
                            agent,
                            run,
                            attempt,
                            reason: String::new(),
                            final_attempt: false,
                        },
                    )
                    .await
            }
        };
        let comment = format!(
            "{}: an {agent} übergeben – Branch {}, Profil {} (Modell {}, Effort {}, max. {} \
Schritte, {} min). Grund: {}.{}",
            markers::START,
            launched.branch,
            launched.record.run.profile,
            launched.record.run.model.as_deref().unwrap_or("Standard"),
            launched.record.run.effort.as_deref().unwrap_or("Standard"),
            launched.record.run.max_turns,
            launched.record.run.timeout_minutes,
            launched.record.run.why,
            launched
                .warning
                .as_deref()
                .map(|warning| format!("\nHinweis: {warning}"))
                .unwrap_or_default(),
        );
        match ctx.comment(&view.ticket.id, &comment).await {
            Ok(()) => ctx.log(&format!(
                "gestartet {} → {agent} in {} [{}: {}/{}]",
                view.ticket.id,
                launched.cwd,
                launched.record.run.profile,
                launched.record.run.model.as_deref().unwrap_or("-"),
                launched.record.run.effort.as_deref().unwrap_or("-"),
            )),
            // Der Worker läuft bereits: kein Zurückgeben, sonst startet der nächste Durchlauf
            // ein zweites Mal
            Err(error) => ctx.log(&format!(
                "FEHLER {}: Start-Kommentar nicht geschrieben: {error}",
                view.ticket.id
            )),
        }
        // Neu angelegter Zielbranch: eigener Kommentar mit dem Merge-Marker (kein zweites
        // Start-Ereignis), Fehler nur im Log
        if let Some(target) = &launched.target_created {
            let body = format!("{}: {target}", markers::MERGE);
            if let Err(error) = ctx.comment(&view.ticket.id, &body).await {
                ctx.log(&format!(
                    "FEHLER {}: Kommentar zum Zielbranch nicht geschrieben: {error}",
                    view.ticket.id
                ));
            }
        }
        Ok(StartOutcome::Started(Box::new(launched.record)))
    }

    /// Worktree (neu oder vorhanden) und Lauf; Warnungen (z. B. nicht wiederhergestellter
    /// Worktree) kommen in den Start-Kommentar, ein neu angelegter Zielbranch in einen eigenen
    async fn launch(
        &self,
        view: &TicketView,
        work: &RepoWork,
        detail: &TicketDetail,
        agent: &str,
        run: &RunSettings,
        attempt: i64,
    ) -> Result<Launched> {
        let ctx = self.ctx;
        let mut warnings: Vec<String> = Vec::new();
        let mut created = false;
        let mut target_created = None;
        let resumed = match detail.ticket.branch.as_deref() {
            Some(_) => Some(work.manager.resume(&view.ticket.id, ctx.actor()).await?),
            None => None,
        };
        let (branch, cwd) = match resumed {
            // Fortsetzung nach Frage oder Abbruch: vorhandener Branch, Worktree notfalls neu
            Some(ResumeResult::Path(path)) => {
                ctx.services()
                    .tickets
                    .move_to(
                        &view.ticket.id,
                        TicketStatus::InProgress,
                        ctx.actor(),
                        &MoveOptions::default(),
                    )
                    .await?;
                let branch = detail
                    .ticket
                    .branch
                    .clone()
                    .expect("Fortsetzung nur mit Branch");
                (branch, path)
            }
            other => {
                if let Some(ResumeResult::Reset(reset)) = other {
                    warnings.push(reset);
                }
                // Ein Worktree je Ticket: eigener Branch vom Zielbranch des Tickets bzw. vom
                // ausgecheckten Branch des Hauptordners; gebündelt: Beitritt derselben Spec
                let join = ctx.cfg.join_worktrees == nexus_core::dispatch::JoinWorktrees::SameSpec;
                let started = work
                    .manager
                    .start(
                        &view.ticket.id,
                        &StartOptions {
                            join,
                            join_only_same_spec: true,
                            repo_board: Some(nexus_core::worktrees::manager::RepoBoard {
                                id: work.board.id,
                                name: work.board.name.clone(),
                            }),
                            ..StartOptions::default()
                        },
                        Path::new(&work.root),
                        ctx.actor(),
                    )
                    .await?;
                if let Some(warning) = started.warning {
                    warnings.push(warning);
                }
                created = !started.joined;
                target_created = started.target_created;
                (started.branch, started.path)
            }
        };
        // Einrichtung (worktreeSetup) nur für einen neu angelegten Worktree – oder wenn sie dort
        // noch aussteht (gescheitert, abgebrochen)
        let steps = &ctx.cfg.worktree_setup;
        if !steps.is_empty() && created {
            work.manager.mark_setup(&branch, SetupState::Pending)?;
        }
        let setup: Option<Vec<WorktreeSetupStep>> =
            if !steps.is_empty() && (created || work.manager.setup_pending(&branch)?) {
                Some(steps.clone())
            } else {
                None
            };
        let values = PromptValues::new(ctx);
        let feedback = values.pending(&detail.comments);
        let answers = values.answers(&view.ticket.id).await?;
        let prompt = ctx.prompts.work(&nexus_core::dispatch::WorkPromptInput {
            ticket_id: &view.ticket.id,
            title: &view.ticket.title,
            agent,
            branch: &branch,
            answers: &answers,
            feedback: &feedback,
            cfg: &ctx.cfg,
        });
        let record = ctx
            .launch(LaunchInput {
                attempt: Some(attempt),
                setup,
                ..LaunchInput::new(
                    RunKind::Work,
                    &view.ticket.id,
                    work.board.id,
                    &view.ticket.title,
                    agent,
                    &cwd,
                    &branch,
                    run.clone(),
                    prompt,
                )
            })
            .await?;
        Ok(Launched {
            record,
            branch,
            cwd,
            warning: Some(warnings.join(" ")).filter(|text| !text.is_empty()),
            target_created,
        })
    }

    /// Start gescheitert: zählt als Abbruch-Versuch (zurück nach Geplant, beim letzten Versuch
    /// Blockiert) und kommt als Lauf mit Kategorie `start-failed` in die Auswertung
    async fn failed(
        &self,
        detail: &TicketDetail,
        message: &str,
        launch: StartFailed,
    ) -> Result<StartOutcome> {
        let ctx = self.ctx;
        ctx.log(&format!("FEHLER {}: {message}", detail.ticket.id));
        let reason = format!("Worker konnte nicht gestartet werden: {message}");
        match self.results.abort(detail, &reason, "", None).await {
            Ok(result) => {
                RunRecorder::new(ctx)
                    .keep_start_failed(&StartFailed {
                        reason,
                        final_attempt: result
                            .abort
                            .as_ref()
                            .is_some_and(|abort| abort.final_attempt),
                        ..launch
                    })
                    .await;
            }
            Err(error) => {
                if BoardRuntimeHolder::is_connection_error(&error) {
                    return Err(error);
                }
                ctx.log(&format!(
                    "FEHLER {}: Zurückgeben nach gescheitertem Start: {error}",
                    detail.ticket.id
                ));
            }
        }
        Ok(StartOutcome::Error(message.to_string()))
    }
}
