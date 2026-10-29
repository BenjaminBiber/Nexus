//! Kommentare zu Dokumenten; portiert aus
//! `packages/server/tests/unit/DocComments.test.ts`.

mod common;

use common::{encode, DocsFixture};
use serde_json::{json, Value};

const SPEC: &str = "docs/superpowers/specs/2026-10-22-login-design.md";

/// Der Körper eines neuen Kommentars (wie `BODY` in TypeScript)
fn body() -> Value {
    json!({
        "text": "Bitte präzisieren", "quote": "Das Ticket",
        "heading": "Login", "lines": [3, 3],
    })
}

/// Projekt, Board und die Spec der Kommentar-Tests
async fn setup(with_repo_path: bool) -> DocsFixture {
    let fixture = DocsFixture::start(with_repo_path).await;
    fixture.write(
        "docs/superpowers/specs",
        "2026-10-22-login-design.md",
        "# Login\n\nDas Ticket wird angelegt.\n",
    );
    fixture.write(".", "geheim.md", "geheim");
    fixture
}

/// Adresse der Kommentare eines Dokuments, mit oder ohne Kommentarnummer
fn url(fixture: &DocsFixture, path: &str, id: &str) -> String {
    let suffix = if id.is_empty() {
        String::new()
    } else {
        format!("/{id}")
    };
    format!("{}/comments{suffix}?path={}", fixture.base(), encode(path))
}

#[tokio::test]
async fn legt_kommentare_an_liest_andert_und_loscht_sie() {
    let fixture = setup(true).await;
    let list = url(&fixture, SPEC, "");

    assert_eq!(fixture.server.get(&list).await.json(), Some(json!([])));

    let created = fixture.server.post(&list, &body().to_string()).await;
    assert_eq!(created.status, 200);
    let comment = created.json().expect("JSON");
    assert_eq!(comment["text"], "Bitte präzisieren");
    assert_eq!(comment["quote"], "Das Ticket");
    assert_eq!(comment["heading"], "Login");
    assert_eq!(comment["lines"], json!([3, 3]));
    assert_eq!(comment["status"], "open");
    let id = comment["id"].as_str().expect("Nummer").to_string();
    assert!(fixture
        .repo
        .path()
        .join("docs/superpowers/specs/2026-10-22-login-design.comments.json")
        .exists());

    let patched = fixture
        .server
        .patch(&url(&fixture, SPEC, &id), r#"{"status":"resolved"}"#)
        .await;
    assert_eq!(patched.json().expect("JSON")["status"], "resolved");

    let removed = fixture.server.delete(&url(&fixture, SPEC, &id)).await;
    assert_eq!(removed.json(), Some(json!({ "ok": true })));
    assert_eq!(fixture.server.get(&list).await.json(), Some(json!([])));
}

#[tokio::test]
async fn weist_nicht_freigegebene_pfade_ab_und_schreibt_dort_nichts() {
    let fixture = setup(true).await;

    for path in ["geheim.md", "../geheim.md", "specs/001-login/spec.md"] {
        let res = fixture
            .server
            .post(&url(&fixture, path, ""), &body().to_string())
            .await;
        assert_eq!(res.status, 404, "{path}");
    }
    assert!(!fixture.repo.path().join("geheim.comments.json").exists());
}

#[tokio::test]
async fn weist_ohne_board_alle_vier_zugriffe_ab_und_legt_keine_datei_an() {
    let fixture = setup(false).await;

    assert_eq!(
        fixture.server.get(&url(&fixture, SPEC, "")).await.status,
        404
    );
    assert_eq!(
        fixture
            .server
            .post(&url(&fixture, SPEC, ""), &body().to_string())
            .await
            .status,
        404
    );
    assert_eq!(
        fixture
            .server
            .patch(&url(&fixture, SPEC, "K-00000000"), r#"{"text":"x"}"#)
            .await
            .status,
        404
    );
    assert_eq!(
        fixture
            .server
            .delete(&url(&fixture, SPEC, "K-00000000"))
            .await
            .status,
        404
    );
    let mut names: Vec<String> =
        std::fs::read_dir(fixture.repo.path().join("docs/superpowers/specs"))
            .expect("Ordner")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
    names.sort();
    assert_eq!(names, vec!["2026-10-22-login-design.md".to_string()]);
}

#[tokio::test]
async fn verlangt_text_und_zitat() {
    let fixture = setup(true).await;
    let mut invalid = body();
    invalid["text"] = json!("  ");

    let res = fixture
        .server
        .post(&url(&fixture, SPEC, ""), &invalid.to_string())
        .await;

    assert_eq!(res.status, 422);
    assert_eq!(res.code().as_deref(), Some("comment-invalid"));
}

#[tokio::test]
async fn meldet_unbekannte_kommentare_mit_404() {
    let fixture = setup(true).await;

    let res = fixture
        .server
        .patch(&url(&fixture, SPEC, "K-00000000"), r#"{"text":"x"}"#)
        .await;

    assert_eq!(res.status, 404);
}
