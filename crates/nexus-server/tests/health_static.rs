//! Health, statische Auslieferung und Seitenrouting; portiert aus
//! `packages/server/tests/unit/ServerApp.test.ts`.

mod common;

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

use common::{TestServer, TestServerOptions};

/// Gebaute Oberfläche mit `index.html`, einer Datei unter `assets/` und einer vorkomprimierten
fn web_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix("nexus-web-")
        .tempdir()
        .unwrap();
    let web = dir.path().to_path_buf();
    fs::create_dir_all(web.join("assets")).unwrap();
    let index = "<html><head><script>window.x=1</script></head><body>Oberfläche</body></html>";
    fs::write(web.join("index.html"), index).unwrap();
    fs::write(web.join("index.html.gz"), gzip(index.as_bytes())).unwrap();
    fs::write(web.join("assets").join("app.js"), "console.log(1)").unwrap();
    fs::write(web.join("assets").join("big.js"), "console.log(2)").unwrap();
    fs::write(
        web.join("assets").join("big.js.gz"),
        gzip(b"console.log(2)"),
    )
    .unwrap();
    (dir, web)
}

/// Inhalt als gzip, wie der Build ihn neben die Datei legt
fn gzip(bytes: &[u8]) -> Vec<u8> {
    // Kein Kompressor im Baum: ein gzip-Rahmen mit einem „stored“-Deflate-Block
    let mut out = vec![0x1f, 0x8b, 0x08, 0, 0, 0, 0, 0, 0, 0xff];
    let len = bytes.len() as u16;
    out.push(0x01);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(bytes);
    out.write_all(&crc32(bytes).to_le_bytes()).unwrap();
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out
}

/// CRC-32 (IEEE), wie gzip ihn am Ende erwartet
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let carry = crc & 1 == 1;
            crc >>= 1;
            if carry {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    !crc
}

/// Server mit gebauter Oberfläche
fn server_with_web() -> (TestServer, tempfile::TempDir) {
    let (dir, web) = web_dir();
    let server = TestServer::with(TestServerOptions {
        web_dir: Some(web),
        ..TestServerOptions::default()
    });
    (server, dir)
}

#[tokio::test]
async fn beantwortet_api_health_mit_namen_version_und_prozess() {
    let (server, _web) = server_with_web();
    let reply = server.get("/api/health").await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.json().unwrap(),
        serde_json::json!({
            "name": "ai-harness",
            "version": "test",
            "pid": std::process::id(),
        })
    );
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8")
    );
    // Eigene Antworten sind als solche gekennzeichnet
    assert_eq!(reply.header("x-nexus-backend"), Some("rust"));
}

#[tokio::test]
async fn liefert_statische_dateien_aus() {
    let (server, _web) = server_with_web();
    let reply = server.get("/assets/app.js").await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.text, "console.log(1)");
}

#[tokio::test]
async fn lasst_dateien_unter_assets_dauerhaft_zwischenspeichern_index_nie_ungeprueft() {
    let (server, _web) = server_with_web();
    assert_eq!(
        server.get("/assets/app.js").await.header("cache-control"),
        Some("public, max-age=31536000, immutable")
    );
    assert_eq!(
        server.get("/").await.header("cache-control"),
        Some("no-cache")
    );
    assert_eq!(
        server.get("/p/abc/board").await.header("cache-control"),
        Some("no-cache")
    );
}

/// CSP der Tauri-App (`apps/desktop/src-tauri/tauri.conf.json`)
const CSP: &str = "default-src 'self'; connect-src 'self' http://127.0.0.1:* ipc: http://ipc.localhost; img-src 'self' data: blob: http://127.0.0.1:*; style-src 'self' 'unsafe-inline'; font-src 'self' data:; script-src 'self' http://127.0.0.1:*/themes/";

#[tokio::test]
async fn liefert_die_oberflache_mit_der_csp_der_app_aus() {
    let (server, _web) = server_with_web();
    // sha256 von `window.x=1`, dem Inline-Skript der index.html
    let page =
        CSP.replace(
            "script-src 'self' http://127.0.0.1:*/themes/",
            "script-src 'self' http://127.0.0.1:*/themes/ 'sha256-hUmDl3yY/7GaaX+Zuy8aEy8IzxgpBs7xccqYHs2/7kQ='",
        );
    for url in ["/", "/index.html", "/p/abc/board"] {
        let reply = server.get(url).await;
        assert_eq!(
            reply.header("content-security-policy"),
            Some(page.as_str()),
            "{url}"
        );
    }
    // Auch vorkomprimiert zählt das Skript der index.html, nicht der Körper der Antwort
    let zipped = server
        .get_with("/", &[("accept-encoding", Some("gzip, br"))])
        .await;
    assert_eq!(
        zipped.header("content-security-policy"),
        Some(page.as_str())
    );
    assert_eq!(
        server
            .get("/assets/app.js")
            .await
            .header("content-security-policy"),
        Some(CSP)
    );
}

#[tokio::test]
async fn api_antworten_tragen_keine_csp() {
    let (server, _web) = server_with_web();
    for url in ["/api/health", "/api/gibtesnicht"] {
        let reply = server.get(url).await;
        assert_eq!(reply.header("content-security-policy"), None, "{url}");
    }
}

#[tokio::test]
async fn liefert_vorkomprimierte_dateien_wenn_der_browser_sie_annimmt() {
    let (server, _web) = server_with_web();
    let zipped = server
        .get_with("/assets/big.js", &[("accept-encoding", Some("gzip"))])
        .await;
    assert_eq!(zipped.header("content-encoding"), Some("gzip"));
    let plain = server.get("/assets/big.js").await;
    assert_eq!(plain.header("content-encoding"), None);
    assert_eq!(plain.text, "console.log(2)");
}

#[tokio::test]
async fn liefert_index_html_fur_seitenadressen() {
    let (server, _web) = server_with_web();
    for url in [
        "/",
        "/p/abc/board",
        "/p/abc/tickets/T-0001",
        "/docs",
        "/p/abc/docs/docs/specs/plan.md?board=15",
    ] {
        let reply = server.get(url).await;
        assert_eq!(reply.status, 200, "{url}");
        assert!(reply.text.contains("Oberfläche"), "{url}: {}", reply.text);
    }
}

#[tokio::test]
async fn antwortet_auf_unbekannte_api_pfade_und_fehlende_dateien_mit_404_als_json() {
    let (server, _web) = server_with_web();
    let api = server.get("/api/gibtesnicht").await;
    assert_eq!(api.status, 404);
    assert_eq!(api.code().as_deref(), Some("not-found"));
    assert_eq!(
        api.json().unwrap()["message"],
        serde_json::json!("Diese Adresse gibt es nicht.")
    );
    assert_eq!(server.get("/fehlt.js").await.status, 404);
}

#[tokio::test]
async fn lehnt_fremde_anfragen_ab_bevor_eine_route_sie_sieht() {
    let (server, _web) = server_with_web();
    let evil = server
        .get_with("/api/health", &[("host", Some("evil.example"))])
        .await;
    assert_eq!(evil.status, 403);
    assert_eq!(evil.code().as_deref(), Some("host-denied"));

    let no_header = server
        .request(
            "POST",
            "/api/projects",
            Some("{}"),
            &[("x-harness", None), ("content-type", None)],
        )
        .await;
    assert_eq!(no_header.status, 403);
    assert_eq!(no_header.code().as_deref(), Some("header-missing"));
}

#[tokio::test]
async fn weist_ohne_gebaute_oberflache_freundlich_darauf_hin() {
    let bare = TestServer::start();
    let reply = bare.get("/").await;
    assert_eq!(reply.status, 200);
    assert!(reply.text.contains("npm run build"), "{}", reply.text);
    assert_eq!(
        reply.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    // Ohne Oberfläche gibt es auch kein Seitenrouting
    assert_eq!(bare.get("/p/abc/board").await.status, 404);
}

#[tokio::test]
async fn beantwortet_head_wie_get_und_liefert_fur_andere_methoden_kein_405() {
    let (server, _web) = server_with_web();
    let head = server.request("HEAD", "/api/health", None, &[]).await;
    assert_eq!(head.status, 200);
    assert!(head.text.is_empty());

    // Ohne Upstream ist eine falsche Methode auf einem bekannten Pfad 404, nicht 405
    let post = server.post("/api/health", "{}").await;
    assert_eq!(post.status, 404);
    assert_eq!(post.code().as_deref(), Some("not-found"));
}
