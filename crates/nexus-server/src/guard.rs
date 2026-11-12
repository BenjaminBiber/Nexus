//! Schutz des lokalen Servers; portiert aus `packages/server/src/security/RequestGuard.ts`.
//!
//! Die Prüfung läuft als Middleware **vor** dem Routing, also auch vor der statischen
//! Auslieferung: eine abgewiesene Anfrage erreicht keine Route.
//!
//! Es gibt zwei Arten von Aufrufern:
//! * **Browser** (`http://127.0.0.1:<port>`, `http://localhost:<port>`): Host und Herkunft
//!   müssen stimmen, Schreibzugriffe brauchen `X-Harness: 1` und JSON.
//! * **Desktop-App** (`tauri://localhost` und `http(s)://tauri.localhost`): Die Oberfläche
//!   kommt aus den Dateien der App und hat damit eine fremde Herkunft. Sie ist nur erlaubt,
//!   wenn sie das Start-Token des Servers mitschickt; das Token ersetzt dort `X-Harness`.
//!   Einzige Ausnahme: Dateien installierter Themes unter `/themes/` darf sie ohne Token
//!   lesen, weil Schriften und Module keine Köpfe mitschicken können.
//!   Im Entwicklungsmodus der App kommt die Oberfläche von Vite; dessen Herkunft lässt
//!   [`RequestGuard::allow_app_origin`] unter denselben Bedingungen zu.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use axum::extract::{Request, State};
use axum::http::header::{HeaderName, HeaderValue};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use nexus_core::registry::AppToken;
use serde::Serialize;

use crate::state::AppState;

/// Herkünfte, unter denen die Desktop-App ihre Oberfläche lädt: macOS und Linux nutzen
/// `tauri://localhost`, Windows (WebView2) `http://tauri.localhost`
pub const TAURI_ORIGINS: [&str; 3] = [
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
];

/// Kopf, in dem die Oberfläche der App ihr Start-Token mitschickt
pub const TOKEN_HEADER: &str = "x-nexus-token";

/// Abfragewert mit dem Start-Token; nur für `GET`/`HEAD`, weil `EventSource` und der
/// Abruf eines Protokolls keine Köpfe setzen können
pub const TOKEN_QUERY: &str = "token";

/// Pfade, die statt JSON eine Aufnahme als Körper annehmen. Der Schutz hängt an der Herkunft
/// und am Pflichtkopf, nicht am Inhaltstyp: `audio/*` ist kein „einfacher“ Inhaltstyp, eine
/// fremde Seite käme damit nicht an der Vorabfrage und der Herkunftsprüfung vorbei.
pub const AUDIO_BODY_PATHS: [&str; 1] = ["/api/speech/test"];

/// Pfad, unter dem die Dateien installierter Themes liegen (`routes::themes`); lesend braucht
/// die App dafür kein Start-Token
pub const THEME_FILES_PREFIX: &str = "/themes/";

/// Methoden, die eine Vorabfrage der App freigibt
const ALLOW_METHODS: &str = "GET, POST, PUT, PATCH, DELETE";
/// Köpfe, die eine Vorabfrage der App freigibt
const ALLOW_HEADERS: &str = "content-type, x-harness, x-nexus-token";
/// So lange darf der Browser die Vorabfrage merken (Sekunden)
const MAX_AGE: &str = "600";

/// Grund einer Ablehnung, wie der Browser ihn als JSON bekommt
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuardDenial {
    pub code: &'static str,
    pub message: &'static str,
}

/// Schützt den lokalen Server: nur eigener Host, nur eigene Herkunft, Schreibzugriffe nur mit
/// Pflichtkopf. Die Oberfläche der App weist sich stattdessen mit dem Start-Token aus.
#[derive(Debug)]
pub struct RequestGuard {
    allowed: RwLock<Allowed>,
    /// Start-Token dieses Serverlaufs; ohne Token kommt keine App durch
    app_token: RwLock<Option<AppToken>>,
}

/// Erlaubte Hosts und Herkünfte; sie wachsen mit [`RequestGuard::allow_port`] und
/// [`RequestGuard::allow_app_origin`]
#[derive(Debug, Default)]
struct Allowed {
    hosts: HashSet<String>,
    origins: HashSet<String>,
    /// Herkünfte der App, die nur mit Start-Token durchkommen; ab Werk [`TAURI_ORIGINS`]
    app_origins: HashSet<String>,
}

impl RequestGuard {
    /// Schutz für einen Port; bei Port 0 kommt der tatsächliche Port nach dem Binden dazu
    pub fn new(port: u16) -> Self {
        let allowed = Allowed {
            app_origins: TAURI_ORIGINS
                .iter()
                .map(|origin| origin.to_string())
                .collect(),
            ..Allowed::default()
        };
        let guard = Self {
            allowed: RwLock::new(allowed),
            app_token: RwLock::new(None),
        };
        guard.allow_port(port);
        guard
    }

    /// Erlaubt `127.0.0.1` und `localhost` auf dem Port (auch nachträglich, z. B. bei Port 0)
    pub fn allow_port(&self, port: u16) {
        let mut allowed = self.allowed.write().expect("Sperre des Schutzes");
        for name in ["127.0.0.1", "localhost"] {
            allowed.hosts.insert(format!("{name}:{port}"));
            allowed.origins.insert(format!("http://{name}:{port}"));
        }
    }

    /// Behandelt eine weitere Herkunft wie die der App: nur mit Start-Token, dann mit CORS.
    /// Die Desktop-App nennt hier im Entwicklungsmodus die Adresse von Vite.
    pub fn allow_app_origin(&self, origin: &str) {
        let mut allowed = self.allowed.write().expect("Sperre des Schutzes");
        allowed.app_origins.insert(origin.to_lowercase());
    }

    /// Die Herkunft einer Anfrage, falls sie von der App kommt (immer klein geschrieben)
    pub fn app_origin(&self, headers: &HeaderMap) -> Option<String> {
        let origin = single(headers, "origin")?.to_lowercase();
        let allowed = self.allowed.read().expect("Sperre des Schutzes");
        allowed.app_origins.contains(&origin).then_some(origin)
    }

    /// Lässt die Oberfläche der Desktop-App zu, sobald sie dieses Token mitschickt
    pub fn allow_app_token(&self, token: AppToken) {
        *self.app_token.write().expect("Sperre des Schutzes") = Some(token);
    }

    /// Prüft eine Anfrage; `None`, wenn sie zulässig ist, sonst der Ablehnungsgrund.
    /// `path` ist der Pfad der Adresse (nur die Routen aus [`AUDIO_BODY_PATHS`] dürfen eine
    /// Aufnahme statt JSON schicken), `query` ihr Abfrageteil ohne `?`; daraus kommt bei
    /// `GET`/`HEAD` das Token der App.
    pub fn check(
        &self,
        method: &Method,
        path: &str,
        headers: &HeaderMap,
        query: Option<&str>,
    ) -> Option<GuardDenial> {
        let allowed = self.allowed.read().expect("Sperre des Schutzes");
        let host = single(headers, "host");
        if !host.is_some_and(|host| allowed.hosts.contains(&host.to_lowercase())) {
            return Some(GuardDenial {
                code: "host-denied",
                message: "Dieser Server antwortet nur auf 127.0.0.1 und localhost.",
            });
        }
        let mut aus_der_app = false;
        if let Some(origin) = single(headers, "origin") {
            let origin = origin.to_lowercase();
            if !allowed.origins.contains(&origin) {
                if !allowed.app_origins.contains(&origin) {
                    return Some(GuardDenial {
                        code: "origin-denied",
                        message: "Anfragen von fremden Webseiten sind nicht erlaubt.",
                    });
                }
                if !self.token_stimmt(method, headers, query) && !theme_datei_lesend(method, path) {
                    return Some(GuardDenial {
                        code: "origin-denied",
                        message: "Die App hat sich nicht ausgewiesen.",
                    });
                }
                aus_der_app = true;
            }
        }
        if !is_read(method) && !json_content(headers) && !audio_body(path, headers) {
            return Some(match aus_der_app {
                true => GuardDenial {
                    code: "header-missing",
                    message: "Schreibende Anfragen brauchen den Inhaltstyp application/json.",
                },
                false => fehlender_pflichtkopf(),
            });
        }
        // Der Pflichtkopf löst bei fremden Seiten eine Vorabfrage aus; die App weist sich
        // stattdessen mit ihrem Token aus und braucht ihn nicht
        if !is_read(method) && !aus_der_app && single(headers, "x-harness") != Some("1") {
            return Some(fehlender_pflichtkopf());
        }
        None
    }

    /// Hat die Anfrage das Start-Token dabei? Der Kopf gilt immer, die Abfrage nur bei
    /// `GET`/`HEAD` – dort gibt es Aufrufer ohne eigene Köpfe (`EventSource`, Protokolle).
    fn token_stimmt(&self, method: &Method, headers: &HeaderMap, query: Option<&str>) -> bool {
        let Some(token) = self.app_token.read().expect("Sperre des Schutzes").clone() else {
            return false;
        };
        if let Some(value) = single(headers, TOKEN_HEADER) {
            return token.matches(value);
        }
        if !is_read(method) {
            return false;
        }
        query
            .and_then(token_aus_abfrage)
            .is_some_and(|value| token.matches(value))
    }
}

/// Liest die Anfrage eine Datei eines installierten Themes? Das darf die Oberfläche der App
/// auch ohne Start-Token: Schriften aus der `theme.css` und `import()` der `main.js` (samt
/// deren relativer Importe) schickt der Browser mit Herkunft, aber ohne eigene Köpfe und ohne
/// Abfragewert. Unter [`THEME_FILES_PREFIX`] liegen nur Theme-Pakete – dieselben Dateien, die
/// jede Seite ohnehin als Stylesheet, Skript oder Bild einbinden kann, weil solche Anfragen
/// keine Herkunft tragen.
fn theme_datei_lesend(method: &Method, path: &str) -> bool {
    is_read(method) && path.starts_with(THEME_FILES_PREFIX)
}

/// Lädt die Oberfläche der Desktop-App unter dieser Herkunft?
pub fn is_tauri_origin(origin: &str) -> bool {
    TAURI_ORIGINS.contains(&origin)
}

/// Wert von `token` im Abfrageteil. Das Token ist Base64url und braucht deshalb keine
/// Dekodierung; alles andere passt ohnehin nicht zum Token.
fn token_aus_abfrage(query: &str) -> Option<&str> {
    query.split('&').find_map(|paar| {
        paar.strip_prefix(TOKEN_QUERY)
            .and_then(|rest| rest.strip_prefix('='))
    })
}

/// Der immer gleiche Grund, wenn einem Schreibzugriff aus dem Browser etwas fehlt
fn fehlender_pflichtkopf() -> GuardDenial {
    GuardDenial {
        code: "header-missing",
        message: "Schreibende Anfragen brauchen den Kopf X-Harness: 1 und den Inhaltstyp application/json.",
    }
}

/// Lesende Methoden brauchen keinen Pflichtkopf
fn is_read(method: &Method) -> bool {
    method == Method::GET || method == Method::HEAD
}

/// JSON als Inhaltstyp: Pflicht bei jedem Schreibzugriff, auch aus der App
fn json_content(headers: &HeaderMap) -> bool {
    single(headers, "content-type")
        .is_some_and(|value| value.to_lowercase().starts_with("application/json"))
}

/// Eine Aufnahme als Körper – nur auf den Pfaden aus [`AUDIO_BODY_PATHS`] und nur mit einem
/// Inhaltstyp, der Audio meint
fn audio_body(path: &str, headers: &HeaderMap) -> bool {
    if !AUDIO_BODY_PATHS.contains(&path) {
        return false;
    }
    single(headers, "content-type").is_some_and(|value| {
        let value = value.to_lowercase();
        value.starts_with("audio/") || value.starts_with("application/octet-stream")
    })
}

/// Köpfe können mehrfach vorkommen; wie in Node zählt der erste Wert. Köpfe, die sich nicht
/// als Text lesen lassen, gelten als nicht vorhanden.
fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

/// Middleware: prüft jede Anfrage vor dem Routing und antwortet im Zweifel mit 403.
/// Anfragen der App bekommen zusätzlich die CORS-Köpfe, sonst sähe die Oberfläche statt
/// einer Antwort nur einen Netzwerkfehler.
pub async fn guard_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let app_origin = state.guard.app_origin(request.headers());
    // Die Vorabfrage schickt der Browser selbst, ohne Kopf und ohne Token; sie gibt nur
    // bekannt, was danach erlaubt ist. Vorabfragen anderer Herkünfte fallen unten durch.
    if request.method() == Method::OPTIONS {
        if let Some(origin) = &app_origin {
            return vorabfrage(origin);
        }
    }
    let denial = state.guard.check(
        request.method(),
        request.uri().path(),
        request.headers(),
        request.uri().query(),
    );
    let mut response = match denial {
        Some(denial) => (StatusCode::FORBIDDEN, crate::error::json(&denial)).into_response(),
        None => next.run(request).await,
    };
    if let Some(origin) = &app_origin {
        cors_kopfe(response.headers_mut(), origin);
    }
    response
}

/// Antwort auf die Vorabfrage der App: 204 ohne Rumpf, dazu die Freigaben
fn vorabfrage(origin: &str) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    let headers = response.headers_mut();
    cors_kopfe(headers, origin);
    setze(headers, "access-control-allow-methods", ALLOW_METHODS);
    setze(headers, "access-control-allow-headers", ALLOW_HEADERS);
    setze(headers, "access-control-max-age", MAX_AGE);
    response
}

/// Die Köpfe, die jede Antwort an die App trägt – auch der Ereignisstrom
fn cors_kopfe(headers: &mut HeaderMap, origin: &str) {
    setze(headers, "access-control-allow-origin", origin);
    // Angehängt statt gesetzt: die statische Auslieferung bringt schon ein eigenes `Vary` mit
    headers.append(
        HeaderName::from_static("vary"),
        HeaderValue::from_static("Origin"),
    );
}

/// Setzt einen Kopf; unbrauchbare Werte gibt es hier nicht, deshalb still übergehen
fn setze(headers: &mut HeaderMap, name: &'static str, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(HeaderName::from_static(name), value);
    }
}
