//! Schutz des lokalen Servers; portiert aus
//! `packages/server/tests/security/RequestGuard.test.ts`.

mod common;

use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};
use common::TestServer;
use nexus_core::registry::AppToken;
use nexus_server::guard::{RequestGuard, TAURI_ORIGINS};

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

/// Pfad, der wie jede gewöhnliche Route nur JSON annimmt
const PFAD: &str = "/api/db/boards";

/// Der Code der Ablehnung, `None` wenn die Anfrage zulässig ist
fn code(guard: &RequestGuard, method: &str, entries: &[(&str, &str)]) -> Option<&'static str> {
    code_mit_abfrage(guard, method, entries, None)
}

/// Wie [`code`], aber auf einem bestimmten Pfad (nur dort zählt eine Aufnahme als Körper)
fn code_auf_pfad(
    guard: &RequestGuard,
    method: &str,
    path: &str,
    entries: &[(&str, &str)],
) -> Option<&'static str> {
    let method = Method::from_bytes(method.as_bytes()).unwrap();
    guard
        .check(&method, path, &headers(entries), None)
        .map(|denial| denial.code)
}

/// Wie [`code`], aber mit einem Abfrageteil (dort steht das Token von `EventSource`)
fn code_mit_abfrage(
    guard: &RequestGuard,
    method: &str,
    entries: &[(&str, &str)],
    query: Option<&str>,
) -> Option<&'static str> {
    let method = Method::from_bytes(method.as_bytes()).unwrap();
    guard
        .check(&method, PFAD, &headers(entries), query)
        .map(|denial| denial.code)
}

/// Die Meldung der Ablehnung, `None` wenn die Anfrage zulässig ist
fn meldung(guard: &RequestGuard, method: &str, entries: &[(&str, &str)]) -> Option<&'static str> {
    let method = Method::from_bytes(method.as_bytes()).unwrap();
    guard
        .check(&method, PFAD, &headers(entries), None)
        .map(|denial| denial.message)
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
fn nimmt_eine_aufnahme_nur_auf_der_testroute_der_spracherkennung() {
    let guard = RequestGuard::new(4710);
    let host = ("host", "127.0.0.1:4710");
    let audio = [host, ("x-harness", "1"), ("content-type", "audio/ogg")];

    // Auf der Testroute ist eine Aufnahme statt JSON erlaubt
    assert_eq!(
        code_auf_pfad(&guard, "POST", "/api/speech/test", &audio),
        None
    );
    assert_eq!(
        code_auf_pfad(
            &guard,
            "POST",
            "/api/speech/test",
            &[
                host,
                ("x-harness", "1"),
                ("content-type", "application/octet-stream")
            ]
        ),
        None
    );
    // Auf jedem anderen Pfad bleibt JSON Pflicht
    assert_eq!(
        code_auf_pfad(&guard, "POST", "/api/db/boards", &audio),
        Some("header-missing")
    );
    // Der Pflichtkopf gilt auch mit Aufnahme
    assert_eq!(
        code_auf_pfad(
            &guard,
            "POST",
            "/api/speech/test",
            &[host, ("content-type", "audio/ogg")]
        ),
        Some("header-missing")
    );
    // Ein anderer Inhaltstyp bleibt auch auf der Testroute abgelehnt
    assert_eq!(
        code_auf_pfad(
            &guard,
            "POST",
            "/api/speech/test",
            &[host, ("x-harness", "1"), ("content-type", "text/plain")]
        ),
        Some("header-missing")
    );
}

#[test]
fn ubernimmt_einen_spater_bekannten_port() {
    let guard = RequestGuard::new(0);
    guard.allow_port(5555);
    assert_eq!(code(&guard, "GET", &[("host", "127.0.0.1:5555")]), None);
}

/// Ein Schutz mit Start-Token, wie ihn der echte Server beim Start bekommt
fn guard_mit_token(token: &str) -> RequestGuard {
    let guard = RequestGuard::new(4710);
    guard.allow_app_token(AppToken::from_text(token));
    guard
}

/// Die Köpfe einer Anfrage aus der App: Host, Herkunft und das Token
fn aus_der_app<'a>(origin: &'a str, token: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("host", "127.0.0.1:4710"),
        ("origin", origin),
        ("x-nexus-token", token),
    ]
}

#[test]
fn laesst_die_app_mit_gueltigem_token_lesen_und_schreiben() {
    let guard = guard_mit_token("geheim");
    for origin in TAURI_ORIGINS {
        assert_eq!(
            code(&guard, "GET", &aus_der_app(origin, "geheim")),
            None,
            "{origin}"
        );

        // Schreibend: das Token ersetzt X-Harness, JSON bleibt Pflicht
        let mut schreibend = aus_der_app(origin, "geheim");
        schreibend.push(("content-type", "application/json"));
        assert_eq!(code(&guard, "POST", &schreibend), None, "{origin}");
        assert_eq!(code(&guard, "DELETE", &schreibend), None, "{origin}");
    }
}

#[test]
fn behandelt_die_herkunft_von_vite_erst_nach_freigabe_wie_die_app() {
    let guard = guard_mit_token("geheim");
    let vite = "http://localhost:5173";
    assert_eq!(
        code(&guard, "GET", &aus_der_app(vite, "geheim")),
        Some("origin-denied")
    );

    guard.allow_app_origin(vite);
    assert_eq!(code(&guard, "GET", &aus_der_app(vite, "geheim")), None);
    assert_eq!(
        code(&guard, "GET", &aus_der_app(vite, "falsch")),
        Some("origin-denied")
    );
    assert_eq!(
        guard.app_origin(&headers(&aus_der_app(vite, "geheim"))),
        Some(vite.to_string())
    );
}

#[test]
fn weist_die_app_ohne_oder_mit_falschem_token_ab() {
    let guard = guard_mit_token("geheim");
    let host = ("host", "127.0.0.1:4710");
    for origin in TAURI_ORIGINS {
        assert_eq!(
            code(&guard, "GET", &[host, ("origin", origin)]),
            Some("origin-denied"),
            "{origin}"
        );
        assert_eq!(
            code(&guard, "GET", &aus_der_app(origin, "falsch")),
            Some("origin-denied"),
            "{origin}"
        );
        assert_eq!(
            code(&guard, "POST", &aus_der_app(origin, "falsch")),
            Some("origin-denied"),
            "{origin}"
        );
    }
    assert_eq!(
        meldung(&guard, "GET", &[host, ("origin", TAURI_ORIGINS[0])]),
        Some("Die App hat sich nicht ausgewiesen.")
    );
}

#[test]
fn laesst_die_app_ohne_bekanntes_token_gar_nicht_durch() {
    // Kein `allow_app_token`: dann gibt es keine Oberfläche der App, die sich ausweisen könnte
    let guard = RequestGuard::new(4710);
    assert_eq!(
        code(&guard, "GET", &aus_der_app(TAURI_ORIGINS[0], "geheim")),
        Some("origin-denied")
    );
}

#[test]
fn nimmt_das_token_bei_lesezugriffen_auch_aus_der_abfrage() {
    // EventSource und der Abruf eines Protokolls können keine Köpfe setzen
    let guard = guard_mit_token("geheim");
    let app = [("host", "127.0.0.1:4710"), ("origin", TAURI_ORIGINS[0])];
    assert_eq!(
        code_mit_abfrage(&guard, "GET", &app, Some("notify=1&token=geheim")),
        None
    );
    assert_eq!(
        code_mit_abfrage(&guard, "HEAD", &app, Some("token=geheim")),
        None
    );
    assert_eq!(
        code_mit_abfrage(&guard, "GET", &app, Some("token=falsch")),
        Some("origin-denied")
    );
    assert_eq!(
        code_mit_abfrage(&guard, "GET", &app, Some("dbBoard=3")),
        Some("origin-denied")
    );
    // Schreibend zählt nur der Kopf; sonst käme ein Link mit Token einem CSRF gleich
    let mut schreibend = app.to_vec();
    schreibend.push(("content-type", "application/json"));
    assert_eq!(
        code_mit_abfrage(&guard, "POST", &schreibend, Some("token=geheim")),
        Some("origin-denied")
    );
}

#[test]
fn verlangt_auch_von_der_app_json_bei_schreibzugriffen() {
    let guard = guard_mit_token("geheim");
    let mut text = aus_der_app(TAURI_ORIGINS[0], "geheim");
    text.push(("content-type", "text/plain"));
    assert_eq!(code(&guard, "POST", &text), Some("header-missing"));
    assert_eq!(
        meldung(&guard, "POST", &text),
        Some("Schreibende Anfragen brauchen den Inhaltstyp application/json.")
    );
}

#[test]
fn bleibt_bei_fremden_herkuenften_hart_auch_mit_token() {
    let guard = guard_mit_token("geheim");
    for origin in ["http://evil.example", "tauri://evil", "null"] {
        assert_eq!(
            code(
                &guard,
                "GET",
                &[
                    ("host", "127.0.0.1:4710"),
                    ("origin", origin),
                    ("x-nexus-token", "geheim")
                ]
            ),
            Some("origin-denied"),
            "{origin}"
        );
    }
    assert_eq!(
        meldung(
            &guard,
            "GET",
            &[
                ("host", "127.0.0.1:4710"),
                ("origin", "http://evil.example")
            ]
        ),
        Some("Anfragen von fremden Webseiten sind nicht erlaubt.")
    );
}

#[tokio::test]
async fn beantwortet_die_vorabfrage_der_app_mit_204_und_cors() {
    let server = TestServer::start();
    for origin in TAURI_ORIGINS {
        let antwort = server
            .request("OPTIONS", "/api/health", None, &[("origin", Some(origin))])
            .await;
        assert_eq!(antwort.status, 204, "{origin}");
        assert_eq!(antwort.header("access-control-allow-origin"), Some(origin));
        assert_eq!(
            antwort.header("access-control-allow-methods"),
            Some("GET, POST, PUT, PATCH, DELETE")
        );
        assert_eq!(
            antwort.header("access-control-allow-headers"),
            Some("content-type, x-harness, x-nexus-token")
        );
        assert_eq!(antwort.header("access-control-max-age"), Some("600"));
        assert_eq!(antwort.header("vary"), Some("Origin"));
        assert!(antwort.text.is_empty(), "{}", antwort.text);
    }
}

#[tokio::test]
async fn weist_die_vorabfrage_einer_fremden_herkunft_weiter_ab() {
    let server = TestServer::start();
    let antwort = server
        .request(
            "OPTIONS",
            "/api/health",
            None,
            &[("origin", Some("http://evil.example"))],
        )
        .await;
    assert_eq!(antwort.status, 403);
    assert_eq!(antwort.code().as_deref(), Some("origin-denied"));
    assert_eq!(antwort.header("access-control-allow-origin"), None);
}

#[tokio::test]
async fn antwortet_der_app_mit_cors_koepfen_auch_bei_einer_ablehnung() {
    let server = TestServer::start();
    let origin = TAURI_ORIGINS[0];

    let gut = server
        .get_with(
            "/api/health",
            &[
                ("origin", Some(origin)),
                ("x-nexus-token", Some(&server.app_token())),
            ],
        )
        .await;
    assert_eq!(gut.status, 200);
    assert_eq!(gut.header("access-control-allow-origin"), Some(origin));

    let schlecht = server
        .get_with("/api/health", &[("origin", Some(origin))])
        .await;
    assert_eq!(schlecht.status, 403);
    assert_eq!(
        schlecht.body(),
        serde_json::json!({
            "code": "origin-denied",
            "message": "Die App hat sich nicht ausgewiesen.",
        })
    );
    assert_eq!(
        schlecht.header("access-control-allow-origin"),
        Some(origin),
        "sonst sieht die Oberfläche nur einen Netzwerkfehler"
    );
}

#[tokio::test]
async fn laesst_den_ereignisstrom_der_app_mit_token_in_der_abfrage_zu() {
    let server = TestServer::start();
    let origin = TAURI_ORIGINS[0];
    let url = format!(
        "/api/projects/unbekannt/events?token={}",
        server.app_token()
    );

    let antwort = server.get_with(&url, &[("origin", Some(origin))]).await;

    // Das Projekt gibt es nicht; entscheidend ist, dass der Schutz die Anfrage durchlässt
    assert_ne!(antwort.code().as_deref(), Some("origin-denied"));
    assert_eq!(antwort.header("access-control-allow-origin"), Some(origin));
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
