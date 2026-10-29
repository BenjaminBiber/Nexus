//! Ticket-Entwürfe aus markierten Stellen oder dem ganzen Dokument (`POST`/`DELETE
//! …/docs/ticket-drafts`); portiert aus `packages/server/tests/unit/DocTicketDrafts.test.ts`.
//!
//! Claude wird nie wirklich gestartet: der Server bekommt die Attrappe
//! [`common::FakeClaude`].

mod common;

use std::sync::Arc;

use common::{encode, fake_runners, reply, DocsFixture, FakeClaude, Reply, TestServerOptions};
use nexus_server::dispatch::RunResult;
use serde_json::{json, Value};

const SPEC: &str = "docs/superpowers/specs/2026-10-22-docs-rework-design.md";

/// Die markierte Stelle der Tests
fn selection() -> Value {
    json!([{ "heading": "Ziel", "startLine": 3, "endLine": 5, "quote": "Eine Quelle für Docs." }])
}

/// Ein vollständiger Entwurf, mit weiteren Feldern überschreibbar
fn draft(title: &str, extra: Value) -> Value {
    let mut item = json!({
        "title": title,
        "description": format!("Beschreibung {title}"),
        "acceptance": format!("Abnahme {title}"),
        "steps": ["Test schreiben"],
        "dependsOn": [],
        "labels": ["server"],
        "source": { "heading": "Ziel", "startLine": 3, "endLine": 5 },
    });
    for (key, value) in extra.as_object().cloned().unwrap_or_default() {
        item[key] = value;
    }
    item
}

/// Server, Projekt, Board mit Repo-Pfad, Testspec und die Attrappe von Claude
async fn start(fake: Arc<FakeClaude>) -> (DocsFixture, String) {
    let options = TestServerOptions {
        claude: Some(fake_runners(fake)),
        ..TestServerOptions::default()
    };
    let fixture = DocsFixture::start_with(options, true).await;
    fixture.write(
        "docs/superpowers/specs",
        "2026-10-22-docs-rework-design.md",
        "# Docs-Rework\n\n## Ziel\n\nEine Quelle für Docs.\n\n## Plan\n\nSchritt eins.\n",
    );
    let base = format!("{}/ticket-drafts", fixture.base());
    (fixture, base)
}

/// Erzeugt Entwürfe zur markierten Stelle
async fn create(fixture: &DocsFixture, base: &str, body: Value) -> Reply {
    fixture.server.post(base, &body.to_string()).await
}

/// Die Titel der Entwürfe einer Antwort
fn titles(json: &Value) -> Vec<String> {
    json["drafts"]
        .as_array()
        .expect("Entwürfe")
        .iter()
        .map(|draft| draft["title"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn liefert_gepruefte_entwuerfe_mit_quelle_abhaengigkeiten_und_kosten() {
    let fake = FakeClaude::replying(
        &json!([
            draft("A", json!({})),
            draft("B", json!({ "dependsOn": [0] }))
        ])
        .to_string(),
    );
    let (fixture, base) = start(fake.clone()).await;

    let res = create(
        &fixture,
        &base,
        json!({ "path": SPEC, "selections": selection(), "note": "bitte klein schneiden" }),
    )
    .await;

    assert_eq!(res.status, 200);
    let json = res.json().expect("JSON");
    assert_eq!(titles(&json), ["A", "B"]);
    assert_eq!(json["drafts"][1]["dependsOn"], json!([0]));
    assert_eq!(
        json["drafts"][0]["source"],
        json!({ "heading": "Ziel", "startLine": 3, "endLine": 5 })
    );
    assert_eq!(json["costUsd"], json!(0.02));
    assert!(json["at"].as_str().is_some_and(|at| !at.is_empty()));
    // Der Hinweis des Nutzers und die markierte Stelle stehen im Prompt
    let prompt = fake.last_prompt();
    assert!(prompt.contains("## Markierte Stellen"));
    assert!(prompt.contains("bitte klein schneiden"));
    assert!(prompt.contains("Eine Quelle für Docs."));
}

#[tokio::test]
async fn meldet_eine_antwort_ohne_json_als_analysis_failed() {
    let fake = FakeClaude::replying("Ich würde drei Tickets vorschlagen.");
    let (fixture, base) = start(fake).await;

    let res = create(
        &fixture,
        &base,
        json!({ "path": SPEC, "selections": selection() }),
    )
    .await;

    assert_eq!(res.status, 422);
    assert_eq!(res.code().as_deref(), Some("analysis-failed"));
    assert!(res.text.contains("nicht lesbar"));
}

#[tokio::test]
async fn verwirft_unvollstaendige_entwuerfe_und_haelt_die_abhaengigkeiten_gerade() {
    let fake = FakeClaude::replying(
        &json!([
            draft("A", json!({})),
            draft("B", json!({ "acceptance": "" })),
            draft("C", json!({ "dependsOn": [0, 1] })),
        ])
        .to_string(),
    );
    let (fixture, base) = start(fake).await;

    let json = create(
        &fixture,
        &base,
        json!({ "path": SPEC, "selections": selection() }),
    )
    .await
    .json()
    .expect("JSON");

    assert_eq!(titles(&json), ["A", "C"]);
    assert_eq!(json["drafts"][1]["dependsOn"], json!([0]));
    assert!(!json["dropped"].as_array().expect("Hinweise").is_empty());
}

#[tokio::test]
async fn bereinigt_kreise_in_depends_on_und_behaelt_alle_entwuerfe() {
    let fake = FakeClaude::replying(
        &json!([
            draft("A", json!({ "dependsOn": [2] })),
            draft("B", json!({ "dependsOn": [0] })),
            draft("C", json!({ "dependsOn": [1] })),
        ])
        .to_string(),
    );
    let (fixture, base) = start(fake).await;

    let json = create(
        &fixture,
        &base,
        json!({ "path": SPEC, "selections": selection() }),
    )
    .await
    .json()
    .expect("JSON");

    let drafts = json["drafts"].as_array().expect("Entwürfe");
    assert_eq!(drafts.len(), 3);
    let kanten: usize = drafts
        .iter()
        .map(|draft| draft["dependsOn"].as_array().expect("Verweise").len())
        .sum();
    assert_eq!(kanten, 2);
    assert!(json["dropped"].to_string().contains("Kreis"));
}

#[tokio::test]
async fn liefert_hoechstens_fuenfzehn_entwuerfe() {
    let list: Vec<Value> = (0..20)
        .map(|i| draft(&format!("T{i}"), json!({})))
        .collect();
    let fake = FakeClaude::replying(&Value::Array(list).to_string());
    let (fixture, base) = start(fake).await;

    let json = create(
        &fixture,
        &base,
        json!({ "path": SPEC, "selections": selection() }),
    )
    .await
    .json()
    .expect("JSON");

    assert_eq!(json["drafts"].as_array().expect("Entwürfe").len(), 15);
}

#[tokio::test]
async fn laesst_je_projekt_nur_eine_erzeugung_gleichzeitig_laufen() {
    let fake = FakeClaude::replying(&json!([draft("A", json!({}))]).to_string()).hold_first();
    let (fixture, base) = start(fake.clone()).await;
    let body = json!({ "path": SPEC, "selections": selection() }).to_string();

    let (erste, _) = tokio::join!(async { fixture.server.post(&base, &body).await }, async {
        fake.wait_for_calls(1).await;
        let zweite = fixture.server.post(&base, &body).await;
        assert_eq!(zweite.status, 409);
        assert_eq!(
            zweite.json().expect("JSON")["details"],
            json!({ "reason": "drafts-running" })
        );
        fake.release();
    });

    assert_eq!(erste.status, 200);
    // danach ist dasselbe Projekt wieder frei
    assert_eq!(fixture.server.post(&base, &body).await.status, 200);
}

#[tokio::test]
async fn zerlegt_mit_whole_das_ganze_dokument_und_laesst_bis_zu_vierzig_entwuerfe_zu() {
    let list: Vec<Value> = (0..45)
        .map(|i| draft(&format!("T{i}"), json!({})))
        .collect();
    let fake = FakeClaude::replying(&Value::Array(list).to_string());
    let (fixture, base) = start(fake.clone()).await;

    let res = create(&fixture, &base, json!({ "path": SPEC, "whole": true })).await;

    assert_eq!(res.status, 200);
    let json = res.json().expect("JSON");
    assert_eq!(json["drafts"].as_array().expect("Entwürfe").len(), 40);
    assert!(json["dropped"]
        .as_array()
        .expect("Hinweise")
        .contains(&json!("Nur die ersten 40 von 45 Entwürfen übernommen.")));
    let prompt = fake.last_prompt();
    assert!(prompt.contains("ganzes Dokument"));
    assert!(!prompt.contains("## Markierte Stellen"));
    assert!(prompt.contains("9\tSchritt eins."));
}

#[tokio::test]
async fn bricht_die_laufende_erzeugung_ab_und_gibt_das_projekt_wieder_frei() {
    let fake = FakeClaude::replying(&json!([draft("A", json!({}))]).to_string()).hold_first();
    let (fixture, base) = start(fake.clone()).await;
    let body = json!({ "path": SPEC, "selections": selection() }).to_string();

    let (erste, _) = tokio::join!(async { fixture.server.post(&base, &body).await }, async {
        fake.wait_for_calls(1).await;
        let cancelled = fixture.server.delete(&base).await;
        assert_eq!(cancelled.json(), Some(json!({ "cancelled": true })));
    });

    assert_eq!(erste.code().as_deref(), Some("drafts-cancelled"));
    // Ohne laufende Erzeugung meldet der Abbruch `false`, und das Projekt ist wieder frei
    assert_eq!(
        fixture.server.delete(&base).await.json(),
        Some(json!({ "cancelled": false }))
    );
    assert_eq!(fixture.server.post(&base, &body).await.status, 200);
}

#[tokio::test]
async fn merkt_sich_die_erzeugten_entwuerfe_und_markiert_das_dokument_in_der_liste() {
    let fake = FakeClaude::replying(&json!([draft("A", json!({}))]).to_string());
    let (fixture, base) = start(fake).await;
    let saved = format!("{base}/saved?path={}", encode(SPEC));
    assert_eq!(
        fixture.server.get(&saved).await.json(),
        Some(json!({ "result": null }))
    );

    let made = create(
        &fixture,
        &base,
        json!({ "path": SPEC, "selections": selection() }),
    )
    .await
    .json()
    .expect("JSON");

    assert_eq!(
        fixture.server.get(&saved).await.json().expect("JSON")["result"],
        made
    );
    // Die Docs-Liste markiert das Dokument mit Entwürfen
    let docs = fixture
        .server
        .get(&fixture.base())
        .await
        .json()
        .expect("JSON");
    let eintrag = docs["docs"]
        .as_array()
        .expect("Dokumente")
        .iter()
        .find(|doc| doc["path"] == json!(SPEC))
        .expect("Testspec")
        .clone();
    assert_eq!(eintrag["hasDrafts"], json!(true));
}

#[tokio::test]
async fn verwirft_die_gespeicherten_entwuerfe_sobald_daraus_tickets_angelegt_sind() {
    let fake = FakeClaude::replying(&json!([draft("A", json!({}))]).to_string());
    let (fixture, base) = start(fake).await;
    let made = create(
        &fixture,
        &base,
        json!({ "path": SPEC, "selections": selection() }),
    )
    .await
    .json()
    .expect("JSON");

    let tickets = fixture
        .server
        .post(
            &format!("{}/tickets", fixture.base()),
            &json!({ "path": SPEC, "drafts": made["drafts"] }).to_string(),
        )
        .await;

    assert_eq!(tickets.status, 200);
    assert_eq!(
        fixture
            .server
            .get(&format!("{base}/saved?path={}", encode(SPEC)))
            .await
            .json(),
        Some(json!({ "result": null }))
    );
}

#[tokio::test]
async fn pruft_pfad_und_markierungen_bevor_claude_startet() {
    let fake = FakeClaude::replying("[]");
    let (fixture, base) = start(fake.clone()).await;

    for path in ["../geheim.md", "docs/superpowers/specs/fehlt.md"] {
        let res = create(
            &fixture,
            &base,
            json!({ "path": path, "selections": selection() }),
        )
        .await;
        assert_eq!(res.code().as_deref(), Some("doc-not-found"), "{path}");
    }
    let leer = create(&fixture, &base, json!({ "path": SPEC, "selections": [] })).await;

    assert_eq!(leer.status, 422);
    assert_eq!(leer.code().as_deref(), Some("invalid-input"));
    assert_eq!(fake.calls(), 0, "Claude wurde nicht gestartet");
}

#[tokio::test]
async fn meldet_einen_fehlenden_claude_befehl_und_ein_zeitlimit_als_analysis_failed() {
    let fake = FakeClaude::with_replies(vec![
        RunResult {
            code: Some(127),
            timed_out: false,
            output: String::new(),
        },
        RunResult {
            code: None,
            timed_out: true,
            output: String::new(),
        },
        reply(&json!([draft("A", json!({}))]).to_string(), 0.0),
    ]);
    let (fixture, base) = start(fake).await;
    let body = json!({ "path": SPEC, "selections": selection() });

    let fehlend = create(&fixture, &base, body.clone()).await;
    let zeitlimit = create(&fixture, &base, body.clone()).await;

    assert_eq!(fehlend.status, 422);
    assert!(fehlend.text.contains("nicht gefunden"));
    assert_eq!(zeitlimit.code().as_deref(), Some("analysis-failed"));
    assert!(zeitlimit.text.contains("5 Minuten"));
    // Danach läuft es wieder
    assert_eq!(create(&fixture, &base, body).await.status, 200);
}
