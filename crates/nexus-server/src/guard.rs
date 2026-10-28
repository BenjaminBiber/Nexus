//! Schutz des lokalen Servers; portiert aus `packages/server/src/security/RequestGuard.ts`.
//!
//! Die Prüfung läuft als Middleware **vor** dem Routing, also auch vor dem Proxy und der
//! statischen Auslieferung: eine abgewiesene Anfrage erreicht den Node-Server nie.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::state::AppState;

/// Grund einer Ablehnung, wie der Browser ihn als JSON bekommt
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuardDenial {
    pub code: &'static str,
    pub message: &'static str,
}

/// Schützt den lokalen Server: nur eigener Host, nur eigene Herkunft, Schreibzugriffe nur mit
/// Pflichtkopf.
#[derive(Debug)]
pub struct RequestGuard {
    allowed: RwLock<Allowed>,
}

/// Erlaubte Hosts und Herkünfte; sie wachsen mit [`RequestGuard::allow_port`]
#[derive(Debug, Default)]
struct Allowed {
    hosts: HashSet<String>,
    origins: HashSet<String>,
}

impl RequestGuard {
    /// Schutz für einen Port; bei Port 0 kommt der tatsächliche Port nach dem Binden dazu
    pub fn new(port: u16) -> Self {
        let guard = Self {
            allowed: RwLock::new(Allowed::default()),
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

    /// Prüft eine Anfrage; `None`, wenn sie zulässig ist, sonst der Ablehnungsgrund
    pub fn check(&self, method: &Method, headers: &HeaderMap) -> Option<GuardDenial> {
        let allowed = self.allowed.read().expect("Sperre des Schutzes");
        let host = single(headers, "host");
        if !host.is_some_and(|host| allowed.hosts.contains(&host.to_lowercase())) {
            return Some(GuardDenial {
                code: "host-denied",
                message: "Dieser Server antwortet nur auf 127.0.0.1 und localhost.",
            });
        }
        if let Some(origin) = single(headers, "origin") {
            if !allowed.origins.contains(&origin.to_lowercase()) {
                return Some(GuardDenial {
                    code: "origin-denied",
                    message: "Anfragen von fremden Webseiten sind nicht erlaubt.",
                });
            }
        }
        if !is_read(method) && !has_required_headers(headers) {
            return Some(GuardDenial {
                code: "header-missing",
                message: "Schreibende Anfragen brauchen den Kopf X-Harness: 1 und den Inhaltstyp application/json.",
            });
        }
        None
    }
}

/// Lesende Methoden brauchen keinen Pflichtkopf
fn is_read(method: &Method) -> bool {
    method == Method::GET || method == Method::HEAD
}

/// Pflichtkopf und JSON als Inhaltstyp: beides löst bei fremden Seiten eine Vorabfrage aus,
/// die nicht beantwortet wird
fn has_required_headers(headers: &HeaderMap) -> bool {
    let type_ok = single(headers, "content-type")
        .is_some_and(|value| value.to_lowercase().starts_with("application/json"));
    single(headers, "x-harness") == Some("1") && type_ok
}

/// Köpfe können mehrfach vorkommen; wie in Node zählt der erste Wert. Köpfe, die sich nicht
/// als Text lesen lassen, gelten als nicht vorhanden.
fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

/// Middleware: prüft jede Anfrage vor dem Routing und antwortet im Zweifel mit 403
pub async fn guard_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(denial) = state.guard.check(request.method(), request.headers()) {
        return (StatusCode::FORBIDDEN, crate::error::json(&denial)).into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    /// Köpfe aus Name/Wert-Paaren
    fn headers(entries: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in entries {
            let name = axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap();
            map.insert(name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    /// Der Code der Ablehnung, `None` wenn zulässig
    fn code(guard: &RequestGuard, method: &str, entries: &[(&str, &str)]) -> Option<&'static str> {
        let method = Method::from_bytes(method.as_bytes()).unwrap();
        guard
            .check(&method, &headers(entries))
            .map(|denial| denial.code)
    }

    /// Pflichtköpfe eines Schreibzugriffs
    const OK: [(&str, &str); 2] = [("content-type", "application/json"), ("x-harness", "1")];

    #[test]
    fn lasst_lesezugriffe_mit_erlaubtem_host_zu() {
        let guard = RequestGuard::new(4710);
        assert_eq!(code(&guard, "GET", &[("host", "127.0.0.1:4710")]), None);
        assert_eq!(code(&guard, "GET", &[("host", "localhost:4710")]), None);
        assert_eq!(code(&guard, "HEAD", &[("host", "localhost:4710")]), None);
    }

    #[test]
    fn lehnt_fremde_hostnamen_ab() {
        let guard = RequestGuard::new(4710);
        for host in ["evil.example", "evil.example:4710", "127.0.0.1:9999"] {
            assert_eq!(code(&guard, "GET", &[("host", host)]), Some("host-denied"));
        }
        assert_eq!(code(&guard, "GET", &[]), Some("host-denied"));
    }

    #[test]
    fn lehnt_eine_fremde_herkunft_ab_auch_bei_lesezugriffen() {
        let guard = RequestGuard::new(4710);
        for origin in ["http://evil.example", "null", "http://127.0.0.1:4711"] {
            assert_eq!(
                code(
                    &guard,
                    "GET",
                    &[("host", "127.0.0.1:4710"), ("origin", origin)]
                ),
                Some("origin-denied")
            );
        }
    }

    #[test]
    fn akzeptiert_die_eigene_herkunft() {
        let guard = RequestGuard::new(4710);
        let mut post: Vec<(&str, &str)> = vec![("host", "localhost:4710")];
        post.extend(OK);
        let mut mit_origin = post.clone();
        mit_origin.push(("origin", "http://localhost:4710"));
        assert_eq!(code(&guard, "POST", &mit_origin), None);

        let mut patch = post;
        patch.push(("origin", "http://127.0.0.1:4710"));
        assert_eq!(code(&guard, "PATCH", &patch), None);
    }

    #[test]
    fn verlangt_bei_schreibzugriffen_den_pflichtkopf_und_json() {
        let guard = RequestGuard::new(4710);
        let host = ("host", "127.0.0.1:4710");
        assert_eq!(code(&guard, "POST", &[host]), Some("header-missing"));
        assert_eq!(
            code(
                &guard,
                "POST",
                &[host, ("content-type", "application/json")]
            ),
            Some("header-missing")
        );
        assert_eq!(
            code(&guard, "POST", &[host, ("x-harness", "1")]),
            Some("header-missing")
        );
        assert_eq!(
            code(
                &guard,
                "DELETE",
                &[host, ("x-harness", "1"), ("content-type", "text/plain")]
            ),
            Some("header-missing")
        );
        assert_eq!(code(&guard, "POST", &[host, OK[0], OK[1]]), None);
        assert_eq!(
            code(
                &guard,
                "POST",
                &[
                    host,
                    ("x-harness", "1"),
                    ("content-type", "application/json; charset=utf-8")
                ]
            ),
            None
        );
    }

    #[test]
    fn ubernimmt_einen_spater_bekannten_port() {
        let guard = RequestGuard::new(0);
        guard.allow_port(5555);
        assert_eq!(code(&guard, "GET", &[("host", "127.0.0.1:5555")]), None);
        // Der alte Eintrag bleibt gültig (Port 0 kommt nie als Host vor)
        assert_eq!(code(&guard, "GET", &[("host", "127.0.0.1:0")]), None);
    }

    #[test]
    fn die_ablehnung_wird_als_json_wie_heute_ausgegeben() {
        let guard = RequestGuard::new(4710);
        let denial = guard.check(&Method::GET, &headers(&[])).unwrap();
        assert_eq!(
            serde_json::to_value(&denial).unwrap(),
            serde_json::json!({
                "code": "host-denied",
                "message": "Dieser Server antwortet nur auf 127.0.0.1 und localhost.",
            })
        );
    }
}
