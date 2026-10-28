//! Schutz des lokalen Servers; portiert aus
//! `packages/server/tests/security/RequestGuard.test.ts`.

mod common;

use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};
use common::TestServer;
use nexus_server::guard::RequestGuard;

/// Pflichtköpfe eines Schreibzugriffs
const OK: [(&str, &str); 2] = [("content-type", "application/json"), ("x-harness", "1")];

/// Köpfe aus Name/Wert-Paaren
fn headers(entries: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in entries {
        let name = HeaderName::from_bytes(name.as_bytes()).unwrap();
        map.insert(name, HeaderValue::from_str(value).unwrap());
    }
    map
}

/// Der Code der Ablehnung, `None` wenn die Anfrage zulässig ist
fn code(guard: &RequestGuard, method: &str, entries: &[(&str, &str)]) -> Option<&'static str> {
    let method = Method::from_bytes(method.as_bytes()).unwrap();
    guard
        .check(&method, &headers(entries))
        .map(|denial| denial.code)
}

#[test]
fn lasst_lesezugriffe_mit_erlaubtem_host_zu() {
    let guard = RequestGuard::new(4710);
    assert_eq!(code(&guard, "GET", &[("host", "127.0.0.1:4710")]), None);
    assert_eq!(code(&guard, "GET", &[("host", "localhost:4710")]), None);
    assert_eq!(code(&guard, "HEAD", &[("host", "localhost:4710")]), None);
}

#[test]
fn lehnt_fremde_hostnamen_ab_schutz_vor_dns_rebinding() {
    let guard = RequestGuard::new(4710);
    for host in ["evil.example", "evil.example:4710", "127.0.0.1:9999"] {
        assert_eq!(
            code(&guard, "GET", &[("host", host)]),
            Some("host-denied"),
            "{host}"
        );
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
            Some("origin-denied"),
            "{origin}"
        );
    }
}

#[test]
fn akzeptiert_die_eigene_herkunft() {
    let guard = RequestGuard::new(4710);
    let mut post: Vec<(&str, &str)> = vec![("host", "localhost:4710")];
    post.extend(OK);
    let mut mit_localhost = post.clone();
    mit_localhost.push(("origin", "http://localhost:4710"));
    assert_eq!(code(&guard, "POST", &mit_localhost), None);

    let mut mit_ip = post;
    mit_ip.push(("origin", "http://127.0.0.1:4710"));
    assert_eq!(code(&guard, "PATCH", &mit_ip), None);
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
}

#[tokio::test]
async fn weist_abgelehnte_anfragen_am_server_mit_demselben_json_ab() {
    let server = TestServer::start();
    let fremd = server
        .get_with("/api/health", &[("host", Some("evil.example"))])
        .await;
    assert_eq!(fremd.status, 403);
    assert_eq!(
        fremd.json().unwrap(),
        serde_json::json!({
            "code": "host-denied",
            "message": "Dieser Server antwortet nur auf 127.0.0.1 und localhost.",
        })
    );

    let herkunft = server
        .get_with("/api/health", &[("origin", Some("http://evil.example"))])
        .await;
    assert_eq!(herkunft.status, 403);
    assert_eq!(
        herkunft.json().unwrap(),
        serde_json::json!({
            "code": "origin-denied",
            "message": "Anfragen von fremden Webseiten sind nicht erlaubt.",
        })
    );

    let kopf = server
        .request("POST", "/api/projects", Some("{}"), &[("x-harness", None)])
        .await;
    assert_eq!(kopf.status, 403);
    assert_eq!(
        kopf.json().unwrap(),
        serde_json::json!({
            "code": "header-missing",
            "message": "Schreibende Anfragen brauchen den Kopf X-Harness: 1 und den Inhaltstyp application/json.",
        })
    );
}
