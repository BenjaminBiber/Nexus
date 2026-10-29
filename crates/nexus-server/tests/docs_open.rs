//! Ein Dokument im Explorer oder in VS Code öffnen; portiert aus
//! `packages/server/tests/unit/DocsOpen.test.ts`.

mod common;

use std::sync::Arc;

use common::{platform_with_opener, DocsFixture, TestServerOptions};
use nexus_platform::{FakeFileOpener, OpenTarget};
use serde_json::json;

const SPEC: &str = "docs/superpowers/specs/2026-10-22-docs-rework-design.md";

/// Server und Board mit Repo-Pfad, die Spec und ein Öffner, dessen Aufrufe der Test nachsieht
async fn start() -> (DocsFixture, Arc<FakeFileOpener>) {
    let (platform, opener) = platform_with_opener();
    let fixture = DocsFixture::start_with(
        TestServerOptions {
            platform: Some(platform),
            ..TestServerOptions::default()
        },
        true,
    )
    .await;
    fixture.write(
        "docs/superpowers/specs",
        "2026-10-22-docs-rework-design.md",
        "# Spec\n",
    );
    (fixture, opener)
}

/// Die bisher geöffneten Dateien
fn opened(opener: &FakeFileOpener) -> Vec<(std::path::PathBuf, OpenTarget)> {
    opener.opened.lock().expect("Öffner").clone()
}

#[tokio::test]
async fn offnet_die_datei_mit_vollem_pfad_im_gewunschten_programm() {
    let (fixture, opener) = start().await;
    let url = format!("{}/open", fixture.base());

    for target in ["explorer", "vscode"] {
        let res = fixture
            .server
            .post(&url, &json!({ "path": SPEC, "target": target }).to_string())
            .await;
        assert_eq!(res.status, 200, "{target}");
    }

    let file = fixture.repo.path().join(SPEC);
    assert_eq!(
        opened(&opener),
        vec![
            (file.clone(), OpenTarget::Explorer),
            (file, OpenTarget::Vscode),
        ]
    );
}

#[tokio::test]
async fn offnet_nichts_ausserhalb_der_freigegebenen_dokumente_und_kein_unbekanntes_ziel() {
    let (fixture, opener) = start().await;
    let url = format!("{}/open", fixture.base());

    let forbidden = fixture
        .server
        .post(
            &url,
            &json!({ "path": "../geheim.md", "target": "explorer" }).to_string(),
        )
        .await;
    assert_eq!(forbidden.code().as_deref(), Some("doc-not-found"));

    let unknown = fixture
        .server
        .post(
            &url,
            &json!({ "path": SPEC, "target": "notepad" }).to_string(),
        )
        .await;
    assert_eq!(unknown.code().as_deref(), Some("request-invalid"));

    assert!(opened(&opener).is_empty());
}
