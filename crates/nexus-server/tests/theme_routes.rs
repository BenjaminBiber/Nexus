//! Themes: `GET /api/themes`, `POST /api/themes/install` (Paket als Base64 im JSON-Körper) und
//! `DELETE /api/themes/{id}`. Die Pakete baut der Test als ZIP im Speicher.

mod common;

use std::io::{Cursor, Write};

use base64::Engine;
use common::TestServer;
use zip::write::SimpleFileOptions;

fn manifest(id: &str, version: &str) -> Vec<u8> {
    format!(
        r#"{{"id":"{id}","name":"Nacht","version":"{version}","author":"Ada","sdkVersion":"^1.0","colorScheme":"dark"}}"#
    )
    .into_bytes()
}

/// Baut ein ZIP im Speicher
fn paket(dateien: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, inhalt) in dateien {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(inhalt).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn gueltig(id: &str, version: &str) -> Vec<u8> {
    let manifest = manifest(id, version);
    paket(&[("manifest.json", &manifest), ("theme.css", b":root {}")])
}

/// Der JSON-Körper zum Hochladen
fn koerper(paket: &[u8]) -> String {
    let data = base64::engine::general_purpose::STANDARD.encode(paket);
    serde_json::json!({ "data": data }).to_string()
}

#[tokio::test]
async fn liste_ist_anfangs_leer() {
    let server = TestServer::start();
    let reply = server.get("/api/themes").await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body(), serde_json::json!([]));
}

#[tokio::test]
async fn gueltiges_paket_wird_installiert_aufgelistet_und_ersetzt() {
    let server = TestServer::start();
    let reply = server
        .post("/api/themes/install", &koerper(&gueltig("nacht", "1.0.0")))
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    assert_eq!(reply.body()["id"], "nacht");
    assert_eq!(reply.body()["version"], "1.0.0");
    assert!(server.home.themes_dir().join("nacht/theme.css").is_file());

    let liste = server.get("/api/themes").await;
    assert_eq!(liste.status, 200);
    assert_eq!(liste.body().as_array().unwrap().len(), 1);
    assert_eq!(liste.body()[0]["id"], "nacht");

    let ersetzt = server
        .post("/api/themes/install", &koerper(&gueltig("nacht", "2.0.0")))
        .await;
    assert_eq!(ersetzt.status, 200);
    let liste = server.get("/api/themes").await;
    assert_eq!(liste.body().as_array().unwrap().len(), 1);
    assert_eq!(liste.body()[0]["version"], "2.0.0");
}

#[tokio::test]
async fn kein_zip_ist_ungueltiges_paket() {
    let server = TestServer::start();
    let reply = server
        .post("/api/themes/install", &koerper(b"kein zip"))
        .await;
    assert_eq!(reply.status, 422);
    assert_eq!(reply.code().as_deref(), Some("theme-package-invalid"));
    assert!(reply.body()["message"]
        .as_str()
        .unwrap()
        .starts_with("Das Paket ist kein gültiges ZIP-Archiv"));
}

#[tokio::test]
async fn fehlendes_theme_css_wird_gemeldet() {
    let server = TestServer::start();
    let manifest = manifest("nacht", "1.0.0");
    let reply = server
        .post(
            "/api/themes/install",
            &koerper(&paket(&[("manifest.json", &manifest)])),
        )
        .await;
    assert_eq!(reply.status, 422);
    assert_eq!(reply.code().as_deref(), Some("theme-package-invalid"));
    assert_eq!(reply.body()["message"], "Im Paket fehlt theme.css.");
}

#[tokio::test]
async fn unsicherer_pfad_und_import_werden_abgelehnt() {
    let server = TestServer::start();
    let manifest = manifest("nacht", "1.0.0");
    let pfad = paket(&[
        ("manifest.json", &manifest),
        ("theme.css", b":root {}"),
        ("../boese.css", b""),
    ]);
    let reply = server.post("/api/themes/install", &koerper(&pfad)).await;
    assert_eq!(reply.status, 422);
    assert_eq!(reply.code().as_deref(), Some("theme-package-invalid"));

    let import = paket(&[
        ("manifest.json", &manifest),
        ("theme.css", b"@import 'x.css';"),
    ]);
    let reply = server.post("/api/themes/install", &koerper(&import)).await;
    assert_eq!(reply.status, 422);
    assert_eq!(reply.code().as_deref(), Some("theme-package-invalid"));
    assert!(server.get("/api/themes").await.body() == serde_json::json!([]));
}

#[tokio::test]
async fn ungueltiges_manifest_liefert_feldfehler() {
    let server = TestServer::start();
    let manifest = br#"{"id":"A","name":"","version":"x","author":"Ada","sdkVersion":"^1.0","colorScheme":"dark"}"#;
    let reply = server
        .post(
            "/api/themes/install",
            &koerper(&paket(&[
                ("manifest.json", manifest),
                ("theme.css", b":root {}"),
            ])),
        )
        .await;
    assert_eq!(reply.status, 422);
    assert_eq!(reply.code().as_deref(), Some("theme-manifest-invalid"));
    let fehler = reply.body()["details"]["errors"]
        .as_array()
        .unwrap()
        .clone();
    assert!(fehler.iter().any(|f| f["field"] == "id"), "{fehler:?}");
    assert!(fehler.iter().all(|f| f["message"].is_string()));
}

#[tokio::test]
async fn kaputtes_oder_fehlendes_base64_ist_400() {
    let server = TestServer::start();
    let reply = server
        .post("/api/themes/install", r#"{"data":"%%%"}"#)
        .await;
    assert_eq!(reply.status, 400);
    assert_eq!(reply.code().as_deref(), Some("bad-request"));

    let reply = server.post("/api/themes/install", "{}").await;
    assert_eq!(reply.status, 400);
    assert_eq!(reply.code().as_deref(), Some("bad-request"));
}

#[tokio::test]
async fn installieren_ohne_kennzeichnung_ist_verboten() {
    let server = TestServer::start();
    let reply = server
        .request(
            "POST",
            "/api/themes/install",
            Some(&koerper(&gueltig("nacht", "1.0.0"))),
            &[("x-harness", None)],
        )
        .await;
    assert_eq!(reply.status, 403);
}

#[tokio::test]
async fn entfernen_vorhandener_unbekannter_und_ungueltiger_ids() {
    let server = TestServer::start();
    server
        .post("/api/themes/install", &koerper(&gueltig("nacht", "1.0.0")))
        .await;
    let reply = server.delete("/api/themes/nacht").await;
    assert_eq!(reply.status, 204);
    assert!(!server.home.themes_dir().join("nacht").exists());

    let reply = server.delete("/api/themes/nacht").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.code().as_deref(), Some("theme-not-found"));

    for id in ["A", "..%2Fx"] {
        let reply = server.delete(&format!("/api/themes/{id}")).await;
        assert_eq!(reply.status, 400, "{id}");
        assert_eq!(reply.code().as_deref(), Some("theme-id-invalid"));
    }
}
