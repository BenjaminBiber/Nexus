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
use crate::dispatch::claude_runner::ClaudeRunners;
use crate::docs::{
    DocQuestionService, DocRevisionService, DocTicketService, DocsService, SavedTicketDrafts,
    TicketDraftService,
};
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
        Ok(Arc::new(Self {
            home: config.home.clone(),
            guard: RequestGuard::new(config.port),
            projects: ProjectService::new(config.home.clone()),
            board: BoardRuntimeHolder::new(board),
            docs: DocsService::new(),
            doc_drafts: SavedTicketDrafts::new(config.home.clone()),
            doc_tickets: DocTicketService::new(),
            ticket_drafts: TicketDraftService::new(config.home.clone()),
            doc_questions: DocQuestionService::new(config.home.clone()),
            doc_revisions: DocRevisionService::new(config.home.clone()),
            claude,
            platform,
            log: ServerLog::new(&config.home),
            upstream,
            shutdown: CancellationToken::new(),
            pid: std::process::id(),
            config,
        }))
    }
}
