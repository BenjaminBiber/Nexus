//! Routengruppe `store_updates`: `GET /api/themes/store/updates` meldet neuere Versionen aus
//! dem Store, `POST /api/themes/store/update` installiert sie – mit Bestätigung, wenn das
//! Update Code oder neue Domains mitbringt. Repo und Download-Server sind eine Attrappe aus
//! axum im eigenen Prozess – nichts verlässt den Rechner.

mod common;

use std::io::{Cursor, Write};
use std::net::SocketAddr;
use std::time::Duration;

use axum::routing::get;
use axum::Router;
use common::{TestServer, TestServerOptions};
use nexus_core::theme::store_install::sha256_hex;
use serde_json::{json, Value};
use zip::write::SimpleFileOptions;

const UPDATES: &str = "/api/themes/store/updates";
const UPDATE: &str = "/api/themes/store/update";

/// Paket `ozean` in `version`; optional mit `main.js` und Domains
fn paket(version: &str, css: &str, network: &[&str], mit_code: bool) -> Vec<u8> {
    let manifest = json!({
        "id": "ozean", "name": "Ozean", "version": version, "author": "Ada",
        "sdkVersion": "^1.0", "colorScheme": "dark",
        "permissions": { "network": network }
    })
    .to_string();
    let mut dateien = vec![
        ("manifest.json", manifest.into_bytes()),
        ("theme.css", css.as_bytes().to_vec()),
    ];
    if mit_code {
        dateien.push(("main.js", b"export default {};".to_vec()));
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, inhalt) in dateien {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&inhalt).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// Startet ein Repo mit einem Eintrag `ozean` in `version` für `bytes`; `anpassen` darf den
/// Eintrag ändern
async fn repo(
    version: &'static str,
    bytes: Vec<u8>,
    anpassen: impl FnOnce(&mut Value) + Send + 'static,
) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("Port der Attrappe");
    let addr = listener.local_addr().expect("Adresse der Attrappe");
    let mut entry = json!({
        "id": "ozean",
        "name": "Ozean",
        "version": version,
        "sdkVersion": "^1.0",
        "downloadUrl": format!("http://127.0.0.1:{}/pkg.nexus", addr.port()),
        "sha256": sha256_hex(&bytes),
    });
    anpassen(&mut entry);
    let index = json!([entry]).to_string();
    let app = Router::new()
        .route("/index.json", get(move || async move { index.clone() }))
        .route("/pkg.nexus", get(move || async move { bytes.clone() }));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn server() -> TestServer {
    TestServer::with(TestServerOptions {
        store_index_timeout: Some(Duration::from_secs(5)),
        store_download_timeout: Some(Duration::from_secs(5)),
        ..Default::default()
    })
}

/// Server mit installiertem `ozean` 1.0.0 (Domain `a.example.com`, ohne Code)
fn mit_installation() -> TestServer {
    let server = server();
    let alt = paket("1.0.0", ":root{--alt:1}", &["a.example.com"], false);
    nexus_core::themes::install_theme(&server.home, &alt).unwrap();
    server
}

async fn add_repo(server: &TestServer, port: u16) -> String {
    let body = json!({ "url": format!("http://127.0.0.1:{port}/index.json") });
    let reply = server
        .post("/api/themes/store/repos", &body.to_string())
        .await;
    assert_eq!(reply.status, 201, "{}", reply.text);
    reply.body()["id"].as_str().unwrap().to_string()
}

async fn update(server: &TestServer, repo: &str, confirm: Option<bool>) -> common::Reply {
    let mut body = json!({ "repoId": repo, "id": "ozean" });
    if let Some(confirm) = confirm {
        body["confirm"] = json!(confirm);
    }
    server.post(UPDATE, &body.to_string()).await
}

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

fn css(server: &TestServer) -> String {
    std::fs::read_to_string(server.home.themes_dir().join("ozean/theme.css")).unwrap()
}

async fn installierte_version(server: &TestServer) -> Value {
    server.get("/api/themes").await.body()[0]["version"].clone()
}

#[tokio::test]
async fn updates_meldet_neuere_version_und_repo_fehler() {
    let addr = repo("1.1.0", paket("1.1.0", ":root{}", &[], false), |_| {}).await;
    let server = mit_installation();
    let repo_id = add_repo(&server, addr.port()).await;
    let geschlossen = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let kaputt = add_repo(&server, geschlossen).await;
    let reply = server.get(UPDATES).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    let body = reply.body();
    assert_eq!(
        body["updates"],
        json!([{
            "id": "ozean", "name": "Ozean", "repoId": repo_id,
            "installedVersion": "1.0.0", "availableVersion": "1.1.0"
        }])
    );
    assert_eq!(body["errors"].as_array().unwrap().len(), 1);
    assert_eq!(body["errors"][0]["repoId"], json!(kaputt));
    assert!(body["errors"][0]["code"]
        .as_str()
        .unwrap()
        .starts_with("store-index-"));
}

#[tokio::test]
async fn updates_ohne_neuere_oder_kompatible_version_leer() {
    for (version, kompatibel) in [("1.0.0", true), ("0.9.0", true), ("2.0.0", false)] {
        let addr = repo(
            version,
            paket(version, ":root{}", &[], false),
            move |entry| {
                if !kompatibel {
                    entry["sdkVersion"] = json!("^99.0");
                }
            },
        )
        .await;
        let server = mit_installation();
        add_repo(&server, addr.port()).await;
        let body = server.get(UPDATES).await.body();
        assert_eq!(body["updates"], json!([]), "Version {version}");
        assert_eq!(body["errors"], json!([]));
    }
}

#[tokio::test]
async fn css_update_wird_direkt_installiert() {
    let addr = repo(
        "1.1.0",
        paket("1.1.0", ":root{--neu:1}", &["a.example.com"], false),
        |_| {},
    )
    .await;
    let server = mit_installation();
    let repo_id = add_repo(&server, addr.port()).await;
    let reply = update(&server, &repo_id, None).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    let body = reply.body();
    assert_eq!(body["previousVersion"], json!("1.0.0"));
    assert_eq!(body["manifest"]["version"], json!("1.1.0"));
    assert_eq!(body["repoId"], json!(repo_id));
    assert_eq!(installierte_version(&server).await, json!("1.1.0"));
    assert_eq!(css(&server), ":root{--neu:1}");
    assert_eq!(themes_inhalt(&server), vec!["ozean"]);
}

#[tokio::test]
async fn update_mit_code_braucht_bestaetigung() {
    let addr = repo(
        "1.1.0",
        paket("1.1.0", ":root{--neu:1}", &["a.example.com"], true),
        |_| {},
    )
    .await;
    let server = mit_installation();
    let repo_id = add_repo(&server, addr.port()).await;
    for confirm in [None, Some(false)] {
        let reply = update(&server, &repo_id, confirm).await;
        assert_eq!(reply.status, 409, "{}", reply.text);
        assert_eq!(
            reply.code().as_deref(),
            Some("store-update-confirmation-required")
        );
        assert_eq!(reply.body()["details"]["hasCode"], json!(true));
        assert_eq!(reply.body()["details"]["newDomains"], json!([]));
        assert_eq!(installierte_version(&server).await, json!("1.0.0"));
        assert_eq!(css(&server), ":root{--alt:1}");
        assert_eq!(themes_inhalt(&server), vec!["ozean"]);
    }
    let reply = update(&server, &repo_id, Some(true)).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    assert_eq!(installierte_version(&server).await, json!("1.1.0"));
    assert!(server.home.themes_dir().join("ozean/main.js").is_file());
    assert_eq!(themes_inhalt(&server), vec!["ozean"]);
}

#[tokio::test]
async fn update_mit_neuer_domain_braucht_bestaetigung() {
    let addr = repo(
        "1.1.0",
        paket(
            "1.1.0",
            ":root{}",
            &["a.example.com", "b.example.com"],
            false,
        ),
        |_| {},
    )
    .await;
    let server = mit_installation();
    let repo_id = add_repo(&server, addr.port()).await;
    let reply = update(&server, &repo_id, None).await;
    assert_eq!(reply.status, 409, "{}", reply.text);
    assert_eq!(reply.body()["details"]["hasCode"], json!(false));
    assert_eq!(
        reply.body()["details"]["newDomains"],
        json!(["b.example.com"])
    );
    assert_eq!(installierte_version(&server).await, json!("1.0.0"));
    let reply = update(&server, &repo_id, Some(true)).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    assert_eq!(installierte_version(&server).await, json!("1.1.0"));
}

#[tokio::test]
async fn gleiche_version_nicht_installiert_und_falsche_pruefsumme() {
    let addr = repo("1.0.0", paket("1.0.0", ":root{}", &[], false), |_| {}).await;
    let server = mit_installation();
    let repo_id = add_repo(&server, addr.port()).await;
    let reply = update(&server, &repo_id, Some(true)).await;
    assert_eq!(reply.status, 409, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-update-not-newer"));

    let leer = crate::server();
    let repo_id = add_repo(&leer, addr.port()).await;
    let reply = update(&leer, &repo_id, None).await;
    assert_eq!(reply.status, 404, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-update-not-installed"));

    let addr = repo("1.1.0", paket("1.1.0", ":root{}", &[], false), |entry| {
        entry["sha256"] = json!("0".repeat(64));
    })
    .await;
    let server = mit_installation();
    let repo_id = add_repo(&server, addr.port()).await;
    let reply = update(&server, &repo_id, Some(true)).await;
    assert_eq!(reply.status, 422, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-checksum-mismatch"));
    assert_eq!(installierte_version(&server).await, json!("1.0.0"));
    assert_eq!(css(&server), ":root{--alt:1}");
    assert_eq!(themes_inhalt(&server), vec!["ozean"]);
}

#[tokio::test]
async fn fehlende_felder_und_ohne_kennzeichnung() {
    let server = crate::server();
    let reply = server.post(UPDATE, r#"{"repoId":"x"}"#).await;
    assert_eq!(reply.status, 400);
    let reply = server
        .request(
            "POST",
            UPDATE,
            Some(r#"{"repoId":"x","id":"y"}"#),
            &[("x-harness", None)],
        )
        .await;
    assert_eq!(reply.status, 403);
}
