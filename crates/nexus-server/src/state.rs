//! Gemeinsamer Zustand aller Routen; Gegenstück zu den `decorate`-Aufrufen in
//! `packages/server/src/ServerApp.ts`.
//!
//! Jede Routengruppe bekommt von hier, was sie braucht: Benutzerordner, Schutz, Projekte,
//! Board-Laufzeit, Systemfunktionen, Protokoll und das Abbruch-Signal.

use std::sync::Arc;

use nexus_core::docs::SuperpowersDocScanner;
use nexus_core::settings::UserSettingsStore;
use nexus_core::HarnessHome;
use nexus_platform::Platform;
use tokio_util::sync::CancellationToken;

use crate::accounts::{
    AccountLoginService, AccountPluginInstaller, AccountUsageOptions, AccountUsageService,
};
use crate::board_runtime::{BoardBackendFactory, BoardRuntimeHolder};
use crate::config::{DispatchAccounts, ServerConfig};
use crate::dispatch::claude_runner::ClaudeRunners;
use crate::dispatch::{
    AccountPool, DispatchScheduler, DispatchService, DispatchServiceDeps,
    DispatchSuggestionService, NoWorkerLauncher, ProcessWorkerLauncher, ServerDispatchNotify,
    StoredAccountPool, SuggestionAnalyzer, WorkerLauncher,
};
use crate::docs::{
    DocQuestionService, DocRevisionService, DocTicketService, DocsRootResolver, DocsService,
    SavedTicketDrafts, TicketDraftService,
};
use crate::events::watcher::MarkerOf;
use crate::events::{ChangeWatcher, EventHub, SpecDocWatcher};
use crate::favorites::FavoriteService;
use crate::guard::RequestGuard;
use crate::log::ServerLog;
use crate::notices::BrowserNoticeHub;
use crate::notifications::{
    DbQuestionWatch, NotificationService, NotificationSetup, NtfySender, Query,
};
use crate::projects::ProjectService;
use crate::sessions::{SessionService, TicketInfoLookup, TicketSummaries};
use crate::telegram::TelegramBot;
use nexus_core::accounts::{AccountPaths, AccountStore, AccountTokenStore};
use nexus_core::dispatch::UsageHistory;

/// Alles, was die Routen des Servers teilen
pub struct AppState {
    pub config: ServerConfig,
    pub home: HarnessHome,
    pub guard: RequestGuard,
    pub projects: ProjectService,
    pub board: Arc<BoardRuntimeHolder>,
    /// Der Dispatcher: Durchläufe, Steuerung, Merge und Review
    pub dispatch: Arc<DispatchService>,
    /// Takt des Dispatchers; [`crate::serve`] startet ihn, das Herunterfahren hält ihn an
    pub dispatch_scheduler: Arc<DispatchScheduler>,
    /// Optimierungs-Vorschläge zur Dispatcher-Einstellung
    pub suggestions: Arc<DispatchSuggestionService>,
    /// Die Claude-Analyse der Vorschläge; sie lässt nur eine Analyse gleichzeitig zu und
    /// gehört deshalb in den Zustand
    pub suggestion_analyzer: Arc<SuggestionAnalyzer>,
    /// Dokumente des Boards: lesen, durchsuchen, Abschnitte schreiben. Der Dienst gehört in
    /// den Zustand, weil er die laufenden Schreibvorgänge je Datei kennt
    pub docs: DocsService,
    /// Gespeicherte Ticket-Entwürfe je Dokument
    pub doc_drafts: SavedTicketDrafts,
    /// Tickets aus Dokumenten anlegen und auflisten
    pub doc_tickets: DocTicketService,
    /// Ticket-Entwürfe von Claude erzeugen lassen; der Dienst kennt die laufenden Erzeugungen
    pub ticket_drafts: TicketDraftService,
    /// Offene Fragen eines Dokuments herauslesen und als Board-Fragen anlegen
    pub doc_questions: DocQuestionService,
    /// Einen Bereich eines Dokuments von Claude umschreiben lassen
    pub doc_revisions: DocRevisionService,
    /// Wie Claude gestartet wird; Tests setzen hier Attrappen ein
    pub claude: ClaudeRunners,
    /// Die Sessions des Servers: Hook-Ereignisse, Liste und `sessions.json`
    pub sessions: SessionService,
    /// Die gemerkten Sessions des Servers (`favorites.json`)
    pub favorites: FavoriteService,
    /// Titel und Status von Tickets für Sessions, Favoriten und Meldungen (mit Zeitlimit)
    pub ticket_info: TicketInfoLookup,
    /// Die Claude-Accounts: Limits, Anmeldung, Plugin und die Wahl des Dispatchers
    pub accounts: AccountServices,
    pub platform: Platform,
    pub log: ServerLog,
    /// Verteiler der Ereignisse je Projekt (`sessions`, später mehr)
    pub events: Arc<EventHub>,
    /// Beobachtet die DB-Boards, die ein Ereignisstrom mit `dbBoard=<id>` angibt
    pub board_changes: Arc<ChangeWatcher>,
    /// Beobachtet die Fragen der Board-Projekte aus `dbProject=<id>`
    pub question_changes: Arc<ChangeWatcher>,
    /// Beobachtet die Superpowers-Dokumente jedes Projekts mit offenem Strom
    pub spec_docs: Arc<SpecDocWatcher>,
    /// Offene Tabs mit `notify=1`; die Meldungen gehen zuerst dorthin
    pub notice_tabs: Arc<BrowserNoticeHub>,
    /// Systemmeldungen, Tab-Meldungen und ntfy
    pub notifications: Arc<NotificationService>,
    /// Beobachtet die offenen eigenen Fragen der Board-Datenbank
    pub db_questions: Arc<DbQuestionWatch>,
    /// Der Telegram-Bot: Poller, Kopplung und Stand. [`crate::serve`] startet ihn, das
    /// Herunterfahren hält ihn an
    pub telegram: Arc<TelegramBot>,
    /// Wird beim Beenden ausgelöst; eigene und durchgeleitete Ereignisströme enden daran
    pub shutdown: CancellationToken,
    /// Prozessnummer, wie `GET /api/health` sie meldet
    pub pid: u32,
}

/// Alles rund um die Claude-Accounts, das die Routen teilen
pub struct AccountServices {
    /// Anmeldung, Plugin und Abo-Limits je Account (ein Zwischenspeicher für alle Routen)
    pub usage: Arc<AccountUsageService>,
    /// Die Accounts des Dispatchers; `None` = ohne Account-Wahl
    pub pool: Option<Arc<dyn AccountPool>>,
    /// Laufende Anmeldungen über `claude auth login`
    pub logins: Arc<AccountLoginService>,
    /// Richtet das Nexus-Plugin in Account-Ordnern ein
    pub plugins: Arc<AccountPluginInstaller>,
    pub store: AccountStore,
    pub paths: AccountPaths,
    /// Ordner der MCACC-Profile (`~/.mcacc/profiles`)
    pub mcacc_dir: std::path::PathBuf,
    /// Accounts, die nach der Anmeldung nach ihrer E-Mail heißen sollen (ohne eigenen Namen
    /// angelegt)
    pub rename_on_login: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl std::fmt::Debug for AccountServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountServices").finish_non_exhaustive()
    }
}

impl AppState {
    /// Zustand aus den Einstellungen; die Board-Laufzeit entsteht erst beim ersten Bedarf
    pub fn new(
        config: ServerConfig,
        board: BoardBackendFactory,
        platform: Platform,
    ) -> nexus_core::Result<Arc<Self>> {
        // Im Vertragsmodus startet nie ein echtes Claude; die Antworten stehen fest
        let claude = match config.is_contract() {
            true => ClaudeRunners::contract(),
            false => ClaudeRunners::default(),
        };
        Self::with_claude(config, board, platform, claude)
    }

    /// Wie [`AppState::new`], mit eigenem Start von Claude (Tests setzen Attrappen ein)
    pub fn with_claude(
        config: ServerConfig,
        board: BoardBackendFactory,
        platform: Platform,
        claude: ClaudeRunners,
    ) -> nexus_core::Result<Arc<Self>> {
        Self::build(config, board, platform, claude)
    }

    /// Baut den Zustand; die Attrappen der Tests stehen in [`ServerConfig`]
    fn build(
        config: ServerConfig,
        board: BoardBackendFactory,
        platform: Platform,
        claude: ClaudeRunners,
    ) -> nexus_core::Result<Arc<Self>> {
        // Offene Tabs zeigen Meldungen selbst (der Klick bleibt im Tab); nur ohne Tab geht
        // die Meldung den Weg des Betriebssystems
        let notice_tabs = BrowserNoticeHub::new();
        let notifications = Arc::new(NotificationService::new(NotificationSetup {
            settings: UserSettingsStore::new(config.home.clone()),
            notifier: platform.notifier.clone(),
            assets_dir: config.toast_dir.clone(),
            host: config.host.clone(),
            port: config.port,
            log: ServerLog::new(&config.home),
            tabs: Some(notice_tabs.clone()),
            // Der Vertrag prüft ntfy gegen eine Attrappe im eigenen Netz; der Abruf bleibt
            // deshalb auch dort der echte
            ntfy: NtfySender::default(),
        }));
        // Der Verteiler entsteht vor dem Zustand: die Sessions melden über ihn, und die
        // Ereignisströme nehmen denselben
        let events = EventHub::new();
        let sessions =
            SessionService::new(config.home.clone(), events.clone(), config.clock.clone());
        let favorites = FavoriteService::new(
            config.home.clone(),
            config.clock.clone(),
            ServerLog::new(&config.home),
        );
        // Claude-Accounts: ein Dienst für Limits und Anmeldung, den Dispatcher, Seitenleiste
        // und Einstellungen teilen (Zwischenspeicher je Account)
        // Im Vertragsmodus zeigt nichts auf den echten Claude-Ordner des Rechners: die Pfade
        // wurzeln im Benutzerordner des Servers, und weder Schlüsselbund noch Anthropic
        // werden gefragt
        let paths = match config.is_contract() {
            true => AccountPaths::new(std::collections::HashMap::new(), config.home.dir()),
            false => AccountPaths::from_process_env(),
        };
        let store = AccountStore::with_paths(config.home.clone(), paths.clone());
        let account_usage = config.account_usage.clone().unwrap_or_else(|| {
            Arc::new(AccountUsageService::with_options(
                Arc::new(AccountStore::with_paths(config.home.clone(), paths.clone())),
                AccountUsageOptions {
                    paths: paths.clone(),
                    clock: config.clock.clone(),
                    keychain: match config.is_contract() {
                        true => crate::accounts::no_keychain(),
                        false => crate::accounts::system_keychain(),
                    },
                    fetch: config.is_contract().then(crate::usage::no_fetch),
                    tokens: Some(AccountTokenStore::new(config.home.clone())),
                    token_post: config.is_contract().then(crate::oauth_refresh::no_post),
                    ..AccountUsageOptions::default()
                },
            ))
        });
        let plugins = config.account_plugins.clone().unwrap_or_else(|| {
            Arc::new(AccountPluginInstaller::new(
                config.home.clone(),
                paths.clone(),
            ))
        });
        // Im Vertragsmodus bindet nichts an echte Accounts oder an Anthropic
        let pool: Option<Arc<dyn AccountPool>> =
            match (&config.dispatch_accounts, config.is_contract()) {
                (_, true) | (DispatchAccounts::Off, _) => None,
                (DispatchAccounts::Pool(pool), _) => Some(pool.clone()),
                (DispatchAccounts::Stored, _) => Some(Arc::new(
                    StoredAccountPool::new(
                        Arc::new(AccountStore::with_paths(config.home.clone(), paths.clone())),
                        account_usage.clone(),
                        paths.clone(),
                    )
                    .with_tokens(AccountTokenStore::new(config.home.clone()))
                    .with_history(UsageHistory::new(config.home.clone())),
                )),
            };
        // Die Watcher fragen über den Zustand ab, in dem sie selbst liegen; sie halten ihn
        // deshalb nur schwach – sonst hielte sich der Zustand selbst am Leben
        let board = Arc::new(BoardRuntimeHolder::new(board));
        // Im Vertragsmodus startet nie ein Worker; sonst startet er dieselbe Programmdatei
        let launcher: Arc<dyn WorkerLauncher> = match config.dispatch_launcher.clone() {
            Some(launcher) => launcher,
            None if config.is_contract() => Arc::new(NoWorkerLauncher),
            None => Arc::new(ProcessWorkerLauncher::new()?),
        };
        let projects = ProjectService::new(config.home.clone());
        // Der Dispatcher meldet über dieselben Benachrichtigungen wie der Server; die Adresse
        // zeigt ins Projekt des Board-Repos
        let dispatch = Arc::new(DispatchService::new(DispatchServiceDeps {
            notifier: Arc::new(ServerDispatchNotify::new(
                ProjectService::new(config.home.clone()),
                notifications.clone(),
            )),
            clock: config.clock.clone(),
            accounts: pool.clone(),
            ..DispatchServiceDeps::new(config.home.clone(), board.clone(), launcher)
        }));
        let dispatch_scheduler = Arc::new(
            DispatchScheduler::new(
                dispatch.clone(),
                std::time::Duration::from_millis(config.dispatch_tick_ms),
            )
            .with_log({
                let log = ServerLog::new(&config.home);
                Arc::new(move |line: &str| log.write(line))
            }),
        );
        let suggestions = Arc::new(DispatchSuggestionService::new(
            board.clone(),
            config.home.clone(),
            config.clock.clone(),
        ));
        let suggestion_analyzer = Arc::new(SuggestionAnalyzer::new(
            suggestions.clone(),
            claude.suggestion.clone(),
            config.home.clone(),
            config.clock.clone(),
        ));
        Ok(Arc::new_cyclic(|weak: &std::sync::Weak<AppState>| Self {
            home: config.home.clone(),
            guard: RequestGuard::new(config.port),
            projects,
            board,
            dispatch,
            dispatch_scheduler,
            suggestions,
            suggestion_analyzer,
            docs: DocsService::new(),
            doc_drafts: SavedTicketDrafts::new(config.home.clone()),
            doc_tickets: DocTicketService::new(),
            ticket_drafts: TicketDraftService::new(config.home.clone()),
            doc_questions: DocQuestionService::new(config.home.clone()),
            doc_revisions: DocRevisionService::new(config.home.clone()),
            claude,
            sessions,
            favorites,
            ticket_info: TicketInfoLookup::new(ticket_summaries(weak)),
            accounts: AccountServices {
                usage: account_usage.clone(),
                pool,
                logins: Arc::new(login_service(
                    weak,
                    config.account_login_spawner.clone(),
                    paths.clone(),
                    config.clock.clone(),
                )),
                plugins,
                store,
                // Im Vertragsmodus schlägt nichts die echten Profile des Rechners vor
                mcacc_dir: config
                    .mcacc_dir
                    .clone()
                    .unwrap_or_else(|| match config.is_contract() {
                        true => config.home.dir().join("mcacc-profiles"),
                        false => user_home().join(".mcacc").join("profiles"),
                    }),
                paths,
                rename_on_login: std::sync::Mutex::new(std::collections::HashSet::new()),
            },
            platform,
            log: ServerLog::new(&config.home),
            events,
            board_changes: ChangeWatcher::boards(
                marker_of(weak, |state, id| {
                    Box::pin(async move {
                        state
                            .board
                            .run(move |backend| async move {
                                use nexus_core::board::database::BoardDatabase as _;
                                backend.services().db.change_marker(id).await
                            })
                            .await
                    })
                }),
                config.db_poll_ms,
            ),
            question_changes: ChangeWatcher::questions(
                marker_of(weak, |state, id| {
                    Box::pin(async move {
                        state
                            .board
                            .run(move |backend| async move {
                                use nexus_core::board::database::BoardDatabase as _;
                                backend.services().db.question_marker(id).await
                            })
                            .await
                    })
                }),
                config.db_poll_ms,
            ),
            spec_docs: SpecDocWatcher::new(signature_of(weak), config.docs_poll_ms),
            db_questions: DbQuestionWatch::new(
                notifiable_questions(weak),
                reachable_projects(weak),
                notifications.clone(),
                config.db_question_poll_ms,
            ),
            // Der Bot pollt erst, wenn `telegram.yaml` ihn einschaltet und ein Token nennt;
            // der Vertrag zeigt über `apiBase` auf seine eigene Attrappe
            telegram: Arc::new(TelegramBot::with_optional_api(
                config.home.clone(),
                config.clock.clone(),
                config.telegram_api.clone(),
                bot_router(weak, config.clock.clone()),
            )),
            notifications,
            notice_tabs,
            shutdown: CancellationToken::new(),
            pid: std::process::id(),
            config,
        }))
    }

    /// Beendet alle Ereignisströme: Verteiler, Watcher und offene Tabs leeren (Ende des
    /// Servers; die Ströme selbst enden am Abbruch-Signal). Laufende Anmeldungen von
    /// Claude-Accounts enden mit
    pub fn close_events(&self) {
        self.accounts.logins.close();
        self.events.close();
        self.board_changes.close();
        self.question_changes.close();
        self.spec_docs.close();
        self.notice_tabs.close();
    }
}

/// Der Router des Bots mit den angemeldeten Befehlen; die Handler halten den Zustand nur
/// schwach, weil der Router im Bot und der Bot im Zustand liegt
fn bot_router(
    weak: &std::sync::Weak<AppState>,
    clock: nexus_core::clock::Clock,
) -> crate::telegram::UpdateRouter {
    let mut router = crate::telegram::UpdateRouter::with_clock(clock.clone());
    crate::telegram::commands::register(&mut router, weak, clock);
    router
}

/// Die Kurzangaben der Tickets aus der Board-Datenbank; hält den Zustand nur schwach
fn ticket_summaries(weak: &std::sync::Weak<AppState>) -> TicketSummaries {
    let weak = weak.clone();
    Arc::new(move |ids: Vec<String>| {
        let weak = weak.clone();
        Box::pin(async move {
            let state = weak.upgrade().ok_or_else(gone)?;
            state
                .board
                .run(move |backend| async move { backend.services().tickets.summaries(&ids).await })
                .await
        })
    })
}

/// Die offenen eigenen Fragen der Board-Datenbank; hält den Zustand nur schwach
fn notifiable_questions(
    weak: &std::sync::Weak<AppState>,
) -> Query<Vec<nexus_core::board::question_types::NotifiableBoardQuestion>> {
    let weak = weak.clone();
    Arc::new(move || {
        let weak = weak.clone();
        Box::pin(async move {
            let state = weak.upgrade().ok_or_else(gone)?;
            state
                .board
                .run(|backend| async move {
                    let owner = backend.actor(None).user_id.clone();
                    backend.services().questions.notifiable(&owner).await
                })
                .await
        })
    })
}

/// Die angemeldeten Projekte samt Erreichbarkeit; hält den Zustand nur schwach
fn reachable_projects(
    weak: &std::sync::Weak<AppState>,
) -> Query<Vec<nexus_core::registry::ProjectState>> {
    let weak = weak.clone();
    Arc::new(move || {
        let weak = weak.clone();
        Box::pin(async move {
            let state = weak.upgrade().ok_or_else(gone)?;
            state.projects.list()
        })
    })
}

/// Baut eine Kennwert-Abfrage, die den Zustand nur schwach hält; ist er weg, endet der
/// Server gerade und der Kennwert zählt nicht mehr
fn marker_of(
    weak: &std::sync::Weak<AppState>,
    query: fn(Arc<AppState>, i32) -> crate::events::watcher::MarkerFuture,
) -> MarkerOf {
    let weak = weak.clone();
    Arc::new(move |id| {
        let weak = weak.clone();
        Box::pin(async move {
            let state = weak.upgrade().ok_or_else(gone)?;
            query(state, id).await
        })
    })
}

/// Die Signatur der Superpowers-Dokumente eines Projekts; ohne Docs-Quelle ein fester Wert.
/// Taucht später eine Quelle auf, meldet das genau eine Änderung.
fn signature_of(weak: &std::sync::Weak<AppState>) -> crate::events::SignatureOf {
    let weak = weak.clone();
    Arc::new(move |pid| {
        let weak = weak.clone();
        Box::pin(async move {
            let state = weak.upgrade().ok_or_else(gone)?;
            let opened = state.projects.open(&pid)?;
            let source = DocsRootResolver::for_state(&state)
                .resolve(&opened.project.root)
                .await?;
            Ok(match source.root {
                Some(root) => SuperpowersDocScanner::new().signature(std::path::Path::new(&root)),
                None => "none".to_string(),
            })
        })
    })
}

/// Die Anmeldung der Accounts; nach dem Erfolg benennt sie den Account nach seiner E-Mail und
/// richtet das Plugin ein. Sie hält den Zustand nur schwach
fn login_service(
    weak: &std::sync::Weak<AppState>,
    spawner: Option<crate::accounts::LoginSpawner>,
    paths: AccountPaths,
    clock: nexus_core::clock::Clock,
) -> AccountLoginService {
    let weak = weak.clone();
    let after_login: crate::accounts::login::AfterLogin = Arc::new(move |account| {
        let weak = weak.clone();
        Box::pin(async move { after_login(weak, account).await })
    });
    match spawner {
        Some(spawner) => AccountLoginService::with_spawner(after_login, spawner, paths, clock),
        None => AccountLoginService::new(after_login, paths, clock),
    }
}

/// Nach erfolgreicher Anmeldung: Name aus der E-Mail (falls ohne Namen angelegt),
/// Nexus-Plugin einrichten, falls es fehlt. Liefert den Stand für die Oberfläche
async fn after_login(
    weak: std::sync::Weak<AppState>,
    account: nexus_core::accounts::ClaudeAccount,
) -> Option<String> {
    let state = weak.upgrade()?;
    let accounts = &state.accounts;
    let info = accounts.usage.info(&account, None, false).await;
    let rename = accounts
        .rename_on_login
        .lock()
        .expect("Sperre der Umbenennungen")
        .remove(&account.id);
    if let (true, Some(email)) = (rename, info.email.as_deref()) {
        let _ = accounts.store.update(
            &account.id,
            &nexus_core::accounts::AccountPatch {
                name: Some(email.to_string()),
                ..nexus_core::accounts::AccountPatch::default()
            },
        );
    }
    if info.plugin {
        return None;
    }
    let command = nexus_core::dispatch::DispatchConfigStore::new(state.home.clone())
        .read()
        .map(|read| read.config.claude.command)
        .unwrap_or_else(|_| "claude".to_string());
    match accounts.plugins.install(&account, &command) {
        Ok(()) => Some("Nexus-Plugin eingerichtet".to_string()),
        Err(error) => Some(format!("Plugin nicht eingerichtet: {error}")),
    }
}

/// Benutzerordner des Rechners (wie `os.homedir()` in Node)
fn user_home() -> std::path::PathBuf {
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_default()
}

/// Der Zustand ist weg: der Server wird gerade beendet
fn gone() -> nexus_core::Error {
    nexus_core::HarnessError::new("server-stopped", "Der Server wird beendet.").into()
}
