//! Routengruppe `store_install`: `POST /api/themes/store/install` lädt ein Paket aus dem
//! Store, prüft sha256 und optionale Signatur und installiert es. Repo und Download-Server
//! sind eine Attrappe aus axum im eigenen Prozess – nichts verlässt den Rechner.

mod common;

use std::io::{Cursor, Write};
use std::net::SocketAddr;
use std::time::Duration;

use axum::routing::get;
use axum::Router;
use common::{TestServer, TestServerOptions};
use nexus_core::theme::store_install::sha256_hex;
use nexus_core::theme::store_install::test_signing::TestSigner;
use serde_json::{json, Value};
use zip::write::SimpleFileOptions;

const INSTALL: &str = "/api/themes/store/install";

fn paket(id: &str, version: &str, css: &str) -> Vec<u8> {
    let manifest = format!(
        r#"{{"id":"{id}","name":"Ozean","version":"{version}","author":"Ada","sdkVersion":"^1.0","colorScheme":"dark"}}"#
    );
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, inhalt) in [
        ("manifest.json", manifest.as_bytes()),
        ("theme.css", css.as_bytes()),
    ] {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(inhalt).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// Index-Eintrag für ein Paket unter `/<datei>` der Attrappe
fn eintrag(addr: SocketAddr, id: &str, datei: &str, bytes: &[u8]) -> Value {
    json!({
        "id": id,
        "name": "Ozean",
        "version": "1.0.0",
        "sdkVersion": "^1.0",
        "downloadUrl": format!("http://127.0.0.1:{}/{datei}", addr.port()),
        "sha256": sha256_hex(bytes),
    })
}

/// Startet die Attrappe; `dateien` bekommt die Adresse und liefert Pfad → Inhalt.
/// Dazu immer `/slow.nexus` (5 s) und `/huge.nexus` (21 MiB)
async fn stub(dateien: impl FnOnce(SocketAddr) -> Vec<(String, Vec<u8>)>) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("Port der Attrappe");
    let addr = listener.local_addr().expect("Adresse der Attrappe");
    let mut app = Router::new()
        .route(
            "/slow.nexus",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "zu spät"
            }),
        )
        .route("/huge.nexus", get(|| async { vec![0u8; 21 * 1024 * 1024] }));
    for (path, inhalt) in dateien(addr) {
        app = app.route(&path, get(move || async move { inhalt.clone() }));
    }
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn server() -> TestServer {
    TestServer::with(TestServerOptions {
        store_index_timeout: Some(Duration::from_secs(5)),
        store_download_timeout: Some(Duration::from_millis(1000)),
        ..Default::default()
    })
}

/// Trägt das Repo der Attrappe ein und liefert seine ID
async fn add_repo(server: &TestServer, addr: SocketAddr, public_key: Option<&str>) -> String {
    let body = json!({
        "url": format!("http://127.0.0.1:{}/index.json", addr.port()),
        "publicKey": public_key,
    });
    let reply = server
        .post("/api/themes/store/repos", &body.to_string())
        .await;
    assert_eq!(reply.status, 201, "{}", reply.text);
    reply.body()["id"].as_str().unwrap().to_string()
}

async fn install(server: &TestServer, repo: &str, id: &str) -> common::Reply {
    server
        .post(INSTALL, &json!({ "repoId": repo, "id": id }).to_string())
        .await
}

/// Unter `themes/` liegt nur, was erwartet ist – keine `.tmp-*`/`.old-*`-Reste
fn themes_inhalt(server: &TestServer) -> Vec<String> {
    let mut namen: Vec<String> = std::fs::read_dir(server.home.themes_dir())
        .map(|dir| {
            dir.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    namen.sort();
    namen
}

/// Index mit einem Eintrag `ozean` für `/<datei>`
async fn repo_mit(
    datei: &'static str,
    bytes: Vec<u8>,
    anpassen: impl FnOnce(&mut Value) + Send + 'static,
) -> SocketAddr {
    stub(move |addr| {
        let mut entry = eintrag(addr, "ozean", datei, &bytes);
        anpassen(&mut entry);
        vec![
            (
                "/index.json".into(),
                json!([entry]).to_string().into_bytes(),
            ),
            ("/pkg.nexus".into(), bytes),
        ]
    })
    .await
}

#[tokio::test]
async fn richtige_pruefsumme_installiert_und_listet() {
    let addr = repo_mit("pkg.nexus", paket("ozean", "1.0.0", ":root{}"), |_| {}).await;
    let server = server();
    let repo = add_repo(&server, addr, None).await;
    let reply = install(&server, &repo, "ozean").await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    let body = reply.body();
    assert_eq!(body["verified"], json!(false));
    assert_eq!(body["manifest"]["id"], json!("ozean"));
    assert_eq!(body["repoId"], json!(repo));
    assert_eq!(body["sha256"].as_str().unwrap().len(), 64);
    assert!(server.home.themes_dir().join("ozean/theme.css").is_file());
    let liste = server.get("/api/themes").await.body();
    assert_eq!(liste[0]["id"], json!("ozean"));
    assert_eq!(themes_inhalt(&server), vec!["ozean"]);
}

#[tokio::test]
async fn falsche_pruefsumme_laesst_alte_installation_unveraendert() {
    let addr = repo_mit(
        "pkg.nexus",
        paket("ozean", "1.0.0", ":root{--neu:1}"),
        |entry| {
            entry["sha256"] = json!("0".repeat(64));
        },
    )
    .await;
    let server = server();
    let alt = paket("ozean", "0.9.0", ":root{--alt:1}");
    nexus_core::themes::install_theme(&server.home, &alt).unwrap();
    let repo = add_repo(&server, addr, None).await;
    let reply = install(&server, &repo, "ozean").await;
    assert_eq!(reply.status, 422, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-checksum-mismatch"));
    assert_eq!(themes_inhalt(&server), vec!["ozean"]);
    let css = std::fs::read_to_string(server.home.themes_dir().join("ozean/theme.css")).unwrap();
    assert_eq!(css, ":root{--alt:1}");
}

#[tokio::test]
async fn downloadfehler_mit_code_und_ohne_reste() {
    let geschlossen = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let addr = stub(move |addr| {
        let bytes = b"egal".to_vec();
        let mut weg = eintrag(addr, "weg", "fehlt.nexus", &bytes);
        weg["downloadUrl"] = json!(format!("http://127.0.0.1:{geschlossen}/x.nexus"));
        let index = json!([
            eintrag(addr, "nicht-da", "fehlt.nexus", &bytes),
            weg,
            eintrag(addr, "langsam", "slow.nexus", &bytes),
            eintrag(addr, "riesig", "huge.nexus", &bytes),
        ]);
        vec![("/index.json".into(), index.to_string().into_bytes())]
    })
    .await;
    let server = server();
    let repo = add_repo(&server, addr, None).await;
    for (id, status, code) in [
        ("nicht-da", 502, "store-download-http"),
        ("langsam", 504, "store-download-timeout"),
        ("riesig", 413, "store-download-too-large"),
    ] {
        let reply = install(&server, &repo, id).await;
        assert_eq!(reply.status, status, "{id}: {}", reply.text);
        assert_eq!(reply.code().as_deref(), Some(code), "{id}");
        assert!(themes_inhalt(&server).is_empty(), "{id}");
    }
    let reply = install(&server, &repo, "nicht-da").await;
    assert!(reply.body()["message"].as_str().unwrap().contains("404"));

    // Unter Windows dauert die Absage eines geschlossenen Ports rund 2 s
    let geduldig = TestServer::with(TestServerOptions {
        store_download_timeout: Some(Duration::from_secs(8)),
        ..Default::default()
    });
    let repo = add_repo(&geduldig, addr, None).await;
    let reply = install(&geduldig, &repo, "weg").await;
    assert_eq!(reply.status, 502, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-download-unreachable"));
    assert!(themes_inhalt(&geduldig).is_empty());
}

#[tokio::test]
async fn gueltige_signatur_ist_verifiziert() {
    let signer = TestSigner::new(3);
    let bytes = paket("ozean", "1.0.0", ":root{}");
    let signatur = signer.sign(&bytes);
    let addr = repo_mit("pkg.nexus", bytes, move |entry| {
        entry["signature"] = json!(signatur);
    })
    .await;
    let server = server();
    let repo = add_repo(&server, addr, Some(&signer.public_key())).await;
    let reply = install(&server, &repo, "ozean").await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    assert_eq!(reply.body()["verified"], json!(true));
}

#[tokio::test]
async fn ungueltige_signatur_installiert_nichts() {
    let signer = TestSigner::new(3);
    let signatur = signer.sign(b"anderes paket");
    let addr = repo_mit(
        "pkg.nexus",
        paket("ozean", "1.0.0", ":root{}"),
        move |entry| {
            entry["signature"] = json!(signatur);
        },
    )
    .await;
    let server = server();
    let repo = add_repo(&server, addr, Some(&signer.public_key())).await;
    let reply = install(&server, &repo, "ozean").await;
    assert_eq!(reply.status, 422, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-signature-invalid"));
    assert!(themes_inhalt(&server).is_empty());
}

#[tokio::test]
async fn ungueltiger_repo_schluessel_ist_400() {
    let server = server();
    let reply = server
        .post(
            "/api/themes/store/repos",
            &json!({ "url": "https://x.de/i.json", "publicKey": "RWkaputt" }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 400, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-repo-key-invalid"));
}

#[tokio::test]
async fn unbekannt_inkompatibel_und_abweichendes_manifest() {
    let addr = stub(|addr| {
        let gut = paket("ozean", "1.0.0", ":root{}");
        let fremd = paket("wald", "1.0.0", ":root{}");
        let mut zukunft = eintrag(addr, "zukunft", "pkg.nexus", &gut);
        zukunft["sdkVersion"] = json!("^2.0");
        let index = json!([zukunft, eintrag(addr, "ozean", "fremd.nexus", &fremd)]);
        vec![
            ("/index.json".into(), index.to_string().into_bytes()),
            ("/pkg.nexus".into(), gut),
            ("/fremd.nexus".into(), fremd),
        ]
    })
    .await;
    let server = server();
    let repo = add_repo(&server, addr, None).await;

    let reply = install(&server, "unbekannt", "ozean").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.code().as_deref(), Some("store-repo-not-found"));
    let reply = install(&server, &repo, "gibt-es-nicht").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.code().as_deref(), Some("store-entry-not-found"));
    let reply = install(&server, &repo, "zukunft").await;
    assert_eq!(reply.status, 409, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-entry-incompatible"));
    let reply = install(&server, &repo, "ozean").await;
    assert_eq!(reply.status, 422, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-manifest-mismatch"));
    assert!(themes_inhalt(&server).is_empty());
}

#[tokio::test]
async fn fehlende_felder_und_ohne_kennzeichnung() {
    let server = server();
    let reply = server.post(INSTALL, r#"{"repoId":"x"}"#).await;
    assert_eq!(reply.status, 400);
    let reply = server
        .request(
            "POST",
            INSTALL,
            Some(r#"{"repoId":"x","id":"y"}"#),
            &[("x-harness", None)],
        )
        .await;
    assert_eq!(reply.status, 403);
}
