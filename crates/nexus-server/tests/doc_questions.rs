//! Fragen aus einem Dokument (`POST`/`DELETE …/docs/question-drafts`, `POST …/docs/questions`);
//! portiert aus `packages/server/tests/unit/DocQuestions.test.ts`.
//!
//! Claude wird nie wirklich gestartet: der Server bekommt die Attrappe
//! [`common::FakeClaude`].

mod common;

use std::sync::Arc;

use common::{fake_runners, DocsFixture, FakeClaude, Reply, TestServerOptions};
use serde_json::{json, Value};

const SPEC: &str = "docs/superpowers/specs/2026-10-22-docs-rework-design.md";

/// Ein Fragen-Entwurf, wie ihn `docs/question-drafts` liefert
fn question(title: &str, extra: Value) -> Value {
    let mut item = json!({
        "title": title,
        "description": format!("Kontext zu {title}"),
        "source": { "heading": "Ziel", "startLine": 3, "endLine": 5 },
    });
    for (key, value) in extra.as_object().cloned().unwrap_or_default() {
        item[key] = value;
    }
    item
}

/// Server, Projekt, Board mit Repo-Pfad und die Testspec; `fake` spielt Claude
async fn start(fake: Arc<FakeClaude>) -> DocsFixture {
    let options = TestServerOptions {
        claude: Some(fake_runners(fake)),
        ..TestServerOptions::default()
    };
    let fixture = DocsFixture::start_with(options, true).await;
    fixture.write(
        "docs/superpowers/specs",
        "2026-10-22-docs-rework-design.md",
        "# Docs-Rework\n\n## Ziel\n\nWelcher Speicher? TBD.\n\n## Plan\n\nSchritt eins.\n",
    );
    fixture
}

/// Lässt Claude die offenen Fragen herauslesen
async fn drafts(fixture: &DocsFixture) -> Reply {
    fixture
        .server
        .post(
            &format!("{}/question-drafts", fixture.base()),
            &json!({ "path": SPEC }).to_string(),
        )
        .await
}

/// Legt die gewählten Entwürfe als Fragen an
async fn create(fixture: &DocsFixture, list: Value) -> Reply {
    fixture
        .server
        .post(
            &format!("{}/questions", fixture.base()),
            &json!({ "path": SPEC, "drafts": list }).to_string(),
        )
        .await
}

/// Eine angelegte Frage des Boards. Die Fragen-Routen gehören zur Gruppe `questions` und sind
/// noch nicht in Rust; gelesen wird deshalb über die Dienste der Board-Laufzeit.
async fn question_of(fixture: &DocsFixture, id: &str) -> Value {
    let detail = fixture
        .backend
        .services
        .questions
        .get(id)
        .await
        .expect("Frage");
    serde_json::to_value(detail).expect("JSON")
}

#[tokio::test]
async fn liefert_gepruefte_entwuerfe_mit_quelle_und_kosten_und_nennt_die_zeilennummern() {
    let answer = json!([
        question("Welcher Speicher?", json!({})),
        question("", json!({})),
        question(
            "Wie viele?",
            json!({ "source": { "heading": "Plan", "startLine": 99, "endLine": 120 } })
        ),
    ]);
    let fake = FakeClaude::with_replies(vec![common::reply(&answer.to_string(), 0.01)]);
    let fixture = start(fake.clone()).await;

    let res = drafts(&fixture).await;

    assert_eq!(res.status, 200);
    let json = res.json().expect("JSON");
    let titel: Vec<&str> = json["drafts"]
        .as_array()
        .expect("Entwürfe")
        .iter()
        .filter_map(|draft| draft["title"].as_str())
        .collect();
    assert_eq!(titel, ["Welcher Speicher?", "Wie viele?"]);
    assert_eq!(
        json["drafts"][0]["source"],
        json!({ "heading": "Ziel", "startLine": 3, "endLine": 5 })
    );
    // Ein Zeilenbereich außerhalb des Dokuments wird zur ersten Zeile
    assert_eq!(
        json["drafts"][1]["source"],
        json!({ "heading": "Plan", "startLine": 1, "endLine": 1 })
    );
    assert_eq!(json["dropped"].as_array().expect("Hinweise").len(), 1);
    assert_eq!(json["costUsd"], json!(0.01));
    assert!(fake.last_prompt().contains("5\tWelcher Speicher? TBD."));
}

#[tokio::test]
async fn liefert_ohne_offene_fragen_eine_leere_liste() {
    let fixture = start(FakeClaude::replying("[]")).await;

    let json = drafts(&fixture).await.json().expect("JSON");

    assert_eq!(json["drafts"], json!([]));
}

#[tokio::test]
async fn meldet_eine_unlesbare_antwort() {
    let fixture = start(FakeClaude::replying("Keine Fragen gefunden.")).await;

    let res = drafts(&fixture).await;

    assert_eq!(res.status, 422);
    assert_eq!(res.code().as_deref(), Some("analysis-failed"));
}

#[tokio::test]
async fn legt_fragen_mit_quelle_und_anlass_an_und_verknuepft_die_tickets_derselben_ueberschrift() {
    let fixture = start(FakeClaude::replying("[]")).await;
    let ticket = fixture
        .server
        .post(
            &format!("{}/tickets", fixture.base()),
            &json!({
                "path": SPEC,
                "drafts": [{ "title": "Speicher bauen", "description": "B", "acceptance": "A",
                             "steps": [], "dependsOn": [], "labels": [],
                             "source": { "heading": "Ziel", "startLine": 3, "endLine": 5 } }],
            })
            .to_string(),
        )
        .await
        .json()
        .expect("JSON");
    let ticket_id = ticket["created"][0]["id"].as_str().expect("Ticket-ID");

    let res = create(
        &fixture,
        json!([
            question("Welcher Speicher?", json!({})),
            { "title": "" },
            question(
                "Wie viele Schritte?",
                json!({ "source": { "heading": "Plan", "startLine": 7, "endLine": 9 } })
            ),
        ]),
    )
    .await;

    assert_eq!(res.status, 200);
    let json = res.json().expect("JSON");
    let indizes: Vec<i64> = json["created"]
        .as_array()
        .expect("Angelegte")
        .iter()
        .filter_map(|item| item["index"].as_i64())
        .collect();
    assert_eq!(indizes, [0, 2]);
    assert_eq!(
        json["skipped"],
        json!([{ "index": 1, "reason": "draft-invalid" }])
    );

    let first = question_of(&fixture, json["created"][0]["id"].as_str().expect("ID")).await;
    assert_eq!(first["title"], json!("Welcher Speicher?"));
    assert_eq!(
        first["description"],
        json!(format!(
            "Kontext zu Welcher Speicher?\n\nQuelle: {SPEC} › Ziel (Z. 3–5)"
        ))
    );
    assert_eq!(first["occasion"], json!(format!("Dokument {SPEC}")));
    assert_eq!(first["state"], json!("open"));
    let tickets: Vec<&str> = first["tickets"]
        .as_array()
        .expect("Tickets")
        .iter()
        .filter_map(|ticket| ticket["id"].as_str())
        .collect();
    assert_eq!(tickets, [ticket_id]);
    let second = question_of(&fixture, json["created"][1]["id"].as_str().expect("ID")).await;
    assert_eq!(second["tickets"], json!([]));
}

#[tokio::test]
async fn entscheidet_fragen_mit_antwort_gleich_fragen_ohne_antwort_bleiben_offen() {
    let fixture = start(FakeClaude::replying("[]")).await;

    let json = create(
        &fixture,
        json!([
            question(
                "Welcher Speicher?",
                json!({ "answer": "  SQLite lokal, MSSQL im Team.  " })
            ),
            question("Wie viele?", json!({ "answer": "   " })),
        ]),
    )
    .await
    .json()
    .expect("JSON");

    assert_eq!(json["created"][0]["answered"], json!(true));
    assert_eq!(json["created"][1]["answered"], json!(false));
    let answered = question_of(&fixture, json["created"][0]["id"].as_str().expect("ID")).await;
    assert_eq!(answered["state"], json!("answered"));
    assert_eq!(answered["decision"]["outcome"], json!("answered"));
    assert_eq!(
        answered["decision"]["text"],
        json!("SQLite lokal, MSSQL im Team.")
    );
    assert_eq!(answered["decision"]["option"], Value::Null);
    let open = question_of(&fixture, json["created"][1]["id"].as_str().expect("ID")).await;
    assert_eq!(open["state"], json!("open"));
}

#[tokio::test]
async fn uebernimmt_keine_antwort_aus_claudes_entwuerfen() {
    let answer = json!([question(
        "Welcher Speicher?",
        json!({ "answer": "erfunden" })
    )]);
    let fixture = start(FakeClaude::replying(&answer.to_string())).await;

    let json = drafts(&fixture).await.json().expect("JSON");

    assert!(json["drafts"][0].get("answer").is_none());
}

#[tokio::test]
async fn verlangt_mindestens_einen_entwurf() {
    let fixture = start(FakeClaude::replying("[]")).await;

    let res = create(&fixture, json!([])).await;

    assert_eq!(res.status, 422);
    assert_eq!(res.code().as_deref(), Some("invalid-input"));
}

#[tokio::test]
async fn bricht_eine_laufende_erzeugung_ab_und_laesst_keine_zweite_daneben_zu() {
    let fake = FakeClaude::replying("[]").hold_first();
    let fixture = start(fake.clone()).await;

    let (erste, _) = tokio::join!(drafts(&fixture), async {
        fake.wait_for_calls(1).await;
        let zweite = drafts(&fixture).await;
        assert_eq!(zweite.code().as_deref(), Some("conflict"));
        assert_eq!(
            zweite.json().expect("JSON")["details"],
            json!({ "reason": "question-drafts-running" })
        );
        let cancelled = fixture
            .server
            .delete(&format!("{}/question-drafts", fixture.base()))
            .await;
        assert_eq!(cancelled.json(), Some(json!({ "cancelled": true })));
    });

    assert_eq!(erste.code().as_deref(), Some("drafts-cancelled"));
    assert_eq!(
        fixture
            .server
            .delete(&format!("{}/question-drafts", fixture.base()))
            .await
            .json(),
        Some(json!({ "cancelled": false }))
    );
}
