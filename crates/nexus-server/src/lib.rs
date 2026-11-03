//! Server von Nexus: HTTP-API, Dispatch-Worker und Benachrichtigungen.
//!
//! Der Server ist eine axum-Anwendung und beantwortet die ganze API selbst; alles, was keine
//! Route kennt, geht an die statische Auslieferung der Oberfläche bzw. endet als 404.

pub mod accounts;
pub mod board_db;
pub mod board_runtime;
pub mod config;
pub mod conflict;
pub mod dispatch;
pub mod docs;
pub mod error;
pub mod events;
pub mod extract;
pub mod favorites;
pub mod guard;
pub mod input;
pub mod log;
pub mod notices;
pub mod notifications;
pub mod oauth_refresh;
pub mod projects;
pub mod routes;
pub mod sessions;
pub mod setup;
pub mod speech;
pub mod state;
pub mod timeline;
pub mod usage;

use std::net::SocketAddr;
use std::sync::Arc;

use nexus_core::board::basics::BoardActor;
use nexus_core::board::config::BoardConfig;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::repo_file::BoardRepoFile;
use nexus_core::board::repo_paths::BoardRepoPaths;
use nexus_core::board::services::BoardServices;
use nexus_core::clock::Clock;
use nexus_core::registry::{ServerInfo, ServerInfoFile};
use nexus_core::{HarnessError, HarnessHome, Result};
use nexus_db::{AnyBoardDatabase, ConnectOptions};
use nexus_platform::Platform;
use tokio::net::TcpListener;

use crate::board_db::{ServerBoardDatabase, ServerBoardServices};
use crate::board_runtime::{BoardBackendFactory, BoardBackendFuture};
use crate::config::ServerConfig;
use crate::dispatch::scheduler::TickGate;
use crate::state::AppState;

/// Ein laufender Server
pub struct ServerHandle {
    /// Der Port, auf dem wirklich gehört wird (bei Port 0 der gewählte)
    pub port: u16,
    /// Der Zustand, den die Routen teilen
    pub state: Arc<AppState>,
    /// Läuft, bis der Server beendet ist
    task: tokio::task::JoinHandle<()>,
    info: ServerInfoFile,
}

impl ServerHandle {
    /// Beendet den Server geordnet: Ereignisströme enden, `server.json` wird gelöscht
    pub async fn stop(self) {
        self.state.log.write("Server wird beendet.");
        self.state.shutdown.cancel();
        // Vor dem Abbau der Board-Laufzeit anhalten, damit kein Durchlauf sie neu aufbaut
        self.state.dispatch_scheduler.stop().await;
        self.state.db_questions.stop();
        // Ausstehende Änderungen der Sessions landen noch in `sessions.json`
        self.state.sessions.close().await;
        self.state.close_events();
        let _ = self.task.await;
        self.state.board.reset().await;
        let _ = self.info.release(self.state.pid);
    }

    /// Wartet, bis der Server von selbst endet (nach [`crate::shutdown_signal`])
    pub async fn joined(self) {
        let _ = self.task.await;
    }
}

/// Startet den Server: Port öffnen, `server.json` schreiben, Anfragen beantworten.
///
/// Ist der Port belegt, kommt `port-busy` mit der Meldung von heute; der Aufrufer schreibt
/// sie ins Protokoll und auf die Fehlerausgabe und endet mit Code 1.
pub async fn serve(
    config: ServerConfig,
    board: BoardBackendFactory,
    platform: Platform,
) -> Result<ServerHandle> {
    let address = format!("{}:{}", config.host, config.port);
    let port = config.port;
    let state = AppState::new(config, board, platform)?;
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|error| bind_error(port, &error))
        .inspect_err(|error| state.log.write(&error.to_string()))?;
    serve_listening(state, listener).await
}

/// Öffnet den Port, bevor etwas anderes startet.
///
/// `nexus serve` bindet damit zuerst den Port: Ist er belegt, fällt das sofort auf, bevor
/// irgendetwas anderes hochkommt.
pub async fn bind(host: &str, port: u16) -> Result<TcpListener> {
    TcpListener::bind(format!("{host}:{port}"))
        .await
        .map_err(|error| bind_error(port, &error).into())
}

/// Startet den Server auf einem schon gebundenen Port (`nexus serve` und die App)
pub async fn serve_on(
    listener: TcpListener,
    config: ServerConfig,
    board: BoardBackendFactory,
    platform: Platform,
) -> Result<ServerHandle> {
    let state = AppState::new(config, board, platform)?;
    serve_listening(state, listener).await
}

/// `port-busy`, wenn der Port belegt ist, sonst `start-failed` – wortgleich zu heute
fn bind_error(port: u16, error: &std::io::Error) -> HarnessError {
    let busy = error.kind() == std::io::ErrorKind::AddrInUse;
    let message = if busy {
        format!("Port {port} ist belegt.")
    } else {
        format!("Start fehlgeschlagen: {error}")
    };
    HarnessError::new(if busy { "port-busy" } else { "start-failed" }, message)
}

/// Der gemeinsame Teil von [`serve`] und [`serve_on`]: `server.json` schreiben, Routen bauen,
/// Anfragen beantworten
async fn serve_listening(state: Arc<AppState>, listener: TcpListener) -> Result<ServerHandle> {
    // Bei Port 0 steht der wirkliche Port erst jetzt fest; der Schutz muss ihn kennen
    let local: SocketAddr = listener
        .local_addr()
        .map_err(nexus_core::Error::unexpected)?;
    let port = local.port();
    state.guard.allow_port(port);
    // Mit diesem Token weist sich die Oberfläche der Desktop-App aus; es steht in
    // `server.json` und darf nirgends ins Protokoll
    state.guard.allow_app_token(state.config.app_token.clone());
    if let Some(origin) = &state.config.dev_origin {
        state.guard.allow_app_origin(origin);
    }
    // Bei Port 0 zeigen die Links in Meldungen erst jetzt auf den richtigen Port
    state.notifications.set_port(port);

    let info = ServerInfoFile::new(state.home.clone());
    let eingetragen = info.claim(&ServerInfo {
        pid: state.pid,
        port,
        started_at: now_iso(),
        app_token: Some(state.config.app_token.clone()),
    })?;
    if !eingetragen {
        state
            .log
            .write("server.json gehört einem anderen laufenden Server und bleibt unverändert.");
    }
    state.log.write(&format!(
        "Server läuft auf http://{}:{port}",
        state.config.host
    ));
    // Meldungen für neue Fragen und Recherchen der Board-Datenbank (Abstand 0 = aus)
    state.db_questions.start();
    // Dispatch läuft immer in Rust; ein Takt von 0 schaltet ihn ab
    state.log.write(&format!(
        "Dispatcher: Takt {} ms",
        state.config.dispatch_tick_ms
    ));
    state
        .dispatch_scheduler
        .start_with(dispatch_gate(info.clone(), state.pid))
        .await;

    let router = routes::build_router(state.clone());
    let shutdown = state.shutdown.clone();
    let task = tokio::spawn(async move {
        let server = axum::serve(listener, router)
            .with_graceful_shutdown(async move { shutdown.cancelled().await });
        if let Err(error) = server.await {
            eprintln!("Server beendet sich mit einem Fehler: {error}");
        }
    });
    Ok(ServerHandle {
        port,
        state,
        task,
        info,
    })
}

/// Freigabe des Dispatcher-Takts: nur der Server, der in `server.json` steht, stößt Durchläufe
/// an. Zwei Server auf demselben Benutzerordner mergten und starteten sonst doppelt; ein
/// zweiter Server übernimmt die Datei auch nicht, wenn der erste endet
fn dispatch_gate(info: ServerInfoFile, pid: u32) -> TickGate {
    Arc::new(move || info.owned_by(pid))
}

/// Wartet auf Strg+C bzw. SIGTERM; danach wird der Server geordnet beendet
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(term) => term,
            Err(_) => return tokio::signal::ctrl_c().await.unwrap_or_default(),
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Zeitpunkt in ISO-Form mit Millisekunden, genau wie `new Date().toISOString()`
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Die Board-Laufzeit aus `board.yaml`: lesen, verbinden, Schema prüfen, Benutzer anlegen.
/// Fehlt die Konfig, meldet schon das Lesen `board-config-missing` (503).
pub fn board_from_config(home: HarnessHome) -> BoardBackendFactory {
    board_runtime::factory(move || open_board(home.clone()))
}

/// Liest `board.yaml`, öffnet die Datenbank daraus und baut die Dienste des Boards
fn open_board(home: HarnessHome) -> BoardBackendFuture {
    Box::pin(async move {
        let config = BoardConfigStore::new(home.clone()).require()?;
        let db = open_database(config.clone()).await?;
        // Die eigenen Repo-Pfade des Benutzers stehen lokal in `board-repos.yaml`, nicht in
        // der gemeinsamen Datenbank
        let repo_paths = BoardRepoPaths::new(
            Arc::new(BoardRepoFile::new(home)),
            Some(config.user_id.clone()),
        );
        let services = BoardServices::new(
            Arc::new(ServerBoardDatabase::Open(db)),
            Clock::system(),
            repo_paths,
        );
        Ok(Arc::new(OpenedBoardBackend { config, services })
            as Arc<dyn board_runtime::BoardBackend>)
    })
}

/// Öffnet die Datenbank der Konfig und prüft ihr Schema.
///
/// Eigene Funktion mit eigener Konfig, weil [`nexus_db::open_checked`] seinen Öffner als
/// asynchronen Abschluss nimmt: nimmt die Funktion die Konfig geliehen, hält der Compiler die
/// umgebende Zukunft nicht mehr für `'static` (rust-lang/rust#100013).
async fn open_database(config: BoardConfig) -> Result<AnyBoardDatabase> {
    Ok(
        nexus_db::open_checked_owned(config, ConnectOptions::default())
            .await?
            .db,
    )
}

/// Die geöffnete Board-Datenbank als Laufzeit des Servers samt den Diensten des Boards
struct OpenedBoardBackend {
    config: BoardConfig,
    services: ServerBoardServices,
}

#[async_trait::async_trait]
impl board_runtime::BoardBackend for OpenedBoardBackend {
    fn services(&self) -> &ServerBoardServices {
        &self.services
    }

    fn actor(&self, name: Option<&str>) -> BoardActor {
        let name = name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.config.user_name);
        BoardActor {
            user_id: self.config.user_id.clone(),
            name: name.to_string(),
        }
    }

    async fn close(&self) -> Result<()> {
        use nexus_core::board::database::BoardDatabase as _;
        self.services.db.close().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_zeitstempel_sieht_aus_wie_in_javascript() {
        let stamp = now_iso();
        assert_eq!(stamp.len(), 24, "{stamp}");
        assert!(stamp.ends_with('Z'), "{stamp}");
        assert_eq!(&stamp[10..11], "T", "{stamp}");
    }
}
