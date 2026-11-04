//! Detailansicht einer Session: Kopf- und Laufdaten, Verlauf in Abschnitten, Änderungen aus
//! Transcript und Git, Fehlerfälle und das Live-Ereignis `session-activity`.

mod common;

use common::TestServer;
use nexus_server::events::LiveEvent;
use serde_json::{json, Value};

/// Arbeitsordner der Beispiel-Session ohne Git (existiert nicht)
const CWD: &str = "D:/Projekte/demo";

/// Gekürztes, anonymisiertes Transcript: Prompt, Antwort, Edit mit Ausgangsstand, Write
/// einer neuen Datei, TodoWrite und Kostenstand
fn transcript() -> String {
    let lines = [
        json!({ "type": "user", "uuid": "u1", "timestamp": "2026-11-04T08:00:00.000Z",
            "entrypoint": "claude-desktop", "cwd": CWD, "gitBranch": "feature/demo", "version": "2.1.0",
            "isSidechain": false, "message": { "role": "user", "content": "Bitte ändern" } }),
        json!({ "type": "assistant", "uuid": "a1", "timestamp": "2026-11-04T08:00:01.000Z",
            "entrypoint": "claude-desktop", "cwd": CWD, "isSidechain": false,
            "message": { "id": "m1", "content": [{ "type": "text", "text": "Mache ich." }],
                "usage": { "input_tokens": 3, "output_tokens": 4, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 5 } } }),
        json!({ "type": "assistant", "uuid": "a2", "timestamp": "2026-11-04T08:00:02.000Z", "cwd": CWD,
            "isSidechain": false, "message": { "id": "m2", "content": [{ "type": "tool_use", "id": "toolu_e",
                "name": "Edit", "input": { "file_path": format!("{CWD}/src/a.rs"), "old_string": "alt", "new_string": "neu" } }] } }),
        json!({ "type": "user", "uuid": "u2", "timestamp": "2026-11-04T08:00:03.000Z", "isSidechain": false,
            "message": { "content": [{ "type": "tool_result", "tool_use_id": "toolu_e", "content": "ok" }] },
            "toolUseResult": { "filePath": format!("{CWD}/src/a.rs"), "originalFile": "eins\nalt\nzwei\n" } }),
        json!({ "type": "assistant", "uuid": "a3", "timestamp": "2026-11-04T08:00:04.000Z", "cwd": CWD,
            "isSidechain": false, "message": { "id": "m3", "content": [{ "type": "tool_use", "id": "toolu_w",
                "name": "Write", "input": { "file_path": format!("{CWD}/b.md"), "content": "# B\n" } }] } }),
        json!({ "type": "user", "uuid": "u3", "timestamp": "2026-11-04T08:00:05.000Z", "isSidechain": false,
            "message": { "content": [{ "type": "tool_result", "tool_use_id": "toolu_w", "content": "ok" }] },
            "toolUseResult": { "type": "create", "filePath": format!("{CWD}/b.md") } }),
        json!({ "type": "assistant", "uuid": "a4", "timestamp": "2026-11-04T08:00:06.000Z", "cwd": CWD,
            "isSidechain": false, "message": { "id": "m4", "content": [{ "type": "tool_use", "id": "toolu_t",
                "name": "TodoWrite", "input": { "todos": [{ "content": "Prüfen", "status": "in_progress" }] } }] } }),
        json!({ "type": "user", "uuid": "u4", "timestamp": "2026-11-04T08:00:07.000Z", "isSidechain": false,
            "message": { "content": [{ "type": "tool_result", "tool_use_id": "toolu_t", "content": "ok" }] } }),
        json!({ "type": "cost-state", "totalCostUSD": 0.25, "totalDuration": 7000,
            "modelUsage": { "claude-demo": { "inputTokens": 10, "outputTokens": 20, "cacheReadInputTokens": 300, "cacheCreationInputTokens": 40 } } }),
    ];
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// Server mit Projekt, Token und einem Ordner für Transcripts
struct Setup {
    server: TestServer,
    pid: String,
    repo: common::TempRepo,
    token: String,
    dir: tempfile::TempDir,
}

impl Setup {
    fn new() -> Setup {
        let server = TestServer::start();
        let (repo, pid) = server.add_project();
        let token = server.token(repo.path());
        Setup {
            server,
            pid,
            repo,
            token,
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// Schreibt ein Transcript und liefert seinen Pfad mit `/`
    fn write(&self, name: &str, content: &str) -> String {
        let path = self.dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path.to_string_lossy().replace('\\', "/")
    }

    /// Startet eine Session per Hook
    async fn start(&self, event: Value) {
        let mut event = event;
        event["kind"] = json!("session-start");
        let reply = self
            .server
            .hook(
                &self.repo.path().to_string_lossy(),
                event,
                Some(&self.token),
            )
            .await;
        assert_eq!(reply.status, 204, "{}", reply.text);
    }

    /// GET unter `/api/projects/{pid}/sessions/{sid}/`
    async fn get(&self, sid: &str, rest: &str) -> common::Reply {
        self.server
            .get(&format!("/api/projects/{}/sessions/{sid}/{rest}", self.pid))
            .await
    }
}

#[tokio::test]
async fn liefert_kopfdaten_aus_hook_und_transcript() {
    let setup = Setup::new();
    let path = setup.write("s1.jsonl", &transcript());
    setup
        .start(json!({ "sessionId": "s1", "cwd": CWD, "transcriptPath": path, "client": "claude-desktop" }))
        .await;
    let reply = setup.get("s1", "detail").await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    let detail = reply.body();
    assert_eq!(detail["client"], "claude-desktop");
    assert_eq!(detail["session"]["client"], "claude-desktop");
    assert_eq!(detail["session"]["runName"], Value::Null);
    assert_eq!(detail["transcript"]["available"], true);
    assert_eq!(detail["transcript"]["path"], path.as_str());
    assert_eq!(detail["transcript"]["sizeBytes"], transcript().len() as i64);
    assert_eq!(detail["branch"], "feature/demo");
    assert_eq!(detail["claudeVersion"], "2.1.0");
    assert_eq!(
        detail["tokens"],
        json!({ "input": 10, "output": 20, "cacheRead": 300, "cacheWrite": 40 })
    );
    assert_eq!(detail["costUsd"], 0.25);
    assert_eq!(detail["durationMs"], 7000);
    assert_eq!(
        detail["context"],
        json!({ "usedTokens": 108, "limitTokens": 200000 })
    );
    assert_eq!(
        detail["todos"],
        json!([{ "content": "Prüfen", "status": "in_progress" }])
    );
    assert_eq!(detail["run"], Value::Null);
}

#[tokio::test]
async fn liefert_den_verlauf_in_abschnitten_und_prueft_die_parameter() {
    let setup = Setup::new();
    let path = setup.write("s1.jsonl", &transcript());
    setup
        .start(json!({ "sessionId": "s1", "cwd": CWD, "transcriptPath": path }))
        .await;
    let all = setup.get("s1", "transcript").await.body();
    assert_eq!(all["available"], true);
    assert_eq!(all["skipped"], 0);
    assert_eq!(all["cursor"], transcript().len().to_string());
    let kinds: Vec<&str> = all["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, vec!["prompt", "text", "tool", "tool", "tool"]);
    let edit = &all["entries"][2];
    assert_eq!(edit["id"], "toolu_e");
    assert_eq!(edit["summary"], "Edit src/a.rs (+1 −1)");
    assert_eq!(
        edit["result"],
        json!({ "text": "ok", "isError": false, "truncated": false })
    );

    let first = setup.get("s1", "transcript?limit=2").await.body();
    assert_eq!(first["entries"].as_array().unwrap().len(), 2);
    let cursor = first["cursor"].as_str().unwrap().to_string();
    let rest = setup
        .get("s1", &format!("transcript?after={cursor}&limit=500"))
        .await
        .body();
    assert_eq!(rest["entries"].as_array().unwrap().len(), 3);
    assert_eq!(rest["entries"][0]["id"], "toolu_e");

    let bad = setup.get("s1", "transcript?after=x").await;
    assert_eq!(bad.status, 400);
    assert_eq!(bad.code().as_deref(), Some("input-invalid"));
    assert_eq!(
        setup.server.state.transcript_watch.len(),
        1,
        "Verlauf wird beobachtet"
    );
}

#[tokio::test]
async fn aenderungen_und_diff_aus_dem_transcript() {
    let setup = Setup::new();
    let path = setup.write("s1.jsonl", &transcript());
    setup
        .start(json!({ "sessionId": "s1", "cwd": CWD, "transcriptPath": path }))
        .await;
    let changes = setup.get("s1", "changes").await.body();
    assert_eq!(
        changes,
        json!({
            "origin": "transcript",
            "base": null,
            "note": "Änderungen per Bash sind nicht enthalten.",
            "files": [
                { "path": "b.md", "status": "added", "added": 1, "removed": 0 },
                { "path": "src/a.rs", "status": "modified", "added": 1, "removed": 1 },
            ],
        })
    );
    let diff = setup.get("s1", "changes/file?path=src/a.rs").await;
    assert_eq!(diff.status, 200, "{}", diff.text);
    assert_eq!(
        diff.body(),
        json!({
            "path": "src/a.rs",
            "hunks": [{ "header": "@@ -1,3 +1,3 @@", "lines": [
                { "kind": "context", "text": "eins" },
                { "kind": "remove", "text": "alt" },
                { "kind": "add", "text": "neu" },
                { "kind": "context", "text": "zwei" },
            ] }],
            "truncated": false,
        })
    );
    let outside = setup.get("s1", "changes/file?path=../x").await;
    assert_eq!(outside.status, 400);
    assert_eq!(outside.code().as_deref(), Some("input-invalid"));
    let unknown = setup.get("s1", "changes/file?path=c.rs").await;
    assert_eq!(unknown.status, 404);
    assert_eq!(unknown.code().as_deref(), Some("change-not-found"));
}

#[tokio::test]
async fn aenderungen_aus_git_in_einem_worktree() {
    let setup = Setup::new();
    let worktree = setup.repo.base().join("wt-detail");
    setup.repo.run_git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "ticket/detail",
        &worktree.to_string_lossy(),
    ]);
    std::fs::write(worktree.join("neu.txt"), "a\nb\n").unwrap();
    let cwd = worktree.to_string_lossy().replace('\\', "/");
    setup.start(json!({ "sessionId": "s1", "cwd": cwd })).await;
    let changes = setup.get("s1", "changes").await.body();
    assert_eq!(changes["origin"], "git", "{changes}");
    assert_eq!(
        changes["files"],
        json!([{ "path": "neu.txt", "status": "added", "added": 2, "removed": 0 }])
    );
    let diff = setup.get("s1", "changes/file?path=neu.txt").await.body();
    assert_eq!(diff["hunks"][0]["header"], "@@ -0,0 +1,2 @@");
}

#[tokio::test]
async fn ohne_transcript_ist_alles_leer_aber_kein_fehler() {
    let setup = Setup::new();
    setup
        .start(json!({ "sessionId": "ohne-transcript-7f3a", "cwd": CWD }))
        .await;
    let detail = setup.get("ohne-transcript-7f3a", "detail").await;
    assert_eq!(detail.status, 200, "{}", detail.text);
    let detail = detail.body();
    assert_eq!(
        detail["client"], "unknown",
        "ohne Hinweis aus Hook oder Transcript"
    );
    assert_eq!(
        detail["transcript"],
        json!({ "available": false, "path": null, "sizeBytes": null })
    );
    let page = setup.get("ohne-transcript-7f3a", "transcript").await.body();
    assert_eq!(
        page,
        json!({ "entries": [], "cursor": "0", "skipped": 0, "available": false })
    );
    let changes = setup.get("ohne-transcript-7f3a", "changes").await.body();
    assert_eq!(
        changes,
        json!({ "origin": "none", "base": null, "note": null, "files": [] })
    );
}

#[tokio::test]
async fn unbekannte_session_ist_404() {
    let setup = Setup::new();
    for rest in ["detail", "transcript", "changes", "changes/file?path=a.rs"] {
        let reply = setup.get("gibt-es-nicht", rest).await;
        assert_eq!(reply.status, 404, "{rest}");
        assert_eq!(reply.code().as_deref(), Some("session-not-found"), "{rest}");
    }
}

#[tokio::test]
async fn eine_session_mit_lauf_ist_von_nexus_und_zeigt_den_lauf() {
    let setup = Setup::new();
    setup
        .start(json!({ "sessionId": "s1", "cwd": CWD, "client": "claude-code", "run": "T-0001-20261104-091500.log" }))
        .await;
    let detail = setup.get("s1", "detail").await.body();
    assert_eq!(detail["client"], "nexus");
    assert_eq!(detail["session"]["client"], "nexus");
    assert_eq!(detail["session"]["runName"], "T-0001-20261104-091500.log");
    assert_eq!(detail["run"]["name"], "T-0001-20261104-091500.log");
    assert_eq!(detail["run"]["kind"], "work");
    assert_eq!(detail["run"]["account"], Value::Null);
}

#[tokio::test]
async fn jeder_hook_meldet_aktivitaet_nach_sessions() {
    let setup = Setup::new();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let _subscription = setup.server.state.events.subscribe(&setup.pid, sender);
    setup.start(json!({ "sessionId": "s1", "cwd": CWD })).await;
    assert_eq!(receiver.recv().await, Some(LiveEvent::Sessions));
    assert_eq!(
        receiver.recv().await,
        Some(LiveEvent::SessionActivity {
            session_id: "s1".into()
        })
    );
}
