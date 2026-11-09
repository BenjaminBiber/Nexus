//! Routengruppe `theme_settings`: Werte je Theme speichern und lesen, auch nach einem
//! Neustart, und Ablehnung ungültiger Eingaben.

mod common;

use common::{TestServer, TestServerOptions};
use serde_json::json;

#[tokio::test]
async fn speichert_und_liest_auch_nach_neustart() {
    let server = TestServer::start();
    let reply = server
        .put(
            "/api/themes/min-craft/settings",
            &json!({ "sounds": false, "style": "pixel" }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    assert_eq!(reply.body(), json!({ "sounds": false, "style": "pixel" }));
    assert!(server.home.theme_settings_file().exists());

    let reply = server.get("/api/themes/min-craft/settings").await;
    assert_eq!(reply.body(), json!({ "sounds": false, "style": "pixel" }));

    let restarted = server.restarted_with(TestServerOptions::default());
    let reply = restarted.get("/api/themes/min-craft/settings").await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body(), json!({ "sounds": false, "style": "pixel" }));
}

#[tokio::test]
async fn unbekanntes_theme_liefert_leeres_objekt() {
    let server = TestServer::start();
    let reply = server.get("/api/themes/unbekannt/settings").await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body(), json!({}));
}

#[tokio::test]
async fn lehnt_ungueltige_eingaben_mit_400_ab() {
    let server = TestServer::start();
    let too_many: serde_json::Map<String, serde_json::Value> =
        (0..51).map(|i| (format!("k{i}"), json!(true))).collect();
    for (path, body) in [
        ("/api/themes/A_B/settings", json!({ "sounds": true })),
        ("/api/themes/ok/settings", json!({ "zahl": 3 })),
        ("/api/themes/ok/settings", json!(["liste"])),
        (
            "/api/themes/ok/settings",
            serde_json::Value::Object(too_many),
        ),
    ] {
        let reply = server.put(path, &body.to_string()).await;
        assert_eq!(reply.status, 400, "{path} {body}");
        assert_eq!(
            reply.code().as_deref(),
            Some("theme-settings-invalid"),
            "{body}"
        );
    }
    let reply = server.get("/api/themes/A_B/settings").await;
    assert_eq!(reply.status, 400);
}
