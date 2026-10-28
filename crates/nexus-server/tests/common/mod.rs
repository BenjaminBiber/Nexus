//! Testumgebung für den Server; Vorbild `packages/server/tests/helpers/TestServer.ts`.
//!
//! Der Server läuft ohne Port: die Anfragen gehen über `tower::ServiceExt::oneshot` direkt in
//! den Router, genau wie `app.inject` in Fastify. Jeder Server bekommt einen eigenen
//! Benutzerordner, eine Board-Laufzeit im Speicher und Attrappen für die Systemfunktionen.
//! Spätere Tickets erweitern diesen Helfer.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Request, StatusCode};
use axum::Router;
use nexus_core::board::basics::BoardActor;
use nexus_core::{HarnessHome, Result};
use nexus_platform::Platform;
use nexus_server::board_runtime::{BoardBackend, BoardBackendFactory, BoardBackendFuture};
use nexus_server::config::ServerConfig;
use nexus_server::groups::RouteGroups;
use nexus_server::routes::build_router;
use nexus_server::state::AppState;
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
    _dir: tempfile::TempDir,
}

/// Einstellungen eines Testservers
#[derive(Default)]
pub struct TestServerOptions {
    pub web_dir: Option<PathBuf>,
    pub upstream: Option<String>,
    pub proxy_groups: RouteGroups,
    pub board: Option<BoardBackendFactory>,
}

impl TestServer {
    /// Startet einen Server mit frischem Benutzerordner
    pub fn start() -> TestServer {
        TestServer::with(TestServerOptions::default())
    }

    /// Startet einen Server mit eigenen Einstellungen
    pub fn with(options: TestServerOptions) -> TestServer {
        let dir = tempfile::Builder::new()
            .prefix("nexus-srv-")
            .tempdir()
            .expect("Benutzerordner");
        let home = HarnessHome::at(dir.path());
        let mut config = ServerConfig::new(PORT, home.clone());
        config.version = "test".into();
        config.web_dir = options.web_dir;
        config.upstream = options.upstream;
        config.proxy_groups = options.proxy_groups;
        let board = options.board.unwrap_or_else(memory_board);
        let state = AppState::new(config, board, Platform::fake()).expect("Zustand");
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

    /// Löscht den Ordner des Projekts (das Projekt gilt danach als nicht erreichbar)
    pub fn remove(&self) {
        let _ = std::fs::remove_dir_all(&self.path);
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

/// Board-Laufzeit im Speicher: merkt sich nur, ob sie geschlossen wurde
#[derive(Debug, Default)]
pub struct MemoryBoardBackend {
    closed: AtomicBool,
    name: String,
}

impl MemoryBoardBackend {
    /// Laufzeit mit einem Namen, an dem Tests sie auseinanderhalten
    pub fn new(name: &str) -> Arc<Self> {
        Arc::new(Self {
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

/// Fabrik, die jedes Mal eine neue Laufzeit im Speicher baut
pub fn memory_board() -> BoardBackendFactory {
    nexus_server::board_runtime::factory(|| -> BoardBackendFuture {
        Box::pin(async { Ok(MemoryBoardBackend::new("speicher") as Arc<dyn BoardBackend>) })
    })
}
