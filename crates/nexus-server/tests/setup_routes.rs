//! Routengruppe `setup`: Plugin-Zustand und -Installation sowie die Migration alter
//! Harness-Repos. Claude ist immer eine Attrappe, alle Ordner liegen in Temp-Verzeichnissen.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use common::{DocsFixture, TestServer, TestServerOptions};
use nexus_core::install::{ClaudeCli, ClaudeOutput, LEGACY_BOARD_RULE_PATH, LEGACY_RULE_MARKER};
use nexus_server::setup::SetupContext;
use serde_json::json;

/// Claude, das jeden Aufruf mit Erfolg beantwortet und ihn sich merkt
#[derive(Debug, Default)]
struct Recorder {
    calls: Mutex<Vec<String>>,
}

impl ClaudeCli for Recorder {
    fn run(&self, args: &[&str], _cwd: Option<&Path>) -> ClaudeOutput {
        self.calls.lock().unwrap().push(args.join(" "));
        ClaudeOutput {
            ok: true,
            out: String::new(),
        }
    }
}

/// Gebautes Plugin in einem eigenen Temp-Ordner
fn bundle(dir: &Path) -> PathBuf {
    let root = dir.join("bundle");
    fs::create_dir_all(root.join(".claude-plugin")).unwrap();
    fs::write(
        root.join(".claude-plugin/plugin.json"),
        r#"{"version":"0.1.5"}"#,
    )
    .unwrap();
    root
}

fn temp() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("nexus-setup-")
        .tempdir()
        .unwrap()
}

#[tokio::test]
async fn meldet_ohne_bundle_und_ohne_claude_ein_fehlendes_plugin() {
    let server = TestServer::start();

    let reply = server.get("/api/setup/plugin").await;

    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.json(),
        Some(json!({
            "bundled": false, "claude": false, "state": "missing",
            "installedVersion": null, "bundledVersion": null,
        }))
    );
}

#[tokio::test]
async fn installiert_das_mitgelieferte_plugin_mit_claude() {
    let dir = temp();
    let claude = Arc::new(Recorder::default());
    let server = TestServer::with(TestServerOptions {
        plugin_bundle: Some(bundle(dir.path())),
        setup: Some(SetupContext::new(
            claude.clone(),
            Some(dir.path().join("claude")),
            dir.path().to_path_buf(),
        )),
        ..TestServerOptions::default()
    });

    let reply = server.post("/api/setup/plugin", "{}").await;

    assert_eq!(reply.status, 200);
    assert_eq!(reply.body()["ok"], true);
    assert!(server
        .home
        .plugin_dir()
        .join("nexus/.claude-plugin/plugin.json")
        .is_file());
    let calls = claude.calls.lock().unwrap().clone();
    assert!(
        calls.contains(&"plugin install nexus@nexus-local".to_string()),
        "{calls:?}"
    );
}

#[tokio::test]
async fn ohne_bundle_ist_die_installation_ein_konflikt() {
    let server = TestServer::start();

    let reply = server.post("/api/setup/plugin", "{}").await;

    assert_eq!(reply.status, 409);
    assert_eq!(reply.code().as_deref(), Some("plugin-not-bundled"));
}

#[tokio::test]
async fn ein_zweiter_gleichzeitiger_lauf_ist_ein_konflikt() {
    let dir = temp();
    let server = TestServer::with(TestServerOptions {
        plugin_bundle: Some(bundle(dir.path())),
        ..TestServerOptions::default()
    });
    let _running = server.state.config.setup.try_begin().unwrap();

    let reply = server.post("/api/setup/plugin", "{}").await;

    assert_eq!(reply.status, 409);
    assert_eq!(reply.code().as_deref(), Some("setup-busy"));
}

#[tokio::test]
async fn erkennt_und_migriert_ein_altes_board_repo() {
    let fixture = DocsFixture::start(true).await;
    let repo = fixture.repo.path();
    fs::create_dir_all(repo.join(".claude/rules")).unwrap();
    fs::write(
        repo.join(".claude/settings.local.json"),
        r#"{"enabledPlugins":{"ai-harness@ai-harness-local":true}}"#,
    )
    .unwrap();
    fs::write(
        repo.join(LEGACY_BOARD_RULE_PATH),
        format!("{LEGACY_RULE_MARKER}\nalt"),
    )
    .unwrap();
    let url = format!("/api/db/boards/{}/migration", fixture.board_id);

    let check = fixture.server.get(&url).await;
    assert_eq!(check.status, 200);
    assert_eq!(check.body()["needed"], true);
    assert_eq!(
        check.body()["reasons"],
        json!(["legacy-plugin", "legacy-rules"])
    );

    let migrated = fixture.server.post(&url, "{}").await;
    assert_eq!(migrated.status, 200, "{:?}", migrated.json());
    assert_eq!(migrated.body()["pluginSwitched"], true);
    assert_eq!(migrated.body()["removed"], json!([LEGACY_BOARD_RULE_PATH]));

    let again = fixture.server.get(&url).await;
    assert_eq!(again.body()["needed"], false);
}

#[tokio::test]
async fn ein_board_ohne_repo_braucht_keine_migration_und_kann_nicht_migriert_werden() {
    let fixture = DocsFixture::start(false).await;
    let url = format!("/api/db/boards/{}/migration", fixture.board_id);

    let check = fixture.server.get(&url).await;
    assert_eq!(
        check.body(),
        json!({ "needed": false, "repoPath": null, "reasons": [] })
    );

    let migrated = fixture.server.post(&url, "{}").await;
    assert_eq!(migrated.status, 400);
}

#[tokio::test]
async fn ein_unbekanntes_board_ist_nicht_gefunden() {
    let fixture = DocsFixture::start(false).await;

    let reply = fixture.server.get("/api/db/boards/9999/migration").await;

    assert_eq!(reply.status, 404);
}
