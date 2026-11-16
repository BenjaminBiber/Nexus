//! Claude-Theme: der Slot `<Claude-Ordner>/skills/nexus-theme/` folgt dem aktiven Theme.
//! Auslöser sind Benutzereinstellungen, Theme-Einstellungen, Installation und Entfernen;
//! `GET /api/claude-theme` meldet den Zustand. Der Claude-Ordner liegt im Benutzerordner des
//! Tests (`<home>/claude`), nie im echten.

mod common;

use std::io::{Cursor, Write};
use std::path::PathBuf;

use base64::Engine;
use common::TestServer;
use zip::write::SimpleFileOptions;

/// Manifest mit Claude-Theme und einer Auswahl `palette`
fn manifest(id: &str, claude: bool) -> String {
    let mut manifest = serde_json::json!({
        "id": id,
        "name": format!("Theme {id}"),
        "version": "1.0.0",
        "author": "Ada",
        "sdkVersion": "^1.3",
        "colorScheme": "dark",
        "settingsSchema": {
            "palette": {
                "type": "select",
                "label": "Palette",
                "default": "a",
                "options": [{ "value": "a", "label": "A" }, { "value": "b", "label": "B" }]
            }
        }
    });
    if claude {
        manifest["claudeTheme"] = serde_json::json!(true);
    }
    manifest.to_string()
}

/// Paket als JSON-Körper für `POST /api/themes/install`
fn paket(id: &str, claude: bool) -> String {
    let manifest = manifest(id, claude);
    let mut dateien: Vec<(&str, &[u8])> = vec![
        ("manifest.json", manifest.as_bytes()),
        ("theme.css", b":root {}"),
    ];
    if claude {
        dateien.push((
            "claude/hooks/hooks.json",
            br#"{ "modules": ["./register.tsx"] }"#,
        ));
        dateien.push((
            "claude/hooks/register.tsx",
            b"export const register = () => {}\n",
        ));
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, inhalt) in dateien {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(inhalt).unwrap();
    }
    let bytes = writer.finish().unwrap().into_inner();
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    serde_json::json!({ "data": data }).to_string()
}

fn slot(server: &TestServer) -> PathBuf {
    server
        .home
        .dir()
        .join("claude")
        .join("skills")
        .join("nexus-theme")
}

async fn installiere(server: &TestServer, id: &str, claude: bool) {
    let reply = server.post("/api/themes/install", &paket(id, claude)).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
}

async fn einstellungen(server: &TestServer, body: serde_json::Value) {
    let reply = server.put("/api/user-settings", &body.to_string()).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
}

#[tokio::test]
async fn slot_folgt_dem_aktiven_theme_und_seinen_einstellungen() {
    let server = TestServer::start();

    // Installiert, aber nicht aktiv: nichts in Claude
    installiere(&server, "neon", true).await;
    assert!(!slot(&server).exists());
    let status = server.get("/api/claude-theme").await;
    assert_eq!(status.status, 200);
    assert_eq!(status.body()["state"], "none");
    assert_eq!(status.body()["enabled"], true);

    // Aktiv: der Mod liegt im Slot
    einstellungen(&server, serde_json::json!({ "theme": "neon" })).await;
    assert!(slot(&server).join("hooks/register.tsx").is_file());
    let status = server.get("/api/claude-theme").await.body();
    assert_eq!(status["state"], "active");
    assert_eq!(status["themeId"], "neon");
    assert_eq!(status["themeName"], "Theme neon");
    assert_eq!(status["path"], slot(&server).display().to_string());

    // Theme-Einstellungen landen in nexus-settings.ts
    let reply = server
        .put("/api/themes/neon/settings", r#"{ "palette": "b" }"#)
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    let module = std::fs::read_to_string(slot(&server).join("hooks/nexus-settings.ts")).unwrap();
    assert!(module.contains(r#"{"palette":"b"}"#), "{module}");

    // Schalter aus: der Slot verschwindet
    einstellungen(&server, serde_json::json!({ "claudeTheme": false })).await;
    assert!(!slot(&server).exists());
    let status = server.get("/api/claude-theme").await.body();
    assert_eq!(status["state"], "none");
    assert_eq!(status["enabled"], false);

    // Wieder an, dann das aktive Theme entfernen
    einstellungen(&server, serde_json::json!({ "claudeTheme": true })).await;
    assert!(slot(&server).is_dir());
    let reply = server.delete("/api/themes/neon").await;
    assert_eq!(reply.status, 204);
    assert!(!slot(&server).exists());
}

#[tokio::test]
async fn theme_ohne_claude_theme_raeumt_den_slot_weg() {
    let server = TestServer::start();
    installiere(&server, "neon", true).await;
    installiere(&server, "schlicht", false).await;
    einstellungen(&server, serde_json::json!({ "theme": "neon" })).await;
    assert!(slot(&server).is_dir());

    einstellungen(&server, serde_json::json!({ "theme": "schlicht" })).await;

    assert!(!slot(&server).exists());
    let status = server.get("/api/claude-theme").await.body();
    assert_eq!(status["state"], "none");
    assert_eq!(status["themeId"], "schlicht");
}

#[tokio::test]
async fn neue_version_des_aktiven_themes_kommt_im_slot_an() {
    let server = TestServer::start();
    installiere(&server, "neon", true).await;
    einstellungen(&server, serde_json::json!({ "theme": "neon" })).await;
    let plugin = slot(&server).join(".claude-plugin/plugin.json");
    assert!(std::fs::read_to_string(&plugin).unwrap().contains("1.0.0"));

    let mut neu: serde_json::Value = serde_json::from_str(&manifest("neon", true)).unwrap();
    neu["version"] = serde_json::json!("1.1.0");
    let neu = neu.to_string();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, inhalt) in [
        ("manifest.json", neu.as_bytes()),
        ("theme.css", b":root {}".as_slice()),
        (
            "claude/hooks/hooks.json",
            br#"{ "modules": ["./register.tsx"] }"#.as_slice(),
        ),
        ("claude/hooks/register.tsx", b"// neu\n".as_slice()),
    ] {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(inhalt).unwrap();
    }
    let data =
        base64::engine::general_purpose::STANDARD.encode(writer.finish().unwrap().into_inner());
    let reply = server
        .post(
            "/api/themes/install",
            &serde_json::json!({ "data": data }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text);

    assert!(std::fs::read_to_string(&plugin).unwrap().contains("1.1.0"));
    assert_eq!(
        std::fs::read_to_string(slot(&server).join("hooks/register.tsx")).unwrap(),
        "// neu\n"
    );
}

#[tokio::test]
async fn fremder_ordner_bricht_die_anfrage_nicht_ab() {
    let server = TestServer::start();
    installiere(&server, "neon", true).await;
    std::fs::create_dir_all(slot(&server)).unwrap();
    std::fs::write(slot(&server).join("eigenes.txt"), "meins").unwrap();

    // Das Speichern gelingt, obwohl der Slot nicht beschrieben werden kann
    einstellungen(&server, serde_json::json!({ "theme": "neon" })).await;

    let status = server.get("/api/claude-theme").await.body();
    assert_eq!(status["state"], "foreign");
    assert!(status["message"]
        .as_str()
        .unwrap()
        .contains("gehört nicht Nexus"));
    assert_eq!(
        std::fs::read_to_string(slot(&server).join("eigenes.txt")).unwrap(),
        "meins"
    );
    assert!(!slot(&server).join("hooks").exists());
}

#[tokio::test]
async fn andere_einstellungen_loesen_keinen_abgleich_aus() {
    let server = TestServer::start();
    installiere(&server, "neon", true).await;
    einstellungen(&server, serde_json::json!({ "theme": "neon" })).await;
    std::fs::remove_dir_all(slot(&server)).unwrap();

    // Theme und Schalter bleiben gleich: der Slot wird nicht neu geschrieben
    einstellungen(&server, serde_json::json!({ "notifications": "off" })).await;

    assert!(!slot(&server).exists());
}
