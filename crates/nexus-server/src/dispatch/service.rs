//! Herz des Dispatchers: ein Durchlauf wertet Läufe aus, gibt Verwaistes zurück, mergt
//! Erledigtes und startet bereite Tickets bis zum WIP-Limit. Portiert aus
//! `packages/server/src/dispatch/DispatchService.ts`.
//!
//! Noch nicht dabei (und deshalb als Schnittstelle hereingereicht): der Merge
//! ([`MergeStep`], P4-15b), die Freigabe vor dem Merge ([`ReviewGate`], P4-15b) und die
//! Routen (P4-16a). „Erneut mergen“, „Abnehmen“, „Zurück an den Agenten“ und die
//! Prompt-Vorschau bauen in P4-15b bzw. P4-16a auf [`DispatchService::exclusive`] auf.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::Local;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::question_ids::QuestionIds;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::board::ticket_service::{TicketListFilter, TicketView};
use nexus_core::board::types::Board;
use nexus_core::clock::{iso, parse_millis, Clock};
use nexus_core::dispatch::{
    DispatchConfig, DispatchConfigStore, DispatchLog, DispatchStateStore, MergeEntry,
    ReadinessBoard, ReadinessTicket, ReviewStore, RunEnd, RunKind, RunLogSplitter, RunRecord,
    RunResult, RunStore, TimeWindowCheck,
};
use nexus_core::{HarnessError, HarnessHome, Result};
use tokio::sync::watch;

use crate::board_runtime::{BoardBackend, BoardRuntimeHolder};
use crate::dispatch::context::{DispatchContext, DispatchStores, RepoWork};
use crate::dispatch::merge_result::{MergeResultReader, MergeRetryOutcome, MergeRetryResult};
use crate::dispatch::merge_service::MergeService;
use crate::dispatch::merge_step::MergeStep;
use crate::dispatch::notifier::{DispatchNotify, NoDispatchNotify};
use crate::dispatch::plan_runner::PlanRunner;
use crate::dispatch::research_runner::{hhmm, ResearchRunner, StartMode};
use crate::dispatch::result_handler::{MergeSessionEnd, ResultHandler, ReviewGate};
use crate::dispatch::review_service::{ReviewFilePatch, ReviewService, ReviewView};
use crate::dispatch::run_recorder::{HandleOutcome, HandleResult, RunRecorder};
use crate::dispatch::status::{
    DispatchMergeInfo, DispatchPlanningInfo, DispatchResearchInfo, DispatchRunInfo, DispatchRunLog,
    DispatchRunLogText, DispatchStateKind, DispatchStatus, DispatchTicketRow,
    QuestionResearchStatus, TicketPlanningStatus,
};
use crate::dispatch::ticket_starter::{StartOutcome, TicketStarter};
use crate::dispatch::worker_launcher::WorkerLauncher;

/// Nach so vielen Minuten über dem Zeitlimit gilt ein Lauf ohne Ergebnis als verschwunden,
/// auch wenn die Prozessnummer lebt
const OVERDUE_MINUTES: i64 = 10;
/// Nach so vielen Minuten gilt eine Übernahme (Geplant mit Agent, ohne Lauf) als
/// hängengeblieben
const CLAIM_STALE_MINUTES: i64 = 5;

/// Prüft, ob ein Prozess noch läuft; im Betrieb [`nexus_core::process::is_alive`], in Tests
/// eine feste Antwort
pub type AliveCheck = Arc<dyn Fn(u32) -> bool + Send + Sync>;

/// Ein ausgewerteter Lauf im Bericht
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedRun {
    pub ticket_id: String,
    /// Ausgang in der Schreibweise des Berichts (`done`, `released`, `limit` …)
    pub outcome: String,
}

/// Bericht eines Durchlaufs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchReport {
    pub at: String,
    /// Warum nichts Neues gestartet wurde, sonst `None`
    pub idle: Option<String>,
    pub started: Vec<String>,
    pub queued: Vec<String>,
    pub evaluated: Vec<EvaluatedRun>,
    pub merged: Vec<String>,
    /// Automatisch gestartete Recherchen (Fragen-IDs)
    pub research: Vec<String>,
    /// Automatisch gestartete Planungen (Ticket-IDs)
    pub planned: Vec<String>,
    pub errors: Vec<String>,
}

impl DispatchReport {
    /// Leerer Bericht zu diesem Zeitpunkt
    pub fn new(at: String) -> Self {
        Self {
            at,
            idle: None,
            started: Vec::new(),
            queued: Vec::new(),
            evaluated: Vec::new(),
            merged: Vec::new(),
            research: Vec::new(),
            planned: Vec::new(),
            errors: Vec::new(),
        }
    }
}

/// Bericht einer Arbeit von Hand („Erneut mergen“, „Abnehmen“, „Zurück an den Agenten“) samt
/// Ergebnis für das angefragte Ticket
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeRetryReport {
    pub report: DispatchReport,
    pub result: MergeRetryResult,
}

/// Was der Dispatch-Service braucht; alles, was Tests ersetzen (Uhr, Prozessprüfung, Umgebung,
/// Merge, Review), steht hier
pub struct DispatchServiceDeps {
    pub home: HarnessHome,
    pub boards: Arc<BoardRuntimeHolder>,
    pub launcher: Arc<dyn WorkerLauncher>,
    pub notifier: Arc<dyn DispatchNotify>,
    pub clock: Clock,
    /// Läuft der Prozess noch? (Standard: Signal 0 bzw. Prozessabfrage unter Windows)
    pub alive: AliveCheck,
    /// Merge-Seite des Dispatchers (Betrieb: [`MergeService`])
    pub merge: Arc<dyn MergeStep>,
    /// Freigabe vor dem Merge (Betrieb: [`ReviewService`])
    pub reviews: Arc<dyn ReviewGate>,
    /// Umgebung, die ein Worker erbt (im Betrieb die des Servers)
    pub env: BTreeMap<String, String>,
    /// Name der Plattform wie in Node (`win32`, `darwin`, `linux`)
    pub platform: String,
}

impl std::fmt::Debug for DispatchServiceDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DispatchServiceDeps")
            .field("platform", &self.platform)
            .finish_non_exhaustive()
    }
}

impl DispatchServiceDeps {
    /// Die Abhängigkeiten des Betriebs: Systemuhr, echte Prozessprüfung, der echte Merge und
    /// die echte Freigabe vor dem Merge, Umgebung und Plattform dieses Prozesses
    pub fn new(
        home: HarnessHome,
        boards: Arc<BoardRuntimeHolder>,
        launcher: Arc<dyn WorkerLauncher>,
    ) -> Self {
        Self {
            home,
            boards,
            launcher,
            notifier: Arc::new(NoDispatchNotify),
            clock: Clock::system(),
            alive: Arc::new(nexus_core::process::is_alive),
            merge: Arc::new(MergeService::new()),
            reviews: Arc::new(ReviewService::new()),
            env: std::env::vars().collect(),
            platform: crate::dispatch::context::node_platform().to_string(),
        }
    }
}

/// Startet, wertet aus, mergt und berichtet – der Dispatcher im Server.
pub struct DispatchService {
    pub stores: DispatchStores,
    deps: DispatchServiceDeps,
    configs: DispatchConfigStore,
    window: TimeWindowCheck,
    /// Bericht des letzten Durchlaufs (für den Zustand)
    last_report: Mutex<Option<DispatchReport>>,
    /// Ergebnis des gerade laufenden Durchlaufs; weitere Aufrufer hängen sich daran
    running: tokio::sync::Mutex<Option<watch::Receiver<Option<DispatchReport>>>>,
    /// Sperre: Durchläufe und die Arbeiten von Hand laufen nacheinander
    lock: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for DispatchService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DispatchService").finish_non_exhaustive()
    }
}

impl DispatchService {
    /// Service auf diesen Abhängigkeiten
    pub fn new(deps: DispatchServiceDeps) -> Self {
        let stores = DispatchStores {
            runs: RunStore::new(deps.home.clone()),
            state: DispatchStateStore::with_clock(deps.home.clone(), deps.clock.clone()),
            log: DispatchLog::with_clock(deps.home.clone(), deps.clock.clone()),
            reviews: ReviewStore::new(deps.home.clone()),
        };
        let configs = DispatchConfigStore::new(deps.home.clone());
        Self {
            stores,
            deps,
            configs,
            window: TimeWindowCheck::new(),
            last_report: Mutex::new(None),
            running: tokio::sync::Mutex::new(None),
            lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Zählt der Lauf zum WIP der Tickets? Nur Umsetzung und Merge-Session; Recherchen und
    /// Planungen laufen nebenher
    pub fn counts_for_wip(run: &RunRecord) -> bool {
        matches!(run.kind, RunKind::Work | RunKind::Merge)
    }

    /// Bericht des letzten Durchlaufs
    pub fn last_report(&self) -> Option<DispatchReport> {
        self.last_report.lock().expect("Bericht").clone()
    }

    /// Ein Durchlauf; steht schon einer an, teilen sich beide Aufrufe dessen Ergebnis. Läuft
    /// gerade eine Arbeit von Hand, wartet der Durchlauf auf ihr Ende (gemeinsame Sperre)
    pub async fn tick(&self) -> DispatchReport {
        // Wer den Platz bekommt, führt den Durchlauf aus; alle anderen warten auf sein Ergebnis
        let mine = {
            let mut slot = self.running.lock().await;
            match slot.clone() {
                Some(receiver) => Err(receiver),
                None => {
                    let (sender, receiver) = watch::channel(None);
                    *slot = Some(receiver);
                    Ok(sender)
                }
            }
        };
        match mine {
            Ok(sender) => {
                let report = self.exclusive(|| self.run_tick()).await;
                *self.running.lock().await = None;
                let _ = sender.send(Some(report.clone()));
                report
            }
            Err(mut receiver) => {
                loop {
                    if let Some(report) = receiver.borrow_and_update().clone() {
                        return report;
                    }
                    // Der Durchlauf ist ohne Ergebnis weggefallen: selbst einen fahren
                    if receiver.changed().await.is_err() {
                        break;
                    }
                }
                self.exclusive(|| self.run_tick()).await
            }
        }
    }

    /// Führt `work` aus, sobald alle vorher angemeldeten Arbeiten (Durchläufe, „Erneut
    /// mergen“, „Abnehmen“ …) fertig sind
    pub async fn exclusive<T, F, Fut>(&self, work: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let _guard = self.lock.lock().await;
        work().await
    }

    /// Pausiert neue Starts für `minutes` Minuten
    pub async fn pause(&self, minutes: i64) -> Result<()> {
        let until = iso(self.deps.clock.now() + chrono::Duration::minutes(minutes));
        let value = until.clone();
        self.stores
            .state
            .update(move |state| state.paused_until = Some(value))
            .await?;
        self.stores
            .log
            .line(&format!("Pause bis {until} (von Hand)"));
        Ok(())
    }

    /// Hebt die Pause auf
    pub async fn resume(&self) -> Result<()> {
        self.stores
            .state
            .update(|state| state.paused_until = None)
            .await?;
        self.stores.log.line("Pause aufgehoben");
        Ok(())
    }

    /// Schaltet den Dispatcher ein oder aus (schreibt `dispatch.yaml`)
    pub async fn set_enabled(&self, on: bool) -> Result<()> {
        let mut raw = self.configs.read()?.raw;
        match raw.as_object_mut() {
            Some(fields) => {
                fields.insert("enabled".into(), serde_json::Value::Bool(on));
            }
            None => raw = serde_json::json!({ "enabled": on }),
        }
        self.configs.write(&raw)?;
        self.stores
            .log
            .line(if on { "eingeschaltet" } else { "ausgeschaltet" });
        Ok(())
    }

    // ------------------------------------------------------------------------------------
    // Merge und Review von Hand
    // ------------------------------------------------------------------------------------

    /// „Erneut mergen“ (Mensch): unter der Sperre der Durchläufe Auftrag setzen und sofort einen
    /// Durchlauf ausführen (kein Takt-Durchlauf dazwischen); liefert den Bericht samt Ergebnis
    /// für dieses Ticket. Fehler (kein Merge möglich, fremdes Board, Merge-Session läuft noch)
    /// werden geworfen
    pub async fn retry_merge(&self, id: &str) -> Result<MergeRetryReport> {
        self.exclusive(|| async {
            let cfg = self.configs.read()?.config;
            let ticket_id = self
                .with_context(cfg, |ctx| async move {
                    let actor = ctx.user_actor(None);
                    self.deps.merge.retry(&ctx, id, &actor).await
                })
                .await?;
            self.merge_report(&ticket_id, self.run_tick().await).await
        })
        .await
    }

    /// „Abnehmen“ (Mensch) eines Human-Interaction-Tickets: unter der Sperre der Durchläufe
    /// erledigen, auf eigenem Board den Merge anstoßen und sofort einen Durchlauf ausführen;
    /// liefert den Bericht samt Ergebnis (gemergt, wartet … oder nur erledigt)
    pub async fn accept(&self, id: &str, by: Option<&str>) -> Result<MergeRetryReport> {
        self.exclusive(|| async {
            let cfg = self.configs.read()?.config;
            let by = by.map(str::trim).filter(|name| !name.is_empty());
            let accepted = self
                .with_context(cfg, |ctx| async move {
                    let actor = ctx.user_actor(by);
                    self.deps.merge.accept(&ctx, id, &actor).await
                })
                .await?;
            let report = self.run_tick().await;
            if !accepted.merge {
                return Ok(MergeRetryReport {
                    report,
                    result: MergeRetryResult {
                        outcome: MergeRetryOutcome::Accepted,
                        text: accepted.text,
                    },
                });
            }
            self.merge_report(&accepted.ticket_id, report).await
        })
        .await
    }

    /// Review vor dem Merge: Dateien des Ticket-Branches gegen das Ziel und Testläufe des Agenten
    pub async fn review(&self, id: &str) -> Result<ReviewView> {
        let cfg = self.configs.read()?.config;
        self.with_context(cfg, |ctx| async move {
            ReviewService::new().view(&ctx, id).await
        })
        .await
    }

    /// Patch einer Datei aus der Dateiliste des Reviews
    pub async fn review_file(&self, id: &str, path: &str) -> Result<ReviewFilePatch> {
        let cfg = self.configs.read()?.config;
        self.with_context(cfg, |ctx| async move {
            ReviewService::new().file_diff(&ctx, id, path).await
        })
        .await
    }

    /// „Zurück an den Agenten“ (Mensch) mit Rückmeldung: unter der Sperre der Durchläufe zurück
    /// nach Geplant, ohne sofortigen Durchlauf; der nächste Durchlauf setzt im selben Worktree
    /// fort
    pub async fn return_to_agent(
        &self,
        id: &str,
        comment: &str,
        by: Option<&str>,
    ) -> Result<MergeRetryReport> {
        self.exclusive(|| async {
            let cfg = self.configs.read()?.config;
            let by = by.map(str::trim).filter(|name| !name.is_empty());
            let returned = self
                .with_context(cfg, |ctx| async move {
                    let actor = ctx.user_actor(by);
                    ReviewService::new()
                        .return_to_agent(&ctx, id, comment, &actor)
                        .await
                })
                .await?;
            Ok(MergeRetryReport {
                report: DispatchReport::new(iso(self.deps.clock.now())),
                result: MergeRetryResult {
                    outcome: MergeRetryOutcome::Returned,
                    text: returned.text,
                },
            })
        })
        .await
    }

    /// Bericht samt Ergebnis für das angefragte Ticket (Status, Grund, Kommentare und
    /// Merge-Auftrag nach dem Durchlauf)
    async fn merge_report(
        &self,
        ticket_id: &str,
        report: DispatchReport,
    ) -> Result<MergeRetryReport> {
        let wanted = ticket_id.to_string();
        // Ein nicht mehr lesbares Ticket (gelöscht, Verbindung weg) liest der Leser als „None“
        let detail = self
            .deps
            .boards
            .run(|backend| {
                let wanted = wanted.clone();
                async move { backend.services().tickets.get(&wanted).await }
            })
            .await
            .ok();
        let entry = self.stores.state.read()?.merges.get(ticket_id).cloned();
        let result = MergeResultReader::new().read(
            ticket_id,
            &report.merged,
            &report.errors,
            detail.as_ref(),
            entry.as_ref(),
        );
        Ok(MergeRetryReport { report, result })
    }

    /// „Recherche starten“ (von Hand): startet sofort, unabhängig von Ein/Aus, Zeitfenster und
    /// Opt-in; abgelehnt bei Pause (z. B. Nutzungslimit), nicht offener Frage, schon laufender
    /// Recherche oder fehlendem Repo. Läuft unter der Sperre der Durchläufe
    pub async fn start_research(&self, question_id: &str) -> Result<DispatchRunInfo> {
        self.exclusive(|| async {
            self.check_not_paused("die Recherche")?;
            let cfg = self.configs.read()?.config;
            let record = self
                .with_context(cfg, |ctx| async move {
                    ResearchRunner::new(&ctx)
                        .start(question_id, StartMode::Manual)
                        .await
                })
                .await?;
            Ok(run_info(&record))
        })
        .await
    }

    /// „Planen“ (von Hand): startet sofort, unabhängig von Ein/Aus, Zeitfenster und Opt-in;
    /// abgelehnt bei Pause (z. B. Nutzungslimit), fremdem Board, falschem Status, offener Frage
    /// oder schon laufender Planung. Läuft unter der Sperre der Durchläufe
    pub async fn start_planning(&self, ticket_id: &str) -> Result<DispatchRunInfo> {
        self.exclusive(|| async {
            self.check_not_paused("die Planung")?;
            let cfg = self.configs.read()?.config;
            let record = self
                .with_context(cfg, |ctx| async move {
                    PlanRunner::new(&ctx)
                        .start(ticket_id, StartMode::Manual)
                        .await
                })
                .await?;
            Ok(run_info(&record))
        })
        .await
    }

    /// Läuft für die Frage gerade eine Recherche? Dazu Pause und Zähler (ohne Datenbank)
    pub async fn research_status(&self, question_id: &str) -> Result<QuestionResearchStatus> {
        let id = QuestionIds::canonical(question_id)?;
        let run = self
            .stores
            .runs
            .list()?
            .into_iter()
            .find(|run| {
                run.kind == RunKind::Research
                    && run.question_id.as_deref().unwrap_or(&run.ticket_id) == id
            })
            .map(|run| run_info(&run));
        let state = self.stores.state.read()?;
        let entry = state.research.get(&id).cloned();
        Ok(QuestionResearchStatus {
            question_id: id,
            run,
            paused_until: self.paused_until(state.paused_until.as_deref()),
            last_started_at: entry.as_ref().map(|entry| entry.last_started_at.clone()),
            count: entry.map(|entry| entry.count).unwrap_or(0),
        })
    }

    /// Läuft für das Ticket gerade eine Planung? Dazu die Pause (ohne Datenbank)
    pub async fn planning_status(&self, ticket_id: &str) -> Result<TicketPlanningStatus> {
        let id = ticket_id.trim().to_uppercase();
        let run = self
            .stores
            .runs
            .list()?
            .into_iter()
            .find(|run| run.kind == RunKind::Plan && run.ticket_id == id)
            .map(|run| run_info(&run));
        let state = self.stores.state.read()?;
        Ok(TicketPlanningStatus {
            ticket_id: id,
            run,
            paused_until: self.paused_until(state.paused_until.as_deref()),
        })
    }

    /// Läufe eines Tickets oder einer Frage aus den Protokollen, neueste zuerst; `running`:
    /// der Lauf steht im Lauf-Speicher und der Worker hat noch kein Ergebnis geschrieben
    pub async fn ticket_runs(&self, id: &str) -> Result<Vec<DispatchRunLog>> {
        let logs = self.stores.log.runs(id)?;
        let names: HashSet<String> = logs.iter().map(|entry| entry.name.clone()).collect();
        let mut active: HashSet<String> = HashSet::new();
        for run in self.stores.runs.list()? {
            let name = format!("{}.log", run.id);
            if names.contains(&name) && self.stores.runs.result(&run.id)?.is_none() {
                active.insert(name);
            }
        }
        Ok(logs
            .into_iter()
            .map(|entry| {
                let running = active.contains(&entry.name);
                DispatchRunLog::new(entry, running)
            })
            .collect())
    }

    /// Ein Lauf-Protokoll (Ende der Datei), zerlegt in Kopf, Arbeitsanweisung und Ausgabe
    pub async fn run_log(&self, name: &str) -> Result<DispatchRunLogText> {
        let text = self.stores.log.read(name)?;
        Ok(DispatchRunLogText {
            name: name.to_string(),
            parts: RunLogSplitter::new().split(&text),
        })
    }

    /// Ein ausdrücklich genanntes Board muss es geben (`board-not-found` wie bei den anderen
    /// Befehlen); ist die Datenbank nicht erreichbar, prüft erst der Zustand (Meldung in
    /// `ticketsError`)
    pub async fn require_board(&self, board_id: Option<i32>) -> Result<()> {
        let Some(board_id) = board_id else {
            return Ok(());
        };
        let found = self
            .deps
            .boards
            .run(|backend| async move {
                backend.services().boards.require(board_id).await?;
                Ok(())
            })
            .await;
        match found {
            Err(error) if error.code() == Some("board-not-found") => Err(error),
            _ => Ok(()),
        }
    }

    /// Zustand für Oberfläche und Kommandozeile; mit Board (Standard: aktives Board) auch die
    /// Bereitschaft seiner Geplant-Tickets
    pub async fn status(&self, requested: Option<i32>) -> Result<DispatchStatus> {
        let board_id = requested.or_else(|| self.active_board());
        let (cfg, config_error) = match self.configs.read() {
            Ok(read) => (Some(read.config), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let now = self.deps.clock.now();
        let state = self.stores.state.read()?;
        let all = self.stores.runs.list()?;
        let active = all.iter().filter(|run| Self::counts_for_wip(run)).count() as i64;
        let researching = all
            .iter()
            .filter(|run| run.kind == RunKind::Research)
            .count() as i64;
        let planning = all.iter().filter(|run| run.kind == RunKind::Plan).count() as i64;
        let paused = self.paused_until(state.paused_until.as_deref());
        let wip = cfg.as_ref().map(|cfg| cfg.wip).unwrap_or(0);
        let mut tickets: Vec<DispatchTicketRow> = Vec::new();
        let mut board_in_scope = false;
        let mut tickets_error: Option<String> = None;
        if let (Some(board_id), Some(cfg)) = (board_id, cfg.as_ref()) {
            match self
                .rows(cfg.clone(), board_id, (wip - active).max(0))
                .await
            {
                Ok(rows) => {
                    tickets = rows.0;
                    board_in_scope = rows.1;
                }
                Err(error) => tickets_error = Some(error.to_string()),
            }
        }
        let kind = match cfg.as_ref() {
            None => DispatchStateKind::Error,
            Some(cfg) if !cfg.enabled => DispatchStateKind::Off,
            Some(_) if paused.is_some() => DispatchStateKind::Paused,
            Some(cfg) if !self.in_window(cfg) => DispatchStateKind::Outside,
            Some(_) if active > 0 => DispatchStateKind::Running,
            Some(_) => DispatchStateKind::Idle,
        };
        let window = cfg.as_ref().and_then(|cfg| cfg.window.clone());
        let message = match kind {
            DispatchStateKind::Error => format!(
                "Einstellung fehlerhaft: {}",
                config_error.clone().unwrap_or_default()
            ),
            DispatchStateKind::Off => "Dispatcher ist ausgeschaltet".to_string(),
            DispatchStateKind::Paused => format!(
                "pausiert bis {}",
                hhmm(
                    paused
                        .as_deref()
                        .and_then(|at| at.parse().ok())
                        .unwrap_or(now)
                )
            ),
            DispatchStateKind::Outside => window
                .as_ref()
                .map(|window| format!("außerhalb {}–{}", window.from, window.to))
                .unwrap_or_default(),
            DispatchStateKind::Running => format!("läuft · {active}/{wip}"),
            DispatchStateKind::Idle => format!("bereit · 0/{wip}"),
        };
        let last = self.last_report();
        Ok(DispatchStatus {
            enabled: cfg.as_ref().map(|cfg| cfg.enabled).unwrap_or(false),
            state: kind,
            message,
            paused_until: paused,
            wip,
            active,
            planning,
            runs: all.iter().map(run_info).collect(),
            research: DispatchResearchInfo {
                auto: cfg.as_ref().map(|cfg| cfg.research.auto).unwrap_or(false),
                wip: cfg.as_ref().map(|cfg| cfg.research.wip).unwrap_or(1),
                active: researching,
                max_per_day: cfg
                    .as_ref()
                    .map(|cfg| cfg.research.max_per_day)
                    .unwrap_or(10),
                today: if state.research_auto.day.as_deref()
                    == Some(ResearchRunner::day(now).as_str())
                {
                    state.research_auto.count
                } else {
                    0
                },
                since: state.research_auto.since.clone(),
                retry_at: future(state.research_auto.retry_at.as_deref(), now),
                error: state.research_auto.last_error.clone(),
            },
            planning_auto: DispatchPlanningInfo {
                auto: cfg.as_ref().map(|cfg| cfg.planning.auto).unwrap_or(false),
                wip: cfg.as_ref().map(|cfg| cfg.planning.wip).unwrap_or(1),
                active: planning,
                since: state.planning_auto.since.clone(),
                retry_at: future(state.planning_auto.retry_at.as_deref(), now),
                error: state.planning_auto.last_error.clone(),
            },
            merges: self
                .merge_infos(state.merges.values().cloned().collect())
                .await,
            board_id,
            board_in_scope,
            tickets,
            tickets_error,
            last_tick: last.as_ref().map(|report| report.at.clone()),
            last_errors: last.map(|report| report.errors).unwrap_or_default(),
            config_file: self.configs.file().to_string_lossy().into_owned(),
            config_error,
            agent_prefix: cfg
                .as_ref()
                .map(|cfg| cfg.agent_prefix.clone())
                .unwrap_or_else(|| "junior-".to_string()),
        })
    }

    // ------------------------------------------------------------------------------------
    // Der Durchlauf
    // ------------------------------------------------------------------------------------

    /// Ablauf eines Durchlaufs ohne Sperre (für P4-15b: „Erneut mergen“ führt ihn innerhalb
    /// von [`DispatchService::exclusive`] aus); Fehler landen im Bericht und in `dispatch.log`
    pub async fn run_tick(&self) -> DispatchReport {
        let mut report = DispatchReport::new(iso(self.deps.clock.now()));
        if let Err(error) = self.tick_steps(&mut report).await {
            report.errors.push(error.to_string());
        }
        for error in &report.errors {
            self.stores.log.line(&format!("FEHLER {error}"));
        }
        *self.last_report.lock().expect("Bericht") = Some(report.clone());
        report
    }

    /// Die Schritte eines Durchlaufs; ein Verbindungsfehler bricht hier ab und steht danach im
    /// Bericht
    async fn tick_steps(&self, report: &mut DispatchReport) -> Result<()> {
        let cfg = self.configs.read()?.config;
        // Ausgeschaltet (auch die Automatiken) und nichts offen: keine Verbindung zur
        // Datenbank nötig (kein Fehler je Minute ohne Board-Konfig)
        if !cfg.enabled
            && !cfg.research.auto
            && !cfg.planning.auto
            && self.stores.runs.list()?.is_empty()
            && self.stores.state.read()?.merges.is_empty()
        {
            self.follow_research_auto(false).await?;
            self.follow_planning_auto(false).await?;
            report.idle = Some("Dispatcher ist ausgeschaltet".to_string());
            return Ok(());
        }
        self.follow_research_auto(cfg.research.auto).await?;
        self.follow_planning_auto(cfg.planning.auto).await?;
        let board = BoardConfigStore::new(self.deps.home.clone()).require()?;
        let me = RepoBoardOwner {
            user_id: board.user_id.clone(),
            active_board: board.active_board,
        };
        self.deps
            .boards
            .run(|backend| async move {
                let ctx = self.context(cfg, me, backend);
                let sessions: Arc<dyn MergeSessionEnd> = self.deps.merge.clone();
                let results = ResultHandler::new(&ctx, self.deps.reviews.clone(), sessions);
                let research = ResearchRunner::new(&ctx);
                self.evaluate_runs(&ctx, &results, &research, report)
                    .await?;
                self.remove_research_orphans(&ctx).await?;
                self.release_stale(&ctx, &results, report).await?;
                let merged = self.deps.merge.process_all(&ctx).await?;
                report.merged.extend(merged.merged);
                report.errors.extend(merged.errors);
                self.start_ready(&ctx, &results, &research, report).await?;
                // Vorgemerkte Läufe (ausgewertet oder gescheiterter Start) in die Auswertung
                // schreiben
                RunRecorder::new(&ctx).flush().await?;
                Ok(())
            })
            .await
    }

    /// Beendete Läufe auswerten; Worker ohne Ergebnis, deren Prozess fehlt oder die weit über
    /// dem Zeitlimit liegen (Prozess-ID vermutlich neu vergeben), gelten als abgestürzt.
    /// Verbindungsfehler brechen den Durchlauf ab, der Lauf bleibt.
    async fn evaluate_runs(
        &self,
        ctx: &DispatchContext,
        results: &ResultHandler<'_>,
        research: &ResearchRunner<'_>,
        report: &mut DispatchReport,
    ) -> Result<()> {
        let recorder = RunRecorder::new(ctx);
        for record in self.stores.runs.list()? {
            let mut reason: Option<String> = None;
            let end = match self.stores.runs.result(&record.id)? {
                Some(end) => end,
                None => {
                    reason = self.vanished(&record);
                    if reason.is_none() {
                        continue;
                    }
                    RunEnd {
                        result: RunResult {
                            code: None,
                            timed_out: false,
                            output: String::new(),
                            ..RunResult::default()
                        },
                        ended_at: ctx.now_iso(),
                    }
                }
            };
            match self
                .evaluate_one(ctx, results, research, &record, &end, reason.as_deref())
                .await
            {
                Ok((outcome, result)) => {
                    report.evaluated.push(EvaluatedRun {
                        ticket_id: record.ticket_id.clone(),
                        outcome: outcome.clone(),
                    });
                    ctx.log(&format!(
                        "beendet {} ({}): {outcome}",
                        record.ticket_id,
                        record.kind.as_str()
                    ));
                    // Vor dem Entfernen vormerken: so kommt jeder ausgewertete Lauf genau
                    // einmal in die Auswertung (Planungen lässt der Recorder aus)
                    recorder.keep(&record, &end, &result).await;
                }
                Err(error) => {
                    if BoardRuntimeHolder::is_connection_error(&error) {
                        return Err(error);
                    }
                    report.errors.push(format!("{}: {error}", record.ticket_id));
                }
            }
            self.stores.runs.remove(&record.id)?;
        }
        Ok(())
    }

    /// Ein Lauf je nach Art: Recherche, Planung oder Umsetzung bzw. Merge-Session
    async fn evaluate_one(
        &self,
        ctx: &DispatchContext,
        results: &ResultHandler<'_>,
        research: &ResearchRunner<'_>,
        record: &RunRecord,
        end: &RunEnd,
        reason: Option<&str>,
    ) -> Result<(String, HandleResult)> {
        match record.kind {
            RunKind::Research => {
                let outcome = research.evaluate(record, end, reason).await?;
                // Recherchen stehen in `board.DispatchRuns` immer als `research`; der Ausgang
                // hier zählt nur für den Bericht
                Ok((
                    outcome.as_str().to_string(),
                    HandleResult::new(HandleOutcome::Other),
                ))
            }
            RunKind::Plan => {
                let outcome = PlanRunner::new(ctx).evaluate(record, end, reason).await?;
                // Planungen lässt der Recorder aus
                Ok((
                    outcome.as_str().to_string(),
                    HandleResult::new(HandleOutcome::Other),
                ))
            }
            _ => {
                let result = results.handle(record, end, reason).await?;
                Ok((result.outcome.as_str().to_string(), result))
            }
        }
    }

    /// Grund, warum ein Lauf ohne Ergebnis als verschwunden gilt, sonst `None`
    fn vanished(&self, record: &RunRecord) -> Option<String> {
        let started = parse_millis(&record.started_at)?;
        let age = self.deps.clock.now().timestamp_millis() - started;
        // Die Einrichtung (worktreeSetup) läuft vor Claude und hat ein eigenes Zeitlimit je
        // Befehl
        let setup_minutes = record.setup.as_ref().map(|setup| setup.len()).unwrap_or(0) as i64
            * record.setup_timeout_minutes.unwrap_or(0);
        if age > (record.run.timeout_minutes + setup_minutes + OVERDUE_MINUTES) * 60_000 {
            return Some(format!(
                "Worker lieferte {OVERDUE_MINUTES} min nach dem Zeitlimit noch kein Ergebnis (Prozess hängt oder Prozess-ID neu vergeben)"
            ));
        }
        let gone = match record.pid {
            None => age > 120_000,
            Some(pid) => !(self.deps.alive)(pid as u32),
        };
        gone.then(|| {
            "Worker-Prozess endete ohne Ergebnis (abgestürzt oder Rechner neu gestartet)"
                .to_string()
        })
    }

    /// Recherche-Automatik nachführen: an ohne Zeitpunkt → ab jetzt (nur neuere Fragen);
    /// aus → Zeitpunkt vergessen, damit erneutes Einschalten neu beginnt
    async fn follow_research_auto(&self, on: bool) -> Result<()> {
        let since = self.stores.state.read()?.research_auto.since.is_some();
        if on == since {
            return Ok(());
        }
        self.research_auto_switched(on).await
    }

    /// Die Recherche-Automatik wurde ein- oder ausgeschaltet (Einstellung gespeichert oder vom
    /// Durchlauf bemerkt): beim Einschalten gilt sie ab jetzt, Aussetzen nach Fehlern und
    /// Fehlertext beginnen neu; beim Ausschalten wird der Zeitpunkt vergessen
    pub async fn research_auto_switched(&self, on: bool) -> Result<()> {
        let at = iso(self.deps.clock.now());
        let value = at.clone();
        self.stores
            .state
            .update(move |state| {
                if on {
                    state.research_auto.since = Some(value);
                    state.research_auto.retry_at = None;
                    state.research_auto.last_error = None;
                } else {
                    state.research_auto.since = None;
                }
            })
            .await?;
        self.stores.log.line(&if on {
            format!("Recherche-Automatik an (für Fragen ab {at})")
        } else {
            "Recherche-Automatik aus".to_string()
        });
        Ok(())
    }

    /// Planungs-Automatik nachführen: an ohne Zeitpunkt → ab jetzt (nur neuere Tickets);
    /// aus → Zeitpunkt vergessen
    async fn follow_planning_auto(&self, on: bool) -> Result<()> {
        let since = self.stores.state.read()?.planning_auto.since.is_some();
        if on == since {
            return Ok(());
        }
        self.planning_auto_switched(on).await
    }

    /// Die Planungs-Automatik wurde ein- oder ausgeschaltet: beim Einschalten gilt sie ab
    /// jetzt, Aussetzen nach Fehlern und Fehlertext beginnen neu; beim Ausschalten wird der
    /// Zeitpunkt vergessen
    pub async fn planning_auto_switched(&self, on: bool) -> Result<()> {
        let at = iso(self.deps.clock.now());
        let value = at.clone();
        self.stores
            .state
            .update(move |state| {
                if on {
                    state.planning_auto.since = Some(value);
                    state.planning_auto.retry_at = None;
                    state.planning_auto.last_error = None;
                } else {
                    state.planning_auto.since = None;
                }
            })
            .await?;
        self.stores.log.line(&if on {
            format!("Planungs-Automatik an (für Tickets ab {at})")
        } else {
            "Planungs-Automatik aus".to_string()
        });
        Ok(())
    }

    /// Verwaiste Recherche- und Planungs-Worktrees (ohne Lauf) in den Repos der eigenen Boards
    /// entfernen; Fehler nur ins Protokoll
    async fn remove_research_orphans(&self, ctx: &DispatchContext) -> Result<()> {
        let runs = self.stores.runs.list()?;
        let active: Vec<String> = cwds(&runs, RunKind::Research);
        let planning: Vec<String> = cwds(&runs, RunKind::Plan);
        let research = ResearchRunner::new(ctx);
        let plans = PlanRunner::new(ctx);
        let boards = ctx.services().boards.owned(&ctx.me.user_id).await?;
        let mut repos: Vec<String> = Vec::new();
        for found in boards {
            if let Some(path) = found.board.repo_path {
                if !repos.contains(&path) {
                    repos.push(path);
                }
            }
        }
        for repo in repos {
            for note in research.remove_orphans(&repo, &active) {
                ctx.log(&note);
            }
            for note in plans.remove_orphans(&repo, &planning) {
                ctx.log(&note);
            }
        }
        Ok(())
    }

    /// Boards im Bereich, die dem eigenen Benutzer gehören (fremde Boards bedient deren
    /// Besitzer)
    async fn own_boards(&self, ctx: &DispatchContext) -> Result<Vec<Board>> {
        let mut boards = Vec::new();
        for board_id in ctx.scope() {
            if let Some(board) = ctx.board(board_id).await? {
                if board.owner_id == ctx.me.user_id {
                    boards.push(board);
                }
            }
        }
        Ok(boards)
    }

    /// Agent-Tickets ohne Lauf zurückgeben: In Arbeit ohne Änderung seit `staleMinutes`
    /// (zählt als Versuch), Geplant mit Agent seit [`CLAIM_STALE_MINUTES`] (hängende
    /// Übernahme, zählt nicht)
    async fn release_stale(
        &self,
        ctx: &DispatchContext,
        results: &ResultHandler<'_>,
        report: &mut DispatchReport,
    ) -> Result<()> {
        let active: HashSet<String> = self
            .stores
            .runs
            .list()?
            .into_iter()
            .map(|run| run.ticket_id)
            .collect();
        let limit = ctx.cfg.stale_minutes * 60_000;
        for board in self.own_boards(ctx).await? {
            let views = match ctx
                .services()
                .tickets
                .list(&TicketListFilter {
                    board_ids: vec![board.id],
                    ..TicketListFilter::default()
                })
                .await
            {
                Ok(views) => views,
                Err(error) => {
                    if BoardRuntimeHolder::is_connection_error(&error) {
                        return Err(error);
                    }
                    report.errors.push(format!("Board {}: {error}", board.id));
                    continue;
                }
            };
            for view in views {
                if !ctx.is_agent(view.ticket.assignee.as_deref())
                    || active.contains(&view.ticket.id)
                {
                    continue;
                }
                let idle = ctx.now().timestamp_millis()
                    - parse_millis(&view.ticket.updated_at).unwrap_or(0);
                if view.ticket.status == TicketStatus::Planned
                    && idle > CLAIM_STALE_MINUTES * 60_000
                {
                    match ctx
                        .services()
                        .tickets
                        .release(&view.ticket.id, ctx.actor(), Some(TicketStatus::Planned))
                        .await
                    {
                        Ok(_) => ctx.log(&format!(
                            "Übernahme {} durch {} hing seit mehr als {CLAIM_STALE_MINUTES} min – freigegeben",
                            view.ticket.id,
                            view.ticket.assignee.as_deref().unwrap_or_default()
                        )),
                        Err(error) => {
                            if BoardRuntimeHolder::is_connection_error(&error) {
                                return Err(error);
                            }
                            report.errors.push(format!("{}: {error}", view.ticket.id));
                        }
                    }
                    continue;
                }
                if view.ticket.status != TicketStatus::InProgress || idle <= limit {
                    continue;
                }
                match self.abort_stale(ctx, results, &view).await {
                    Ok(outcome) => report.evaluated.push(EvaluatedRun {
                        ticket_id: view.ticket.id.clone(),
                        outcome,
                    }),
                    Err(error) => {
                        if BoardRuntimeHolder::is_connection_error(&error) {
                            return Err(error);
                        }
                        report.errors.push(format!("{}: {error}", view.ticket.id));
                    }
                }
            }
        }
        Ok(())
    }

    /// Bricht ein Agent-Ticket ohne Lebenszeichen ab; der Ausgang für den Bericht
    async fn abort_stale(
        &self,
        ctx: &DispatchContext,
        results: &ResultHandler<'_>,
        view: &TicketView,
    ) -> Result<String> {
        let detail = ctx.services().tickets.get(&view.ticket.id).await?;
        let reason = format!(
            "Kein Lebenszeichen seit mehr als {} min (Worker vermutlich abgestürzt oder Rechner neu gestartet)",
            ctx.cfg.stale_minutes
        );
        let result = results.abort(&detail, &reason, "", None).await?;
        Ok(result.outcome.as_str().to_string())
    }

    /// Bereite Tickets bis zum WIP-Limit starten (nur eingeschaltet, ohne Pause, im
    /// Zeitfenster), danach – unabhängig vom Schalter für Tickets – die Recherche- und die
    /// Planungs-Automatik
    async fn start_ready(
        &self,
        ctx: &DispatchContext,
        results: &ResultHandler<'_>,
        research: &ResearchRunner<'_>,
        report: &mut DispatchReport,
    ) -> Result<()> {
        let now = self.deps.clock.now();
        let paused = self.paused_until(self.stores.state.read()?.paused_until.as_deref());
        let in_window = self.in_window(&ctx.cfg);
        if !ctx.cfg.enabled {
            report.idle = Some("Dispatcher ist ausgeschaltet".to_string());
        } else if let Some(until) = &paused {
            report.idle = Some(format!(
                "Pausiert bis {}",
                hhmm(until.parse().unwrap_or(now))
            ));
        } else if !in_window {
            let window = ctx.cfg.window.as_ref().expect("Fenster außerhalb der Zeit");
            report.idle = Some(format!(
                "Außerhalb des Zeitfensters {}–{}",
                window.from, window.to
            ));
        }
        if report.idle.is_none() {
            self.start_tickets(ctx, results, report).await?;
        }
        let blocked = paused.is_some() || !in_window;
        if ctx.cfg.research.auto && !blocked {
            let auto = research.start_auto().await?;
            report.research.extend(auto.started);
            report.errors.extend(auto.errors);
        }
        if ctx.cfg.planning.auto && !blocked {
            // Geplant wird nur, solange im WIP der Tickets noch Platz ist (Planungen zählen
            // selbst nicht mit)
            let free = ctx.cfg.wip
                - self
                    .stores
                    .runs
                    .list()?
                    .iter()
                    .filter(|run| Self::counts_for_wip(run))
                    .count() as i64;
            let auto = PlanRunner::new(ctx).start_auto(free).await?;
            report.planned.extend(auto.started);
            report.errors.extend(auto.errors);
        }
        Ok(())
    }

    /// Bereite Tickets der eigenen Boards bis zum WIP-Limit starten (Recherche-Läufe zählen
    /// nicht mit)
    async fn start_tickets(
        &self,
        ctx: &DispatchContext,
        results: &ResultHandler<'_>,
        report: &mut DispatchReport,
    ) -> Result<()> {
        let all = self.stores.runs.list()?;
        let mut free =
            ctx.cfg.wip - all.iter().filter(|run| Self::counts_for_wip(run)).count() as i64;
        // Ein Ticket, das gerade geplant wird, startet nicht (der Plan ändert sich noch)
        let planning: HashSet<String> = all
            .iter()
            .filter(|run| run.kind == RunKind::Plan)
            .map(|run| run.ticket_id.clone())
            .collect();
        let starter = TicketStarter::new(ctx, results);
        for board in self.own_boards(ctx).await? {
            if let Err(error) = self
                .start_board(ctx, &starter, &board, &planning, &mut free, report)
                .await
            {
                if BoardRuntimeHolder::is_connection_error(&error) {
                    return Err(error);
                }
                report.errors.push(format!("Board {}: {error}", board.id));
            }
        }
        Ok(())
    }

    /// Die bereiten Tickets eines Boards; fachliche Fehler eines Tickets landen im Bericht,
    /// die übrigen Tickets werden trotzdem bedient
    async fn start_board(
        &self,
        ctx: &DispatchContext,
        starter: &TicketStarter<'_>,
        board: &Board,
        planning: &HashSet<String>,
        free: &mut i64,
        report: &mut DispatchReport,
    ) -> Result<()> {
        let readiness = ReadinessBoard {
            owner_id: board.owner_id.clone(),
            repo_path: board.repo_path.clone(),
        };
        let exists = |path: &str| Path::new(path).exists();
        let mut work: Option<Arc<RepoWork>> = None;
        let views = ctx
            .services()
            .tickets
            .list(&TicketListFilter {
                board_ids: vec![board.id],
                ..TicketListFilter::default()
            })
            .await?;
        for view in planned(views) {
            if planning.contains(&view.ticket.id)
                || ctx
                    .readiness()
                    .reason(
                        &ReadinessTicket::from(&view),
                        &readiness,
                        &ctx.me.user_id,
                        &exists,
                    )
                    .is_some()
            {
                continue;
            }
            if *free <= 0 {
                report.queued.push(view.ticket.id.clone());
                continue;
            }
            let work = match &work {
                Some(work) => work.clone(),
                None => {
                    let fresh = ctx.repo(board).await?;
                    work = Some(fresh.clone());
                    fresh
                }
            };
            match starter.start(&view, &work).await {
                Ok(StartOutcome::Started(_)) => {
                    report.started.push(view.ticket.id.clone());
                    *free -= 1;
                }
                Ok(StartOutcome::Error(error)) => {
                    report.errors.push(format!("{}: {error}", view.ticket.id));
                }
                Ok(StartOutcome::Skipped(_)) => {}
                Err(error) => {
                    if BoardRuntimeHolder::is_connection_error(&error) {
                        return Err(error);
                    }
                    report.errors.push(format!("{}: {error}", view.ticket.id));
                }
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------------------------
    // Hilfen des Zustands
    // ------------------------------------------------------------------------------------

    /// Bereitschaft der Geplant-Tickets eines Boards für den Zustand
    async fn rows(
        &self,
        cfg: DispatchConfig,
        board_id: i32,
        free: i64,
    ) -> Result<(Vec<DispatchTicketRow>, bool)> {
        let board = BoardConfigStore::new(self.deps.home.clone()).require()?;
        let me = RepoBoardOwner {
            user_id: board.user_id.clone(),
            active_board: board.active_board,
        };
        self.deps
            .boards
            .run(|backend| async move {
                let ctx = DispatchContext::new(
                    cfg,
                    me,
                    self.deps.home.clone(),
                    backend,
                    self.stores.clone(),
                    self.deps.launcher.clone(),
                    self.deps.clock.clone(),
                    Arc::new(NoDispatchNotify),
                    self.deps.env.clone(),
                    self.deps.platform.clone(),
                );
                let Some(target) = ctx.board(board_id).await? else {
                    return Ok((Vec::new(), false));
                };
                let readiness = ReadinessBoard {
                    owner_id: target.owner_id.clone(),
                    repo_path: target.repo_path.clone(),
                };
                let exists = |path: &str| Path::new(path).exists();
                let mut left = free;
                let mut work: Option<Option<Arc<RepoWork>>> = None;
                let mut tickets = Vec::new();
                let views = ctx
                    .services()
                    .tickets
                    .list(&TicketListFilter {
                        board_ids: vec![board_id],
                        without_texts: true,
                        ..TicketListFilter::default()
                    })
                    .await?;
                for view in planned(views) {
                    let mut reason = ctx.readiness().reason(
                        &ReadinessTicket::from(&view),
                        &readiness,
                        &ctx.me.user_id,
                        &exists,
                    );
                    if reason.is_none() {
                        // Läuft im Worktree des Tickets schon ein Worker, wartet es, ohne
                        // einen freien Platz zu belegen
                        if work.is_none() {
                            work = Some(ctx.repo(&target).await.ok());
                        }
                        if let Some(repo) = work.as_ref().and_then(|work| work.as_ref()) {
                            reason = ctx
                                .worktree_busy(&view.ticket.id, view.ticket.branch.as_deref(), repo)
                                .await
                                .unwrap_or(None);
                        }
                    }
                    if reason.is_none() {
                        reason = Some(if left > 0 {
                            "bereit".to_string()
                        } else {
                            "bereit – wartet auf freien Platz".to_string()
                        });
                        left -= 1;
                    }
                    tickets.push(DispatchTicketRow {
                        id: view.ticket.id.clone(),
                        title: view.ticket.title.clone(),
                        reason: reason.unwrap_or_default(),
                    });
                }
                let in_scope = ctx.scope().contains(&board_id);
                Ok((tickets, in_scope))
            })
            .await
    }

    /// Merge-Aufträge für den Zustand mit dem Zielbranch ihres Tickets (nicht
    /// `MergeEntry.target`, das erst die Merge-Session setzt); ohne Aufträge keine Abfrage,
    /// ist das Ticket oder die Datenbank nicht lesbar, steht kein Ziel
    async fn merge_infos(&self, entries: Vec<MergeEntry>) -> Vec<DispatchMergeInfo> {
        let mut targets: BTreeMap<String, Option<String>> = BTreeMap::new();
        if !entries.is_empty() {
            let ids: Vec<String> = entries
                .iter()
                .map(|entry| entry.ticket_id.clone())
                .collect();
            let found = self
                .deps
                .boards
                .run(|backend| async move {
                    let mut targets = BTreeMap::new();
                    for id in ids {
                        let target = backend
                            .services()
                            .tickets
                            .get(&id)
                            .await
                            .ok()
                            .and_then(|detail| detail.ticket.target_branch.clone());
                        targets.insert(id, target);
                    }
                    Ok(targets)
                })
                .await;
            if let Ok(found) = found {
                targets = found;
            }
        }
        entries
            .into_iter()
            .map(|entry| DispatchMergeInfo {
                target: targets.get(&entry.ticket_id).cloned().flatten(),
                ticket_id: entry.ticket_id,
                phase: entry.phase,
                branch: entry.branch,
            })
            .collect()
    }

    /// Aktives Board aus `board.yaml` (ohne oder mit kaputter Datei: keines)
    fn active_board(&self) -> Option<i32> {
        BoardConfigStore::new(self.deps.home.clone())
            .read()
            .ok()
            .flatten()
            .and_then(|config| config.active_board)
    }

    /// Die Pause, solange sie in der Zukunft liegt
    fn paused_until(&self, paused: Option<&str>) -> Option<String> {
        future(paused, self.deps.clock.now())
    }

    /// Dürfen jetzt neue Worker starten (Zeitfenster der Einstellung)?
    fn in_window(&self, cfg: &DispatchConfig) -> bool {
        self.window.contains(
            self.deps.clock.now().with_timezone(&Local).time(),
            cfg.window.as_ref(),
        )
    }

    /// Fehler `dispatch-paused`, solange der Dispatcher pausiert
    fn check_not_paused(&self, what: &str) -> Result<()> {
        let state = self.stores.state.read()?;
        let Some(until) = self.paused_until(state.paused_until.as_deref()) else {
            return Ok(());
        };
        let at = until.parse().unwrap_or_else(|_| self.deps.clock.now());
        Err(HarnessError::new(
            "dispatch-paused",
            format!(
                "Der Dispatcher pausiert bis {} (z. B. wegen Nutzungslimit) – {what} danach starten oder die Pause aufheben.",
                hhmm(at)
            ),
        )
        .into())
    }

    /// Kontext des eigenen Benutzers auf dieser Einstellung (nur lesend bzw. für Arbeiten von
    /// Hand, die selbst unter der Sperre laufen)
    async fn with_context<T, F, Fut>(&self, cfg: DispatchConfig, work: F) -> Result<T>
    where
        F: FnOnce(DispatchContext) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let board = BoardConfigStore::new(self.deps.home.clone()).require()?;
        let me = RepoBoardOwner {
            user_id: board.user_id.clone(),
            active_board: board.active_board,
        };
        self.deps
            .boards
            .run(|backend| async move { work(self.context(cfg, me, backend)).await })
            .await
    }

    /// Kontext eines Durchlaufs auf dieser Laufzeit
    fn context(
        &self,
        cfg: DispatchConfig,
        me: RepoBoardOwner,
        backend: Arc<dyn BoardBackend>,
    ) -> DispatchContext {
        DispatchContext::new(
            cfg,
            me,
            self.deps.home.clone(),
            backend,
            self.stores.clone(),
            self.deps.launcher.clone(),
            self.deps.clock.clone(),
            self.deps.notifier.clone(),
            self.deps.env.clone(),
            self.deps.platform.clone(),
        )
    }
}

/// Lauf für Oberfläche und Kommandozeile
fn run_info(record: &RunRecord) -> DispatchRunInfo {
    DispatchRunInfo {
        id: record.id.clone(),
        kind: record.kind,
        ticket_id: record.ticket_id.clone(),
        title: record.title.clone(),
        profile: record.run.profile.clone(),
        model: record.run.model.clone(),
        effort: record.run.effort.clone(),
        started_at: record.started_at.clone(),
        log: format!("{}.log", record.id),
        question_id: record.question_id.clone(),
    }
}

/// Geplant-Tickets in Board-Reihenfolge: Priorität, dann Nummer
fn planned(views: Vec<TicketView>) -> Vec<TicketView> {
    let mut planned: Vec<TicketView> = views
        .into_iter()
        .filter(|view| view.ticket.status == TicketStatus::Planned)
        .collect();
    planned.sort_by_key(|view| (view.ticket.priority.rank(), view.ticket.number));
    planned
}

/// Ordner der Läufe einer Art
fn cwds(runs: &[RunRecord], kind: RunKind) -> Vec<String> {
    runs.iter()
        .filter(|run| run.kind == kind)
        .map(|run| run.cwd.clone())
        .collect()
}

/// Der Zeitpunkt, solange er in der Zukunft liegt
fn future(at: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> Option<String> {
    let at = at?;
    let millis = parse_millis(at)?;
    (millis > now.timestamp_millis()).then(|| at.to_string())
}
