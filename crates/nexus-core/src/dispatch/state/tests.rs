//! Modultests der Dateiablage des Dispatchers; übernommen aus
//! `packages/core/tests/unit/dispatch/{DispatchStores,RunLogOutcome,RunLogSplitter}.test.ts`.
//! Dazu die Gegenprobe mit den Fixtures, die ein tsx-Skript aus den TS-Klassen geschrieben hat.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, TimeZone, Utc};
use serde_json::{json, Value};
use tempfile::TempDir;

use crate::clock::{iso, Clock};
use crate::dispatch::invocation::RunKind;
use crate::dispatch::state::{
    DispatchLog, DispatchStateStore, MergeEntry, MergePhase, PlanningAutoState, ResearchAutoState,
    ReviewEntry, ReviewStore, RunEnd, RunLogEntry, RunLogOutcome, RunLogParts, RunLogSplitter,
    RunRecord, RunState, RunStore, RUN_LOG_OUTPUT_MARKER,
};
use crate::dispatch::types::{RunResult, TestRun};
use crate::dispatch::{RunSettings, UsageSnapshot};
use crate::registry::HarnessHome;
use crate::test_support::make_temp_dir;

/// Temporärer Benutzerordner für einen Test
fn temp_home() -> (TempDir, HarnessHome) {
    let dir = make_temp_dir("nexus-dstore-");
    let home = HarnessHome::at(dir.path());
    (dir, home)
}

/// Lauf-Einstellung wie `RUN` im TypeScript-Test
fn run_settings() -> RunSettings {
    RunSettings {
        profile: "standard".to_string(),
        why: "x".to_string(),
        model: Some("opus".to_string()),
        effort: Some("high".to_string()),
        max_turns: 80,
        timeout_minutes: 90,
        reason: None,
        config_hash: None,
        rule_index: None,
        escalated_from: None,
        allowed_accounts: None,
        accounts_from: None,
        account: None,
        account_why: None,
        usage_at_start: None,
    }
}

/// Lauf-Datensatz mit Standardwerten
fn record(id: &str, started_at: &str) -> RunRecord {
    RunRecord {
        id: id.to_string(),
        kind: RunKind::Work,
        ticket_id: "ARC-0001".to_string(),
        board_id: 1,
        title: "T".to_string(),
        agent: "junior-ARC-0001".to_string(),
        cwd: "C:/wt".to_string(),
        branch: "ticket/x".to_string(),
        target: None,
        run: run_settings(),
        attempt: None,
        command: "claude".to_string(),
        args: vec!["-p".to_string()],
        prompt: "p".to_string(),
        log_file: "C:/log".to_string(),
        started_at: started_at.to_string(),
        pid: None,
        setup: None,
        setup_timeout_minutes: None,
        question_id: None,
        research_before: None,
        planned_before: None,
        repo_root: None,
    }
}

/// Ortszeit als Zeitpunkt (die Protokollnamen rechnen in Ortszeit)
fn local(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> chrono::DateTime<Utc> {
    Local
        .with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .expect("eindeutige Ortszeit")
        .with_timezone(&Utc)
}

#[test]
fn run_store_legt_laufe_an_merkt_pid_und_ergebnis_und_entfernt_beides() {
    let (dir, home) = temp_home();
    let runs = RunStore::new(home);
    runs.create(&record("b", "2026-10-15T10:01:00.000Z"))
        .unwrap();
    runs.create(&record("a", "2026-10-15T10:00:00.000Z"))
        .unwrap();
    runs.set_pid("a", Some(77)).unwrap();
    assert_eq!(
        runs.list()
            .unwrap()
            .iter()
            .map(|r| (r.id.clone(), r.pid))
            .collect::<Vec<_>>(),
        vec![("a".to_string(), Some(77)), ("b".to_string(), None)]
    );
    assert_eq!(runs.result("a").unwrap(), None);
    runs.finish(
        "a",
        &RunEnd {
            result: RunResult {
                code: Some(0),
                timed_out: false,
                output: "ok".to_string(),
                ..RunResult::default()
            },
            ended_at: "2026-10-15T10:05:00.000Z".to_string(),
        },
    )
    .unwrap();
    let end = runs.result("a").unwrap().expect("Ergebnis");
    assert_eq!(end.result.code, Some(0));
    assert_eq!(end.result.output, "ok");
    assert_eq!(
        runs.list()
            .unwrap()
            .iter()
            .map(|r| r.id.clone())
            .collect::<Vec<_>>(),
        vec!["a".to_string(), "b".to_string()]
    );
    runs.remove("a").unwrap();
    assert_eq!(runs.read("a").unwrap(), None);
    assert!(!dir.path().join("dispatch/runs/a.result.json").exists());
}

#[test]
fn run_store_merkt_lauf_zeilen_vor_und_vergisst_sie_wieder() {
    let (_dir, home) = temp_home();
    let runs = RunStore::new(home);
    assert!(runs.kept_stats::<Value>().unwrap().is_empty());
    runs.keep_stats("b", &json!({ "runId": "b" })).unwrap();
    runs.keep_stats("a", &json!({ "runId": "a" })).unwrap();
    assert_eq!(
        runs.kept_stats::<Value>().unwrap(),
        vec![
            ("a".to_string(), json!({ "runId": "a" })),
            ("b".to_string(), json!({ "runId": "b" })),
        ]
    );
    // Die vorgemerkten Zeilen stehen im Unterordner `stats` und gehören nicht zu den Läufen
    assert!(runs.list().unwrap().is_empty());
    runs.forget_stats("a").unwrap();
    assert_eq!(
        runs.kept_stats::<Value>().unwrap().len(),
        1,
        "nach dem Vergessen bleibt nur b"
    );
    runs.forget_stats("fehlt").unwrap();
}

#[tokio::test]
async fn dispatch_state_store_schreibt_anderungen_nacheinander() {
    let (_dir, home) = temp_home();
    let state = DispatchStateStore::new(home);
    assert_eq!(state.read().unwrap(), DispatchStateStore::empty());
    let (first, second) = tokio::join!(
        state.update(|s| s.paused_until = Some("2026-10-15T11:00:00.000Z".to_string())),
        state.update(|s| {
            s.merges.insert(
                "ARC-0001".to_string(),
                MergeEntry {
                    ticket_id: "ARC-0001".to_string(),
                    branch: "b".to_string(),
                    phase: MergePhase::Pending,
                    session_used: false,
                    wait_noted: false,
                    target: None,
                    conflict_seen: None,
                    held_by: None,
                    since: "x".to_string(),
                },
            );
        })
    );
    first.unwrap();
    second.unwrap();
    let read = state.read().unwrap();
    assert_eq!(
        read.paused_until.as_deref(),
        Some("2026-10-15T11:00:00.000Z")
    );
    assert_eq!(read.merges["ARC-0001"].phase, MergePhase::Pending);
}

#[test]
fn dispatch_state_store_erganzt_die_automatik_in_alteren_dateien() {
    let (dir, home) = temp_home();
    fs::create_dir_all(dir.path().join("dispatch")).unwrap();
    fs::write(
        home.dispatch_state_file(),
        r#"{"pausedUntil":null,"merges":{},"research":{},"researchAuto":{"since":"x"}}"#,
    )
    .unwrap();
    let state = DispatchStateStore::new(home).read().unwrap();
    assert_eq!(
        state.research_auto,
        ResearchAutoState {
            since: Some("x".to_string()),
            day: None,
            count: 0,
            retry_at: None,
            last_error: None,
        }
    );
    assert!(state.planning.is_empty());
    assert_eq!(state.planning_auto, PlanningAutoState::default());
}

#[tokio::test]
async fn dispatch_state_store_verschiebt_kaputtes_json_beiseite_und_wirft_bei_anderen_lesefehlern()
{
    let (dir, home) = temp_home();
    let state = DispatchStateStore::new(home.clone());
    fs::create_dir_all(dir.path().join("dispatch")).unwrap();
    fs::write(home.dispatch_state_file(), "{kaputt").unwrap();
    assert_eq!(state.read().unwrap(), DispatchStateStore::empty());
    let names: Vec<String> = fs::read_dir(dir.path().join("dispatch"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().any(|n| n.starts_with("state.json.corrupt-")));
    assert!(!names.iter().any(|n| n == "state.json"));

    state
        .update(|s| s.paused_until = Some("x".to_string()))
        .await
        .unwrap();
    assert_eq!(state.read().unwrap().paused_until.as_deref(), Some("x"));

    fs::remove_file(home.dispatch_state_file()).unwrap();
    fs::create_dir(home.dispatch_state_file()).unwrap();
    assert!(
        state.read().is_err(),
        "ein Ordner statt Datei ist ein Lesefehler"
    );
}

#[test]
fn run_store_halb_geschriebene_datei_gilt_als_nicht_vorhanden() {
    let (dir, home) = temp_home();
    let runs = RunStore::new(home);
    let run_dir = dir.path().join("dispatch").join("runs");
    fs::create_dir_all(&run_dir).unwrap();
    fs::write(run_dir.join("halb.json"), "{\"id\": \"ha").unwrap();
    assert_eq!(runs.read("halb").unwrap(), None);
    assert_eq!(runs.read("fehlt").unwrap(), None);
    // Ein Ordner statt Datei steht für einen Lesefehler, der kein „nicht gefunden“ ist
    fs::create_dir(run_dir.join("ordner.json")).unwrap();
    assert!(runs.read("ordner").is_err());
}

#[test]
fn dispatch_log_schreibt_zeilen_benennt_lauf_protokolle_und_liest_nur_erlaubte_namen() {
    let (dir, home) = temp_home();
    let log = DispatchLog::with_clock(home, Clock::fixed(local(2026, 10, 15, 9, 5, 7)));
    log.line("gestartet ARC-0001");
    assert_eq!(
        fs::read_to_string(dir.path().join("dispatch-logs").join("dispatch.log")).unwrap(),
        "2026-10-15 09:05:07  gestartet ARC-0001\n"
    );
    let name = log.run_name("ARC-0001", RunKind::Merge, local(2026, 10, 15, 9, 5, 7));
    assert_eq!(name, "ARC-0001-20261015-090507-merge");
    fs::write(log.file_path(&format!("{name}.log")), "Inhalt").unwrap();
    assert_eq!(
        log.list().unwrap(),
        vec!["dispatch.log".to_string(), format!("{name}.log")]
    );
    assert_eq!(log.read(&format!("{name}.log")).unwrap(), "Inhalt");
    assert_eq!(
        log.read("../board.yaml").unwrap_err().code(),
        Some("log-name-invalid")
    );
    assert_eq!(
        log.read("fehlt.log").unwrap_err().code(),
        Some("log-not-found")
    );
}

#[test]
fn dispatch_log_beginnt_dispatch_log_uber_einem_megabyte_neu() {
    let (dir, home) = temp_home();
    let log = DispatchLog::with_clock(home, Clock::fixed(local(2026, 10, 15, 9, 5, 7)));
    fs::create_dir_all(dir.path().join("dispatch-logs")).unwrap();
    fs::write(log.file_path("dispatch.log"), "x".repeat(1024 * 1024 + 1)).unwrap();
    log.line("weiter");
    assert_eq!(
        fs::read_to_string(log.file_path("dispatch.log")).unwrap(),
        "2026-10-15 09:05:07  weiter\n"
    );
}

#[test]
fn dispatch_log_listet_die_laufe_eines_tickets_oder_einer_frage() {
    let (dir, home) = temp_home();
    let log = DispatchLog::new(home);
    assert_eq!(log.runs("ARC-0001").unwrap(), Vec::new());
    fs::create_dir_all(dir.path().join("dispatch-logs")).unwrap();
    for name in [
        "ARC-0001-20261015-090507.log",
        "ARC-0001-20261016-101500-merge.log",
        "ARC-0001-20261014-080000.log",
        "ARC-00010-20261016-110000.log",
        "ARC-0001-x.log",
        "dispatch.log",
        "ARC-F0002-20261016-120000-research.log",
    ] {
        fs::write(log.file_path(name), "x").unwrap();
    }
    assert_eq!(
        log.runs("ARC-0001").unwrap(),
        vec![
            RunLogEntry {
                name: "ARC-0001-20261016-101500-merge.log".to_string(),
                kind: RunKind::Merge,
                started: Some(iso(local(2026, 10, 16, 10, 15, 0))),
            },
            RunLogEntry {
                name: "ARC-0001-20261015-090507.log".to_string(),
                kind: RunKind::Work,
                started: Some(iso(local(2026, 10, 15, 9, 5, 7))),
            },
            RunLogEntry {
                name: "ARC-0001-20261014-080000.log".to_string(),
                kind: RunKind::Work,
                started: Some(iso(local(2026, 10, 14, 8, 0, 0))),
            },
        ]
    );
    assert_eq!(
        log.runs("arc-f0002").unwrap(),
        vec![RunLogEntry {
            name: "ARC-F0002-20261016-120000-research.log".to_string(),
            kind: RunKind::Research,
            started: Some(iso(local(2026, 10, 16, 12, 0, 0))),
        }]
    );
    assert_eq!(log.runs("../x").unwrap_err().code(), Some("id-invalid"));
}

#[test]
fn dispatch_log_liest_nur_das_ende_grosser_dateien_und_nur_namen_aus_dem_ordner() {
    let (dir, home) = temp_home();
    let log = DispatchLog::new(home);
    fs::create_dir_all(dir.path().join("dispatch-logs")).unwrap();
    let content = format!("{}{}ENDE", "ä".repeat(500_000), "a".repeat(300_000));
    fs::write(log.file_path("gross.log"), content).unwrap();
    let text = log.read("gross.log").unwrap();
    assert_eq!(text.chars().count(), 200_001);
    assert!(text.starts_with("…a"));
    assert!(text.ends_with("ENDE"));
    assert_eq!(
        log.read("CON.log").unwrap_err().code(),
        Some("log-not-found")
    );
}

#[test]
fn review_store_speichert_liest_und_entfernt_eintrage() {
    let (_dir, home) = temp_home();
    let reviews = ReviewStore::new(home.clone());
    assert_eq!(reviews.read("ARC-0001").unwrap(), None);
    let entry = ReviewEntry {
        ticket_id: "ARC-0001".to_string(),
        branch: "ticket/x".to_string(),
        run_id: "r1".to_string(),
        log_file: "C:/log".to_string(),
        at: "2026-10-19T10:00:00.000Z".to_string(),
        tests: vec![TestRun {
            command: "npm test".to_string(),
            ok: Some(true),
            output: "ok".to_string(),
        }],
    };
    reviews.save(&entry).unwrap();
    assert_eq!(reviews.read("ARC-0001").unwrap(), Some(entry.clone()));
    reviews
        .save(&ReviewEntry {
            run_id: "r2".to_string(),
            tests: Vec::new(),
            ..entry
        })
        .unwrap();
    assert_eq!(
        reviews.read("ARC-0001").unwrap().map(|e| e.run_id),
        Some("r2".to_string())
    );
    reviews.remove("ARC-0001").unwrap();
    assert_eq!(reviews.read("ARC-0001").unwrap(), None);
    fs::create_dir_all(home.dispatch_reviews_dir()).unwrap();
    fs::write(home.dispatch_reviews_dir().join("ARC-0002.json"), "{ halb").unwrap();
    assert_eq!(reviews.read("ARC-0002").unwrap(), None);
}

#[test]
fn review_store_ersetzt_verbotene_zeichen_im_dateinamen() {
    let (_dir, home) = temp_home();
    let reviews = ReviewStore::new(home.clone());
    reviews
        .save(&ReviewEntry {
            ticket_id: "../ARC 1".to_string(),
            branch: "b".to_string(),
            run_id: "r".to_string(),
            log_file: "l".to_string(),
            at: "2026-10-19T10:00:00.000Z".to_string(),
            tests: Vec::new(),
        })
        .unwrap();
    assert!(home.dispatch_reviews_dir().join("___ARC_1.json").exists());
}

#[test]
fn run_log_outcome_liest_exit_code_null_und_endzeit() {
    assert_eq!(
        RunLogOutcome::new()
            .read("Anweisung\nAusgabe\n\n[dispatch] 2026-10-16T09:20:00.000Z Ende: Exit-Code 0\n"),
        crate::dispatch::state::RunOutcome {
            state: RunState::Finished,
            exit_code: Some(0),
            timed_out: false,
            ended_at: Some("2026-10-16T09:20:00.000Z".to_string()),
        }
    );
}

#[test]
fn run_log_outcome_liest_exit_code_ungleich_null() {
    let out = RunLogOutcome::new().read("[dispatch] 2026-10-16T09:20:00.000Z Ende: Exit-Code 1\n");
    assert_eq!(out.state, RunState::Finished);
    assert_eq!(out.exit_code, Some(1));
    assert!(!out.timed_out);
}

#[test]
fn run_log_outcome_erkennt_das_zeitlimit() {
    let text = "\n[dispatch] Zeitlimit erreicht – Claude wird beendet\n\n[dispatch] 2026-10-16T10:00:00.000Z Ende: Exit-Code null, Zeitlimit\n";
    assert_eq!(
        RunLogOutcome::new().read(text),
        crate::dispatch::state::RunOutcome {
            state: RunState::Finished,
            exit_code: None,
            timed_out: true,
            ended_at: Some("2026-10-16T10:00:00.000Z".to_string()),
        }
    );
}

#[test]
fn run_log_outcome_meldet_einen_laufenden_lauf_ohne_ende() {
    assert_eq!(
        RunLogOutcome::new().read("Anweisung\nAusgabe"),
        crate::dispatch::state::RunOutcome {
            state: RunState::Running,
            exit_code: None,
            timed_out: false,
            ended_at: None,
        }
    );
    let out = RunLogOutcome::new().read("[dispatch] Zeitlimit erreicht – Claude wird beendet\n");
    assert_eq!(out.state, RunState::Running);
    assert!(out.timed_out);
}

#[test]
fn run_log_outcome_nimmt_das_letzte_ende() {
    let text = "[dispatch] 2026-10-16T09:00:00.000Z Ende: Exit-Code 1\n[dispatch] 2026-10-16T09:30:00.000Z Ende: Exit-Code 0\n";
    let out = RunLogOutcome::new().read(text);
    assert_eq!(out.exit_code, Some(0));
    assert_eq!(out.ended_at.as_deref(), Some("2026-10-16T09:30:00.000Z"));
}

/// Kopf eines Lauf-Protokolls wie `HEAD` im TypeScript-Test
const HEAD: &str = "[dispatch] 2026-10-16T07:00:00.000Z junior-ARC-0001 startet in C:/w (Umsetzung)\n[dispatch] Profil standard: …\n[dispatch] claude -p\n";

#[test]
fn run_log_splitter_trennt_kopf_anweisung_und_ausgabe_am_marker() {
    let text = format!("{HEAD}[dispatch] Arbeitsanweisung:\nDu bist junior.\n\nSetze um.\n\n{RUN_LOG_OUTPUT_MARKER}\n09:00:01  Claude startet (opus)\n\n[dispatch] Ende: Exit-Code 0\n");
    assert_eq!(
        RunLogSplitter::new().split(&text),
        RunLogParts {
            header: HEAD.trim_end().to_string(),
            prompt: Some("Du bist junior.\n\nSetze um.".to_string()),
            body: "09:00:01  Claude startet (opus)\n\n[dispatch] Ende: Exit-Code 0\n".to_string(),
        }
    );
}

#[test]
fn run_log_splitter_erkennt_das_ende_der_anweisung_an_der_ersten_ausgabe() {
    let splitter = RunLogSplitter::new();
    let json_log = format!(
        "{HEAD}[dispatch] Arbeitsanweisung:\nTu es.\n\n{{\"type\":\"result\",\"result\":\"ok\"}}\n[dispatch] Ende: Exit-Code 0\n"
    );
    assert_eq!(
        splitter.split(&json_log),
        RunLogParts {
            header: HEAD.trim_end().to_string(),
            prompt: Some("Tu es.".to_string()),
            body: "{\"type\":\"result\",\"result\":\"ok\"}\n[dispatch] Ende: Exit-Code 0\n"
                .to_string(),
        }
    );
    let running = format!("{HEAD}[dispatch] Arbeitsanweisung:\nTu es.\n\n");
    assert_eq!(
        splitter.split(&running),
        RunLogParts {
            header: HEAD.trim_end().to_string(),
            prompt: Some("Tu es.".to_string()),
            body: String::new(),
        }
    );
    let stamped =
        format!("{HEAD}[dispatch] Arbeitsanweisung:\nTu es.\n\n09:00:01  Claude startet (opus)\n");
    assert_eq!(
        splitter.split(&stamped).body,
        "09:00:01  Claude startet (opus)\n"
    );
}

#[test]
fn run_log_splitter_liefert_ohne_anweisung_den_ganzen_text_als_ausgabe() {
    assert_eq!(
        RunLogSplitter::new().split("…09:00:01  Claude: weiter\n"),
        RunLogParts {
            header: String::new(),
            prompt: None,
            body: "…09:00:01  Claude: weiter\n".to_string(),
        }
    );
}

#[test]
fn run_log_splitter_kommt_mit_crlf_zurecht() {
    let text = format!(
        "[dispatch] x\r\n[dispatch] Arbeitsanweisung:\r\nA\r\n\r\n{RUN_LOG_OUTPUT_MARKER}\r\n09:00:01  B\r\n"
    );
    assert_eq!(
        RunLogSplitter::new().split(&text),
        RunLogParts {
            header: "[dispatch] x".to_string(),
            prompt: Some("A".to_string()),
            body: "09:00:01  B\r\n".to_string(),
        }
    );
}

#[test]
fn ein_lauf_ohne_usage_at_start_laesst_sich_lesen() {
    let alt = json!({
        "model": "opus",
        "effort": "high",
        "maxTurns": 80,
        "timeoutMinutes": 90,
        "profile": "standard",
        "why": "x",
        "account": "standard"
    });
    let run: RunSettings = serde_json::from_value(alt).unwrap();
    assert_eq!(run.usage_at_start, None);
    // Ohne Wert wird das Feld auch nicht geschrieben
    assert!(serde_json::to_value(&run)
        .unwrap()
        .get("usageAtStart")
        .is_none());

    let neu = RunSettings {
        usage_at_start: Some(UsageSnapshot {
            five_hour: Some(40),
            seven_day: Some(12),
            five_hour_resets_at: None,
            seven_day_resets_at: None,
        }),
        ..run
    };
    let value = serde_json::to_value(&neu).unwrap();
    assert_eq!(value["usageAtStart"]["fiveHour"], json!(40));
    assert_eq!(serde_json::from_value::<RunSettings>(value).unwrap(), neu);
}

/// Benutzerordner der Fixtures, die ein tsx-Skript aus den TS-Klassen geschrieben hat
fn fixture_home() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("dispatch")
}

/// JSON einer Datei
fn value_of(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap_or_else(|e| panic!("{path:?}: {e}")))
        .unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

#[test]
fn liest_die_fixtures_aus_den_typescript_klassen() {
    let home = HarnessHome::at(fixture_home());

    let state = DispatchStateStore::new(home.clone()).read().unwrap();
    assert_eq!(
        state.paused_until.as_deref(),
        Some("2026-10-27T08:15:00.123Z")
    );
    assert_eq!(state.merges.len(), 2);
    let held = &state.merges["ARC-0001"];
    assert_eq!(held.phase, MergePhase::Waiting);
    assert_eq!(held.target.as_deref(), Some("main"));
    assert_eq!(held.conflict_seen, Some(true));
    assert_eq!(held.held_by, Some(Some("busy".to_string())));
    assert_eq!(state.merges["ARC-0002"].conflict_seen, None);
    assert_eq!(state.research["ARC-F0001"].count, 2);
    assert_eq!(state.research_auto.day.as_deref(), Some("2026-10-27"));
    assert_eq!(state.planning["ARC-0003"].auto_at, None);
    assert_eq!(
        state.planning_auto.retry_at.as_deref(),
        Some("2026-10-27T09:00:00.000Z")
    );

    let runs = RunStore::new(home.clone());
    let all = runs.list().unwrap();
    assert_eq!(
        all.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["r1", "r2"]
    );
    let first = &all[0];
    assert_eq!(first.kind, RunKind::Work);
    assert_eq!(first.pid, Some(4711));
    assert_eq!(first.attempt, Some(2));
    assert_eq!(first.run.max_turns, 250);
    assert_eq!(first.setup.as_ref().map(|s| s.len()), Some(1));
    assert_eq!(first.planned_before, Some(None));
    assert_eq!(all[1].kind, RunKind::Merge);
    assert_eq!(all[1].run.model, None);

    let end = runs.result("r1").unwrap().expect("Ergebnis");
    assert_eq!(end.ended_at, "2026-10-27T09:45:00.456Z");
    assert!(end.result.timed_out);
    assert_eq!(end.result.setup_ok, Some(false));
    assert_eq!(
        end.result.setup_failed.as_ref().map(|f| f.code),
        Some(Some(1))
    );
    assert_eq!(
        runs.kept_stats::<Value>().unwrap(),
        vec![(
            "r1".to_string(),
            json!({ "runId": "r1", "ticketId": "ARC-0001", "costUsd": 1.5, "ok": true })
        )]
    );

    let review = ReviewStore::new(home)
        .read("ARC-0001")
        .unwrap()
        .expect("Review");
    assert_eq!(review.run_id, "r1");
    assert_eq!(review.tests.len(), 2);
    assert_eq!(review.tests[1].ok, None);
}

#[tokio::test]
async fn schreibt_dieselben_dateien_wie_die_typescript_klassen() {
    let fixtures = fixture_home();
    let source = HarnessHome::at(&fixtures);
    let (dir, home) = temp_home();

    let read_state = DispatchStateStore::new(source.clone()).read().unwrap();
    DispatchStateStore::new(home.clone())
        .update(|s| *s = read_state.clone())
        .await
        .unwrap();

    let source_runs = RunStore::new(source.clone());
    let runs = RunStore::new(home.clone());
    for record in source_runs.list().unwrap() {
        runs.create(&record).unwrap();
    }
    runs.finish("r1", &source_runs.result("r1").unwrap().unwrap())
        .unwrap();
    for (id, row) in source_runs.kept_stats::<Value>().unwrap() {
        runs.keep_stats(&id, &row).unwrap();
    }
    ReviewStore::new(home)
        .save(
            &ReviewStore::new(source)
                .read("ARC-0001")
                .unwrap()
                .expect("Review"),
        )
        .unwrap();

    for name in [
        "dispatch/state.json",
        "dispatch/runs/r1.json",
        "dispatch/runs/r1.result.json",
        "dispatch/runs/r2.json",
        "dispatch/runs/stats/r1.json",
        "dispatch/reviews/ARC-0001.json",
    ] {
        assert_eq!(
            value_of(&dir.path().join(name)),
            value_of(&fixtures.join(name)),
            "{name} weicht von der Fixture ab"
        );
    }
}
