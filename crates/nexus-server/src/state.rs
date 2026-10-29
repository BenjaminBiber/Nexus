//! Gemeinsamer Zustand aller Routen; Gegenstück zu den `decorate`-Aufrufen in
//! `packages/server/src/ServerApp.ts`.
//!
//! Jede Routengruppe der Phase 4 bekommt von hier, was sie braucht: Benutzerordner, Schutz,
//! Projekte, Board-Laufzeit, Systemfunktionen, Protokoll, Upstream und das Abbruch-Signal.

use std::collections::HashSet;
use std::sync::Arc;

use nexus_core::docs::SuperpowersDocScanner;
use nexus_core::HarnessHome;
use nexus_platform::Platform;
use tokio_util::sync::CancellationToken;

use crate::board_runtime::{BoardBackendFactory, BoardRuntimeHolder};
use crate::config::ServerConfig;
use crate::docs::{DocTicketService, DocsRootResolver, DocsService, SavedTicketDrafts};
use crate::events::watcher::MarkerOf;
use crate::events::{ChangeWatcher, EventHub, SpecDocWatcher, BRIDGED_EVENTS};
use crate::guard::RequestGuard;
use crate::log::ServerLog;
use crate::notices::BrowserNoticeHub;
use crate::projects::ProjectService;
use crate::proxy::Upstream;

/// Alles, was die Routen des Servers teilen
pub struct AppState {
    pub config: ServerConfig,
    pub home: HarnessHome,
    pub guard: RequestGuard,
    pub projects: ProjectService,
    pub board: BoardRuntimeHolder,
    /// Dokumente des Boards: lesen, durchsuchen, Abschnitte schreiben. Der Dienst gehört in
    /// den Zustand, weil er die laufenden Schreibvorgänge je Datei kennt
    pub docs: DocsService,
    /// Gespeicherte Ticket-Entwürfe je Dokument
    pub doc_drafts: SavedTicketDrafts,
    /// Tickets aus Dokumenten anlegen und auflisten
    pub doc_tickets: DocTicketService,
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
    /// Offene Tabs mit `notify=1`; P4-11 stellt darüber die Meldungen zu
    pub notice_tabs: Arc<BrowserNoticeHub>,
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
        let upstream = config
            .upstream
            .as_deref()
            .map(Upstream::parse)
            .transpose()?;
        // Die Watcher fragen über den Zustand ab, in dem sie selbst liegen; sie halten ihn
        // deshalb nur schwach – sonst hielte sich der Zustand selbst am Leben
        Ok(Arc::new_cyclic(|weak: &std::sync::Weak<AppState>| Self {
            home: config.home.clone(),
            guard: RequestGuard::new(config.port),
            projects: ProjectService::new(config.home.clone()),
            board: BoardRuntimeHolder::new(board),
            docs: DocsService::new(),
            doc_drafts: SavedTicketDrafts::new(config.home.clone()),
            doc_tickets: DocTicketService::new(),
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
            notice_tabs: BrowserNoticeHub::new(),
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
