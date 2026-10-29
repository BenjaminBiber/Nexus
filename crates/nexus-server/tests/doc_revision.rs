//! Docs umschreiben (`POST …/docs/revise`); portiert aus
//! `packages/server/tests/unit/docs/DocRevision.test.ts`.
//!
//! Claude wird nie wirklich gestartet: der Server bekommt die Attrappe
//! [`common::FakeClaude`].

mod common;

use std::sync::Arc;

use common::{fake_runners, reply, DocsFixture, FakeClaude, Reply, TestServerOptions};
use nexus_core::docs::DocHash;
use nexus_server::dispatch::RunResult;
use serde_json::{json, Value};

const SPEC: &str = "docs/superpowers/specs/2026-10-22-docs-rework-design.md";
const CONTENT: &str =
    "# Docs-Rework\n\nEinleitung.\n\n## Ziel\n\nEine Quelle.\nZweite Zeile.\n\n## Offen\n\nNichts.\n";

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
        CONTENT,
    );
    fixture
}

/// Körper mit gültigem Fingerabdruck für die Zeilen „Eine Quelle.“ und „Zweite Zeile.“
fn body(over: Value) -> Value {
    let mut request = json!({
        "path": SPEC, "startLine": 7, "endLine": 8,
        "instruction": "Kürzer fassen.", "expectedHash": DocHash::of(CONTENT),
    });
    for (key, value) in over.as_object().cloned().unwrap_or_default() {
        request[key] = value;
    }
    request
}

/// Schreibt den Bereich um
async fn revise(fixture: &DocsFixture, body: Value) -> Reply {
    fixture
        .server
        .post(&format!("{}/revise", fixture.base()), &body.to_string())
        .await
}

#[tokio::test]
async fn schickt_abschnitt_bereich_und_anweisung_an_claude_und_liefert_die_antwort() {
    let fake = FakeClaude::replying("Eine einzige Quelle.");
    let fixture = start(fake.clone()).await;

    let res = revise(&fixture, body(json!({}))).await;

    assert_eq!(res.status, 200);
    assert_eq!(
        res.json(),
        Some(json!({
            "original": "Eine Quelle.\nZweite Zeile.",
            "proposal": "Eine einzige Quelle.",
            "startLine": 7, "endLine": 8, "hash": DocHash::of(CONTENT),
        }))
    );
    let prompt = fake.last_prompt();
    assert!(prompt.contains("Eine Quelle.\nZweite Zeile."));
    assert!(prompt.contains("Kürzer fassen."));
    // umgebender Abschnitt „## Ziel“ als Kontext, aber nicht der Abschnitt „## Offen“
    assert!(prompt.contains("## Ziel"));
    assert!(!prompt.contains("## Offen"));
    let args = fake.last_args();
    assert!(args.windows(2).any(|paar| paar == ["--max-turns", "1"]));
    assert!(args
        .windows(2)
        .any(|paar| paar == ["--disallowedTools", "Bash"]));
    // Die Route schreibt nichts
    assert_eq!(fixture.read(SPEC), CONTENT);
}

#[tokio::test]
async fn entfernt_einen_markdown_zaun_der_die_ganze_antwort_umschliesst() {
    let fixture = start(FakeClaude::replying(
        "```markdown\nEine einzige Quelle.\n```\n",
    ))
    .await;

    let res = revise(&fixture, body(json!({}))).await;

    assert_eq!(res.status, 200);
    assert_eq!(
        res.json().expect("JSON")["proposal"],
        json!("Eine einzige Quelle.")
    );
}

#[tokio::test]
async fn antwortet_auf_einen_veralteten_fingerabdruck_mit_409() {
    let fixture = start(FakeClaude::replying("x")).await;

    let res = revise(&fixture, body(json!({ "expectedHash": "veraltet" }))).await;

    assert_eq!(res.status, 409);
    assert_eq!(res.code().as_deref(), Some("doc-changed"));
    assert_eq!(
        res.json().expect("JSON")["details"]["hash"],
        json!(DocHash::of(CONTENT))
    );
}

#[tokio::test]
async fn meldet_einen_fehlenden_claude_befehl_als_analysis_failed() {
    let fixture = start(FakeClaude::with_replies(vec![RunResult {
        code: Some(127),
        timed_out: false,
        output: String::new(),
    }]))
    .await;

    let res = revise(&fixture, body(json!({}))).await;

    assert_eq!(res.status, 422);
    assert_eq!(res.code().as_deref(), Some("analysis-failed"));
    assert!(res.text.contains("nicht gefunden"));
}

#[tokio::test]
async fn meldet_einen_zeituberschritt_als_analysis_failed() {
    let fixture = start(FakeClaude::with_replies(vec![RunResult {
        code: None,
        timed_out: true,
        output: String::new(),
    }]))
    .await;

    let res = revise(&fixture, body(json!({}))).await;

    assert_eq!(res.status, 422);
    assert!(res.text.contains("5 Minuten"));
}

#[tokio::test]
async fn meldet_einen_exit_code_ungleich_null_mit_auszug() {
    let fixture = start(FakeClaude::with_replies(vec![RunResult {
        code: Some(3),
        timed_out: false,
        output: "kaputt im Modell".into(),
    }]))
    .await;

    let res = revise(&fixture, body(json!({}))).await;

    assert_eq!(res.status, 422);
    assert!(res.text.contains("Exit-Code 3"));
    assert!(res.text.contains("kaputt im Modell"));
}

#[tokio::test]
async fn meldet_eine_leere_antwort_als_analysis_failed() {
    let fixture = start(FakeClaude::with_replies(vec![reply("   \n", 0.01)])).await;

    let res = revise(&fixture, body(json!({}))).await;

    assert_eq!(res.status, 422);
    assert_eq!(res.code().as_deref(), Some("analysis-failed"));
}

#[tokio::test]
async fn weist_einen_bereich_hinter_dem_dateiende_zurueck() {
    let fixture = start(FakeClaude::replying("x")).await;

    let res = revise(&fixture, body(json!({ "startLine": 7, "endLine": 999 }))).await;

    assert_eq!(res.status, 400);
    assert_eq!(res.code().as_deref(), Some("doc-range-invalid"));
}

#[tokio::test]
async fn weist_startzeile_null_schon_im_korper_zurueck() {
    let fixture = start(FakeClaude::replying("x")).await;

    let res = revise(&fixture, body(json!({ "startLine": 0, "endLine": 2 }))).await;

    assert_eq!(res.status, 400);
    assert_eq!(res.code().as_deref(), Some("doc-range-invalid"));
}

#[tokio::test]
async fn verlangt_pfad_hash_und_anweisung() {
    let fixture = start(FakeClaude::replying("x")).await;

    let res = revise(&fixture, body(json!({ "instruction": "  " }))).await;

    assert_eq!(res.status, 422);
    assert_eq!(res.code().as_deref(), Some("request-invalid"));
}

#[tokio::test]
async fn antwortet_auf_eine_zweite_anfrage_waehrend_eines_laufs_mit_409() {
    let fake = FakeClaude::replying("fertig").hold_first();
    let fixture = start(fake.clone()).await;

    let (erste, _) = tokio::join!(revise(&fixture, body(json!({}))), async {
        // warten, bis der erste Lauf wirklich bei Claude angekommen ist
        fake.wait_for_calls(1).await;
        let zweite = revise(&fixture, body(json!({}))).await;
        assert_eq!(zweite.status, 409);
        assert_eq!(
            zweite.json().expect("JSON")["details"]["reason"],
            json!("revise-running")
        );
        fake.release();
    });

    assert_eq!(erste.status, 200);
}

#[tokio::test]
async fn gibt_gesperrte_und_ausgebrochene_pfade_als_nicht_gefunden_aus() {
    let fixture = start(FakeClaude::replying("x")).await;

    for path in ["GLOSSARY.md", "../geheim.md"] {
        let res = revise(&fixture, body(json!({ "path": path }))).await;

        assert_eq!(res.status, 404, "{path}");
        assert_eq!(res.code().as_deref(), Some("doc-not-found"), "{path}");
    }
}
