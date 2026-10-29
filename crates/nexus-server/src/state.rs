//! Gemeinsamer Zustand aller Routen; Gegenstück zu den `decorate`-Aufrufen in
//! `packages/server/src/ServerApp.ts`.
//!
//! Jede Routengruppe der Phase 4 bekommt von hier, was sie braucht: Benutzerordner, Schutz,
//! Projekte, Board-Laufzeit, Systemfunktionen, Protokoll, Upstream und das Abbruch-Signal.

use std::sync::Arc;

use nexus_core::HarnessHome;
use nexus_platform::Platform;
use tokio_util::sync::CancellationToken;

use crate::board_runtime::{BoardBackendFactory, BoardRuntimeHolder};
use crate::config::ServerConfig;
use crate::docs::{DocTicketService, DocsService, SavedTicketDrafts};
use crate::guard::RequestGuard;
use crate::log::ServerLog;
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
        Ok(Arc::new(Self {
            home: config.home.clone(),
            guard: RequestGuard::new(config.port),
            projects: ProjectService::new(config.home.clone()),
            board: BoardRuntimeHolder::new(board),
            docs: DocsService::new(),
            doc_drafts: SavedTicketDrafts::new(config.home.clone()),
            doc_tickets: DocTicketService::new(),
            platform,
            log: ServerLog::new(&config.home),
            upstream,
            shutdown: CancellationToken::new(),
            pid: std::process::id(),
            config,
        }))
    }
}
