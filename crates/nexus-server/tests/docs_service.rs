//! Lesen, Suchen und Schreiben der Dokumente; portiert aus
//! `packages/server/tests/unit/DocsService.test.ts`.

mod common;

use common::{encode, DocsFixture};
use nexus_core::docs::DocHash;
use serde_json::{json, Value};

const SPEC: &str = "docs/superpowers/specs/2026-10-22-docs-rework-design.md";
const PLAN: &str = "docs/superpowers/plans/2026-10-22-docs-rework.md";

/// Projekt, Board und die freigegebenen Dateien; wie `setup()` in TypeScript
async fn setup(with_repo_path: bool) -> DocsFixture {
    let fixture = DocsFixture::start(with_repo_path).await;
    fixture.write_documents();
    fixture
}

/// Die Dokumente der Liste nach ihrem Pfad
fn by_path(list: &Value) -> std::collections::HashMap<String, Value> {
    list["docs"]
        .as_array()
        .expect("Dokumente")
        .iter()
        .map(|doc| (doc["path"].as_str().unwrap_or_default().to_string(), doc.clone()))
        .collect()
}

/// Enthält `doc` alle Felder von `expected`?
fn matches(doc: &Value, expected: &Value) -> bool {
    expected
        .as_object()
        .expect("Erwartung")
        .iter()
        .all(|(key, value)| doc.get(key) == Some(value))
}

#[tokio::test]
async fn listet_superpowers_specs_und_plane_glossar_und_adrs_aus_dem_repo_des_boards() {
    let fixture = setup(true).await;

    let res = fixture.server.get(&fixture.base()).await;

    assert_eq!(res.status, 200);
    let list = res.json().expect("JSON");
    assert!(matches(
        &list["source"],
        &json!({ "root": fixture.root(), "boardName": "Sprint", "reason": Value::Null })
    ));
    let docs = by_path(&list);
    assert!(matches(
        &docs[SPEC],
        &json!({
            "group": "spec", "kind": "spec", "topic": "docs-rework",
            "date": "2026-10-22", "title": "Docs-Rework", "exists": true,
        })
    ));
    assert!(matches(
        &docs[PLAN],
        &json!({ "group": "plan", "kind": "plan", "topic": "docs-rework", "date": "2026-10-22" })
    ));
    assert!(docs[SPEC]["mtimeMs"].as_f64().unwrap_or_default() > 0.0);
    assert!(matches(
        &docs["GLOSSARY.md"],
        &json!({ "group": "glossary", "kind": Value::Null, "exists": true })
    ));
    assert!(matches(
        &docs["docs/adr/0001-entscheidung.md"],
        &json!({ "group": "adr", "kind": Value::Null })
    ));
    assert!(!docs.contains_key("specs/001-login/spec.md"));
    assert!(!docs.contains_key("geheim.md"));
}

#[tokio::test]
async fn nennt_ohne_board_den_grund_und_liefert_keine_dokumente() {
    let (server, _backend) = common::TestServer::start_with_board().await;
    let (_repo, pid) = server.add_project();

    let res = server.get(&format!("/api/projects/{pid}/docs")).await;

    assert_eq!(res.status, 200);
    assert_eq!(
        res.json(),
        Some(json!({
            "source": { "root": null, "boardId": null, "boardName": null, "reason": "no-board" },
            "docs": [],
        }))
    );
}

#[tokio::test]
async fn nennt_den_grund_wenn_dem_board_der_repo_pfad_fehlt() {
    let fixture = setup(false).await;
    fixture
        .server
        .post(
            &format!("/api/db/boards/{}/activate", fixture.board_id),
            "{}",
        )
        .await;

    let res = fixture.server.get(&fixture.base()).await;

    assert_eq!(res.status, 200);
    let list = res.json().expect("JSON");
    assert!(matches(
        &list["source"],
        &json!({ "root": Value::Null, "boardId": fixture.board_id, "reason": "no-repo-path" })
    ));
    assert_eq!(list["docs"], json!([]));
}

#[tokio::test]
async fn liefert_inhalt_hash_und_abschnitte_freigegebener_dateien() {
    let fixture = setup(true).await;

    let res = fixture
        .server
        .get(&format!("{}/file?path={}", fixture.base(), encode(SPEC)))
        .await;

    assert_eq!(res.status, 200);
    let doc = res.json().expect("JSON");
    assert_eq!(doc["path"], SPEC);
    assert!(doc["content"]
        .as_str()
        .unwrap_or_default()
        .contains("Das Ticket wird angelegt."));
    let hash = doc["hash"].as_str().unwrap_or_default();
    assert_eq!(hash.len(), 64);
    assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
    assert_eq!(
        doc["sections"],
        json!([
            { "heading": "Docs-Rework", "level": 1, "startLine": 1, "endLine": 8 },
            { "heading": "Ziel", "level": 2, "startLine": 5, "endLine": 8 },
        ])
    );
    assert_eq!(
        fixture
            .server
            .get(&format!("{}/file?path=GLOSSARY.md", fixture.base()))
            .await
            .status,
        200
    );
    assert_eq!(
        fixture
            .server
            .get(&format!("{}/file?path={}", fixture.base(), encode(PLAN)))
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn weist_nicht_freigegebene_pfade_und_ausbruchsversuche_beim_lesen_mit_404_ab() {
    let fixture = setup(true).await;

    for path in [
        "README.md",
        "geheim.md",
        "specs/001-login/spec.md",
        "../geheim.md",
        "../../etc/passwd",
        "/etc/passwd",
        "C:/Windows/win.ini",
        "docs/superpowers/specs/../../../geheim.md",
        r"docs\superpowers\specs\..\..\..\geheim.md",
        "%2e%2e/geheim.md",
        ".git/config",
        "docs/adr/../../geheim.md",
        "docs/adr/",
        "docs/superpowers/specs/",
    ] {
        let res = fixture
            .server
            .get(&format!("{}/file?path={}", fixture.base(), encode(path)))
            .await;
        assert_eq!(res.status, 404, "{path}");
        assert!(!res.text.contains("geheim"), "{path}");
    }
}

#[tokio::test]
async fn meldet_eine_freigegebene_aber_fehlende_datei_mit_404() {
    let fixture = setup(true).await;

    let res = fixture
        .server
        .get(&format!(
            "{}/file?path={}",
            fixture.base(),
            encode("docs/superpowers/plans/2026-01-01-fehlt.md")
        ))
        .await;

    assert_eq!(res.status, 404);
    assert!(res.text.contains("2026-01-01-fehlt.md"));
}

#[tokio::test]
async fn verlangt_den_parameter_path() {
    let fixture = setup(true).await;

    assert_eq!(
        fixture
            .server
            .get(&format!("{}/file", fixture.base()))
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn meldet_ohne_quelle_auch_beim_lesen_404() {
    let (server, _backend) = common::TestServer::start_with_board().await;
    let (_repo, pid) = server.add_project();

    let res = server
        .get(&format!("/api/projects/{pid}/docs/file?path=GLOSSARY.md"))
        .await;

    assert_eq!(res.status, 404);
}

#[tokio::test]
async fn sucht_ohne_beachtung_der_schreibweise_mit_datei_zeile_und_auszug() {
    let fixture = setup(true).await;

    let res = fixture
        .server
        .get(&format!("{}/search?q=ticket", fixture.base()))
        .await;

    assert_eq!(res.status, 200);
    let hits = res.json().expect("JSON");
    let mut paths: Vec<String> = hits
        .as_array()
        .expect("Treffer")
        .iter()
        .map(|hit| hit["path"].as_str().unwrap_or_default().to_string())
        .collect();
    paths.sort();
    let mut expected = vec![
        "GLOSSARY.md".to_string(),
        SPEC.to_string(),
        "docs/adr/0001-entscheidung.md".to_string(),
    ];
    expected.sort();
    assert_eq!(paths, expected);
    let spec = hits
        .as_array()
        .expect("Treffer")
        .iter()
        .find(|hit| hit["path"] == SPEC)
        .expect("Treffer in der Spec");
    assert_eq!(spec["line"], 3);
    assert!(spec["excerpt"]
        .as_str()
        .unwrap_or_default()
        .contains("Das Ticket wird angelegt."));
}

#[tokio::test]
async fn durchsucht_keine_nicht_freigegebenen_dateien() {
    let fixture = setup(true).await;

    for query in ["geheim", "Spec-Kit"] {
        let res = fixture
            .server
            .get(&format!("{}/search?q={}", fixture.base(), encode(query)))
            .await;
        assert_eq!(res.json(), Some(json!([])), "{query}");
    }
}

#[tokio::test]
async fn verlangt_mindestens_zwei_zeichen() {
    let fixture = setup(true).await;

    let res = fixture
        .server
        .get(&format!("{}/search?q=a", fixture.base()))
        .await;

    assert_eq!(res.status, 422);
    assert_eq!(res.code().as_deref(), Some("query-short"));
}

/// Projekt, Board und die freigegebenen Dateien der Schreib-Tests; die Spec hat dort eine
/// andere Einleitung als beim Lesen
async fn setup_write() -> DocsFixture {
    let fixture = setup(true).await;
    fixture.write(
        "docs/superpowers/specs",
        "2026-10-22-docs-rework-design.md",
        "# Docs-Rework\n\nEinleitung.\n\n## Ziel\n\nEine Quelle.\n",
    );
    fixture
}

#[tokio::test]
async fn ersetzt_einen_abschnitt_und_liefert_neuen_hash_samt_abschnitten() {
    let fixture = setup_write().await;
    let hash = fixture.hash_of(SPEC).await;

    let res = fixture
        .server
        .put(
            &format!("{}/section", fixture.base()),
            &json!({
                "path": SPEC, "startLine": 5, "endLine": 7,
                "content": "## Neues Ziel\n\nZwei Quellen.", "expectedHash": hash,
            })
            .to_string(),
        )
        .await;

    assert_eq!(res.status, 200);
    let after = fixture.read(SPEC);
    assert_eq!(
        after,
        "# Docs-Rework\n\nEinleitung.\n\n## Neues Ziel\n\nZwei Quellen.\n"
    );
    let result = res.json().expect("JSON");
    assert_eq!(result["path"], SPEC);
    assert_eq!(result["hash"], DocHash::of(&after));
    assert_eq!(
        result["sections"],
        json!([
            { "heading": "Docs-Rework", "level": 1, "startLine": 1, "endLine": 8 },
            { "heading": "Neues Ziel", "level": 2, "startLine": 5, "endLine": 8 },
        ])
    );
}

#[tokio::test]
async fn lasst_von_zwei_gleichzeitigen_anderungen_gegen_denselben_stand_nur_eine_durch() {
    let fixture = setup_write().await;
    let hash = fixture.hash_of(SPEC).await;
    let url = format!("{}/section", fixture.base());
    let body = |content: &str| {
        json!({
            "path": SPEC, "startLine": 5, "endLine": 7,
            "content": content, "expectedHash": hash,
        })
        .to_string()
    };

    let (first, second) = (body("## A"), body("## B"));
    let (a, b) = tokio::join!(
        fixture.server.put(&url, &first),
        fixture.server.put(&url, &second),
    );

    let mut status = [a.status.as_u16(), b.status.as_u16()];
    status.sort_unstable();
    assert_eq!(status, [200, 409]);
    let winner = if a.status == 200 { "## A" } else { "## B" };
    assert_eq!(
        fixture.read(SPEC),
        format!("# Docs-Rework\n\nEinleitung.\n\n{winner}\n")
    );
}

#[tokio::test]
async fn lehnt_einen_veralteten_hash_mit_409_ab_und_lasst_die_datei_unverandert() {
    let fixture = setup_write().await;
    let before = fixture.read(SPEC);

    let res = fixture
        .server
        .put(
            &format!("{}/section", fixture.base()),
            &json!({
                "path": SPEC, "startLine": 5, "endLine": 7, "content": "## Anders",
                "expectedHash": DocHash::of("ganz andere Fassung"),
            })
            .to_string(),
        )
        .await;

    assert_eq!(res.status, 409);
    assert_eq!(res.code().as_deref(), Some("doc-changed"));
    assert_eq!(
        res.json().expect("JSON")["details"]["hash"],
        DocHash::of(&before)
    );
    assert_eq!(fixture.read(SPEC), before);
}

#[tokio::test]
async fn lehnt_ungultige_zeilenbereiche_mit_400_ab() {
    let fixture = setup_write().await;
    let before = fixture.read(SPEC);
    let hash = fixture.hash_of(SPEC).await;

    for range in [
        (json!(0), json!(2)),
        (json!(5), json!(4)),
        (json!(1), json!(99)),
        (json!(-1), json!(1)),
        (json!(1.5), json!(2)),
    ] {
        let body = json!({
            "path": SPEC, "startLine": range.0, "endLine": range.1,
            "content": "x", "expectedHash": hash,
        });
        let res = fixture
            .server
            .put(&format!("{}/section", fixture.base()), &body.to_string())
            .await;
        assert_eq!(res.status, 400, "{body}");
        assert_eq!(res.code().as_deref(), Some("doc-range-invalid"), "{body}");
    }
    assert_eq!(fixture.read(SPEC), before);
}

#[tokio::test]
async fn lehnt_einen_unvollstandigen_body_mit_422_ab() {
    let fixture = setup_write().await;
    let before = fixture.read(SPEC);
    let hash = fixture.hash_of(SPEC).await;

    for body in [
        json!({}),
        json!({ "path": SPEC, "startLine": 1, "endLine": 2, "content": "x" }),
        json!({ "startLine": 1, "endLine": 2, "content": "x", "expectedHash": hash }),
        json!({ "path": SPEC, "startLine": 1, "endLine": 2, "expectedHash": hash }),
    ] {
        let res = fixture
            .server
            .put(&format!("{}/section", fixture.base()), &body.to_string())
            .await;
        assert_eq!(res.status, 422, "{body}");
        assert_eq!(res.code().as_deref(), Some("request-invalid"), "{body}");
    }
    assert_eq!(fixture.read(SPEC), before);
}

#[tokio::test]
async fn erlaubt_einen_leeren_inhalt_abschnitt_loschen() {
    let fixture = setup_write().await;
    let hash = fixture.hash_of(PLAN).await;

    let res = fixture
        .server
        .put(
            &format!("{}/section", fixture.base()),
            &json!({
                "path": PLAN, "startLine": 3, "endLine": 3, "content": "", "expectedHash": hash,
            })
            .to_string(),
        )
        .await;

    assert_eq!(res.status, 200);
    assert_eq!(fixture.read(PLAN), "# Plan Docs-Rework\n\n\n");
}

#[tokio::test]
async fn lasst_glossar_und_adrs_nicht_schreiben() {
    let fixture = setup_write().await;

    for path in ["GLOSSARY.md", "docs/adr/0001-entscheidung.md"] {
        let before = fixture.read(path);
        let hash = fixture.hash_of(path).await;
        let res = fixture
            .server
            .put(
                &format!("{}/section", fixture.base()),
                &json!({
                    "path": path, "startLine": 1, "endLine": 1,
                    "content": "# Neu", "expectedHash": hash,
                })
                .to_string(),
            )
            .await;
        assert_eq!(res.status, 422, "{path}");
        assert_eq!(res.code().as_deref(), Some("doc-not-writable"), "{path}");
        assert_eq!(fixture.read(path), before, "{path}");
    }
}

#[tokio::test]
async fn weist_nicht_freigegebene_pfade_und_ausbruchsversuche_beim_schreiben_mit_404_ab() {
    let fixture = setup_write().await;

    for path in [
        "geheim.md",
        "../geheim.md",
        "docs/superpowers/specs/../../../geheim.md",
        r"docs\superpowers\specs\x.md",
        "/etc/passwd",
        "docs/superpowers/specs/fehlt.md",
    ] {
        let res = fixture
            .server
            .put(
                &format!("{}/section", fixture.base()),
                &json!({
                    "path": path, "startLine": 1, "endLine": 1,
                    "content": "x", "expectedHash": "egal",
                })
                .to_string(),
            )
            .await;
        assert_eq!(res.status, 404, "{path}");
        assert_eq!(res.code().as_deref(), Some("doc-not-found"), "{path}");
    }
    assert_eq!(fixture.read("geheim.md"), "geheim");
}

#[tokio::test]
async fn behalt_crlf_als_zeilenende_der_datei_bei() {
    let fixture = setup_write().await;
    fixture.write(
        "docs/superpowers/plans",
        "2026-10-22-docs-rework.md",
        "# Plan\r\n\r\nAlt.\r\n",
    );
    let hash = fixture.hash_of(PLAN).await;

    let res = fixture
        .server
        .put(
            &format!("{}/section", fixture.base()),
            &json!({
                "path": PLAN, "startLine": 3, "endLine": 3, "content": "Neu.",
                "expectedHash": hash,
            })
            .to_string(),
        )
        .await;

    assert_eq!(res.status, 200);
    let after = fixture.read(PLAN);
    assert_eq!(after, "# Plan\r\n\r\nNeu.\r\n");
    assert!(!after.replace("\r\n", "").contains('\n'));
}
