//! Routengruppe `store_repos`: Store-Repos auflisten, eintragen, ablehnen, entfernen und
//! über einen Neustart behalten.

mod common;

use common::{TestServer, TestServerOptions};
use serde_json::{json, Value};

const BASE: &str = "/api/themes/store/repos";
const INDEX: &str = "https://raw.githubusercontent.com/a/b/main/nexus-themes.json";

/// Trägt eine Adresse ein und prüft den Status
async fn add(server: &TestServer, body: Value, status: u16) -> common::Reply {
    let reply = server.post(BASE, &body.to_string()).await;
    assert_eq!(reply.status, status, "{}", reply.text);
    reply
}

async fn list(server: &TestServer) -> Vec<Value> {
    let reply = server.get(BASE).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    reply.body().as_array().expect("Liste").clone()
}

#[tokio::test]
async fn leere_liste() {
    let server = TestServer::start();
    assert!(list(&server).await.is_empty());
}

#[tokio::test]
async fn traegt_ein_201_und_listet() {
    let server = TestServer::start();
    let repo = add(&server, json!({ "url": INDEX }), 201).await.body();
    assert_eq!(repo["url"], json!(INDEX));
    for key in ["id", "url", "addedAt"] {
        assert!(repo.get(key).is_some(), "{key}");
    }
    assert_eq!(list(&server).await, vec![repo]);
    assert!(server.home.store_dir().join("repos.json").exists());
}

#[tokio::test]
async fn lehnt_ungueltige_eingaben_ab_400() {
    let server = TestServer::start();
    for body in [
        json!({ "url": "http://example.com/i.json" }),
        json!({ "url": "https://u:p@example.com/i.json" }),
        json!({ "url": "nicht-url" }),
        json!({ "url": 5 }),
        json!({}),
    ] {
        let reply = add(&server, body, 400).await;
        assert_eq!(reply.body()["code"], json!("store-repo-invalid"));
    }
    assert!(list(&server).await.is_empty());
}

#[tokio::test]
async fn dublette_409() {
    let server = TestServer::start();
    add(&server, json!({ "url": INDEX }), 201).await;
    let reply = add(
        &server,
        json!({ "url": " HTTPS://RAW.githubusercontent.com/a/b/main/nexus-themes.json" }),
        409,
    )
    .await;
    assert_eq!(reply.body()["code"], json!("store-repo-exists"));
    assert_eq!(list(&server).await.len(), 1);
}

#[tokio::test]
async fn entfernt_204_und_unbekannt_404() {
    let server = TestServer::start();
    let repo = add(&server, json!({ "url": INDEX }), 201).await.body();
    let id = repo["id"].as_str().expect("id");
    let reply = server.delete(&format!("{BASE}/{id}")).await;
    assert_eq!(reply.status, 204, "{}", reply.text);
    assert!(list(&server).await.is_empty());

    let reply = server.delete(&format!("{BASE}/{id}")).await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.body()["code"], json!("store-repo-not-found"));
}

#[tokio::test]
async fn bleibt_nach_neustart_erhalten() {
    let server = TestServer::start();
    let repo = add(&server, json!({ "url": INDEX }), 201).await.body();
    let again = server.restarted_with(TestServerOptions::default());
    assert_eq!(list(&again).await, vec![repo]);
}

#[tokio::test]
async fn ohne_harness_kopf_403() {
    let server = TestServer::start();
    let reply = server
        .request(
            "POST",
            BASE,
            Some(&json!({ "url": INDEX }).to_string()),
            &[("x-harness", None)],
        )
        .await;
    assert_eq!(reply.status, 403);
    assert!(list(&server).await.is_empty());
}
