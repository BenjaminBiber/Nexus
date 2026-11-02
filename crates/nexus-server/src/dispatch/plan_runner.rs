//! Planungsläufe an Tickets („Planen“ im Ticket-Panel); portiert aus
//! `packages/server/src/dispatch/PlanRunner.ts`.
//!
//! Eigenes Board mit Repo prüfen, Planungs-Claim setzen, losgelösten Worktree anlegen,
//! `claude -p` nur lesend starten, danach auswerten (Plan gespeichert? Frage angelegt? sonst
//! Kommentar am Ticket) und den Worktree immer wieder entfernen.

use std::path::Path;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::planning_candidate::PlanningCandidate;
use nexus_core::board::ticket_service::{TicketDetail, TicketListFilter, TicketView};
use nexus_core::board::types::Board;
use nexus_core::clock::{iso, parse_millis};
use nexus_core::dispatch::git::{ResearchWorktree, ResearchWorktreeOptions, WorktreePurpose};
use nexus_core::dispatch::{markers, PlanPromptInput, RunEnd, RunKind, RunRecord};
use nexus_core::{HarnessError, Result};

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::context::{DispatchContext, LaunchInput};
use crate::dispatch::notifier::{DispatchNotice, DispatchNoticeKind};
use crate::dispatch::research_runner::{file_name, hhmm, StartMode};

/// Ergebnis der Auswertung eines Planungslaufs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanOutcome {
    Planned,
    Question,
    Empty,
    Limit,
    Gone,
}

impl PlanOutcome {
    /// Name des Ausgangs wie in TypeScript
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanOutcome::Planned => "planned",
            PlanOutcome::Question => "question",
            PlanOutcome::Empty => "empty",
            PlanOutcome::Limit => "limit",
            PlanOutcome::Gone => "gone",
        }
    }
}

/// Bericht des automatischen Anstoßes eines Ticks
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanAutoReport {
    pub started: Vec<String>,
    pub errors: Vec<String>,
}

/// Fehler, die am Ticket selbst liegen: die Automatik merkt das Ticket als erledigt und versucht
/// es nicht wieder
const TICKET_ERRORS: [&str; 6] = [
    "plan-no-repo",
    "plan-not-possible",
    "plan-waiting-question",
    "plan-claimed",
    "plan-running",
    "ticket-not-found",
];
/// Nach einem Umgebungs- oder Repo-Fehler setzt die Automatik so lange aus
pub const PLANNING_RETRY_MINUTES: i64 = 30;
/// Status, in denen ein Ticket nicht (mehr) geplant wird
const NOT_PLANNABLE: [TicketStatus; 3] = [
    TicketStatus::InProgress,
    TicketStatus::Done,
    TicketStatus::NotNeeded,
];
/// Spielraum zwischen der Uhr des Dispatchers (Start des Laufs) und der Uhr der Datenbank
const CLOCK_SKEW_MS: i64 = 60_000;
/// So viele Zeichen Ausgabe stehen in einem Kommentar
const TAIL: usize = 1500;

/// Startet und wertet Planungsläufe aus.
#[derive(Debug)]
pub struct PlanRunner<'a> {
    ctx: &'a DispatchContext,
    worktrees: ResearchWorktree,
}

impl<'a> PlanRunner<'a> {
    /// Läufe auf diesem Kontext
    pub fn new(ctx: &'a DispatchContext) -> Self {
        Self {
            ctx,
            worktrees: ResearchWorktree::with_options(
                WorktreePurpose::Plan,
                ResearchWorktreeOptions {
                    home: Some(ctx.home.clone()),
                    ..ResearchWorktreeOptions::default()
                },
            ),
        }
    }

    /// Name des Planungs-Agents eines Tickets
    pub fn agent_name(ticket_id: &str) -> String {
        format!("planner-{ticket_id}")
    }

    /// Startet die Planung eines Tickets (von Hand oder automatisch); Fehler als [`HarnessError`]
    pub async fn start(&self, ticket_id: &str, mode: StartMode) -> Result<RunRecord> {
        let ctx = self.ctx;
        let ticket = ctx.services().tickets.get(ticket_id).await?;
        let board = self.check(&ticket).await?;
        let id = ticket.ticket.id.clone();
        let agent = Self::agent_name(&id);
        let work = ctx.repo(&board).await?;
        let run = ctx.chooser().planning();
        ctx.services()
            .planning
            .claim(&id, &agent, ctx.actor())
            .await?;
        let worktree = match self.worktrees.create(Path::new(&work.root), &id) {
            Ok(worktree) => worktree,
            Err(error) => {
                self.release(&id, &agent).await;
                return Err(error);
            }
        };
        if let Some(warning) = &worktree.warning {
            ctx.log(&format!("Hinweis {id}: {warning}"));
        }
        let prompt = ctx.prompts.plan(&PlanPromptInput {
            ticket_id: &id,
            title: &ticket.ticket.title,
            agent: &agent,
            cfg: &ctx.cfg,
        });
        let launched = ctx
            .launch(LaunchInput {
                planned_before: Some(ticket.ticket.planned_at.clone()),
                repo_root: Some(work.root.clone()),
                ..LaunchInput::new(
                    RunKind::Plan,
                    &id,
                    board.id,
                    &ticket.ticket.title,
                    &agent,
                    &worktree.path,
                    "",
                    run.clone(),
                    prompt,
                )
            })
            .await;
        let record = match launched {
            Ok(record) => record,
            Err(error) => {
                if let Some(left) = self.worktrees.remove(Path::new(&work.root), &worktree.path) {
                    ctx.log(&format!("FEHLER {id}: {left}"));
                }
                self.release(&id, &agent).await;
                return Err(error);
            }
        };
        let at = ctx.now_iso();
        let entry_id = id.clone();
        ctx.stores
            .state
            .update(move |state| {
                let before = state.planning.get(&entry_id).cloned();
                state.planning.insert(
                    entry_id.clone(),
                    nexus_core::dispatch::PlanningEntry {
                        ticket_id: entry_id,
                        last_started_at: at.clone(),
                        count: before.as_ref().map(|entry| entry.count).unwrap_or(0) + 1,
                        auto_at: match mode {
                            StartMode::Auto => Some(at),
                            StartMode::Manual => before.and_then(|entry| entry.auto_at),
                        },
                    },
                );
            })
            .await?;
        ctx.log(&format!(
            "Planung gestartet {id} ({}) → {agent} in {} [{}: {}/{}]",
            mode.label(),
            worktree.path,
            run.profile,
            run.model.as_deref().unwrap_or("-"),
            run.effort.as_deref().unwrap_or("-"),
        ));
        Ok(record)
    }

    /// Stößt Tickets mit Planungsbedarf auf eigenen Boards mit Repo-Pfad an (dazugekommen seit dem
    /// Einschalten der Automatik, Priorität und Nummer zuerst, je Ticket höchstens einmal), solange
    /// im WIP der Tickets Platz ist, bis `planning.wip` gleichzeitige Läufe. Fehler des Tickets
    /// (kein Repo, offene Frage, fremder Claim) merken es als erledigt; Umgebungs- und Repo-Fehler
    /// brechen ab, ohne zu merken, und setzen die Automatik [`PLANNING_RETRY_MINUTES`] aus (je
    /// Fehlertext ein Bericht)
    pub async fn start_auto(&self, free_wip_slots: i64) -> Result<PlanAutoReport> {
        let ctx = self.ctx;
        let mut report = PlanAutoReport::default();
        let now = ctx.now();
        let state = ctx.stores.state.read()?;
        let auto = state.planning_auto.clone();
        let Some(since) = auto.since.clone() else {
            return Ok(report);
        };
        if free_wip_slots <= 0
            || auto
                .retry_at
                .as_deref()
                .and_then(parse_millis)
                .is_some_and(|at| at > now.timestamp_millis())
        {
            return Ok(report);
        }
        let mut free = ctx.cfg.planning.wip
            - ctx
                .stores
                .runs
                .list()?
                .iter()
                .filter(|run| run.kind == RunKind::Plan)
                .count() as i64;
        if free <= 0 {
            return Ok(report);
        }
        for ticket in self.candidates(&since, now.timestamp_millis()).await? {
            if free <= 0 {
                break;
            }
            let id = ticket.ticket.id.clone();
            // Schon einmal angestoßen (automatisch oder von Hand): kein automatischer Neuversuch
            if state.planning.contains_key(&id) {
                continue;
            }
            match self.start(&id, StartMode::Auto).await {
                Ok(_) => {
                    report.started.push(id);
                    free -= 1;
                    ctx.stores
                        .state
                        .update(|state| {
                            state.planning_auto.retry_at = None;
                            state.planning_auto.last_error = None;
                        })
                        .await?;
                }
                Err(error) => {
                    if BoardRuntimeHolder::is_connection_error(&error) {
                        return Err(error);
                    }
                    let message = error.to_string();
                    if error
                        .code()
                        .is_some_and(|code| TICKET_ERRORS.contains(&code))
                    {
                        self.mark_tried(&id).await?;
                        report.errors.push(format!("{id}: {message}"));
                        continue;
                    }
                    let retry_at = now + chrono::Duration::minutes(PLANNING_RETRY_MINUTES);
                    let stored = iso(retry_at);
                    let noted = message.clone();
                    ctx.stores
                        .state
                        .update(move |state| {
                            state.planning_auto.retry_at = Some(stored);
                            state.planning_auto.last_error = Some(noted);
                        })
                        .await?;
                    if auto.last_error.as_deref() != Some(message.as_str()) {
                        report.errors.push(format!(
                            "Planungs-Automatik ausgesetzt bis {} (bei {id}): {message}",
                            hhmm(retry_at)
                        ));
                    }
                    break;
                }
            }
        }
        Ok(report)
    }

    /// Wertet einen beendeten Planungslauf aus; der Worktree wird in jedem Fall entfernt
    pub async fn evaluate(
        &self,
        record: &RunRecord,
        end: &RunEnd,
        reason: Option<&str>,
    ) -> Result<PlanOutcome> {
        let outcome = self.outcome(record, end, reason).await;
        if let Some(root) = &record.repo_root {
            if let Some(left) = self.worktrees.remove(Path::new(root), &record.cwd) {
                self.ctx
                    .log(&format!("FEHLER {}: {left}", record.ticket_id));
            }
        }
        outcome
    }

    /// Verwaiste Planungs-Worktrees eines Repos entfernen (`active`: Ordner laufender Planungen);
    /// Meldungen je Worktree
    pub fn remove_orphans(&self, repo_path: &str, active: &[String]) -> Vec<String> {
        self.worktrees.remove_orphans(Path::new(repo_path), active)
    }

    /// Kandidaten der Automatik: Tickets mit Planungsbedarf auf eigenen Boards mit Repo-Pfad, die
    /// seit `since` angelegt oder geändert wurden, ohne offene Frage und ohne fremden Claim
    /// ([`PlanningCandidate`]); Priorität, dann Nummer
    async fn candidates(&self, since: &str, now: i64) -> Result<Vec<TicketView>> {
        let ctx = self.ctx;
        let boards: Vec<Board> = ctx
            .services()
            .boards
            .owned(&ctx.me.user_id)
            .await?
            .into_iter()
            .map(|found| found.board)
            .filter(|board| board.repo_path.is_some())
            .collect();
        let from = parse_millis(since).unwrap_or(i64::MAX);
        let mut found: Vec<TicketView> = Vec::new();
        for board in boards {
            let views = ctx
                .services()
                .tickets
                .list(&TicketListFilter {
                    board_ids: vec![board.id],
                    needs_planning: Some(true),
                    ..TicketListFilter::default()
                })
                .await?;
            found.extend(views.into_iter().filter(|view| {
                PlanningCandidate::matches(&view.ticket, now, view.waiting_on_questions.len())
                    && parse_millis(&view.ticket.created_at)
                        .unwrap_or(0)
                        .max(parse_millis(&view.ticket.updated_at).unwrap_or(0))
                        >= from
            }));
        }
        found.sort_by(|a, b| {
            a.ticket
                .priority
                .rank()
                .cmp(&b.ticket.priority.rank())
                .then_with(|| a.ticket.number.cmp(&b.ticket.number))
        });
        Ok(found)
    }

    /// Merkt ein nicht startbares Ticket als angestoßen, damit die Automatik es nicht jede Minute
    /// neu versucht
    async fn mark_tried(&self, ticket_id: &str) -> Result<()> {
        let at = self.ctx.now_iso();
        let id = ticket_id.to_string();
        self.ctx
            .stores
            .state
            .update(move |state| {
                let before = state.planning.get(&id).cloned();
                state.planning.insert(
                    id.clone(),
                    nexus_core::dispatch::PlanningEntry {
                        ticket_id: id,
                        last_started_at: before
                            .as_ref()
                            .map(|entry| entry.last_started_at.clone())
                            .unwrap_or_else(|| at.clone()),
                        count: before.map(|entry| entry.count).unwrap_or(0),
                        auto_at: Some(at),
                    },
                );
            })
            .await?;
        Ok(())
    }

    /// Darf das Ticket jetzt geplant werden? Liefert das eigene Board mit Repo-Pfad, sonst
    /// [`HarnessError`]
    async fn check(&self, ticket: &TicketDetail) -> Result<Board> {
        let ctx = self.ctx;
        let id = ticket.ticket.id.clone();
        if NOT_PLANNABLE.contains(&ticket.ticket.status) {
            return Err(HarnessError::new(
                "plan-not-possible",
                format!(
                    "{id} steht auf „{}“ – geplant werden nur Tickets in Backlog, In Planung, \
Geplant oder Blockiert.",
                    ticket.ticket.status.as_str()
                ),
            )
            .into());
        }
        if !ticket.view.waiting_on_questions.is_empty() {
            let ids: Vec<&str> = ticket
                .view
                .waiting_on_questions
                .iter()
                .map(|question| question.id.as_str())
                .collect();
            return Err(HarnessError::new(
                "plan-waiting-question",
                format!(
                    "{id} wartet auf Frage {} – erst entscheiden, dann planen.",
                    ids.join(", ")
                ),
            )
            .into());
        }
        let runs: Vec<_> = ctx
            .stores
            .runs
            .list()?
            .into_iter()
            .filter(|run| run.ticket_id == id)
            .collect();
        if runs.iter().any(|run| run.kind == RunKind::Plan) {
            return Err(HarnessError::new(
                "plan-running",
                format!("Für {id} läuft schon eine Planung."),
            )
            .into());
        }
        if let Some(run) = runs.first() {
            return Err(HarnessError::new(
                "plan-not-possible",
                format!(
                    "Für {id} läuft gerade ein anderer Lauf ({}).",
                    run.kind.as_str()
                ),
            )
            .into());
        }
        let board = ctx.board(ticket.ticket.board_id).await?;
        match board {
            Some(board) if board.owner_id == ctx.me.user_id && board.repo_path.is_some() => {
                Ok(board)
            }
            _ => Err(HarnessError::new(
                "plan-no-repo",
                format!(
                    "{id} liegt nicht auf einem eigenen Board mit Repo-Pfad – geplant wird nur \
auf eigenen Boards, deren Repo-Pfad gesetzt ist."
                ),
            )
            .into()),
        }
    }

    /// Planungs-Claim des Agents freigeben; Fehler nur ins Protokoll
    async fn release(&self, ticket_id: &str, agent: &str) {
        if let Err(error) = self
            .ctx
            .services()
            .planning
            .release(ticket_id, agent, self.ctx.actor())
            .await
        {
            self.ctx.log(&format!(
                "FEHLER {ticket_id}: Planungs-Claim nicht freigegeben: {error}"
            ));
        }
    }

    /// Hat der Lauf einen Plan gespeichert? `plannedAt` neuer als beim Start und vom Agent des
    /// Laufs
    fn planned(&self, ticket: &TicketDetail, record: &RunRecord) -> bool {
        let Some(planned_at) = ticket.ticket.planned_at.as_deref() else {
            return false;
        };
        if ticket.ticket.planned_by.as_deref() != Some(record.agent.as_str()) {
            return false;
        }
        match record.planned_before.clone().flatten() {
            Some(before) => {
                parse_millis(planned_at).unwrap_or(0) > parse_millis(&before).unwrap_or(0)
            }
            None => true,
        }
    }

    /// Steht der Kommentar „Planung ohne Ergebnis“ dieses Laufs schon am Ticket?
    fn commented(&self, ticket: &TicketDetail, record: &RunRecord) -> bool {
        let started = parse_millis(&record.started_at).unwrap_or(0) - CLOCK_SKEW_MS;
        let log = file_name(&record.log_file);
        ticket.comments.iter().any(|comment| {
            comment.body.starts_with(markers::PLAN_EMPTY)
                && comment.body.contains(&log)
                && parse_millis(&comment.created_at).is_some_and(|at| at >= started)
        })
    }

    /// Auswertung ohne Aufräumen: Plan da → planned; neue offene Frage → question;
    /// Nutzungslimit/Claude fehlt → Pause; sonst Kommentar am Ticket. Außer bei planned wird der
    /// Planungs-Claim freigegeben
    async fn outcome(
        &self,
        record: &RunRecord,
        end: &RunEnd,
        reason: Option<&str>,
    ) -> Result<PlanOutcome> {
        let ctx = self.ctx;
        let id = record.ticket_id.clone();
        let ticket = match ctx.services().tickets.get(&id).await {
            Ok(ticket) => ticket,
            Err(error)
                if error
                    .code()
                    .is_some_and(|code| code.ends_with("-not-found")) =>
            {
                return Ok(PlanOutcome::Gone)
            }
            Err(error) => return Err(error),
        };
        if self.planned(&ticket, record) {
            ctx.log(&format!("Planung {id} gespeichert"));
            return Ok(PlanOutcome::Planned);
        }
        self.release(&id, &record.agent).await;
        if !ticket.view.waiting_on_questions.is_empty() {
            let ids: Vec<&str> = ticket
                .view
                .waiting_on_questions
                .iter()
                .map(|question| question.id.as_str())
                .collect();
            ctx.log(&format!("Planung {id} wartet auf Frage {}", ids.join(", ")));
            return Ok(PlanOutcome::Question);
        }
        let done = self.commented(&ticket, record);
        let minutes = ctx.cfg.pause_minutes_on_limit;
        if reason.is_none()
            && (ctx.output.claude_missing(&end.result) || ctx.output.hit_usage_limit(&end.result))
        {
            if done {
                return Ok(PlanOutcome::Limit);
            }
            let missing = ctx.output.claude_missing(&end.result);
            let mut until = iso(ctx.now() + chrono::Duration::minutes(minutes));
            let mut why = format!(
                "Claude Code wurde nicht gefunden (Befehl „{}“)",
                record.command
            );
            let mut wait = format!("Der Dispatcher startet {minutes} min lang nichts Neues");
            if missing {
                let stored = until.clone();
                ctx.stores
                    .state
                    .update(move |state| {
                        state.paused_until = Some(stored);
                    })
                    .await?;
            } else {
                // Nutzungslimit: nur der Account des Laufs pausiert (mit einem Account der
                // ganze Dispatcher wie bisher)
                let pause = ctx.pause_account(record.run.account.as_deref()).await?;
                until = pause.until.clone();
                let bis = ctx.hhmm(&until);
                why = if pause.multi {
                    format!("Nutzungslimit von Account {} erreicht", pause.account)
                } else {
                    "Nutzungslimit erreicht".to_string()
                };
                wait = if !pause.multi {
                    format!("Der Dispatcher startet bis {bis} nichts Neues")
                } else if pause.global {
                    format!(
                        "Alle Accounts sind pausiert, der Dispatcher startet bis {bis} nichts Neues"
                    )
                } else {
                    format!("Account {} pausiert bis {bis}", pause.account)
                };
            }
            ctx.comment(
                &id,
                &format!(
                    "{}: {why}. {wait}; „Planen“ danach erneut klicken.\n\nProtokoll: {}",
                    markers::PLAN_EMPTY,
                    file_name(&record.log_file)
                ),
            )
            .await?;
            ctx.log(&format!("Pause bis {until}: {why} (Planung {id})"));
            ctx.notify(&DispatchNotice {
                kind: DispatchNoticeKind::Paused,
                ticket_id: Some(id.clone()),
                title: ticket.ticket.title.clone(),
                text: format!("{why} – Pause bis {}", ctx.hhmm(&until)),
                repo_path: None,
            })
            .await;
            return Ok(PlanOutcome::Limit);
        }
        if done {
            return Ok(PlanOutcome::Empty);
        }
        let summary = ctx.output.summarize(&end.result.output);
        let why = match reason {
            Some(reason) => reason.to_string(),
            None => ctx.output.abort_reason(
                &end.result,
                &summary,
                &record.run,
                Some("Lauf endete, ohne einen Plan oder eine Frage zu speichern"),
            ),
        };
        let mut lines = vec![
            format!("{}: {why}.", markers::PLAN_EMPTY),
            "Kein automatischer Neuversuch – „Planen“ im Ticket stößt eine neue Planung an."
                .to_string(),
        ];
        let shown = ctx
            .output
            .tail(&ctx.output.shown(&end.result.output, &summary), TAIL);
        if !shown.is_empty() {
            lines.extend([
                String::new(),
                "Letzte Ausgabe:".to_string(),
                "```".to_string(),
                shown,
                "```".to_string(),
            ]);
        }
        lines.extend([
            String::new(),
            format!("Protokoll: {}", file_name(&record.log_file)),
        ]);
        ctx.comment(&id, &lines.join("\n")).await?;
        ctx.log(&format!("Planung {id} ohne Ergebnis: {why}"));
        Ok(PlanOutcome::Empty)
    }
}
