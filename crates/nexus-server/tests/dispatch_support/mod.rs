//! Testumgebung für Start und Auswertung eines Durchlaufs; Vorbild
//! `packages/server/tests/helpers/DispatchFixture.ts` und `FakeLauncher.ts`.
//!
//! Neben dem Kontext steht hier der ganze [`DispatchService`]: [`DispatchFixture::tick`] fährt
//! einen Durchlauf wie im Betrieb (mit `dispatch.yaml` und `board.yaml` im Benutzerordner).
//! [`DispatchFixture::start`] und [`DispatchFixture::evaluate`] bleiben für die Tests, die nur
//! die zwei Schritte eines einzelnen Tickets prüfen.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nexus_core::board::basics::{BoardActor, TicketStatus};
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::dispatch_run_types::DispatchRun;
use nexus_core::board::ticket_service::{
    NewTicketInput, TicketDetail, TicketPatch, TicketView, WriteOptions,
};
use nexus_core::board::types::Board;
use nexus_core::clock::{iso, Clock};
use nexus_core::dispatch::git::MergeOutcome;
use nexus_core::dispatch::{
    DispatchConfig, DispatchConfigNormalizer, DispatchConfigStore, DispatchLog, DispatchStateStore,
    MergeStrategy, ReviewStore, RunEnd, RunRecord, RunResult, RunStore,
};
use nexus_core::{HarnessError, HarnessHome, Result};
use nexus_server::board_runtime::{factory, BoardBackend, BoardBackendFuture, BoardRuntimeHolder};
use nexus_server::dispatch::account_pool::AccountPool;
use nexus_server::dispatch::context::{DispatchContext, DispatchStores, RepoWork};
use nexus_server::dispatch::merge_git::{MergeGit, RealMergeGit};
use nexus_server::dispatch::merge_service::MergeService;
use nexus_server::dispatch::merge_step::{AcceptOutcome, MergeStep, MergeStepReport};
use nexus_server::dispatch::notifier::{DispatchNotice, DispatchNotify};
use nexus_server::dispatch::result_handler::{MergeSessionEnd, ResultHandler};
use nexus_server::dispatch::run_recorder::{HandleResult, RunRecorder};
use nexus_server::dispatch::service::{DispatchReport, DispatchService, DispatchServiceDeps};
use nexus_server::dispatch::ticket_starter::{StartOutcome, TicketStarter};
use nexus_server::dispatch::worker_launcher::WorkerLauncher;
use serde_json::{json, Value};

use crate::common::{memory_config, BoardBackendWithServices, TempRepo};

/// Ergebnis-Ereignis eines gelungenen Laufs (wie `OK_OUTPUT` in TypeScript)
pub const OK_OUTPUT: &str =
    r#"{"type":"result","subtype":"success","is_error":false,"result":"fertig"}"#;

/// Startet nichts, merkt sich nur die Aufrufe; Vorbild `FakeLauncher.ts`
#[derive(Debug, Default)]
pub struct FakeLauncher {
    launched: Mutex<Vec<String>>,
    /// Umgebung je gestartetem Lauf
    envs: Mutex<Vec<(String, BTreeMap<String, String>)>>,
    /// Fehlermeldung, mit der der Start scheitert (wie `launcher.fail` in TypeScript)
    fail: Mutex<Option<String>>,
    next_pid: AtomicI64,
}

impl FakeLauncher {
    /// Starter, der immer eine Prozessnummer liefert
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            next_pid: AtomicI64::new(4242),
            ..Self::default()
        })
    }

    /// Lässt jeden weiteren Start mit dieser Meldung scheitern
    pub fn fail_with(&self, message: &str) {
        *self.fail.lock().expect("Attrappe") = Some(message.to_string());
    }

    /// Lässt Starts wieder gelingen
    pub fn succeed(&self) {
        *self.fail.lock().expect("Attrappe") = None;
    }

    /// Läufe, die gestartet wurden
    pub fn launched(&self) -> Vec<String> {
        self.launched.lock().expect("Attrappe").clone()
    }

    /// Umgebung des zuletzt gestarteten Laufs
    pub fn last_env(&self) -> BTreeMap<String, String> {
        self.envs
            .lock()
            .expect("Attrappe")
            .last()
            .map(|(_, env)| env.clone())
            .expect("kein Start")
    }

    /// Umgebung, mit der der Lauf gestartet wurde
    pub fn env_of(&self, run_id: &str) -> BTreeMap<String, String> {
        self.envs
            .lock()
            .expect("Attrappe")
            .iter()
            .find(|(id, _)| id == run_id)
            .map(|(_, env)| env.clone())
            .unwrap_or_else(|| panic!("kein Start für {run_id}"))
    }
}

impl WorkerLauncher for FakeLauncher {
    fn launch(&self, run_id: &str, env: &BTreeMap<String, String>) -> Result<Option<u32>> {
        if let Some(message) = self.fail.lock().expect("Attrappe").clone() {
            return Err(HarnessError::new("worker-start-failed", message).into());
        }
        self.launched
            .lock()
            .expect("Attrappe")
            .push(run_id.to_string());
        self.envs
            .lock()
            .expect("Attrappe")
            .push((run_id.to_string(), env.clone()));
        Ok(Some(self.next_pid.fetch_add(1, Ordering::SeqCst) as u32))
    }
}

/// Sammelt die Meldungen, statt sie zu schicken
#[derive(Debug, Default)]
pub struct CollectedNotices(Mutex<Vec<DispatchNotice>>);

impl CollectedNotices {
    /// Sammler ohne Meldungen
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Alle gesammelten Meldungen
    pub fn all(&self) -> Vec<DispatchNotice> {
        self.0.lock().expect("Meldungen").clone()
    }

    /// Die jüngste Meldung
    pub fn last(&self) -> Option<DispatchNotice> {
        self.0.lock().expect("Meldungen").last().cloned()
    }
}

#[async_trait]
impl DispatchNotify for CollectedNotices {
    async fn notify(&self, notice: &DispatchNotice) -> Result<()> {
        self.0.lock().expect("Meldungen").push(notice.clone());
        Ok(())
    }
}

/// Antwort, die die Attrappe statt des echten Merges gibt; Vorbild
/// `vi.spyOn(BranchMerger.prototype, 'merge')`
#[derive(Debug, Clone)]
pub enum MergeAnswer {
    /// Git weist ab, ohne etwas zu verändern
    Refused(String),
    /// Konflikt in diesen Dateien
    Conflict(Vec<String>),
    /// Der eigene Merge ließ sich nicht zurücksetzen; `details` = mit Ordner, Git-Meldung und
    /// Befehl (wie außerhalb des Hauptordners)
    AbortFailed { details: bool },
}

/// Die Git-Seite des Merges mit gelenkten Antworten und Zählern; alles, was nicht gelenkt ist,
/// macht das echte Git.
#[derive(Debug, Default)]
pub struct FakeMergeGit {
    real: RealMergeGit,
    answer: Mutex<Option<MergeAnswer>>,
    /// Datei, die der gelenkte Merge vor seiner Antwort im Ordner anlegt (Name, Inhalt)
    before_merge: Mutex<Option<(String, String)>>,
    /// Meldung, mit der `contains` scheitert
    contains_error: Mutex<Option<String>>,
    merge_calls: AtomicI64,
    open_calls: AtomicI64,
    merge_roots: Mutex<Vec<String>>,
}

impl FakeMergeGit {
    /// Attrappe, die zunächst alles ans echte Git weitergibt
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Lässt jeden weiteren Merge so antworten
    pub fn answer_merge(&self, answer: MergeAnswer) {
        *self.answer.lock().expect("Attrappe") = Some(answer);
    }

    /// Legt vor jeder gelenkten Antwort diese Datei im Ordner des Merges an
    pub fn write_before_merge(&self, file: &str, content: &str) {
        *self.before_merge.lock().expect("Attrappe") =
            Some((file.to_string(), content.to_string()));
    }

    /// Lässt `contains` mit dieser Meldung scheitern
    pub fn fail_contains(&self, message: &str) {
        *self.contains_error.lock().expect("Attrappe") = Some(message.to_string());
    }

    /// Zahl der Merge-Versuche
    pub fn merge_calls(&self) -> i64 {
        self.merge_calls.load(Ordering::SeqCst)
    }

    /// Zahl der geöffneten Merge-Worktrees
    pub fn open_calls(&self) -> i64 {
        self.open_calls.load(Ordering::SeqCst)
    }

    /// Ordner des letzten Merge-Versuchs
    pub fn last_merge_root(&self) -> String {
        self.merge_roots
            .lock()
            .expect("Attrappe")
            .last()
            .cloned()
            .expect("kein Merge-Versuch")
    }
}

impl MergeGit for FakeMergeGit {
    fn inspect(&self, root: &Path) -> Result<nexus_core::dispatch::git::CheckoutState> {
        self.real.inspect(root)
    }

    fn running(&self, root: &Path) -> Result<Option<String>> {
        self.real.running(root)
    }

    fn merge(
        &self,
        root: &Path,
        branch: &str,
        message: Option<&str>,
        strategy: MergeStrategy,
    ) -> Result<MergeOutcome> {
        self.merge_calls.fetch_add(1, Ordering::SeqCst);
        self.merge_roots
            .lock()
            .expect("Attrappe")
            .push(root.to_string_lossy().into_owned());
        let answer = self.answer.lock().expect("Attrappe").clone();
        let Some(answer) = answer else {
            return self.real.merge(root, branch, message, strategy);
        };
        if let Some((file, content)) = self.before_merge.lock().expect("Attrappe").clone() {
            std::fs::write(root.join(file), content).expect("Datei im Ordner des Merges");
        }
        match answer {
            MergeAnswer::Refused(message) => Ok(MergeOutcome::Refused { message }),
            MergeAnswer::Conflict(files) => Ok(MergeOutcome::Conflict { files }),
            MergeAnswer::AbortFailed { details } => {
                let error = HarnessError::new(
                    "merge-abort-failed",
                    "Der Hauptordner steht noch im Merge-Zustand: kaputt. Bitte von Hand \
aufräumen (git merge --abort).",
                );
                Err(if details {
                    error
                        .with_details(json!({
                            "root": root.to_string_lossy(),
                            "git": "kaputt",
                            "command": "git merge --abort",
                        }))
                        .into()
                } else {
                    error.into()
                })
            }
        }
    }

    fn contains(&self, root: &Path, ancestor: &str, branch: &str) -> Result<bool> {
        if let Some(message) = self.contains_error.lock().expect("Attrappe").clone() {
            return Err(HarnessError::new("git-failed", message).into());
        }
        self.real.contains(root, ancestor, branch)
    }

    fn head(&self, root: &Path, reference: &str) -> Result<String> {
        self.real.head(root, reference)
    }

    fn merged_earlier(&self, root: &Path, branch: &str, target: &str) -> Option<String> {
        self.real.merged_earlier(root, branch, target)
    }

    fn overlapping(&self, root: &Path, branch: &str) -> Result<Vec<String>> {
        self.real.overlapping(root, branch)
    }

    fn ensure_target(
        &self,
        root: &Path,
        name: &str,
    ) -> Result<nexus_core::dispatch::git::EnsuredBranch> {
        self.real.ensure_target(root, name)
    }

    fn checked_out_at(&self, root: &Path, name: &str) -> Result<Option<String>> {
        self.real.checked_out_at(root, name)
    }

    fn merge_worktree_path(&self, root: &Path, name: &str) -> String {
        self.real.merge_worktree_path(root, name)
    }

    fn open_merge_worktree(&self, root: &Path, name: &str) -> Result<String> {
        self.open_calls.fetch_add(1, Ordering::SeqCst);
        self.real.open_merge_worktree(root, name)
    }

    fn close_merge_worktree(&self, root: &Path, path: &str) -> Option<String> {
        self.real.close_merge_worktree(root, path)
    }
}

/// Merge-Seite, die die Reihenfolge der Aufrufe mitschreibt und „Erneut mergen“ an einem Tor
/// anhält; Vorbild `vi.spyOn(MergeService.prototype, …)` im Sperren-Test
#[derive(Debug)]
pub struct OrderedMerge {
    inner: MergeService,
    order: Mutex<Vec<String>>,
    gate: tokio::sync::Semaphore,
}

impl OrderedMerge {
    /// Merge-Seite mit geschlossenem Tor
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: MergeService::new(),
            order: Mutex::new(Vec::new()),
            gate: tokio::sync::Semaphore::new(0),
        })
    }

    /// Öffnet das Tor: „Erneut mergen“ läuft weiter
    pub fn release(&self) {
        self.gate.add_permits(1);
    }

    /// Reihenfolge der Aufrufe
    pub fn order(&self) -> Vec<String> {
        self.order.lock().expect("Reihenfolge").clone()
    }

    /// Vergisst die bisherigen Aufrufe; in TypeScript wird die Attrappe erst hier gesetzt
    pub fn forget(&self) {
        self.order.lock().expect("Reihenfolge").clear();
    }

    /// Schreibt einen Aufruf mit
    fn note(&self, what: &str) {
        self.order.lock().expect("Reihenfolge").push(what.into());
    }
}

#[async_trait]
impl MergeSessionEnd for OrderedMerge {
    async fn session_ended(
        &self,
        ctx: &DispatchContext,
        record: &RunRecord,
        end: &RunEnd,
    ) -> Result<()> {
        self.inner.session_ended(ctx, record, end).await
    }
}

#[async_trait]
impl MergeStep for OrderedMerge {
    async fn process_all(&self, ctx: &DispatchContext) -> Result<MergeStepReport> {
        self.note("merge-tick");
        MergeStep::process_all(&self.inner, ctx).await
    }

    async fn retry(&self, ctx: &DispatchContext, id: &str, actor: &BoardActor) -> Result<String> {
        self.note("retry");
        self.gate.acquire().await.expect("Tor").forget();
        let ticket = MergeStep::retry(&self.inner, ctx, id, actor).await;
        self.note("retry fertig");
        ticket
    }

    async fn accept(
        &self,
        ctx: &DispatchContext,
        id: &str,
        actor: &BoardActor,
    ) -> Result<AcceptOutcome> {
        MergeStep::accept(&self.inner, ctx, id, actor).await
    }
}

/// Alles, was Start und Auswertung eines Tickets brauchen: eigener Benutzerordner, Git-Repo als
/// Board-Repo, Board im Speicher, Attrappe statt Worker.
pub struct DispatchFixture {
    dir: tempfile::TempDir,
    pub home: HarnessHome,
    pub repo: TempRepo,
    pub backend: Arc<BoardBackendWithServices>,
    pub board: Board,
    pub launcher: Arc<FakeLauncher>,
    pub notices: Arc<CollectedNotices>,
    pub ctx: DispatchContext,
    pub cfg: DispatchConfig,
    /// Der ganze Dispatcher auf diesem Benutzerordner (liest `dispatch.yaml` und `board.yaml`)
    pub service: Arc<DispatchService>,
    /// Die Board-Laufzeit hinter dem Dispatcher; der Schattenbetrieb braucht dieselbe
    pub holder: Arc<BoardRuntimeHolder>,
    /// Leben die Prozesse der Läufe noch? (wie `f.alive` in TypeScript)
    alive: Arc<AtomicBool>,
    clock: Clock,
    now: Arc<Mutex<DateTime<Utc>>>,
}

impl DispatchFixture {
    /// Legt alles an; die Einstellung ist die aus dem Beispiel, eingeschaltet und ohne Freigabe
    /// vor dem Merge – `patch` überschreibt einzelne Felder der rohen Einstellung
    pub async fn create(patch: Value) -> Self {
        Self::create_with(patch, None).await
    }

    /// Wie [`DispatchFixture::create`], aber mit einer eigenen Merge-Seite (Attrappe statt
    /// [`MergeService`]); `None` nimmt die echte
    pub async fn create_with(patch: Value, merge: Option<Arc<dyn MergeStep>>) -> Self {
        Self::create_full(patch, merge, |_| None).await
    }

    /// Wie [`DispatchFixture::create`], mit Claude-Accounts; der Pool entsteht zum
    /// Benutzerordner der Fixture
    pub async fn create_with_accounts(
        patch: Value,
        pool: impl FnOnce(&HarnessHome) -> Option<Arc<dyn AccountPool>>,
    ) -> Self {
        Self::create_full(patch, None, pool).await
    }

    /// Alles zusammen: Einstellung, Merge-Seite und Account-Pool
    async fn create_full(
        patch: Value,
        merge: Option<Arc<dyn MergeStep>>,
        pool: impl FnOnce(&HarnessHome) -> Option<Arc<dyn AccountPool>>,
    ) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("nexus-dispatch-")
            .tempdir()
            .expect("Ordner");
        let home = HarnessHome::at(dir.path());
        let repo = TempRepo::new();
        repo.run_git(&["config", "user.name", "t"]);
        repo.run_git(&["config", "user.email", "t@t"]);
        let backend = BoardBackendWithServices::create().await;
        let actor = backend.actor();
        backend
            .services
            .projects
            .create("ARC", "Arches", &actor)
            .await
            .expect("Projekt");
        let board = backend
            .services
            .boards
            .create("ARC", "Sprint", &actor)
            .await
            .expect("Board");
        let board = backend
            .services
            .boards
            .set_repo_path(board.board.id, Some(&repo.path().to_string_lossy()))
            .await
            .expect("Repo-Pfad")
            .board;
        let raw = raw_config(patch);
        let cfg = DispatchConfigNormalizer::new()
            .normalize(&raw)
            .expect("Einstellung");
        // Der Service liest seine Einstellung aus der Datei, nicht aus dem Kontext
        DispatchConfigStore::new(home.clone())
            .write(&raw)
            .expect("dispatch.yaml schreiben");
        BoardConfigStore::new(home.clone())
            .write(&nexus_core::board::config::BoardConfig {
                active_board: Some(board.id),
                ..memory_config()
            })
            .expect("board.yaml schreiben");
        let now = Arc::new(Mutex::new(Utc::now()));
        let clock = {
            let now = now.clone();
            Clock::from_fn(move || *now.lock().expect("Uhr"))
        };
        let launcher = FakeLauncher::new();
        let notices = CollectedNotices::new();
        let stores = DispatchStores {
            runs: RunStore::new(home.clone()),
            state: DispatchStateStore::with_clock(home.clone(), clock.clone()),
            log: DispatchLog::with_clock(home.clone(), clock.clone()),
            reviews: ReviewStore::new(home.clone()),
        };
        let me = nexus_core::board::repo_board_resolver::RepoBoardOwner {
            user_id: actor.user_id.clone(),
            active_board: Some(board.id),
        };
        let ctx = DispatchContext::new(
            cfg.clone(),
            me,
            home.clone(),
            backend.clone() as Arc<dyn BoardBackend>,
            stores,
            launcher.clone(),
            clock.clone(),
            notices.clone(),
            BTreeMap::new(),
            // Die Tests laufen auf jeder Plattform gegen denselben Zweig: ohne Windows keine
            // Suche nach der Git Bash
            "linux",
        );
        let accounts = pool(&home);
        let ctx = ctx.with_accounts(accounts.clone());
        let alive = Arc::new(AtomicBool::new(true));
        let holder = {
            let shared = backend.clone();
            Arc::new(BoardRuntimeHolder::new(factory(
                move || -> BoardBackendFuture {
                    let backend = shared.clone();
                    Box::pin(async move { Ok(backend as Arc<dyn BoardBackend>) })
                },
            )))
        };
        let mut deps = DispatchServiceDeps {
            notifier: notices.clone(),
            clock: clock.clone(),
            alive: {
                let alive = alive.clone();
                Arc::new(move |_| alive.load(Ordering::SeqCst))
            },
            env: BTreeMap::new(),
            platform: "linux".into(),
            accounts,
            ..DispatchServiceDeps::new(home.clone(), holder, launcher.clone())
        };
        if let Some(merge) = merge {
            deps.merge = merge;
        }
        let holder = deps.boards.clone();
        let service = Arc::new(DispatchService::new(deps));
        Self {
            dir,
            home,
            repo,
            backend,
            board,
            launcher,
            notices,
            ctx,
            cfg,
            service,
            holder,
            alive,
            clock,
            now,
        }
    }

    /// Ein Durchlauf des ganzen Dispatchers (wie `f.service.tick()` in TypeScript)
    pub async fn tick(&self) -> DispatchReport {
        self.service.tick().await
    }

    /// Setzt, ob die Prozesse der Läufe noch leben (`false` = Worker verschwunden)
    pub fn set_alive(&self, alive: bool) {
        self.alive.store(alive, Ordering::SeqCst);
    }

    /// Schreibt einzelne Felder der Einstellung neu, wie ein Mensch in `dispatch.yaml`
    pub fn write_config(&self, patch: Value) {
        let store = DispatchConfigStore::new(self.home.clone());
        let mut raw = store.read().expect("Einstellung lesen").raw;
        let object = raw.as_object_mut().expect("Einstellung als Objekt");
        for (key, value) in patch.as_object().expect("Änderung als Objekt") {
            object.insert(key.clone(), value.clone());
        }
        store.write(&raw).expect("dispatch.yaml schreiben");
    }

    /// Schreibt das Protokoll eines Laufs, wie es sonst der Worker-Prozess tut (Kopf,
    /// Arbeitsanweisung, Ausgabe)
    pub fn write_log(&self, run_id: &str, prompt: &str, output: &str) -> String {
        let dir = self.home.dispatch_logs_dir();
        std::fs::create_dir_all(&dir).expect("Protokollordner");
        let name = format!("{run_id}.log");
        std::fs::write(
            dir.join(&name),
            format!(
                "[dispatch] Start {run_id}\n{}\n{prompt}\n{}\n{output}\n",
                nexus_core::dispatch::RUN_LOG_PROMPT_MARKER,
                nexus_core::dispatch::RUN_LOG_OUTPUT_MARKER,
            ),
        )
        .expect("Protokoll schreiben");
        name
    }

    /// Entfernt alle eingetragenen Läufe (wie „Lauf-Datei verloren“ in TypeScript)
    pub fn drop_all_runs(&self) {
        for record in self.ctx.stores.runs.list().expect("Läufe") {
            self.drop_run(&record.id);
        }
    }

    /// Ein zweiter Kontext auf denselben Daten, aber mit einem anderen aktiven Board (im Betrieb
    /// kommt es aus der Board-Einstellung des Benutzers)
    pub fn context_with_active_board(&self, active_board: Option<i32>) -> DispatchContext {
        let stores = DispatchStores {
            runs: RunStore::new(self.home.clone()),
            state: DispatchStateStore::with_clock(self.home.clone(), self.clock.clone()),
            log: DispatchLog::with_clock(self.home.clone(), self.clock.clone()),
            reviews: ReviewStore::new(self.home.clone()),
        };
        DispatchContext::new(
            self.cfg.clone(),
            nexus_core::board::repo_board_resolver::RepoBoardOwner {
                user_id: self.backend.actor().user_id,
                active_board,
            },
            self.home.clone(),
            self.backend.clone() as Arc<dyn BoardBackend>,
            stores,
            self.launcher.clone(),
            self.clock.clone(),
            self.notices.clone(),
            BTreeMap::new(),
            "linux",
        )
    }

    /// Ordner, in dem Benutzerordner und Hilfsdateien des Tests liegen
    pub fn dir(&self) -> &std::path::Path {
        self.dir.path()
    }

    /// Der Agent committet eine Datei im Worktree und meldet „erledigt“ wie `nexus board done`
    pub async fn finish_as_agent(&self, ticket_id: &str, file: &str, content: &str) {
        let worktree = self
            .ticket(ticket_id)
            .await
            .worktree
            .expect("Worktree des Tickets");
        std::fs::write(std::path::Path::new(&worktree).join(file), content).expect("Datei");
        for args in [
            vec!["-C", &worktree, "add", file],
            vec![
                "-C",
                &worktree,
                "commit",
                "-q",
                "-m",
                &format!("{ticket_id}: {file}"),
            ],
        ] {
            self.repo.run_git(&args);
        }
        self.work()
            .await
            .manager
            .finish(
                ticket_id,
                &nexus_core::worktrees::manager::FinishOptions::default(),
                std::path::Path::new(&worktree),
                &self.agent(ticket_id),
            )
            .await
            .expect("Ticket als Agent abschließen");
    }

    /// Der Handelnde des Testbenutzers
    pub fn actor(&self) -> BoardActor {
        self.backend.actor()
    }

    /// Handelnder als Junior-Agent des Tickets
    pub fn agent(&self, ticket_id: &str) -> BoardActor {
        BoardActor {
            user_id: self.backend.actor().user_id,
            name: format!("junior-{ticket_id}"),
        }
    }

    /// Legt ein bereites Ticket an (Geplant, Label `auto`, Plan, Handbuch, Abnahme)
    pub async fn ready(&self, title: &str, patch: TicketPatch) -> String {
        let actor = self.actor();
        let created = self
            .backend
            .services
            .tickets
            .create(
                self.board.id,
                &NewTicketInput {
                    acceptance: Some("Tests grün".into()),
                    labels: Some(vec!["auto".into()]),
                    needs_planning: Some(false),
                    ..NewTicketInput::titled(title)
                },
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket anlegen");
        self.backend
            .services
            .tickets
            .update(
                &created.ticket.id,
                &TicketPatch {
                    plan: Some("## Plan\nUmsetzen".into()),
                    handbook: Some("npm test".into()),
                    status: Some(TicketStatus::Planned),
                    ..patch
                },
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket vorbereiten");
        created.ticket.id
    }

    /// Das Board samt Repo und Worktree-Verwaltung
    pub async fn work(&self) -> Arc<RepoWork> {
        self.ctx.repo(&self.board).await.expect("Repo des Boards")
    }

    /// Sicht auf ein Ticket, wie der Tick sie hat
    pub async fn view(&self, ticket_id: &str) -> TicketView {
        self.detail(ticket_id).await.view
    }

    /// Das Ticket selbst (Deref auf `TicketDetail` lässt es nicht herausnehmen)
    pub async fn ticket(&self, ticket_id: &str) -> nexus_core::board::Ticket {
        self.detail(ticket_id).await.view.ticket
    }

    /// Legt eine offene Frage zum Ticket an und liefert ihre Kennung
    pub async fn question(&self, ticket_id: &str, title: &str) -> String {
        let detail = self
            .backend
            .services
            .questions
            .create(
                "ARC",
                &nexus_core::board::question_service::NewBoardQuestionInput {
                    ticket_ids: Some(vec![ticket_id.to_string()]),
                    ..nexus_core::board::question_service::NewBoardQuestionInput::titled(title)
                },
                &nexus_core::board::question_types::BoardQuestionActor {
                    actor: self.agent(ticket_id),
                    kind: nexus_core::actors::AuthorKind::Session,
                    session: None,
                },
            )
            .await
            .expect("Frage anlegen");
        detail.view.question.id
    }

    /// Ticket mit Kommentaren
    pub async fn detail(&self, ticket_id: &str) -> TicketDetail {
        self.backend
            .services
            .tickets
            .get(ticket_id)
            .await
            .expect("Ticket")
    }

    /// Startet ein Ticket wie der Tick: frisch lesen, prüfen, claim, Worktree, Lauf
    pub async fn start(&self, ticket_id: &str) -> Result<StartOutcome> {
        let view = self.view(ticket_id).await;
        let work = self.work().await;
        let results = ResultHandler::without_gates(&self.ctx);
        TicketStarter::new(&self.ctx, &results)
            .start(&view, &work)
            .await
    }

    /// Jüngster Lauf zum Ticket
    pub fn run(&self, ticket_id: &str) -> RunRecord {
        self.ctx
            .stores
            .runs
            .list()
            .expect("Läufe")
            .into_iter()
            .rfind(|run| run.ticket_id == ticket_id)
            .unwrap_or_else(|| panic!("kein Lauf für {ticket_id}"))
    }

    /// Schreibt das Ergebnis des jüngsten Laufs, als hätte der Worker es geschrieben
    pub fn end(&self, ticket_id: &str, output: &str, code: Option<i64>) -> RunRecord {
        self.end_with(
            ticket_id,
            RunResult {
                code,
                output: output.to_string(),
                ..RunResult::default()
            },
        )
    }

    /// Wie [`DispatchFixture::end`], aber mit einem selbst gebauten Ergebnis
    pub fn end_with(&self, ticket_id: &str, result: RunResult) -> RunRecord {
        let record = self.run(ticket_id);
        self.ctx
            .stores
            .runs
            .finish(
                &record.id,
                &RunEnd {
                    result,
                    ended_at: self.ctx.now_iso(),
                },
            )
            .expect("Ergebnis schreiben");
        record
    }

    /// Das geschriebene Ergebnis eines Laufs
    pub fn result(&self, run_id: &str) -> RunEnd {
        self.ctx
            .stores
            .runs
            .result(run_id)
            .expect("Ergebnis lesen")
            .unwrap_or_else(|| panic!("kein Ergebnis für {run_id}"))
    }

    /// Entfernt den ausgewerteten Lauf, wie der Tick es nach der Auswertung tut
    pub fn drop_run(&self, run_id: &str) {
        self.ctx.stores.runs.remove(run_id).expect("Lauf entfernen");
    }

    /// Wertet den jüngsten beendeten Lauf aus und merkt ihn für die Auswertung vor – die zwei
    /// Schritte, die der Tick nach dem Ende eines Laufs tut
    pub async fn evaluate(&self, ticket_id: &str) -> Result<HandleResult> {
        let record = self.run(ticket_id);
        let end = self
            .ctx
            .stores
            .runs
            .result(&record.id)?
            .expect("Ergebnis des Laufs");
        let results = ResultHandler::without_gates(&self.ctx);
        let result = results.handle(&record, &end, None).await?;
        RunRecorder::new(&self.ctx)
            .keep(&record, &end, &result)
            .await;
        // Der ausgewertete Lauf ist erledigt; der Tick entfernt ihn hier
        self.ctx.stores.runs.remove(&record.id)?;
        Ok(result)
    }

    /// Überträgt die vorgemerkten Lauf-Zeilen in die Datenbank
    pub async fn flush(&self) -> Result<usize> {
        RunRecorder::new(&self.ctx).flush().await
    }

    /// Alle erfassten Läufe, nach Versuch sortiert
    pub async fn rows(&self) -> Vec<DispatchRun> {
        let mut rows = self
            .backend
            .services
            .dispatch_runs
            .list_all(
                "2000-01-01T00:00:00.000Z".parse().expect("von"),
                "2100-01-01T00:00:00.000Z".parse().expect("bis"),
            )
            .await
            .expect("Läufe");
        rows.sort_by_key(|row| row.attempt.unwrap_or(0));
        rows
    }

    /// Texte aller Kommentare des Tickets
    pub async fn comments(&self, ticket_id: &str) -> Vec<String> {
        self.detail(ticket_id)
            .await
            .comments
            .into_iter()
            .map(|comment| comment.body)
            .collect()
    }

    /// Vorgemerkte Lauf-Zeilen
    pub fn kept(&self) -> Vec<(String, DispatchRun)> {
        self.ctx.stores.runs.kept_stats().expect("Vorgemerkte")
    }

    /// Stellt die Uhr des Dispatchers vor
    pub fn advance(&self, minutes: i64) {
        let mut now = self.now.lock().expect("Uhr");
        *now += chrono::Duration::minutes(minutes);
    }

    /// Stellt die Uhr auf diese Ortszeit des heutigen Tages (für das Zeitfenster, das in
    /// Ortszeit gilt)
    pub fn set_local_time(&self, hour: u32, minute: u32) {
        use chrono::{Local, TimeZone};
        let today = Local::now().date_naive();
        let local = today.and_hms_opt(hour, minute, 0).expect("gültige Uhrzeit");
        let at = Local
            .from_local_datetime(&local)
            .single()
            .expect("eindeutige Ortszeit");
        *self.now.lock().expect("Uhr") = at.with_timezone(&Utc);
    }

    /// Die Uhr des Dispatchers; der Schattenbetrieb rechnet mit derselben
    pub fn clock(&self) -> Clock {
        self.clock.clone()
    }

    /// Die aktuelle Zeit des Dispatchers als ISO-Text
    pub fn now_iso(&self) -> String {
        iso(*self.now.lock().expect("Uhr"))
    }

    /// Ein Zeitpunkt in `minutes` Minuten als ISO-Text
    pub fn now_iso_in(&self, minutes: i64) -> String {
        iso(*self.now.lock().expect("Uhr") + chrono::Duration::minutes(minutes))
    }
}

/// Die Beispiel-Einstellung, eingeschaltet, ohne Freigabe vor dem Merge und mit `patch`
pub fn config(patch: Value) -> DispatchConfig {
    DispatchConfigNormalizer::new()
        .normalize(&raw_config(patch))
        .expect("Einstellung")
}

/// Die rohe Beispiel-Einstellung, eingeschaltet, ohne Freigabe vor dem Merge und mit `patch`
pub fn raw_config(patch: Value) -> Value {
    let mut raw = DispatchConfigNormalizer::new().example();
    let object = raw.as_object_mut().expect("Einstellung als Objekt");
    object.insert("enabled".into(), json!(true));
    object.insert("reviewBeforeMerge".into(), json!(false));
    if let Some(fields) = patch.as_object() {
        for (key, value) in fields {
            object.insert(key.clone(), value.clone());
        }
    }
    raw
}
