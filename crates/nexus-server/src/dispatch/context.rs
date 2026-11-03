//! Alles, was ein Durchlauf des Dispatchers braucht: Einstellung, eigener Benutzer,
//! Board-Dienste, lokale Dateien und der Starter der Worker. Portiert aus
//! `packages/server/src/dispatch/DispatchContext.ts`.
//!
//! Die Zähler und Prüfer der Einstellung ([`ProfileChooser`], [`DispatchReadiness`],
//! [`AttemptCounter`]) leihen sich in Rust die Einstellung; sie stehen deshalb nicht als
//! Felder im Kontext, sondern kommen aus je einer Methode mit der Lebensdauer des Kontexts.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nexus_core::accounts::STANDARD_ACCOUNT_ID;
use nexus_core::board::basics::BoardActor;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::board::types::Board;
use nexus_core::clock::{iso, Clock};
use nexus_core::dispatch::{
    hhmm, AccountChoice, AttemptCounter, ClaudeInvocation, DispatchConfig, DispatchLog,
    DispatchPrompt, DispatchReadiness, DispatchStateStore, GitBashLocator, JoinWorktrees,
    ProfileChooser, ReviewStore, RunKind, RunOutputReader, RunRecord, RunSettings, RunStore,
    WorktreeSetupStep,
};
use nexus_core::project::layout::HarnessLayout;
use nexus_core::worktrees::manager::{WorktreeManager, WorktreeOptions};
use nexus_core::{HarnessError, HarnessHome, HarnessProject, Result};
use tokio::sync::Mutex;

use crate::board_db::{ServerBoardDatabase, ServerBoardServices};
use crate::board_runtime::BoardBackend;
use crate::dispatch::account_pool::{AccountPool, RunAccounts};
use crate::dispatch::notifier::{DispatchNotice, DispatchNotify};
use crate::dispatch::worker_launcher::WorkerLauncher;

/// Die Dateiablage des Dispatchers unter `~/.harness`
#[derive(Debug, Clone)]
pub struct DispatchStores {
    pub runs: RunStore,
    pub state: DispatchStateStore,
    pub log: DispatchLog,
    pub reviews: ReviewStore,
}

/// Board mit Repo: Hauptordner und Worktree-Verwaltung
pub struct RepoWork {
    pub board: Board,
    pub root: String,
    pub manager: WorktreeManager<ServerBoardDatabase>,
}

impl std::fmt::Debug for RepoWork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepoWork")
            .field("board", &self.board.id)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// Was ein neuer Lauf mitbringt; `extra_tools` steht nicht in der Lauf-Datei
#[derive(Debug, Clone)]
pub struct LaunchInput {
    pub kind: RunKind,
    pub ticket_id: String,
    pub board_id: i32,
    pub title: String,
    pub agent: String,
    pub cwd: String,
    pub branch: String,
    pub target: Option<String>,
    pub run: RunSettings,
    pub prompt: String,
    /// Zusätzlich erlaubte Tools (Merge-Session); eine Recherche (kind research) bekommt
    /// immer nur `RESEARCH_TOOLS`
    pub extra_tools: Vec<String>,
    /// Recherche: Frage, Zahl ihrer Recherchen beim Start, Hauptordner des Repos
    pub question_id: Option<String>,
    pub research_before: Option<i64>,
    pub repo_root: Option<String>,
    /// Planung: `plannedAt` des Tickets beim Start
    pub planned_before: Option<Option<String>>,
    /// Umsetzung in einem neu angelegten Worktree: Einrichtungsbefehle vor Claude
    pub setup: Option<Vec<WorktreeSetupStep>>,
    /// Umsetzung: Nummer des Versuchs
    pub attempt: Option<i64>,
}

impl LaunchInput {
    /// Ein Lauf mit den Pflichtangaben; die Sonderfelder bleiben leer
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: RunKind,
        ticket_id: impl Into<String>,
        board_id: i32,
        title: impl Into<String>,
        agent: impl Into<String>,
        cwd: impl Into<String>,
        branch: impl Into<String>,
        run: RunSettings,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            ticket_id: ticket_id.into(),
            board_id,
            title: title.into(),
            agent: agent.into(),
            cwd: cwd.into(),
            branch: branch.into(),
            target: None,
            run,
            prompt: prompt.into(),
            extra_tools: Vec::new(),
            question_id: None,
            research_before: None,
            repo_root: None,
            planned_before: None,
            setup: None,
            attempt: None,
        }
    }
}

/// Ergebnis der Account-Wahl eines Laufs
#[derive(Debug, Clone)]
pub enum ChosenAccount {
    /// Der Lauf mit gewähltem Account
    Run(Box<RunSettings>),
    /// Kein Account frei: der Grund fürs Warten
    Waiting(String),
}

/// Eine gesetzte Pause nach einem Nutzungslimit
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountPause {
    /// Der pausierte Account
    pub account: String,
    /// Ende der Pause (ISO)
    pub until: String,
    /// Der ganze Dispatcher pausiert mit
    pub global: bool,
    /// Mehr als ein aktiver Account
    pub multi: bool,
}

/// Einstellung, eigener Benutzer, Board-Dienste, lokale Dateien und der Starter der Worker.
pub struct DispatchContext {
    pub cfg: DispatchConfig,
    pub me: RepoBoardOwner,
    pub home: HarnessHome,
    pub stores: DispatchStores,
    pub output: RunOutputReader,
    pub prompts: DispatchPrompt,
    backend: Arc<dyn BoardBackend>,
    launcher: Arc<dyn WorkerLauncher>,
    clock: Clock,
    notifier: Arc<dyn DispatchNotify>,
    actor: BoardActor,
    invocation: ClaudeInvocation,
    git_bash: GitBashLocator,
    /// Umgebung, die der Worker erbt (im Betrieb die des Servers)
    env: BTreeMap<String, String>,
    /// Name der Plattform wie in Node (`win32`, `darwin`, `linux`)
    platform: String,
    /// Nur lesen: [`DispatchContext::repo`] richtet den Harness-Ordner des Repos nicht ein,
    /// sondern öffnet ihn. Der Schattenbetrieb fasst damit auch das Repo nicht an
    read_only: bool,
    /// Hauptordner und Worktree-Verwaltung je Board, einmal je Durchlauf eingerichtet
    repos: Mutex<HashMap<i32, Arc<RepoWork>>>,
    /// Die Claude-Accounts; `None` = ohne Account-Wahl (Tests, Vertragsmodus)
    accounts: Option<Arc<dyn AccountPool>>,
}

impl std::fmt::Debug for DispatchContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DispatchContext")
            .field("dispatcher", &self.cfg.dispatcher_name)
            .field("platform", &self.platform)
            .finish_non_exhaustive()
    }
}

impl DispatchContext {
    /// Kontext eines Durchlaufs; `env` und `platform` kommen im Betrieb aus dem Prozess
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cfg: DispatchConfig,
        me: RepoBoardOwner,
        home: HarnessHome,
        backend: Arc<dyn BoardBackend>,
        stores: DispatchStores,
        launcher: Arc<dyn WorkerLauncher>,
        clock: Clock,
        notifier: Arc<dyn DispatchNotify>,
        env: BTreeMap<String, String>,
        platform: impl Into<String>,
    ) -> Self {
        let actor = backend.actor(Some(&cfg.dispatcher_name));
        Self {
            cfg,
            me,
            home,
            stores,
            output: RunOutputReader::new(),
            prompts: DispatchPrompt::new(),
            backend,
            launcher,
            clock,
            notifier,
            actor,
            invocation: ClaudeInvocation::new(),
            git_bash: GitBashLocator::new(),
            env,
            platform: platform.into(),
            read_only: false,
            repos: Mutex::new(HashMap::new()),
            accounts: None,
        }
    }

    /// Kontext mit Account-Wahl; ohne Pool bleibt es bei der Umgebung des Servers
    pub fn with_accounts(mut self, accounts: Option<Arc<dyn AccountPool>>) -> Self {
        self.accounts = accounts;
        self
    }

    /// Die Claude-Accounts dieses Durchlaufs
    pub fn accounts(&self) -> Option<&dyn AccountPool> {
        self.accounts.as_deref()
    }

    /// Kontext, der auch den Harness-Ordner des Repos nicht anlegt (Schattenbetrieb)
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// Kontext mit der Umgebung und der Plattform dieses Prozesses
    #[allow(clippy::too_many_arguments)]
    pub fn with_process_env(
        cfg: DispatchConfig,
        me: RepoBoardOwner,
        home: HarnessHome,
        backend: Arc<dyn BoardBackend>,
        stores: DispatchStores,
        launcher: Arc<dyn WorkerLauncher>,
        clock: Clock,
        notifier: Arc<dyn DispatchNotify>,
    ) -> Self {
        Self::new(
            cfg,
            me,
            home,
            backend,
            stores,
            launcher,
            clock,
            notifier,
            std::env::vars().collect(),
            node_platform(),
        )
    }

    /// Dienste des Boards
    pub fn services(&self) -> &ServerBoardServices {
        self.backend.services()
    }

    /// Der Handelnde des Dispatchers
    pub fn actor(&self) -> &BoardActor {
        &self.actor
    }

    /// Der Handelnde eines Agenten des Dispatchers: Recherchen, Pläne und ihre Kommentare tragen
    /// seinen Namen unter dem eigenen Benutzer
    pub fn agent_actor(&self, name: &str) -> BoardActor {
        self.backend.actor(Some(name))
    }

    /// Der Handelnde eines Menschen (Arbeiten von Hand); ohne Namen der eigene Benutzer
    pub fn user_actor(&self, by: Option<&str>) -> BoardActor {
        self.backend.actor(by)
    }

    /// Aktuelle Zeit des Dispatchers
    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.clock.now()
    }

    /// Aktuelle Zeit als ISO-Text
    pub fn now_iso(&self) -> String {
        iso(self.now())
    }

    /// Profilwahl auf dieser Einstellung
    pub fn chooser(&self) -> ProfileChooser<'_> {
        ProfileChooser::new(&self.cfg)
    }

    /// Bereitschaftsprüfung auf dieser Einstellung
    pub fn readiness(&self) -> DispatchReadiness<'_> {
        DispatchReadiness::new(&self.cfg)
    }

    /// Versuchszähler auf dieser Einstellung
    pub fn attempts(&self) -> AttemptCounter<'_> {
        AttemptCounter::new(&self.cfg)
    }

    /// Name des Junior-Agents eines Tickets
    pub fn agent_name(&self, ticket_id: &str) -> String {
        format!("{}{ticket_id}", self.cfg.agent_prefix)
    }

    /// Gehört die Zuweisung einem Agenten des Dispatchers?
    pub fn is_agent(&self, assignee: Option<&str>) -> bool {
        assignee.is_some_and(|name| name.starts_with(&self.cfg.agent_prefix))
    }

    /// Kommentar des Dispatchers
    pub async fn comment(&self, ticket_id: &str, body: &str) -> Result<()> {
        self.services()
            .comments
            .add(ticket_id, body, &self.actor, false)
            .await?;
        Ok(())
    }

    /// Meldung; Fehler halten den Durchlauf nicht auf
    pub async fn notify(&self, notice: &DispatchNotice) {
        if let Err(error) = self.notifier.notify(notice).await {
            self.log(&format!("Meldung nicht zugestellt: {error}"));
        }
    }

    /// Zeile in `dispatch.log`
    pub fn log(&self, text: &str) {
        self.stores.log.line(text);
    }

    /// Board zur ID mit dem eigenen (wirksamen) Repo-Pfad oder `None`
    pub async fn board(&self, id: i32) -> Result<Option<Board>> {
        Ok(self
            .services()
            .boards
            .find(id)
            .await?
            .map(|found| found.board))
    }

    /// Boards, die der Dispatcher bedient: aus der Einstellung, sonst das aktive
    pub fn scope(&self) -> Vec<i32> {
        match &self.cfg.boards {
            Some(boards) => boards.iter().map(|id| *id as i32).collect(),
            None => self.me.active_board.into_iter().collect(),
        }
    }

    /// Hauptordner und Worktree-Verwaltung des Board-Repos (je Durchlauf einmal eingerichtet)
    pub async fn repo(&self, board: &Board) -> Result<Arc<RepoWork>> {
        let Some(repo_path) = board.repo_path.clone() else {
            return Err(HarnessError::new(
                "board-repo-missing",
                format!("Board {} hat keinen Repo-Pfad.", board.id),
            )
            .into());
        };
        let mut repos = self.repos.lock().await;
        if let Some(work) = repos.get(&board.id) {
            return Ok(work.clone());
        }
        let project = if self.read_only {
            HarnessProject::open(Path::new(&repo_path))?
        } else {
            HarnessProject::init(Path::new(&repo_path))?
        };
        let work = Arc::new(RepoWork {
            board: board.clone(),
            root: project.root.clone(),
            manager: self.manager(project.layout, &project.root),
        });
        repos.insert(board.id, work.clone());
        Ok(work)
    }

    /// Nie zwei Worker im selben Worktree: Grund zum Warten, wenn auf dem Branch, den das
    /// Ticket nutzen würde (Fortsetzung: sein Branch, außer er gehört nicht mehr zum Ticket;
    /// Neustart: nur gebündelt der Branch, dem es automatisch beitreten würde – sonst
    /// bekommt es einen eigenen neuen), gerade ein Lauf beliebiger Art aktiv ist
    pub async fn worktree_busy(
        &self,
        ticket_id: &str,
        branch: Option<&str>,
        work: &RepoWork,
    ) -> Result<Option<String>> {
        let own = match branch {
            Some(branch)
                if work
                    .manager
                    .stale_reason(ticket_id, Some(branch))?
                    .is_none() =>
            {
                Some(branch.to_string())
            }
            _ => None,
        };
        let join = branch.is_none() && self.cfg.join_worktrees == JoinWorktrees::SameSpec;
        let branch = match own {
            Some(branch) => Some(branch),
            None if join => work.manager.join_candidate(ticket_id, true).await?,
            None => None,
        };
        let Some(branch) = branch else {
            return Ok(None);
        };
        let run = self
            .stores
            .runs
            .list()?
            .into_iter()
            .find(|run| run.branch == branch);
        Ok(run.map(|run| format!("wartet – Worktree belegt durch {}", run.ticket_id)))
    }

    /// Wählt den Claude-Account eines Laufs (ohne Account-Verwaltung: Standard) und trägt ihn
    /// samt Begründung in die Einstellungen ein; ist keiner frei, kommt der Grund zum Warten
    /// zurück
    pub async fn with_account(&self, run: RunSettings) -> Result<ChosenAccount> {
        if run.account.is_some() {
            return Ok(ChosenAccount::Run(Box::new(run)));
        }
        let Some(accounts) = self.accounts.as_deref() else {
            return Ok(ChosenAccount::Run(Box::new(run)));
        };
        let pauses: BTreeMap<String, String> = self
            .stores
            .state
            .read()?
            .account_pauses
            .into_iter()
            .collect();
        // Nur Umsetzungsläufe, die noch laufen (ohne Ergebnis des Workers), haben Reserve offen
        let mut running = Vec::new();
        for record in self.stores.runs.list()? {
            if record.kind == RunKind::Work && self.stores.runs.result(&record.id)?.is_none() {
                running.push(record);
            }
        }
        let choice = accounts
            .pick(
                &RunAccounts::from(&run),
                &self.cfg,
                self.now(),
                &pauses,
                &running,
            )
            .await;
        Ok(match choice {
            AccountChoice::Waiting { reason, .. } => ChosenAccount::Waiting(reason),
            AccountChoice::Picked {
                account,
                why,
                snapshot,
            } => ChosenAccount::Run(Box::new(RunSettings {
                account: Some(account.id),
                account_why: Some(why),
                usage_at_start: Some(snapshot),
                ..run
            })),
        })
    }

    /// Pause nach einem Nutzungslimit: nur der Account des Laufs, bis zur Rücksetzung seines
    /// vollen Fensters laut Anthropic (falls bekannt), sonst `pause_minutes_on_limit`. Sind
    /// danach alle aktiven Accounts pausiert, pausiert der ganze Dispatcher bis zur frühesten
    /// Account-Pause (`global`). Mit nur einem aktiven Account pausiert immer der ganze
    /// Dispatcher
    pub async fn pause_account(&self, account: Option<&str>) -> Result<AccountPause> {
        let id = account.unwrap_or(STANDARD_ACCOUNT_ID).to_string();
        let now = self.now();
        let mut until = iso(now + chrono::Duration::minutes(self.cfg.pause_minutes_on_limit));
        let multi = match self.accounts.as_deref() {
            Some(accounts) => accounts.multi().await,
            None => false,
        };
        if let Some(accounts) = self.accounts.as_deref() {
            let reset = accounts
                .resets_at(&id, self.cfg.account_limit_percent)
                .await;
            if let Some(reset) = reset.filter(|at| after(at, now)) {
                until = reset;
            }
        }
        let enabled: Vec<String> = match self.accounts.as_deref() {
            Some(accounts) => accounts
                .enabled()
                .await
                .into_iter()
                .map(|account| account.id)
                .collect(),
            None => vec![STANDARD_ACCOUNT_ID.to_string()],
        };
        let global = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = global.clone();
        let stored = until.clone();
        let key = id.clone();
        self.stores
            .state
            .update(move |state| {
                state.account_pauses.retain(|_, at| after(at, now));
                state.account_pauses.insert(key, stored.clone());
                let pauses: Vec<Option<String>> = enabled
                    .iter()
                    .map(|id| state.account_pauses.get(id).cloned())
                    .collect();
                if !multi || pauses.iter().all(Option::is_some) {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    state.paused_until = if multi {
                        let mut all: Vec<String> = pauses.into_iter().flatten().collect();
                        all.sort();
                        all.first().cloned()
                    } else {
                        Some(stored)
                    };
                }
            })
            .await?;
        Ok(AccountPause {
            account: id,
            until,
            global: global.load(std::sync::atomic::Ordering::SeqCst),
            multi,
        })
    }

    /// HH:MM Ortszeit eines ISO-Zeitpunkts
    pub fn hhmm(&self, at: &str) -> String {
        hhmm(at)
    }

    /// Legt den Lauf an und startet den Worker; scheitert der Start, bleibt kein Lauf zurück.
    /// Läuft der Worker, bleibt der Lauf auch dann, wenn die Prozessnummer nicht gespeichert
    /// werden kann (ohne Nummer gilt er nach 2 min als verschwunden).
    pub async fn launch(&self, input: LaunchInput) -> Result<RunRecord> {
        let at = self.now();
        let id = self.stores.log.run_name(&input.ticket_id, input.kind, at);
        // Unter Windows braucht der Worker die Git Bash (sonst gelten die Bash-Regeln nicht);
        // ohne sie startet nichts
        let windows = self.platform == "win32";
        let git_bash = if windows {
            self.cfg
                .claude
                .git_bash_path
                .clone()
                .or_else(|| self.git_bash.locate(&self.env, &self.platform))
        } else {
            None
        };
        if windows && git_bash.is_none() {
            return Err(HarnessError::new(
                "git-bash-missing",
                "Git Bash nicht gefunden – Git for Windows installieren oder claude.gitBashPath in den Dispatcher-Einstellungen setzen.",
            )
            .into());
        }
        // Ohne gewählten Account (Planung, Recherche, Merge-Session) wählt der Start selbst;
        // keiner frei → Fehler, nichts gestartet
        let input = match self.with_account(input.run.clone()).await? {
            ChosenAccount::Waiting(reason) => {
                return Err(HarnessError::new(
                    "account-unavailable",
                    format!("Kein Claude-Account frei: {reason}"),
                )
                .into())
            }
            ChosenAccount::Run(run) => LaunchInput { run: *run, ..input },
        };
        let (config_dir, oauth_token) =
            match (self.accounts.as_deref(), input.run.account.as_deref()) {
                (Some(accounts), Some(account)) => (
                    accounts.config_dir(account).await,
                    accounts.oauth_token(account).await,
                ),
                _ => (None, None),
            };
        let setup_timeout_minutes = input
            .setup
            .as_ref()
            .is_some_and(|setup| !setup.is_empty())
            .then_some(self.cfg.worktree_setup_timeout_minutes);
        let record = RunRecord {
            id: id.clone(),
            kind: input.kind,
            ticket_id: input.ticket_id.clone(),
            board_id: i64::from(input.board_id),
            title: input.title,
            agent: input.agent,
            cwd: input.cwd,
            branch: input.branch,
            target: input.target,
            run: input.run.clone(),
            attempt: input.attempt,
            command: self.cfg.claude.command.clone(),
            args: self
                .invocation
                .args(&self.cfg, &input.run, &input.extra_tools, input.kind),
            prompt: input.prompt,
            log_file: self
                .stores
                .log
                .file_path(&format!("{id}.log"))
                .to_string_lossy()
                .into_owned(),
            started_at: iso(at),
            pid: None,
            setup: input.setup,
            setup_timeout_minutes,
            question_id: input.question_id,
            research_before: input.research_before,
            planned_before: input.planned_before,
            repo_root: input.repo_root,
        };
        self.stores.runs.create(&record)?;
        let ticket = match input.kind {
            RunKind::Research => None,
            _ => Some(input.ticket_id.as_str()),
        };
        let mut env = self.invocation.env(
            &self.env,
            &self.cfg,
            ticket,
            git_bash.as_deref(),
            config_dir.as_deref(),
        );
        self.invocation
            .apply_oauth_token(&mut env, &self.cfg, oauth_token.as_deref());
        let pid = match self.launcher.launch(&id, &env) {
            Ok(pid) => pid.map(i64::from),
            Err(error) => {
                self.stores.runs.remove(&id)?;
                return Err(error);
            }
        };
        if let Err(first) = self.stores.runs.set_pid(&id, pid) {
            // Ein zweiter Versuch, bevor der Lauf ohne Prozessnummer bleibt
            let _ = first;
            if let Err(error) = self.stores.runs.set_pid(&id, pid) {
                self.log(&format!(
                    "FEHLER {}: Prozess-ID {} nicht gespeichert: {error}",
                    input.ticket_id,
                    pid.map(|pid| pid.to_string()).unwrap_or_default()
                ));
                return Ok(record);
            }
        }
        Ok(RunRecord { pid, ..record })
    }

    /// Worktree-Verwaltung des Repos, wie der Dispatcher sie braucht
    fn manager(&self, layout: HarnessLayout, root: &str) -> WorktreeManager<ServerBoardDatabase> {
        WorktreeManager::new(
            layout,
            self.services().clone(),
            PathBuf::from(root),
            WorktreeOptions {
                join_worktrees: self.cfg.join_worktrees,
                home: Some(self.home.clone()),
                ..WorktreeOptions::default()
            },
        )
    }
}

/// Liegt der ISO-Zeitpunkt nach `now`? Ein unlesbarer Zeitpunkt zählt wie `NaN` in
/// JavaScript: der Vergleich ist falsch
fn after(at: &str, now: chrono::DateTime<chrono::Utc>) -> bool {
    at.parse::<chrono::DateTime<chrono::Utc>>()
        .is_ok_and(|at| at > now)
}

/// Name der Plattform wie `process.platform` in Node
pub fn node_platform() -> &'static str {
    if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}
