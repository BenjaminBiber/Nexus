//! Routengruppe `snippets`: CSS-Schnipsel anlegen, ändern, an-/ausschalten, löschen,
//! auflisten, Ablehnungen und die Auslieferung der aktiven Schnipsel.

mod common;

use common::TestServer;
use serde_json::json;

/// Legt einen Schnipsel an und prüft den Status
async fn put(server: &TestServer, id: &str, css: &str, status: u16) -> common::Reply {
    let reply = server
        .put(
            &format!("/api/snippets/{id}"),
            &json!({ "name": id, "css": css }).to_string(),
        )
        .await;
    assert_eq!(reply.status, status, "{}", reply.text);
    reply
}

#[tokio::test]
async fn legt_an_201_und_aendert_200() {
    let server = TestServer::start();
    let created = put(&server, "rot", "a { color: red }", 201).await;
    assert_eq!(created.body()["enabled"], json!(true));
    assert!(server.home.snippets_dir().join("rot.css").exists());

    let changed = put(&server, "rot", "a { color: darkred }", 200).await;
    assert_eq!(changed.body()["css"], json!("a { color: darkred }"));
}

#[tokio::test]
async fn listet_alle_schnipsel() {
    let server = TestServer::start();
    put(&server, "eins", "a{}", 201).await;
    put(&server, "zwei", "b{}", 201).await;
    let reply = server.get("/api/snippets").await;
    assert_eq!(reply.status, 200);
    let list = reply.body();
    let list = list.as_array().expect("Liste");
    assert_eq!(list.len(), 2);
    for key in ["id", "name", "enabled", "order", "updatedAt", "css"] {
        assert!(list[0].get(key).is_some(), "{key}");
    }
    assert_eq!(list[0]["id"], json!("eins"));
}

#[tokio::test]
async fn schaltet_an_und_aus() {
    let server = TestServer::start();
    put(&server, "rot", "a{}", 201).await;
    let reply = server
        .patch(
            "/api/snippets/rot",
            &json!({ "enabled": false }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body()["enabled"], json!(false));
}

#[tokio::test]
async fn loescht_204() {
    let server = TestServer::start();
    put(&server, "rot", "a{}", 201).await;
    assert_eq!(server.delete("/api/snippets/rot").await.status, 204);
    assert!(!server.home.snippets_dir().join("rot.css").exists());
}

#[tokio::test]
async fn unbekannte_id_ergibt_404() {
    let server = TestServer::start();
    assert_eq!(server.delete("/api/snippets/fehlt").await.status, 404);
    let reply = server
        .patch(
            "/api/snippets/fehlt",
            &json!({ "enabled": true }).to_string(),
        )
        .await;
    assert_eq!(reply.status, 404);
}

#[tokio::test]
async fn lehnt_import_und_externe_urls_ab_und_speichert_nichts() {
    let server = TestServer::start();
    put(&server, "imp", "@import 'x.css';", 400).await;
    put(&server, "esc", r"@\69mport 'x.css';", 400).await;
    put(
        &server,
        "ext",
        "a { background: url(https://x.de/a.png) }",
        400,
    )
    .await;
    put(&server, "proto", "a { background: url(//x.de/a.png) }", 400).await;
    put(&server, "Ungueltig", "a{}", 400).await;
    put(&server, "gross", &"a".repeat(64 * 1024 + 1), 400).await;
    assert_eq!(server.get("/api/snippets").await.body(), json!([]));
}

#[tokio::test]
async fn erlaubt_data_urls() {
    let server = TestServer::start();
    put(
        &server,
        "bild",
        "a { background: url(\"data:image/png;base64,AAAA\") }",
        201,
    )
    .await;
}

#[tokio::test]
async fn liefert_nur_aktive_schnipsel_aus() {
    let server = TestServer::start();
    let empty = server.get("/snippets/active.css").await;
    assert_eq!(empty.status, 200);
    assert_eq!(empty.text, "");

    put(&server, "an", "a { color: red }", 201).await;
    put(&server, "aus", "b { color: blue }", 201).await;
    server
        .patch(
            "/api/snippets/aus",
            &json!({ "enabled": false }).to_string(),
        )
        .await;
    let reply = server.get("/snippets/active.css").await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.header("content-type"),
        Some("text/css; charset=utf-8")
    );
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    assert!(reply.text.contains("/* snippet: an */"));
    assert!(reply.text.contains("color: red"));
    assert!(!reply.text.contains("color: blue"));
}
