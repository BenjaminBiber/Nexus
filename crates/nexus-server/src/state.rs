//! Gemeinsamer Zustand aller Routen; Gegenstück zu den `decorate`-Aufrufen in
//! `packages/server/src/ServerApp.ts`.
//!
//! Jede Routengruppe der Phase 4 bekommt von hier, was sie braucht: Benutzerordner, Schutz,
//! Projekte, Board-Laufzeit, Systemfunktionen, Protokoll, Upstream und das Abbruch-Signal.

use std::collections::HashSet;
use std::sync::Arc;

use nexus_core::docs::SuperpowersDocScanner;
use nexus_core::settings::UserSettingsStore;
use nexus_core::HarnessHome;
use nexus_platform::Platform;
use tokio_util::sync::CancellationToken;

use crate::board_runtime::{BoardBackendFactory, BoardRuntimeHolder};
use crate::config::ServerConfig;
use crate::dispatch::claude_runner::ClaudeRunners;
use crate::dispatch::{
    DispatchScheduler, DispatchService, DispatchServiceDeps, NoWorkerLauncher,
    ProcessWorkerLauncher, ServerDispatchNotify, WorkerLauncher,
};
use crate::docs::{
    DocQuestionService, DocRevisionService, DocTicketService, DocsRootResolver, DocsService,
    SavedTicketDrafts, TicketDraftService,
};
use crate::events::watcher::MarkerOf;
use crate::events::{ChangeWatcher, EventHub, SpecDocWatcher, BRIDGED_EVENTS};
use crate::guard::RequestGuard;
use crate::log::ServerLog;
use crate::notices::BrowserNoticeHub;
use crate::notifications::{
    DbQuestionWatch, NotificationService, NotificationSetup, NtfySender, Query,
};
use crate::projects::ProjectService;
use crate::proxy::Upstream;

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
    /// Ereignisse, die noch im Node-Server entstehen und deshalb gebrückt werden
    pub bridged_events: HashSet<&'static str>,
    /// Node-Server, an den alles Unbekannte geht; `None` = kein Proxy
    pub upstream: Option<Upstream>,
    /// Wird beim Beenden ausgelöst; eigene und durchgeleitete Ereignisströme enden daran
    pub shutdown: CancellationToken,
    /// Prozessnummer, wie `GET /api/health` sie meldet
    pub pid: u32,
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
        let upstream = config
            .upstream
            .as_deref()
            .map(Upstream::parse)
            .transpose()?;
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
        Ok(Arc::new_cyclic(|weak: &std::sync::Weak<AppState>| Self {
            home: config.home.clone(),
            guard: RequestGuard::new(config.port),
            projects,
            board,
            dispatch,
            dispatch_scheduler,
            docs: DocsService::new(),
            doc_drafts: SavedTicketDrafts::new(config.home.clone()),
            doc_tickets: DocTicketService::new(),
            ticket_drafts: TicketDraftService::new(config.home.clone()),
            doc_questions: DocQuestionService::new(config.home.clone()),
            doc_revisions: DocRevisionService::new(config.home.clone()),
            claude,
            platform,
            log: ServerLog::new(&config.home),
            events: EventHub::new(),
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
            notifications,
            notice_tabs,
            bridged_events: BRIDGED_EVENTS.into_iter().collect(),
            upstream,
            shutdown: CancellationToken::new(),
            pid: std::process::id(),
            config,
        }))
    }

    /// Beendet alle Ereignisströme: Verteiler, Watcher und offene Tabs leeren (Ende des
    /// Servers; die Ströme selbst enden am Abbruch-Signal)
    pub fn close_events(&self) {
        self.events.close();
        self.board_changes.close();
        self.question_changes.close();
        self.spec_docs.close();
        self.notice_tabs.close();
    }
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

/// Der Zustand ist weg: der Server wird gerade beendet
fn gone() -> nexus_core::Error {
    nexus_core::HarnessError::new("server-stopped", "Der Server wird beendet.").into()
}
