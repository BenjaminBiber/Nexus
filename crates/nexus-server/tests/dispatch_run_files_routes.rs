//! Routen zur Ablage eines Laufs: Verlauf aus dem Transcript und Screenshots neben dem Protokoll.

mod common;

use chrono::Utc;
use common::TestServer;
use nexus_core::dispatch::state::{RunFiles, RunSession};
use serde_json::json;

/// Dateiname eines Lauf-Protokolls
const NAME: &str = "ARC-0001-20261016-101500.log";

/// Die Ablage zum Lauf `NAME` im Benutzerordner des Testservers
fn ablage(server: &TestServer) -> RunFiles {
    RunFiles::for_run(&server.home, NAME).expect("gültiger Name")
}

#[tokio::test]
async fn ohne_ablage_gibt_es_weder_verlauf_noch_screenshots() {
    let server = TestServer::start();

    let verlauf = server
        .get(&format!("/api/dispatch/runs/{NAME}/transcript?after=7"))
        .await;
    assert_eq!(verlauf.status, 200);
    assert_eq!(
        verlauf.json(),
        Some(json!({ "entries": [], "cursor": "7", "skipped": 0, "available": false }))
    );

    let liste = server
        .get(&format!("/api/dispatch/runs/{NAME}/shots"))
        .await;
    assert_eq!(liste.status, 200);
    assert_eq!(liste.json(), Some(json!([])));

    let bild = server
        .get(&format!("/api/dispatch/runs/{NAME}/shots/001.png"))
        .await;
    assert_eq!(bild.status, 404);
    assert_eq!(bild.code().as_deref(), Some("shot-not-found"));
}

#[tokio::test]
async fn liefert_die_screenshots_eines_laufs() {
    let server = TestServer::start();
    ablage(&server)
        .add_shot("toolu_1", "image/png", b"PNG-Testbild", Utc::now())
        .expect("abgelegt")
        .expect("Bild angenommen");

    let liste = server
        .get(&format!("/api/dispatch/runs/{NAME}/shots"))
        .await;
    assert_eq!(liste.status, 200);
    let liste = liste.json().expect("JSON");
    assert_eq!(liste.as_array().map(Vec::len), Some(1));
    assert_eq!(liste[0]["file"], "001.png");
    assert_eq!(liste[0]["toolUseId"], "toolu_1");
    assert_eq!(liste[0]["mediaType"], "image/png");
    assert_eq!(liste[0]["bytes"], 12);

    let bild = server
        .get(&format!("/api/dispatch/runs/{NAME}/shots/001.png"))
        .await;
    assert_eq!(bild.status, 200);
    assert_eq!(bild.headers["content-type"], "image/png");
    assert_eq!(
        bild.headers["cache-control"],
        "private, max-age=31536000, immutable"
    );
    assert_eq!(bild.headers["x-content-type-options"], "nosniff");
    assert_eq!(bild.text, "PNG-Testbild");
}

#[tokio::test]
async fn liefert_nur_bilder_aus_dem_index() {
    let server = TestServer::start();
    let files = ablage(&server);
    files
        .add_shot("toolu_1", "image/png", b"PNG-Testbild", Utc::now())
        .expect("abgelegt");
    // Liegen bloß im Ordner, stehen aber nicht im Index
    std::fs::write(files.dir().join("002.png"), b"fremd").expect("Datei");
    std::fs::write(files.dir().join("fremd.png"), b"fremd").expect("Datei");
    files
        .write_session(&RunSession {
            session_id: "sitzung-1".to_string(),
            cwd: "D:/nirgends".to_string(),
        })
        .expect("Session");

    for datei in ["002.png", "fremd.png", "session.json", "shots.jsonl"] {
        let reply = server
            .get(&format!("/api/dispatch/runs/{NAME}/shots/{datei}"))
            .await;
        assert_eq!(reply.status, 404, "{datei}");
        assert_eq!(reply.code().as_deref(), Some("shot-not-found"), "{datei}");
    }
}

#[tokio::test]
async fn weist_ungueltige_namen_und_zahlen_ab() {
    let server = TestServer::start();

    for pfad in ["transcript", "shots", "shots/001.png"] {
        let reply = server
            .get(&format!("/api/dispatch/runs/x.txt/{pfad}"))
            .await;
        assert_eq!(reply.status, 422, "{pfad}");
        assert_eq!(reply.code().as_deref(), Some("log-name-invalid"), "{pfad}");
    }

    for abfrage in ["after=abc", "limit=-1"] {
        let reply = server
            .get(&format!("/api/dispatch/runs/{NAME}/transcript?{abfrage}"))
            .await;
        assert_eq!(reply.status, 400, "{abfrage}");
        assert_eq!(reply.code().as_deref(), Some("input-invalid"), "{abfrage}");
    }
}

#[tokio::test]
async fn eine_session_ohne_transcript_ist_kein_fehler() {
    let server = TestServer::start();
    ablage(&server)
        .write_session(&RunSession {
            session_id: "sitzung-ohne-transcript-4f1c".to_string(),
            cwd: "D:/nirgends/agent-laeufe".to_string(),
        })
        .expect("Session");

    let verlauf = server
        .get(&format!("/api/dispatch/runs/{NAME}/transcript"))
        .await;
    assert_eq!(verlauf.status, 200);
    assert_eq!(
        verlauf.json(),
        Some(json!({ "entries": [], "cursor": "0", "skipped": 0, "available": false }))
    );
}
