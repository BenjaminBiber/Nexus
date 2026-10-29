//! Die gespeicherten Ticket-Entwürfe eines Dokuments; portiert aus den Claude-freien Fällen
//! von `packages/server/tests/unit/DocTicketDrafts.test.ts`.
//!
//! Das Erzeugen der Entwürfe (`POST …/docs/ticket-drafts`) ruft Claude und bleibt bis P4-8b
//! beim Node-Server; hier wird deshalb über `PUT …/docs/ticket-drafts/saved` gespeichert.

mod common;

use common::{encode, DocsFixture};
use serde_json::{json, Value};

const SPEC: &str = "docs/superpowers/specs/2026-10-22-docs-rework-design.md";

/// Ein vollständiger Entwurf
fn draft(title: &str) -> Value {
    json!({
        "title": title,
        "description": format!("Beschreibung {title}"),
        "acceptance": format!("Abnahme {title}"),
        "steps": ["Test schreiben"],
        "dependsOn": [],
        "labels": ["server"],
        "source": { "heading": "Ziel", "startLine": 3, "endLine": 5 },
    })
}

/// Server, Projekt, Board mit Repo-Pfad und die Testspec
async fn start() -> DocsFixture {
    let fixture = DocsFixture::start(true).await;
    fixture.write(
        "docs/superpowers/specs",
        "2026-10-22-docs-rework-design.md",
        "# Docs-Rework\n\n## Ziel\n\nEine Quelle für Docs.\n\n## Plan\n\nSchritt eins.\n",
    );
    fixture
}

/// Adresse der gespeicherten Entwürfe eines Dokuments
fn saved_url(fixture: &DocsFixture, path: &str) -> String {
    format!(
        "{}/ticket-drafts/saved?path={}",
        fixture.base(),
        encode(path)
    )
}

#[tokio::test]
async fn merkt_sich_die_entwurfe_je_datei_speichert_anderungen_und_verwirft_sie() {
    let fixture = start().await;
    let saved = saved_url(&fixture, SPEC);
    assert_eq!(
        fixture.server.get(&saved).await.json(),
        Some(json!({ "result": null }))
    );

    let put = fixture
        .server
        .put(
            &format!("{}/ticket-drafts/saved", fixture.base()),
            &json!({ "path": SPEC, "drafts": [draft("A"), draft("B")] }).to_string(),
        )
        .await;
    assert_eq!(put.status, 200);

    let result = fixture.server.get(&saved).await.json().expect("JSON")["result"].clone();
    let titles: Vec<&str> = result["drafts"]
        .as_array()
        .expect("Entwürfe")
        .iter()
        .filter_map(|item| item["title"].as_str())
        .collect();
    assert_eq!(titles, vec!["A", "B"]);
    // Ohne frühere Erzeugung gibt es keine Kosten, aber einen Zeitpunkt
    assert_eq!(result["costUsd"], Value::Null);
    assert!(result["at"].as_str().is_some_and(|at| !at.is_empty()));

    assert_eq!(fixture.server.delete(&saved).await.status, 200);
    assert_eq!(
        fixture.server.get(&saved).await.json(),
        Some(json!({ "result": null }))
    );
}

/// Die Markierung „Entwürfe vorhanden“ der Testspec in der Docs-Liste
async fn flagged(fixture: &DocsFixture) -> Value {
    fixture
        .server
        .get(&fixture.base())
        .await
        .json()
        .expect("JSON")["docs"]
        .as_array()
        .expect("Dokumente")
        .iter()
        .find(|doc| doc["path"] == SPEC)
        .expect("Testspec")
        .get("hasDrafts")
        .cloned()
        .unwrap_or(Value::Null)
}

#[tokio::test]
async fn markiert_in_der_docs_liste_die_dateien_mit_gespeicherten_entwurfen() {
    let fixture = start().await;
    assert_eq!(flagged(&fixture).await, Value::Null);

    fixture
        .server
        .put(
            &format!("{}/ticket-drafts/saved", fixture.base()),
            &json!({ "path": SPEC, "drafts": [draft("A")] }).to_string(),
        )
        .await;
    assert_eq!(flagged(&fixture).await, json!(true));

    fixture.server.delete(&saved_url(&fixture, SPEC)).await;
    assert_eq!(flagged(&fixture).await, Value::Null);
}

#[tokio::test]
async fn verwirft_die_gespeicherten_entwurfe_sobald_daraus_tickets_angelegt_sind() {
    let fixture = start().await;
    fixture
        .server
        .put(
            &format!("{}/ticket-drafts/saved", fixture.base()),
            &json!({ "path": SPEC, "drafts": [draft("A")] }).to_string(),
        )
        .await;

    let created = fixture
        .server
        .post(
            &format!("{}/tickets", fixture.base()),
            &json!({ "path": SPEC, "drafts": [draft("A")] }).to_string(),
        )
        .await;

    assert_eq!(created.status, 200);
    assert_eq!(
        fixture.server.get(&saved_url(&fixture, SPEC)).await.json(),
        Some(json!({ "result": null }))
    );
}

#[tokio::test]
async fn pruft_pfad_und_entwurfe() {
    let fixture = start().await;
    let url = format!("{}/ticket-drafts/saved", fixture.base());

    for path in ["../geheim.md", "docs/superpowers/specs/fehlt.md"] {
        let res = fixture
            .server
            .put(
                &url,
                &json!({ "path": path, "drafts": [draft("A")] }).to_string(),
            )
            .await;
        assert_eq!(res.code().as_deref(), Some("doc-not-found"), "{path}");
    }

    let no_drafts = fixture
        .server
        .put(&url, &json!({ "path": SPEC }).to_string())
        .await;
    assert_eq!(no_drafts.status, 422);
    assert_eq!(no_drafts.code().as_deref(), Some("invalid-input"));
}
