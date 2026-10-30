//! Testumgebung für den Server; Vorbild `packages/server/tests/helpers/TestServer.ts`.
//!
//! Der Server läuft ohne Port: die Anfragen gehen über `tower::ServiceExt::oneshot` direkt in
//! den Router, genau wie `app.inject` in Fastify. Jeder Server bekommt einen eigenen
//! Benutzerordner, eine Board-Laufzeit im Speicher und Attrappen für die Systemfunktionen.
//! Spätere Tickets erweitern diesen Helfer.

#![allow(dead_code)]

pub mod notify;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Request, StatusCode};
use axum::Router;
use nexus_core::board::basics::BoardActor;
use nexus_core::board::config::{BoardConfig, BoardProvider};
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::database::BoardDatabase as _;
use nexus_core::board::in_memory::InMemoryBoardDatabase;
use nexus_core::board::repo_paths::{BoardRepoPaths, MemoryBoardRepoStore};
use nexus_core::board::services::BoardServices;
use nexus_core::board::types::BoardUser;
use nexus_core::clock::Clock;
use nexus_core::{tx, HarnessError, HarnessHome, Result};
use nexus_platform::Platform;
use nexus_server::board_db::{ServerBoardDatabase, ServerBoardServices};
use nexus_server::board_runtime::{BoardBackend, BoardBackendFactory, BoardBackendFuture};
use nexus_server::config::ServerConfig;
use nexus_server::dispatch::{
    ClaudeRunInput, ClaudeRunner, ClaudeRunners, RunResult, WorkerLauncher,
};
use nexus_server::routes::build_router;
use nexus_server::state::AppState;
use nexus_server::usage::ClaudeUsageService;
use tower::ServiceExt as _;

/// Port, den der Schutz des Testservers erlaubt
pub const PORT: u16 = 4710;

/// Eine Antwort des Testservers
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub text: String,
}

impl Reply {
    /// Der Körper als JSON (`None`, wenn es keines ist)
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_str(&self.text).ok()
    }

    /// Der Körper als JSON; unbrauchbare Antworten brechen den Test ab
    pub fn body(&self) -> serde_json::Value {
        self.json()
            .unwrap_or_else(|| panic!("Antwort ist kein JSON: {}", self.text))
    }

    /// Ein Kopf der Antwort als Text
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    /// Der Fehlercode im JSON-Körper
    pub fn code(&self) -> Option<String> {
        self.json()?
            .get("code")?
            .as_str()
            .map(|code| code.to_string())
    }
}

/// Server mit eigenem Benutzerordner, ohne Port
pub struct TestServer {
    pub state: Arc<AppState>,
    pub home: HarnessHome,
    router: Router,
    /// Der eigene Benutzerordner; ein neu gestarteter Server teilt den des ersten
    _dir: Option<tempfile::TempDir>,
}

/// Einstellungen eines Testservers
#[derive(Default)]
pub struct TestServerOptions {
    pub web_dir: Option<PathBuf>,
    pub board: Option<BoardBackendFactory>,
    /// Systemfunktionen des Servers; ohne Angabe die Attrappen
    pub platform: Option<Platform>,
    /// Die Board-Laufzeit aus `board.yaml` statt der im Speicher: nur so prüfen Tests die
    /// Antworten ohne und mit kaputter Konfig
    pub board_from_config: bool,
    /// Abstand des Herzschlags im Ereignisstrom; ohne Angabe der Standard
    pub heartbeat_ms: Option<u64>,
    /// Takt der Watcher; ohne Angabe der Standard
    pub poll_ms: Option<u64>,
    /// Der Start von Claude; ohne Angabe der echte Prozess (Tests der Docs-Routen setzen hier
    /// immer Attrappen ein, damit nie das echte `claude` startet)
    pub claude: Option<ClaudeRunners>,
    /// Uhr des Servers; ohne Angabe die des Rechners
    pub clock: Option<Clock>,
    /// Start der Worker; ohne Angabe eine Attrappe, die nichts startet
    pub dispatch_launcher: Option<Arc<dyn WorkerLauncher>>,
    /// Die Abo-Limits von Claude; ohne Angabe der echte Dienst (den Tests nie fragen)
    pub usage: Option<ClaudeUsageService>,
    /// Benutzerordner eines schon laufenden Servers (Neustart mit demselben Bestand);
    /// ohne Angabe ein frischer
    pub home: Option<HarnessHome>,
}

/// Startet nichts und liefert keine Prozessnummer; so startet in Tests nie ein echter Worker
#[derive(Debug, Default)]
pub struct NoWorkerLauncher;

impl WorkerLauncher for NoWorkerLauncher {
    fn launch(
        &self,
        _run_id: &str,
        _env: &std::collections::BTreeMap<String, String>,
    ) -> Result<Option<u32>> {
        Ok(None)
    }
}

impl TestServer {
    /// Startet einen Server mit frischem Benutzerordner
    pub fn start() -> TestServer {
        TestServer::with(TestServerOptions::default())
    }

    /// Startet einen Server mit eigenen Einstellungen
    pub fn with(options: TestServerOptions) -> TestServer {
        let dir = match options.home {
            Some(_) => None,
            None => Some(
                tempfile::Builder::new()
                    .prefix("nexus-srv-")
                    .tempdir()
                    .expect("Benutzerordner"),
            ),
        };
        let home = options
            .home
            .unwrap_or_else(|| HarnessHome::at(dir.as_ref().expect("Benutzerordner").path()));
        let mut config = ServerConfig::new(PORT, home.clone());
        config.version = "test".into();
        config.web_dir = options.web_dir;
        if let Some(heartbeat_ms) = options.heartbeat_ms {
            config.heartbeat_ms = heartbeat_ms;
        }
        if let Some(poll_ms) = options.poll_ms {
            config.db_poll_ms = poll_ms;
            config.docs_poll_ms = poll_ms;
        }
        if let Some(clock) = options.clock {
            config.clock = clock;
        }
        config.dispatch_launcher = Some(
            options
                .dispatch_launcher
                .unwrap_or_else(|| Arc::new(NoWorkerLauncher)),
        );
        let board = options.board.unwrap_or_else(|| {
            if options.board_from_config {
                nexus_server::board_from_config(home.clone())
            } else {
                memory_board()
            }
        });
        let platform = options.platform.unwrap_or_else(Platform::fake);
        let claude = options.claude.unwrap_or_default();
        let state = AppState::with_claude_and_usage(config, board, platform, claude, options.usage)
            .expect("Zustand");
        // Wie beim echten Start: das Token der Einstellungen lässt die Oberfläche der App durch
        state.guard.allow_app_token(state.config.app_token.clone());
        TestServer {
            router: build_router(state.clone()),
            state,
            home,
            _dir: dir,
        }
    }

    /// Anfrage mit den Standardköpfen eines erlaubten Browsers
    pub async fn request(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
        headers: &[(&str, Option<&str>)],
    ) -> Reply {
        let response = self.send(method, url, body, headers).await;
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, 8 * 1024 * 1024)
            .await
            .expect("Körper der Antwort");
        Reply {
            status: parts.status,
            headers: parts.headers,
            text: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    /// Wie [`TestServer::request`], liefert aber die Antwort mit noch offenem Körper (Ströme)
    pub async fn send(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
        headers: &[(&str, Option<&str>)],
    ) -> axum::response::Response {
        let write = method != "GET" && method != "HEAD";
        let mut request = Request::builder()
            .method(method)
            .uri(url)
            .header("host", format!("127.0.0.1:{PORT}"));
        if write {
            request = request
                .header("content-type", "application/json")
                .header("x-harness", "1");
        }
        let mut request = request
            .body(
                body.map(|body| Body::from(body.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .expect("Anfrage");
        // Angegebene Köpfe überschreiben die Standardköpfe; `None` lässt einen weg
        for (name, value) in headers {
            let name = HeaderName::from_bytes(name.as_bytes()).expect("Kopfname");
            match value {
                Some(value) => {
                    request
                        .headers_mut()
                        .insert(name, HeaderValue::from_str(value).expect("Kopfwert"));
                }
                None => {
                    request.headers_mut().remove(name);
                }
            }
        }
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("Antwort des Servers")
    }

    /// Das Start-Token dieses Servers, mit dem sich die Oberfläche der App ausweist
    pub fn app_token(&self) -> String {
        self.state.config.app_token.as_str().to_string()
    }

    /// `GET` mit zusätzlichen Köpfen
    pub async fn get_with(&self, url: &str, headers: &[(&str, Option<&str>)]) -> Reply {
        self.request("GET", url, None, headers).await
    }

    /// `GET`
    pub async fn get(&self, url: &str) -> Reply {
        self.request("GET", url, None, &[]).await
    }

    /// `POST` mit JSON-Körper
    pub async fn post(&self, url: &str, body: &str) -> Reply {
        self.request("POST", url, Some(body), &[]).await
    }

    /// `PUT` mit JSON-Körper
    pub async fn put(&self, url: &str, body: &str) -> Reply {
        self.request("PUT", url, Some(body), &[]).await
    }

    /// `PATCH` mit JSON-Körper
    pub async fn patch(&self, url: &str, body: &str) -> Reply {
        self.request("PATCH", url, Some(body), &[]).await
    }

    /// `DELETE`
    pub async fn delete(&self, url: &str) -> Reply {
        self.request("DELETE", url, Some("{}"), &[]).await
    }

    /// Das Hook-Token eines Projektordners (`harness init` hat es angelegt)
    pub fn token(&self, repo: &std::path::Path) -> String {
        let project = nexus_core::HarnessProject::open(repo).expect("Projekt öffnen");
        nexus_core::project::TokenStore::new(project.layout.clone())
            .read()
            .expect("Token lesen")
    }

    /// Schickt ein Hook-Ereignis mit einem Token; `None` lässt den Kopf weg
    pub async fn hook(
        &self,
        project: &str,
        event: serde_json::Value,
        token: Option<&str>,
    ) -> Reply {
        let body = serde_json::json!({ "project": project, "event": event }).to_string();
        let headers: Vec<(&str, Option<&str>)> = vec![("x-harness-token", token)];
        self.request("POST", "/api/hook", Some(&body), &headers)
            .await
    }

    /// Startet denselben Server neu: gleicher Benutzerordner, frischer Zustand
    pub fn restarted_with(&self, options: TestServerOptions) -> TestServer {
        TestServer::with(TestServerOptions {
            home: Some(self.home.clone()),
            ..options
        })
    }
}

/// Der Testbenutzer der Board-Laufzeit im Speicher (wie `MEMORY_USER` in TypeScript)
pub const MEMORY_USER_ID: &str = "11111111-1111-1111-1111-111111111111";
pub const MEMORY_USER_NAME: &str = "Florian";

/// Platzhalter-Konfig für Tests (kein echter Server, kein echtes Passwort)
pub fn memory_config() -> BoardConfig {
    BoardConfig {
        // `file` gilt nur bei Anbieter sqlite
        provider: BoardProvider::Mssql,
        file: String::new(),
        server: "memory".into(),
        port: 1433,
        database: "AiHarness_Test".into(),
        user: "test".into(),
        password: "test".into(),
        encrypt: false,
        trust_server_certificate: true,
        user_id: MEMORY_USER_ID.into(),
        user_name: MEMORY_USER_NAME.into(),
        active_board: None,
    }
}

impl TestServer {
    /// Server mit Board-Laufzeit im Speicher und geschriebener `board.yaml`; Gegenstück zu
    /// `TestServer.startWithBoard()` in TypeScript
    pub async fn start_with_board() -> (TestServer, Arc<BoardBackendWithServices>) {
        TestServer::start_with_board_and(TestServerOptions::default()).await
    }

    /// Wie [`TestServer::start_with_board`], aber mit eigenen Einstellungen
    pub async fn start_with_board_and(
        mut options: TestServerOptions,
    ) -> (TestServer, Arc<BoardBackendWithServices>) {
        let backend = BoardBackendWithServices::create().await;
        let shared = backend.clone();
        options.board = Some(nexus_server::board_runtime::factory(
            move || -> BoardBackendFuture {
                let backend = shared.clone();
                Box::pin(async move { Ok(backend as Arc<dyn BoardBackend>) })
            },
        ));
        let server = TestServer::with(options);
        BoardConfigStore::new(server.home.clone())
            .write(&memory_config())
            .expect("board.yaml schreiben");
        (server, backend)
    }
}

/// Board-Laufzeit im Speicher mit allen Diensten; Gegenstück zu
/// `packages/server/tests/helpers/MemoryBoardBackend.ts`
pub struct BoardBackendWithServices {
    pub services: ServerBoardServices,
    pub repos: Arc<MemoryBoardRepoStore>,
    closed: AtomicBool,
}

impl BoardBackendWithServices {
    /// Laufzeit mit dem Testbenutzer in der Datenbank
    pub async fn create() -> Arc<Self> {
        let db = Arc::new(ServerBoardDatabase::Memory(Box::new(
            InMemoryBoardDatabase::new(),
        )));
        let user = BoardUser {
            id: MEMORY_USER_ID.into(),
            name: MEMORY_USER_NAME.into(),
        };
        tx!(db, [user], |u| { u.users().upsert(&user).await })
            .await
            .expect("Testbenutzer anlegen");
        let repos = Arc::new(MemoryBoardRepoStore::new());
        let paths = BoardRepoPaths::new(repos.clone(), Some(MEMORY_USER_ID.to_string()));
        Arc::new(Self {
            services: BoardServices::new(db, Clock::system(), paths),
            repos,
            closed: AtomicBool::new(false),
        })
    }

    /// Setzt den eigenen Repo-Pfad des Testbenutzers für ein Board (Gegenstück zu
    /// `new BoardRepoPaths(backend.repoStore, MEMORY_USER.id).set(id, path)` in TypeScript)
    pub fn set_repo_path(&self, board_id: i32, path: &std::path::Path) {
        self.services
            .repo_paths
            .set(board_id, Some(&path.to_string_lossy()))
            .expect("Repo-Pfad setzen");
    }

    /// Der Handelnde des Testbenutzers
    pub fn actor(&self) -> BoardActor {
        BoardActor {
            user_id: MEMORY_USER_ID.into(),
            name: MEMORY_USER_NAME.into(),
        }
    }

    /// Wurde sie geschlossen?
    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Jeder weitere Zugriff scheitert, als wäre die Verbindung weg
    pub fn break_connection(&self) {
        self.fail_with(Some(HarnessError::new(
            "board-db-unreachable",
            "Datenbank weg",
        )));
    }

    /// Jeder weitere Zugriff scheitert, als fehlte die Erweiterung der Läufe
    pub fn outdate_schema(&self) {
        self.fail_with(Some(HarnessError::new(
            "board-schema-outdated",
            "Erweiterung dispatch-run-details fehlt",
        )));
    }

    /// Hebt [`BoardBackendWithServices::break_connection`] bzw.
    /// [`BoardBackendWithServices::outdate_schema`] wieder auf
    pub fn mend_connection(&self) {
        self.fail_with(None);
    }

    /// Setzt den Fehler der Datenbank im Speicher
    fn fail_with(&self, error: Option<HarnessError>) {
        match &*self.services.db {
            ServerBoardDatabase::Memory(db) => db.fail_with(error),
            ServerBoardDatabase::Open(_) => panic!("nur für die Datenbank im Speicher"),
        }
    }
}

#[async_trait]
impl BoardBackend for BoardBackendWithServices {
    fn services(&self) -> &ServerBoardServices {
        &self.services
    }

    fn actor(&self, name: Option<&str>) -> BoardActor {
        let name = name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(MEMORY_USER_NAME);
        BoardActor {
            user_id: MEMORY_USER_ID.into(),
            name: name.to_string(),
        }
    }

    async fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Legt ein eingerichtetes Testprojekt an, meldet es am Server an und liefert es samt ID
impl TestServer {
    pub fn add_project(&self) -> (TempRepo, String) {
        let repo = TempRepo::new();
        nexus_core::HarnessProject::init(repo.path()).expect("Projekt einrichten");
        let added = nexus_core::registry::ProjectRegistry::new(self.home.clone())
            .add(repo.path())
            .expect("Projekt anmelden");
        (repo, added.project.id)
    }
}

/// Temporäres Git-Projekt für Tests (Ordnername mit Leerzeichen und Umlaut); entspricht
/// `packages/core/tests/unit/helpers/TempRepo.ts`
pub struct TempRepo {
    _base: tempfile::TempDir,
    path: PathBuf,
}

impl TempRepo {
    /// Legt das Projekt mit einem ersten Commit an
    pub fn new() -> Self {
        let base = tempfile::Builder::new()
            .prefix("nexus-srv-repo-")
            .tempdir()
            .expect("Ordner");
        // Aufgelöster Pfad: unter Windows steht TEMP oft als 8.3-Kurzname, unter macOS zeigt
        // /var auf /private/var – Git liefert Pfade immer aufgelöst zurück
        let resolved = std::fs::canonicalize(base.path()).unwrap_or_else(|_| base.path().into());
        let resolved = resolved
            .to_string_lossy()
            .strip_prefix(r"\\?\")
            .map(PathBuf::from)
            .unwrap_or(resolved);
        let path = resolved.join("Mein Projekt ä");
        std::fs::create_dir(&path).expect("Projektordner");
        let repo = Self { _base: base, path };
        repo.git(&["init", "-q"]);
        repo.git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--allow-empty",
            "-q",
            "-m",
            "init",
        ]);
        repo
    }

    /// Ordner des Projekts
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Ordner über dem Projekt; dort legen Tests fremde Worktrees ab
    pub fn base(&self) -> &std::path::Path {
        self.path.parent().expect("Ordner über dem Projekt")
    }

    /// Löscht den Ordner des Projekts (das Projekt gilt danach als nicht erreichbar)
    pub fn remove(&self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }

    /// Führt Git im Projekt aus (auch für Tests, die eigene Branches brauchen)
    pub fn run_git(&self, args: &[&str]) {
        self.git(args);
    }

    /// Führt Git im Projekt aus und liefert die Ausgabe ohne Leerzeichen am Rand
    pub fn git_out(&self, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .expect("git");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Führt Git im Projekt aus
    fn git(&self, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .expect("git");
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Default for TempRepo {
    fn default() -> Self {
        Self::new()
    }
}

/// Board-Laufzeit im Speicher: merkt sich nur, ob sie geschlossen wurde. Ihre Dienste laufen
/// über eine leere Datenbank im Speicher – Tests, die wirklich Daten brauchen, nehmen
/// [`BoardBackendWithServices`].
pub struct MemoryBoardBackend {
    services: ServerBoardServices,
    closed: AtomicBool,
    name: String,
}

impl MemoryBoardBackend {
    /// Laufzeit mit einem Namen, an dem Tests sie auseinanderhalten
    pub fn new(name: &str) -> Arc<Self> {
        Arc::new(Self {
            services: BoardServices::with_memory_paths(
                Arc::new(ServerBoardDatabase::Memory(Box::new(
                    InMemoryBoardDatabase::new(),
                ))),
                Clock::system(),
            ),
            closed: AtomicBool::new(false),
            name: name.to_string(),
        })
    }

    /// Wurde sie geschlossen?
    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Ihr Name
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl BoardBackend for MemoryBoardBackend {
    fn services(&self) -> &ServerBoardServices {
        &self.services
    }

    fn actor(&self, name: Option<&str>) -> BoardActor {
        BoardActor {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            name: name.unwrap_or("Vertrag").to_string(),
        }
    }

    async fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Server, Projekt und Board mit Repo-Pfad für die Docs-Routen; Gegenstück zum `setup()`
/// der Docs-Tests in TypeScript
pub struct DocsFixture {
    pub server: TestServer,
    pub backend: Arc<BoardBackendWithServices>,
    pub repo: TempRepo,
    /// ID des angemeldeten Projekts (Adresse der Docs-Routen)
    pub pid: String,
    /// ID des angelegten Boards
    pub board_id: i32,
}

impl DocsFixture {
    /// Server, Projekt und Board; `with_repo_path` ordnet das Board dem Projektordner zu
    pub async fn start(with_repo_path: bool) -> Self {
        Self::start_with(TestServerOptions::default(), with_repo_path).await
    }

    /// Wie [`DocsFixture::start`], mit eigenen Einstellungen des Servers
    pub async fn start_with(options: TestServerOptions, with_repo_path: bool) -> Self {
        let (server, backend) = TestServer::start_with_board_and(options).await;
        let (repo, pid) = server.add_project();
        server
            .post("/api/db/projects", r#"{"key":"ARC","name":"Arches"}"#)
            .await;
        let created = server
            .post("/api/db/boards", r#"{"project":"ARC","name":"Sprint"}"#)
            .await;
        let board_id = created
            .json()
            .and_then(|json| json["id"].as_i64())
            .expect("Board-ID") as i32;
        if with_repo_path {
            backend.set_repo_path(board_id, repo.path());
        }
        Self {
            server,
            backend,
            repo,
            pid,
            board_id,
        }
    }

    /// Die Adresse der Docs-Routen dieses Projekts
    pub fn base(&self) -> String {
        format!("/api/projects/{}/docs", self.pid)
    }

    /// Der Repo-Pfad des Boards, wie die Quelle ihn ausgibt (immer mit `/`)
    pub fn root(&self) -> String {
        self.repo.path().to_string_lossy().replace('\\', "/")
    }

    /// Schreibt eine Datei im Repo des Boards
    pub fn write(&self, dir: &str, name: &str, text: &str) {
        let full = self.repo.path().join(dir);
        std::fs::create_dir_all(&full).expect("Ordner");
        std::fs::write(full.join(name), text).expect("Datei");
    }

    /// Inhalt einer Datei im Repo des Boards
    pub fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.repo.path().join(path)).expect("Datei")
    }

    /// Legt die freigegebenen Dokumente der Docs-Tests an
    pub fn write_documents(&self) {
        self.write(
            "docs/superpowers/specs",
            "2026-10-22-docs-rework-design.md",
            "# Docs-Rework\n\nDas Ticket wird angelegt.\n\n## Ziel\n\nEine Quelle.\n",
        );
        self.write(
            "docs/superpowers/plans",
            "2026-10-22-docs-rework.md",
            "# Plan Docs-Rework\n\nSchritt eins.\n",
        );
        self.write(
            "docs/adr",
            "0001-entscheidung.md",
            "# Entscheidung\n\nTickets liegen im Git-Verzeichnis.\n",
        );
        self.write(
            ".",
            "GLOSSARY.md",
            "# Glossar\n\n**Ticket**: Arbeitseinheit\n",
        );
        self.write("specs/001-login", "spec.md", "alte Spec-Kit-Datei");
        self.write(".", "geheim.md", "geheim");
    }

    /// Aktueller Fingerabdruck eines freigegebenen Dokuments (so, wie der Editor ihn beim
    /// Lesen bekommt)
    pub async fn hash_of(&self, path: &str) -> String {
        let url = format!("{}/file?path={}", self.base(), encode(path));
        self.server
            .get(&url)
            .await
            .json()
            .and_then(|json| json["hash"].as_str().map(str::to_string))
            .expect("Fingerabdruck")
    }
}

/// Prozentkodierung eines Abfragewerts (wie `encodeURIComponent`)
pub fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => char::from(byte).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// Systemfunktionen mit einem Öffner, dessen Aufrufe der Test nachsehen kann
pub fn platform_with_opener() -> (Platform, Arc<nexus_platform::FakeFileOpener>) {
    let opener = Arc::new(nexus_platform::FakeFileOpener::default());
    let platform = Platform {
        files: Box::new(SharedFileOpener(opener.clone())),
        ..Platform::fake()
    };
    (platform, opener)
}

/// Reicht das Öffnen an einen geteilten Öffner weiter, damit der Test seine Aufrufe sieht
struct SharedFileOpener(Arc<nexus_platform::FakeFileOpener>);

#[async_trait]
impl nexus_platform::FileOpener for SharedFileOpener {
    async fn open(
        &self,
        file: &std::path::Path,
        target: nexus_platform::OpenTarget,
    ) -> nexus_platform::Result<()> {
        self.0.open(file, target).await
    }
}

/// Fabrik, die jedes Mal eine neue Laufzeit im Speicher baut
pub fn memory_board() -> BoardBackendFactory {
    nexus_server::board_runtime::factory(|| -> BoardBackendFuture {
        Box::pin(async { Ok(MemoryBoardBackend::new("speicher") as Arc<dyn BoardBackend>) })
    })
}

/// Antwort von `claude -p --output-format json` mit diesem Text als Ergebnis; Gegenstück zu
/// `reply()` in den TypeScript-Tests
pub fn reply(result: &str, cost: f64) -> RunResult {
    RunResult {
        code: Some(0),
        timed_out: false,
        output: serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false,
            "result": result, "total_cost_usd": cost,
        })
        .to_string(),
    }
}

/// Ersatz für den Start von Claude: merkt sich die Läufe, antwortet der Reihe nach und kann den
/// ersten Lauf hängen lassen (bis [`FakeClaude::release`] oder ein Abbruch kommt). Gegenstück zu
/// den `run`-Attrappen in `TestServer.ts`.
#[derive(Default)]
pub struct FakeClaude {
    /// Die Läufe in ihrer Reihenfolge (Anweisung, Argumente, Arbeitsordner)
    calls: std::sync::Mutex<Vec<ClaudeRunInput>>,
    /// Antworten der Reihe nach; ist keine mehr da, gilt die letzte noch einmal
    replies: std::sync::Mutex<std::collections::VecDeque<RunResult>>,
    /// Hängt der erste Lauf, bis er freigegeben oder abgebrochen wird?
    hold: AtomicBool,
    gate: tokio::sync::Notify,
}

impl FakeClaude {
    /// Attrappe, die immer mit diesem Ergebnistext antwortet
    pub fn replying(result: &str) -> Arc<Self> {
        Arc::new(Self {
            replies: std::sync::Mutex::new([reply(result, 0.02)].into()),
            ..Self::default()
        })
    }

    /// Attrappe, die der Reihe nach diese Ergebnisse liefert
    pub fn with_replies(replies: Vec<RunResult>) -> Arc<Self> {
        Arc::new(Self {
            replies: std::sync::Mutex::new(replies.into()),
            ..Self::default()
        })
    }

    /// Lässt den ersten Lauf hängen, bis [`FakeClaude::release`] kommt oder er abgebrochen wird
    pub fn hold_first(self: &Arc<Self>) -> Arc<Self> {
        self.hold.store(true, Ordering::SeqCst);
        self.clone()
    }

    /// Gibt den hängenden Lauf frei
    pub fn release(&self) {
        self.gate.notify_one();
    }

    /// Wartet, bis so viele Läufe begonnen haben (statt eines festen Schlafs)
    pub async fn wait_for_calls(&self, count: usize) {
        for _ in 0..2000 {
            if self.calls() >= count {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("Claude wurde nicht {count}-mal gerufen");
    }

    /// Wie oft Claude gerufen wurde
    pub fn calls(&self) -> usize {
        self.calls.lock().expect("Läufe").len()
    }

    /// Die Anweisung des letzten Laufs
    pub fn last_prompt(&self) -> String {
        self.last().prompt
    }

    /// Die Argumente des letzten Laufs
    pub fn last_args(&self) -> Vec<String> {
        self.last().args
    }

    /// Der letzte Lauf
    pub fn last(&self) -> ClaudeRunInput {
        self.calls
            .lock()
            .expect("Läufe")
            .last()
            .cloned()
            .expect("ein Lauf")
    }
}

#[async_trait]
impl ClaudeRunner for FakeClaude {
    async fn run(&self, input: ClaudeRunInput) -> RunResult {
        let cancel = input.cancel.clone();
        self.calls.lock().expect("Läufe").push(input);
        if self.hold.swap(false, Ordering::SeqCst) {
            match cancel {
                Some(cancel) => {
                    tokio::select! {
                        _ = self.gate.notified() => {}
                        _ = cancel.cancelled() => {}
                    }
                }
                None => self.gate.notified().await,
            }
        }
        let mut replies = self.replies.lock().expect("Antworten");
        // Die letzte Antwort gilt weiter, damit spätere Läufe sofort antworten
        if replies.len() > 1 {
            replies.pop_front().expect("Antwort")
        } else {
            replies.front().cloned().unwrap_or_default()
        }
    }
}

/// Die Läufe des Zustands, alle über dieselbe Attrappe
pub fn fake_runners(fake: Arc<FakeClaude>) -> ClaudeRunners {
    ClaudeRunners {
        ticket_draft: fake.clone(),
        revise: fake.clone(),
        suggestion: fake,
    }
}
