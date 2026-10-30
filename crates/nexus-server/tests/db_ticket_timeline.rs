//! `GET /api/db/tickets/{id}/timeline`; portiert aus
//! `packages/server/tests/unit/DbTicketTimeline.test.ts`.
//!
//! Die Zeitleiste verbindet die Ereignisse der Board-Datenbank mit zwei rechnerlokalen
//! Quellen: den Lauf-Protokollen des Dispatchers und den Commits des Ticket-Branches.

mod common;

use std::sync::Arc;

use common::{BoardBackendWithServices, TempRepo, TestServer};
use nexus_core::board::basics::BoardActor;
use nexus_core::board::repo_file::BoardRepoEntries;
use nexus_core::board::repo_paths::BoardRepoStore as _;
use nexus_core::board::ticket_service::{ClaimOptions, NewTicketInput, TicketPatch, WriteOptions};
use nexus_core::dispatch::markers;
use serde_json::Value;

/// Legt einen Commit mit festem Autor an (sonst hinge der Name an der Git-Konfig des Rechners)
fn commit(repo: &TempRepo, message: &str) {
    repo.run_git(&["add", "."]);
    repo.run_git(&[
        "-c",
        "user.name=Tester",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "-m",
        message,
    ]);
}

/// Der Handelnde unter einem anderen Namen (Dispatcher, Agent)
fn actor_named(name: &str) -> BoardActor {
    BoardActor {
        user_id: common::MEMORY_USER_ID.into(),
        name: name.to_string(),
    }
}

/// Server mit Projekt ARC, Board „Sprint“ und dem Ticket ARC-0001
async fn start() -> (TestServer, Arc<BoardBackendWithServices>, i32) {
    let (server, backend) = TestServer::start_with_board().await;
    backend
        .services
        .projects
        .create("ARC", "Arches", &backend.actor())
        .await
        .expect("Projekt anlegen");
    let board_id = backend
        .services
        .boards
        .create("ARC", "Sprint", &backend.actor())
        .await
        .expect("Board anlegen")
        .board
        .id;
    backend
        .services
        .tickets
        .create(
            board_id,
            &NewTicketInput::titled("Login"),
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");
    (server, backend, board_id)
}

/// Die Zeitleiste eines Tickets als JSON
async fn timeline(server: &TestServer, url: &str) -> Value {
    let reply = server.get(url).await;
    assert_eq!(reply.status, 200, "{}", reply.text);
    reply
        .json()
        .unwrap_or_else(|| panic!("kein JSON: {}", reply.text))
}

/// Die Werte eines Feldes aller Ereignisse
fn field(events: &Value, key: &str) -> Vec<String> {
    events
        .as_array()
        .expect("Ereignisse")
        .iter()
        .map(|event| event[key].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn liefert_die_db_ereignisse_und_meldet_fehlende_lokale_quellen() {
    let (server, backend, _board_id) = start().await;
    backend
        .services
        .comments
        .add("ARC-0001", "Hallo", &backend.actor(), false)
        .await
        .expect("Kommentar");
    // Kleingeschriebene ID: die Zeitleiste gibt die kanonische zurück
    let json = timeline(&server, "/api/db/tickets/arc-0001/timeline").await;
    assert_eq!(json["ticketId"], "ARC-0001");
    assert_eq!(field(&json["events"], "type"), ["field:status", "comment"]);
    assert_eq!(json["events"][0]["kind"], "status");
    assert_eq!(json["events"][0]["actor"]["name"], common::MEMORY_USER_NAME);
    assert_eq!(json["events"][0]["actor"]["type"], "person");
    assert_eq!(json["sources"]["runs"], "unavailable");
    assert_eq!(json["sources"]["git"], "unavailable");
}

#[tokio::test]
async fn erkennt_agenten_nach_der_dispatch_konfiguration() {
    let (server, backend, _board_id) = start().await;
    std::fs::write(
        server.home.dispatch_config_file(),
        "dispatcherName: robo\nagentPrefix: azubi-\n",
    )
    .expect("Dispatch-Konfig schreiben");
    backend
        .services
        .comments
        .add(
            "ARC-0001",
            &format!(
                "{}: an azubi-ARC-0001 übergeben – Branch b, Profil klein (Modell x).",
                markers::START
            ),
            &actor_named("robo"),
            false,
        )
        .await
        .expect("Kommentar des Dispatchers");
    backend
        .services
        .tickets
        .claim(
            "ARC-0001",
            "azubi-ARC-0001",
            &actor_named("azubi-ARC-0001"),
            &ClaimOptions::default(),
        )
        .await
        .expect("Ticket übernehmen");

    let json = timeline(&server, "/api/db/tickets/ARC-0001/timeline").await;
    let events = json["events"].as_array().expect("Ereignisse").clone();
    let start = events
        .iter()
        .find(|event| event["type"] == "dispatch:start")
        .expect("Start-Ereignis");
    assert_eq!(start["kind"], "run");
    assert_eq!(start["actor"]["type"], "agent");
    assert_eq!(start["ref"]["profile"], "klein");
    let assignee = events
        .iter()
        .find(|event| event["type"] == "field:assignee")
        .expect("Zuweisung");
    assert_eq!(assignee["actor"]["name"], "azubi-ARC-0001");
    assert_eq!(assignee["actor"]["type"], "agent");
}

#[tokio::test]
async fn erganzt_lauf_protokolle_mit_ausgang() {
    let (server, _backend, _board_id) = start().await;
    let dir = server.home.dispatch_logs_dir();
    std::fs::create_dir_all(&dir).expect("Protokollordner");
    // Der Start im Namen ist Ortszeit (09:15 liegt je nach Zeitzone zwischen 19:15Z am Vortag
    // und 21:15Z); das Ende liegt in jeder Zeitzone danach
    std::fs::write(
        dir.join("ARC-0001-20991016-091500.log"),
        "Anweisung\n[dispatch] 2099-10-16T23:30:00.000Z Ende: Exit-Code 1\n",
    )
    .expect("Protokoll");
    // Gehört zu ARC-00010, nicht zu ARC-0001
    std::fs::write(dir.join("ARC-00010-20991016-091500.log"), "fremd").expect("fremdes Protokoll");

    let json = timeline(&server, "/api/db/tickets/ARC-0001/timeline").await;
    assert_eq!(json["sources"]["runs"], "ok");
    let runs: Vec<Value> = json["events"]
        .as_array()
        .expect("Ereignisse")
        .iter()
        .filter(|event| event["kind"] == "run")
        .cloned()
        .collect();
    assert_eq!(
        runs.iter()
            .map(|event| event["type"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["run-log", "run-end"]
    );
    assert_eq!(runs[0]["actor"]["name"], "junior-ARC-0001");
    assert_eq!(runs[0]["actor"]["type"], "agent");
    assert_eq!(runs[0]["ref"]["runLog"], "ARC-0001-20991016-091500.log");
    assert_eq!(runs[0]["ref"]["exitCode"], 1);
    assert_eq!(runs[1]["at"], "2099-10-16T23:30:00.000Z");
    assert!(
        runs[1]["text"]
            .as_str()
            .expect("Text")
            .contains("Exit-Code 1"),
        "{}",
        runs[1]["text"]
    );
}

#[tokio::test]
async fn ubergeht_lauf_protokolle_von_vor_der_anlage_des_tickets() {
    let (server, _backend, _board_id) = start().await;
    let dir = server.home.dispatch_logs_dir();
    std::fs::create_dir_all(&dir).expect("Protokollordner");
    std::fs::write(
        dir.join("ARC-0001-20201021-150715.log"),
        "alt\n[dispatch] 2020-10-21T13:27:34.000Z Ende: Exit-Code 0\n",
    )
    .expect("altes Protokoll");
    std::fs::write(dir.join("ARC-0001-20991016-091500-plan.log"), "neu").expect("neues Protokoll");

    let json = timeline(&server, "/api/db/tickets/ARC-0001/timeline").await;
    assert_eq!(json["sources"]["runs"], "ok");
    let logs: Vec<String> = json["events"]
        .as_array()
        .expect("Ereignisse")
        .iter()
        .filter(|event| event["kind"] == "run")
        .map(|event| event["ref"]["runLog"].as_str().unwrap_or_default().into())
        .collect();
    assert_eq!(logs, ["ARC-0001-20991016-091500-plan.log"]);
}

#[tokio::test]
async fn erganzt_die_commits_des_ticket_branches_aus_dem_repo_des_boards() {
    let (server, backend, board_id) = start().await;
    let repo = TempRepo::new();
    repo.run_git(&["checkout", "-q", "-b", "ticket/ARC-0001-login"]);
    std::fs::write(repo.path().join("a.txt"), "a").expect("Datei");
    commit(&repo, "Login-Formular");
    repo.run_git(&["checkout", "-q", "-"]);
    set_repo(&backend, board_id, repo.path());
    set_branch(&backend, Some("ticket/ARC-0001-login"), None).await;

    let json = timeline(&server, "/api/db/tickets/ARC-0001/timeline").await;
    assert_eq!(json["sources"]["git"], "ok");
    let commit = json["events"]
        .as_array()
        .expect("Ereignisse")
        .iter()
        .find(|event| event["type"] == "commit")
        .expect("Commit-Ereignis")
        .clone();
    assert_eq!(commit["kind"], "git");
    assert_eq!(commit["text"], "Login-Formular");
    assert_eq!(commit["actor"]["name"], "Tester");
    assert_eq!(commit["actor"]["type"], "person");
    let hash = commit["ref"]["commit"].as_str().expect("Commit");
    assert_eq!(hash.len(), 40, "{hash}");
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()), "{hash}");
}

#[tokio::test]
async fn erganzt_bei_zielbranch_nur_die_commits_seit_dem_zielbranch() {
    let (server, backend, board_id) = start().await;
    let repo = TempRepo::new();
    repo.run_git(&["branch", "-M", "main"]);
    repo.run_git(&["checkout", "-q", "-b", "release/1"]);
    std::fs::write(repo.path().join("r.txt"), "r").expect("Datei");
    commit(&repo, "Stand von release/1");
    repo.run_git(&["checkout", "-q", "-b", "ticket/ARC-0001-login"]);
    std::fs::write(repo.path().join("a.txt"), "a").expect("Datei");
    commit(&repo, "Login-Formular");
    repo.run_git(&["checkout", "-q", "main"]);
    set_repo(&backend, board_id, repo.path());
    set_branch(&backend, Some("ticket/ARC-0001-login"), Some("release/1")).await;

    let json = timeline(&server, "/api/db/tickets/ARC-0001/timeline").await;
    assert_eq!(json["sources"]["git"], "ok");
    let texts: Vec<String> = json["events"]
        .as_array()
        .expect("Ereignisse")
        .iter()
        .filter(|event| event["type"] == "commit")
        .map(|event| event["text"].as_str().unwrap_or_default().into())
        .collect();
    assert_eq!(texts, ["Login-Formular"]);
}

#[tokio::test]
async fn meldet_ein_unbekanntes_ticket_mit_404() {
    let (server, _backend, _board_id) = start().await;
    let reply = server.get("/api/db/tickets/ARC-0099/timeline").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.code().as_deref(), Some("ticket-not-found"));
}

/// Trägt den Repo-Pfad des Boards beim Benutzer ein
fn set_repo(backend: &Arc<BoardBackendWithServices>, board_id: i32, path: &std::path::Path) {
    let mut entries = BoardRepoEntries::new();
    entries.insert(
        i64::from(board_id),
        Some(path.to_string_lossy().into_owned()),
    );
    backend.repos.write(&entries).expect("Repo-Pfad schreiben");
}

/// Setzt Branch und Zielbranch des Tickets
async fn set_branch(
    backend: &Arc<BoardBackendWithServices>,
    branch: Option<&str>,
    target: Option<&str>,
) {
    backend
        .services
        .tickets
        .update(
            "ARC-0001",
            &TicketPatch {
                branch: branch.map(|branch| Some(branch.to_string())),
                target_branch: target.map(|target| Some(target.to_string())),
                ..TicketPatch::default()
            },
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Branch setzen");
}
