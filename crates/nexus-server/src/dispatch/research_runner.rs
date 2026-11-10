//! Recherche-Läufe zu offenen Fragen; portiert aus
//! `packages/server/src/dispatch/ResearchRunner.ts`.
//!
//! Board mit Repo finden, losgelösten Worktree anlegen, `claude -p` nur lesend starten, danach
//! auswerten (neue Recherche des Agents da? sonst Kommentar an der Frage) und den Worktree
//! immer wieder entfernen.

use std::collections::HashSet;
use std::path::Path;

use chrono::{DateTime, Local, Utc};
use nexus_core::actors::AuthorKind;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::question_concern::{ConcernSubject, QuestionConcern};
use nexus_core::board::question_service::{BoardQuestionListFilter, BoardQuestionWriteOptions};
use nexus_core::board::question_types::{
    BoardQuestionActor, BoardQuestionDetail, BoardQuestionOption, BoardQuestionView,
};
use nexus_core::board::ticket_service::{TicketPatch, WriteOptions};
use nexus_core::board::types::{Board, Ticket};
use nexus_core::clock::{iso, parse_millis};
use nexus_core::dispatch::autopilot::{
    asked_by_dispatch_agent, autopilot_for, is_autopilot_question, AUTOPILOT_RESEARCH_ATTEMPTS,
};
use nexus_core::dispatch::config::AUTOPILOT_PREFIX;
use nexus_core::dispatch::git::{ResearchWorktree, ResearchWorktreeOptions, WorktreePurpose};
use nexus_core::dispatch::{
    markers, ResearchAutoState, ResearchPromptInput, RunEnd, RunKind, RunRecord, RunSettings,
};
use nexus_core::questions::state::QuestionState;
use nexus_core::questions::DecisionInput;
use nexus_core::{HarnessError, Result};

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::context::{DispatchContext, LaunchInput};
use crate::dispatch::notifier::{DispatchNotice, DispatchNoticeKind};

/// Von Hand oder automatisch angestoßen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartMode {
    Manual,
    Auto,
}

impl StartMode {
    /// Wie der Anstoß im Protokoll steht
    pub fn label(&self) -> &'static str {
        match self {
            StartMode::Manual => "von Hand",
            StartMode::Auto => "automatisch",
        }
    }
}

/// Ergebnis der Auswertung eines Recherche-Laufs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchOutcome {
    Researched,
    Empty,
    Limit,
    Gone,
}

impl ResearchOutcome {
    /// Name des Ausgangs wie in TypeScript
    pub fn as_str(&self) -> &'static str {
        match self {
            ResearchOutcome::Researched => "researched",
            ResearchOutcome::Empty => "empty",
            ResearchOutcome::Limit => "limit",
            ResearchOutcome::Gone => "gone",
        }
    }
}

/// Bericht des automatischen Anstoßes eines Ticks
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResearchAutoReport {
    pub started: Vec<String>,
    pub errors: Vec<String>,
}

/// Fehler, die an der Frage selbst liegen: die Automatik merkt die Frage als erledigt und
/// versucht sie nicht wieder
const QUESTION_ERRORS: [&str; 3] = [
    "research-no-repo",
    "question-not-open",
    "question-not-found",
];
/// Nach einem Umgebungs- oder Repo-Fehler setzt die Automatik so lange aus
pub const RESEARCH_RETRY_MINUTES: i64 = 30;
/// Spielraum zwischen der Uhr des Dispatchers (Start des Laufs) und der Uhr der Datenbank
/// (Zeitpunkt der Recherche)
const CLOCK_SKEW_MS: i64 = 60_000;
/// So viele Zeichen Ausgabe stehen in einem Kommentar
const TAIL: usize = 1500;

/// Startet und wertet Recherche-Läufe aus.
#[derive(Debug)]
pub struct ResearchRunner<'a> {
    ctx: &'a DispatchContext,
    worktrees: ResearchWorktree,
}

impl<'a> ResearchRunner<'a> {
    /// Läufe auf diesem Kontext
    pub fn new(ctx: &'a DispatchContext) -> Self {
        Self {
            ctx,
            worktrees: ResearchWorktree::with_options(
                WorktreePurpose::Research,
                ResearchWorktreeOptions {
                    home: Some(ctx.home.clone()),
                    ..ResearchWorktreeOptions::default()
                },
            ),
        }
    }

    /// Name des Recherche-Agents einer Frage
    pub fn agent_name(question_id: &str) -> String {
        format!("research-{question_id}")
    }

    /// Kalendertag (Ortszeit) als JJJJ-MM-TT – Schlüssel des Tageszählers der Automatik
    pub fn day(at: DateTime<Utc>) -> String {
        at.with_timezone(&Local).format("%Y-%m-%d").to_string()
    }

    /// Startet die Recherche einer offenen Frage (von Hand oder automatisch); Fehler als
    /// [`HarnessError`]
    pub async fn start(&self, question_id: &str, mode: StartMode) -> Result<RunRecord> {
        self.start_with(question_id, mode, self.ctx.chooser().research())
            .await
    }

    /// Wie [`ResearchRunner::start`], mit gewählten Einstellungen des Laufs
    async fn start_with(
        &self,
        question_id: &str,
        mode: StartMode,
        run: RunSettings,
    ) -> Result<RunRecord> {
        let ctx = self.ctx;
        let question = ctx.services().questions.get(question_id).await?;
        let id = question.view.question.id.clone();
        if question.view.question.state != QuestionState::Open {
            return Err(HarnessError::new(
                "question-not-open",
                format!("Frage {id} ist nicht offen – recherchiert werden nur offene Fragen."),
            )
            .into());
        }
        if self.running(&id).await? {
            return Err(HarnessError::new(
                "research-running",
                format!("Für Frage {id} läuft schon eine Recherche."),
            )
            .into());
        }
        let board = self.locate(&question.view).await?;
        let work = ctx.repo(&board).await?;
        let agent = Self::agent_name(&id);
        let worktree = self.worktrees.create(Path::new(&work.root), &id)?;
        if let Some(warning) = &worktree.warning {
            ctx.log(&format!("Hinweis {id}: {warning}"));
        }
        let prompt = ctx.prompts.research(&ResearchPromptInput {
            question_id: &id,
            title: &question.view.question.title,
            agent: &agent,
            cfg: &ctx.cfg,
        });
        let launched = ctx
            .launch(LaunchInput {
                question_id: Some(id.clone()),
                research_before: Some(question.research.len() as i64),
                repo_root: Some(work.root.clone()),
                ..LaunchInput::new(
                    RunKind::Research,
                    &id,
                    board.id,
                    &question.view.question.title,
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
                return Err(error);
            }
        };
        let at = ctx.now_iso();
        let entry_id = id.clone();
        ctx.stores
            .state
            .update(move |state| {
                let before = state.research.get(&entry_id).cloned();
                state.research.insert(
                    entry_id.clone(),
                    nexus_core::dispatch::ResearchEntry {
                        question_id: entry_id,
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
            "Recherche gestartet {id} ({}) → {agent} in {} [{}: {}/{}]",
            mode.label(),
            worktree.path,
            run.profile,
            run.model.as_deref().unwrap_or("-"),
            run.effort.as_deref().unwrap_or("-"),
        ));
        Ok(record)
    }

    /// Wertet einen beendeten Recherche-Lauf aus; der Worktree wird in jedem Fall entfernt.
    /// Bei einer Autopilot-Frage übernimmt der Dispatcher danach die Empfehlung (siehe
    /// [`ResearchRunner::decide_autopilot`])
    pub async fn evaluate(
        &self,
        record: &RunRecord,
        end: &RunEnd,
        reason: Option<&str>,
    ) -> Result<ResearchOutcome> {
        let outcome = self.outcome(record, end, reason).await;
        if let Some(root) = &record.repo_root {
            if let Some(left) = self.worktrees.remove(Path::new(root), &record.cwd) {
                self.ctx
                    .log(&format!("FEHLER {}: {left}", record.ticket_id));
            }
        }
        let outcome = outcome?;
        if matches!(
            outcome,
            ResearchOutcome::Researched | ResearchOutcome::Empty
        ) {
            self.decide_autopilot(record, outcome).await?;
        }
        Ok(outcome)
    }

    /// Stößt offene Autopilot-Fragen an (siehe [`is_autopilot_question`]), älteste zuerst –
    /// auch bei ausgeschalteter Recherche-Automatik und ohne Zählung gegen
    /// `research.maxPerDay`, aber nur bis `research.wip` gleichzeitige Läufe. Je Frage höchstens
    /// [`AUTOPILOT_RESEARCH_ATTEMPTS`] Recherchen ohne Empfehlung, jede weitere eine Stufe höher
    /// auf der Leiter. Umgebungs- und Repo-Fehler setzen wie bei [`ResearchRunner::start_auto`]
    /// aus
    pub async fn start_autopilot(&self) -> Result<ResearchAutoReport> {
        let ctx = self.ctx;
        let mut report = ResearchAutoReport::default();
        if !ctx.cfg.autopilot.enabled {
            return Ok(report);
        }
        let now = ctx.now();
        let state = ctx.stores.state.read()?;
        let auto = state.research_auto.clone();
        if auto
            .retry_at
            .as_deref()
            .and_then(parse_millis)
            .is_some_and(|at| at > now.timestamp_millis())
        {
            return Ok(report);
        }
        let mut free = ctx.cfg.research.wip - self.running_count()?;
        if free <= 0 {
            return Ok(report);
        }
        for question in self.autopilot_candidates().await? {
            if free <= 0 {
                break;
            }
            let id = question.question.id.clone();
            let tried = state.autopilot_research.get(&id).copied().unwrap_or(0);
            if tried >= AUTOPILOT_RESEARCH_ATTEMPTS || self.running(&id).await? {
                continue;
            }
            let run = ctx.chooser().research_attempt(tried + 1);
            match self.start_with(&id, StartMode::Auto, run).await {
                Ok(_) => {
                    report.started.push(id);
                    free -= 1;
                }
                Err(error) => {
                    if BoardRuntimeHolder::is_connection_error(&error) {
                        return Err(error);
                    }
                    if error
                        .code()
                        .is_some_and(|code| QUESTION_ERRORS.contains(&code))
                    {
                        report.errors.push(format!("{id}: {error}"));
                        continue;
                    }
                    self.suspend(&auto, now, &id, &error, &mut report).await?;
                    break;
                }
            }
        }
        Ok(report)
    }

    /// Verwaiste Recherche-Worktrees eines Repos entfernen (`active`: Ordner laufender
    /// Recherchen); Meldungen je Worktree
    pub fn remove_orphans(&self, repo_path: &str, active: &[String]) -> Vec<String> {
        self.worktrees.remove_orphans(Path::new(repo_path), active)
    }

    /// Stößt neue offene Fragen ohne Recherche an, die den eigenen Benutzer betreffen (entstanden
    /// seit dem Einschalten der Automatik, älteste zuerst, je Frage höchstens einmal, nie nach
    /// einem Lauf von Hand), bis `research.wip` gleichzeitige Läufe und `research.maxPerDay`
    /// Starts am Tag. Fehler der Frage (kein Repo, nicht offen) merken sie als erledigt;
    /// Umgebungs- und Repo-Fehler brechen ab, ohne zu merken, und setzen die Automatik
    /// [`RESEARCH_RETRY_MINUTES`] aus (je Fehlertext ein Bericht)
    pub async fn start_auto(&self) -> Result<ResearchAutoReport> {
        let ctx = self.ctx;
        let mut report = ResearchAutoReport::default();
        let now = ctx.now();
        let state = ctx.stores.state.read()?;
        let auto = state.research_auto.clone();
        let Some(since) = auto.since.clone() else {
            return Ok(report);
        };
        if auto
            .retry_at
            .as_deref()
            .and_then(parse_millis)
            .is_some_and(|at| at > now.timestamp_millis())
        {
            return Ok(report);
        }
        let today = Self::day(now);
        let mut used = if auto.day.as_deref() == Some(today.as_str()) {
            auto.count
        } else {
            0
        };
        let mut free = ctx.cfg.research.wip - self.running_count()?;
        if free <= 0 || used >= ctx.cfg.research.max_per_day {
            return Ok(report);
        }
        for question in self.candidates(&since).await? {
            if free <= 0 || used >= ctx.cfg.research.max_per_day {
                break;
            }
            let id = question.question.id.clone();
            // Schon einmal angestoßen (automatisch oder von Hand): kein automatischer Neuversuch
            if state.research.contains_key(&id) || self.running(&id).await? {
                continue;
            }
            match self.start(&id, StartMode::Auto).await {
                Ok(_) => {
                    report.started.push(id);
                    free -= 1;
                    used += 1;
                    let day = today.clone();
                    ctx.stores
                        .state
                        .update(move |state| {
                            state.research_auto.day = Some(day);
                            state.research_auto.count = used;
                            state.research_auto.retry_at = None;
                            state.research_auto.last_error = None;
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
                        .is_some_and(|code| QUESTION_ERRORS.contains(&code))
                    {
                        self.mark_tried(&id).await?;
                        report.errors.push(format!("{id}: {message}"));
                        continue;
                    }
                    self.suspend(&auto, now, &id, &error, &mut report).await?;
                    break;
                }
            }
        }
        Ok(report)
    }

    /// Setzt die Automatik nach einem Umgebungs- oder Repo-Fehler
    /// [`RESEARCH_RETRY_MINUTES`] aus; derselbe Fehlertext landet nur einmal im Bericht
    async fn suspend(
        &self,
        auto: &ResearchAutoState,
        now: DateTime<Utc>,
        id: &str,
        error: &nexus_core::Error,
        report: &mut ResearchAutoReport,
    ) -> Result<()> {
        let message = error.to_string();
        let retry_at = now + chrono::Duration::minutes(RESEARCH_RETRY_MINUTES);
        let stored = iso(retry_at);
        let noted = message.clone();
        self.ctx
            .stores
            .state
            .update(move |state| {
                state.research_auto.retry_at = Some(stored);
                state.research_auto.last_error = Some(noted);
            })
            .await?;
        if auto.last_error.as_deref() != Some(message.as_str()) {
            report.errors.push(format!(
                "Recherche-Automatik ausgesetzt bis {} (bei {id}): {message}",
                hhmm(retry_at)
            ));
        }
        Ok(())
    }

    /// Zahl der laufenden Recherche-Läufe
    fn running_count(&self) -> Result<i64> {
        Ok(self
            .ctx
            .stores
            .runs
            .list()?
            .iter()
            .filter(|run| run.kind == RunKind::Research)
            .count() as i64)
    }

    /// Board, in dessen Repo recherchiert wird (siehe `pick`), sonst `HarnessError`
    /// „research-no-repo“
    pub async fn locate(&self, question: &BoardQuestionView) -> Result<Board> {
        let own = self.own_boards().await?;
        if let Some(board) = self.pick(question, &own) {
            return Ok(board);
        }
        Err(HarnessError::new(
            "research-no-repo",
            format!(
                "Für Frage {} ist kein eigenes Board mit Repo-Pfad bekannt – die Frage mit einem \
Ticket eines eigenen Boards verknüpfen oder bei einem eigenen Board des Projekts {} den \
Repo-Pfad setzen.",
                question.question.id, question.project_key
            ),
        )
        .into())
    }

    /// Wahl des Boards aus den eigenen Boards: 1. Board eines verknüpften Tickets mit Repo-Pfad,
    /// 2. das aktive Board, wenn es zum Projekt der Frage gehört und einen Repo-Pfad hat, 3. sonst
    /// das eigene Board des Projekts mit Repo-Pfad mit der kleinsten ID
    fn pick(&self, question: &BoardQuestionView, own: &[Board]) -> Option<Board> {
        let with_repo: Vec<&Board> = own
            .iter()
            .filter(|board| board.repo_path.is_some())
            .collect();
        for ticket in &question.tickets {
            if let Some(board) = with_repo.iter().find(|board| board.id == ticket.board_id) {
                return Some((*board).clone());
            }
        }
        let mut project: Vec<&Board> = with_repo
            .into_iter()
            .filter(|board| board.project_id == question.question.project_id)
            .collect();
        project.sort_by_key(|board| board.id);
        project
            .iter()
            .find(|board| Some(board.id) == self.ctx.me.active_board)
            .or(project.first())
            .map(|board| (*board).clone())
    }

    /// Betrifft die Frage den eigenen Benutzer? Gemeinsame Regel „Eigene“ aus core
    /// ([`QuestionConcern`])
    fn concerns_me(&self, question: &BoardQuestionView, own: &HashSet<i32>) -> bool {
        QuestionConcern::concerns(
            &ConcernSubject {
                created_by_id: question.question.created_by_id.clone(),
                board_ids: question
                    .tickets
                    .iter()
                    .map(|ticket| ticket.board_id)
                    .collect(),
            },
            &self.ctx.me.user_id,
            own,
        )
    }

    /// Eigene Boards mit eigenem (wirksamem) Repo-Pfad
    async fn own_boards(&self) -> Result<Vec<Board>> {
        Ok(self
            .ctx
            .services()
            .boards
            .owned(&self.ctx.me.user_id)
            .await?
            .into_iter()
            .map(|found| found.board)
            .collect())
    }

    /// Läuft für die Frage schon ein Recherche-Lauf?
    async fn running(&self, question_id: &str) -> Result<bool> {
        Ok(self.ctx.stores.runs.list()?.iter().any(|run| {
            run.kind == RunKind::Research
                && run.question_id.as_deref().unwrap_or(&run.ticket_id) == question_id
        }))
    }

    /// Merkt eine nicht startbare Frage als angestoßen, damit die Automatik sie nicht jede Minute
    /// neu versucht
    async fn mark_tried(&self, question_id: &str) -> Result<()> {
        let at = self.ctx.now_iso();
        let id = question_id.to_string();
        self.ctx
            .stores
            .state
            .update(move |state| {
                let before = state.research.get(&id).cloned();
                state.research.insert(
                    id.clone(),
                    nexus_core::dispatch::ResearchEntry {
                        question_id: id,
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

    /// Kandidaten der Automatik: offene Fragen ohne Recherche, entstanden seit `since`, die den
    /// eigenen Benutzer betreffen und für die es ein Board mit Repo gibt (dieselbe Wahl wie
    /// [`ResearchRunner::locate`]), älteste zuerst
    async fn candidates(&self, since: &str) -> Result<Vec<BoardQuestionView>> {
        let from = parse_millis(since).unwrap_or(i64::MAX);
        Ok(self
            .open_own()
            .await?
            .into_iter()
            .filter(|view| {
                !view.researched
                    && parse_millis(&view.question.created_at).is_some_and(|at| at >= from)
            })
            .collect())
    }

    /// Kandidaten des Autopilots: offene Fragen wie bei [`ResearchRunner::candidates`] (mit
    /// oder ohne Recherche, unabhängig vom Einschalten der Automatik), die Autopilot-Fragen sind
    async fn autopilot_candidates(&self) -> Result<Vec<BoardQuestionView>> {
        let mut found = Vec::new();
        for view in self.open_own().await? {
            if self.autopilot_tickets(&view).await?.is_some() {
                found.push(view);
            }
        }
        Ok(found)
    }

    /// Betroffene Tickets einer Autopilot-Frage, für die der Autopilot gilt (nur Tickets
    /// eigener Boards); `None`, wenn die Frage keine Autopilot-Frage ist
    async fn autopilot_tickets(&self, view: &BoardQuestionView) -> Result<Option<Vec<Ticket>>> {
        let ctx = self.ctx;
        let question = &view.question;
        if question.state != QuestionState::Open
            || !asked_by_dispatch_agent(&ctx.cfg, question.origin_kind, &question.origin_name)
        {
            return Ok(None);
        }
        let own: HashSet<i32> = self
            .own_boards()
            .await?
            .iter()
            .map(|board| board.id)
            .collect();
        let mut tickets = Vec::new();
        for reference in view
            .tickets
            .iter()
            .filter(|ticket| own.contains(&ticket.board_id))
        {
            let ticket = ctx.services().tickets.get(&reference.id).await?.view.ticket;
            tickets.push(ticket);
        }
        let labels: Vec<Vec<String>> = tickets.iter().map(|t| t.labels.clone()).collect();
        if !is_autopilot_question(&ctx.cfg, question, &labels) {
            return Ok(None);
        }
        Ok(Some(
            tickets
                .into_iter()
                .filter(|ticket| autopilot_for(&ctx.cfg, &ticket.labels))
                .collect(),
        ))
    }

    /// Nach einer ausgewerteten Recherche zu einer Autopilot-Frage: Hat die neueste Recherche
    /// dieses Laufs genau eine empfohlene Option, beantwortet der Dispatcher die Frage damit und
    /// kommentiert die Tickets. Sonst zählt der Versuch; nach
    /// [`AUTOPILOT_RESEARCH_ATTEMPTS`] Versuchen gehen die Autopilot-Tickets auf Blockiert
    async fn decide_autopilot(&self, record: &RunRecord, outcome: ResearchOutcome) -> Result<()> {
        let ctx = self.ctx;
        if !ctx.cfg.autopilot.enabled {
            return Ok(());
        }
        let id = record
            .question_id
            .clone()
            .unwrap_or_else(|| record.ticket_id.clone());
        let question = match ctx.services().questions.get(&id).await {
            Ok(question) => question,
            Err(error) if error.code() == Some("question-not-found") => return Ok(()),
            Err(error) => return Err(error),
        };
        let Some(tickets) = self.autopilot_tickets(&question.view).await? else {
            return Ok(());
        };
        let recommended: Vec<&BoardQuestionOption> = match outcome {
            ResearchOutcome::Researched => question
                .research
                .last()
                .map(|newest| newest.options.iter().filter(|o| o.recommended).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        if let [option] = recommended.as_slice() {
            return self.answer_autopilot(&id, option, &tickets, record).await;
        }
        let entry = id.clone();
        let state = ctx
            .stores
            .state
            .update(move |state| {
                *state.autopilot_research.entry(entry).or_insert(0) += 1;
            })
            .await?;
        let tried = state.autopilot_research.get(&id).copied().unwrap_or(0);
        if tried < AUTOPILOT_RESEARCH_ATTEMPTS {
            ctx.log(&format!(
                "Autopilot {id}: keine eindeutige Empfehlung (Versuch {tried}/{AUTOPILOT_RESEARCH_ATTEMPTS}), nächste Recherche mit größerem Profil"
            ));
            return Ok(());
        }
        let reason = format!("{AUTOPILOT_PREFIX} keine Empfehlung zu {id}");
        for ticket in tickets
            .iter()
            .filter(|ticket| !matches!(ticket.status, TicketStatus::Done | TicketStatus::NotNeeded))
        {
            self.block(ticket, &reason).await?;
        }
        ctx.log(&format!("Autopilot {id}: {reason} nach {tried} Recherchen"));
        Ok(())
    }

    /// Beantwortet eine Autopilot-Frage mit der empfohlenen Option (Akteur: der Dispatcher als
    /// Session des Laufs) und kommentiert die Autopilot-Tickets
    async fn answer_autopilot(
        &self,
        id: &str,
        option: &BoardQuestionOption,
        tickets: &[Ticket],
        record: &RunRecord,
    ) -> Result<()> {
        let ctx = self.ctx;
        ctx.services()
            .questions
            .answer(
                id,
                &DecisionInput {
                    option: Some(option.id.clone()),
                    text: None,
                    reason: Some(format!(
                        "{AUTOPILOT_PREFIX} Empfehlung der Recherche übernommen"
                    )),
                },
                &BoardQuestionActor {
                    actor: ctx.actor().clone(),
                    kind: AuthorKind::Session,
                    session: Some(record.id.clone()),
                },
                &BoardQuestionWriteOptions::default(),
            )
            .await?;
        let entry = id.to_string();
        ctx.stores
            .state
            .update(move |state| {
                state.autopilot_research.shift_remove(&entry);
            })
            .await?;
        for ticket in tickets {
            ctx.comment(
                &ticket.id,
                &format!(
                    "{AUTOPILOT_PREFIX} Frage {id} mit „{}“ entschieden",
                    option.title
                ),
            )
            .await?;
        }
        ctx.log(&format!(
            "Autopilot {id}: Option {} „{}“ übernommen",
            option.id, option.title
        ));
        Ok(())
    }

    /// Blockiert ein Autopilot-Ticket mit Grund und meldet es
    async fn block(&self, ticket: &Ticket, reason: &str) -> Result<()> {
        let ctx = self.ctx;
        ctx.services()
            .tickets
            .update(
                &ticket.id,
                &TicketPatch {
                    status: Some(TicketStatus::Blocked),
                    blocked_reason: Some(Some(reason.to_string())),
                    assignee: Some(None),
                    ..TicketPatch::default()
                },
                ctx.actor(),
                &WriteOptions {
                    force: true,
                    ..WriteOptions::default()
                },
            )
            .await?;
        let board = ctx.board(ticket.board_id).await?;
        ctx.notify(&DispatchNotice {
            kind: DispatchNoticeKind::Blocked,
            ticket_id: Some(ticket.id.clone()),
            title: ticket.title.clone(),
            text: reason.to_string(),
            repo_path: board.and_then(|board| board.repo_path),
        })
        .await;
        Ok(())
    }

    /// Offene Fragen der Projekte mit eigenem Board mit Repo, die den eigenen Benutzer betreffen
    /// und für die es ein Board mit Repo gibt, älteste zuerst
    async fn open_own(&self) -> Result<Vec<BoardQuestionView>> {
        let ctx = self.ctx;
        let own = self.own_boards().await?;
        let own_ids: HashSet<i32> = own.iter().map(|board| board.id).collect();
        let project_ids: HashSet<i32> = own
            .iter()
            .filter(|board| board.repo_path.is_some())
            .map(|board| board.project_id)
            .collect();
        if project_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut found: Vec<BoardQuestionView> = Vec::new();
        for project in ctx.services().projects.list().await? {
            if !project_ids.contains(&project.id) {
                continue;
            }
            let list = ctx
                .services()
                .questions
                .list(
                    &project.key,
                    &BoardQuestionListFilter {
                        state: Some(QuestionState::Open),
                        ..BoardQuestionListFilter::default()
                    },
                    None,
                )
                .await?;
            found.extend(
                list.questions
                    .into_iter()
                    .map(|item| item.view)
                    .filter(|view| {
                        self.concerns_me(view, &own_ids) && self.pick(view, &own).is_some()
                    }),
            );
        }
        found.sort_by(|a, b| {
            a.question
                .created_at
                .cmp(&b.question.created_at)
                .then_with(|| a.question.id.cmp(&b.question.id))
        });
        Ok(found)
    }

    /// Hat der Lauf eine Recherche gespeichert? Zählt nur eine Recherche mit dem Agentnamen des
    /// Laufs als Autor ab dem Start des Laufs; nur neue Recherchen ganz ohne Autor zählen über die
    /// Zahl der Recherchen beim Start
    fn researched(&self, question: &BoardQuestionDetail, record: &RunRecord) -> bool {
        let started = parse_millis(&record.started_at).unwrap_or(0) - CLOCK_SKEW_MS;
        if question.research.iter().any(|research| {
            research.by_name == record.agent
                && parse_millis(&research.at).is_some_and(|at| at >= started)
        }) {
            return true;
        }
        let before = record.research_before.unwrap_or(0).max(0) as usize;
        question
            .research
            .iter()
            .skip(before)
            .any(|research| research.by_name.trim().is_empty())
    }

    /// Steht der Kommentar „Recherche ohne Ergebnis“ dieses Laufs schon an der Frage (z. B.
    /// Verbindungsfehler danach)?
    fn commented(&self, question: &BoardQuestionDetail, record: &RunRecord) -> bool {
        let started = parse_millis(&record.started_at).unwrap_or(0) - CLOCK_SKEW_MS;
        question.comments.iter().any(|comment| {
            comment.author_name == record.agent
                && comment.body.starts_with(markers::RESEARCH_EMPTY)
                && parse_millis(&comment.created_at).is_some_and(|at| at >= started)
        })
    }

    /// Auswertung ohne Aufräumen: neue Recherche → ok; Nutzungslimit/Claude fehlt → Pause; sonst
    /// Kommentar an der Frage. Steht der Kommentar dieses Laufs schon da (Auswertung nach einem
    /// Verbindungsfehler wiederholt), wird nichts doppelt geschrieben
    async fn outcome(
        &self,
        record: &RunRecord,
        end: &RunEnd,
        reason: Option<&str>,
    ) -> Result<ResearchOutcome> {
        let ctx = self.ctx;
        let id = record
            .question_id
            .clone()
            .unwrap_or_else(|| record.ticket_id.clone());
        let question = match ctx.services().questions.get(&id).await {
            Ok(question) => question,
            Err(error) if error.code() == Some("question-not-found") => {
                return Ok(ResearchOutcome::Gone)
            }
            Err(error) => return Err(error),
        };
        if self.researched(&question, record) {
            ctx.log(&format!(
                "Recherche {id} gespeichert ({} Recherche(n))",
                question.research.len()
            ));
            return Ok(ResearchOutcome::Researched);
        }
        let actor = BoardQuestionActor {
            actor: ctx.agent_actor(&record.agent),
            kind: AuthorKind::Session,
            session: None,
        };
        let minutes = ctx.cfg.pause_minutes_on_limit;
        let done = self.commented(&question, record);
        if reason.is_none()
            && (ctx.output.claude_missing(&end.result) || ctx.output.hit_usage_limit(&end.result))
        {
            if done {
                return Ok(ResearchOutcome::Limit);
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
            ctx.services()
                .questions
                .comment(
                    &id,
                    &format!(
                        "{}: {why}. {wait}; die Recherche danach bitte erneut starten.",
                        markers::RESEARCH_EMPTY
                    ),
                    &actor,
                )
                .await?;
            ctx.log(&format!("Pause bis {until}: {why} (Recherche {id})"));
            ctx.notify(&DispatchNotice {
                kind: DispatchNoticeKind::Paused,
                ticket_id: None,
                title: question.view.question.title.clone(),
                text: format!("{why} – Pause bis {}", ctx.hhmm(&until)),
                repo_path: None,
            })
            .await;
            return Ok(ResearchOutcome::Limit);
        }
        if done {
            return Ok(ResearchOutcome::Empty);
        }
        let summary = ctx.output.summarize(&end.result.output);
        let why = match reason {
            Some(reason) => reason.to_string(),
            None => ctx.output.abort_reason(
                &end.result,
                &summary,
                &record.run,
                Some("Lauf endete, ohne eine Recherche an der Frage zu speichern"),
            ),
        };
        let mut lines = vec![
            format!("{}: {why}.", markers::RESEARCH_EMPTY),
            "Kein automatischer Neuversuch – „Recherche starten“ auf der Seite der Frage stößt \
eine neue an."
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
        ctx.services()
            .questions
            .comment(&id, &lines.join("\n"), &actor)
            .await?;
        ctx.log(&format!("Recherche {id} ohne Ergebnis: {why}"));
        Ok(ResearchOutcome::Empty)
    }
}

/// HH:MM Ortszeit
pub fn hhmm(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local).format("%H:%M").to_string()
}

/// Dateiname eines Pfads (`basename`)
pub fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}
