//! `nexus dispatch-worker <lauf>`: der versteckte Befehl und ein echter losgelöster Worker.
//!
//! Der Integrationstest startet den Worker über den `ProcessWorkerLauncher` aus einem eigenen
//! Hilfsprozess und wartet, bis dieser Starter beendet ist. Protokoll und Ergebnisdatei
//! entstehen danach – der Worker überlebt also das Ende des Prozesses, der ihn gestartet hat.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nexus_core::dispatch::{RunKind, RunRecord, RunSettings, RunStore};
use nexus_core::HarnessHome;
use nexus_server::dispatch::{ProcessWorkerLauncher, WorkerLauncher};

/// Programmdatei der Kommandozeile
fn nexus() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_nexus"))
}

/// Der Ersatz für Claude Code liegt neben der Kommandozeile (beide baut `cargo test`)
fn fake_claude() -> PathBuf {
    let name = if cfg!(windows) {
        "nexus-fake-claude.exe"
    } else {
        "nexus-fake-claude"
    };
    let pfad = nexus().parent().expect("Zielordner").join(name);
    assert!(
        pfad.exists(),
        "{} fehlt; `cargo test --workspace` baut ihn mit",
        pfad.display()
    );
    pfad
}

/// Umgebungsvariable, die den Hilfsprozess zum Starter macht
const START_RUN: &str = "NEXUS_TEST_WORKER_RUN";

#[test]
fn der_versteckte_befehl_steht_nicht_in_der_hilfe() {
    let hilfe = assert_cmd::Command::new(nexus())
        .arg("--help")
        .output()
        .expect("Hilfe");
    let text = String::from_utf8_lossy(&hilfe.stdout);
    assert!(text.contains("serve"), "die Hilfe erscheint: {text}");
    assert!(!text.contains("dispatch-worker"), "{text}");

    let fehler = assert_cmd::Command::new(nexus())
        .args(["dispatch-worker", "--help"])
        .output()
        .expect("Hilfe des Befehls");
    // Versteckt heißt nicht unbekannt: der Befehl selbst antwortet
    assert!(
        String::from_utf8_lossy(&fehler.stdout).contains("dispatch-worker")
            || fehler.status.success(),
        "stdout: {}, stderr: {}",
        String::from_utf8_lossy(&fehler.stdout),
        String::from_utf8_lossy(&fehler.stderr)
    );
}

#[test]
fn startet_einen_echten_losgeloesten_worker_der_den_starter_ueberlebt() {
    // Im Hilfsprozess startet dieser Test nur den Worker und endet (siehe unten)
    if let Ok(run_id) = std::env::var(START_RUN) {
        let env: BTreeMap<String, String> = std::env::vars().collect();
        let exe = std::env::var("NEXUS_TEST_WORKER_EXE").expect("Programmdatei");
        let pid = ProcessWorkerLauncher::with_exe(exe).launch(&run_id, &env);
        assert!(pid.is_some(), "der Worker wurde gestartet");
        return;
    }

    let dir = tempfile::tempdir().expect("Ordner");
    let home = HarnessHome::at(dir.path());
    let log_file = dir.path().join("losgeloest.log");
    let runs = RunStore::new(home.clone());
    runs.create(&lauf("ARC-0011-x", dir.path(), &log_file))
        .expect("Lauf angelegt");

    // Der Starter läuft als eigener Prozess: dieselbe Testdatei, nur dieser Test
    let starter = assert_cmd::Command::new(std::env::current_exe().expect("Testprogramm"))
        .args([
            "--exact",
            "startet_einen_echten_losgeloesten_worker_der_den_starter_ueberlebt",
            "--nocapture",
        ])
        .env("HARNESS_HOME", dir.path())
        .env(START_RUN, "ARC-0011-x")
        .env("NEXUS_TEST_WORKER_EXE", nexus())
        .env("FAKE_CLAUDE_MODE", "ok")
        .env("HARNESS_TICKET", "ARC-0011")
        .output()
        .expect("Starter läuft");
    assert!(
        starter.status.success(),
        "Starter: {}",
        String::from_utf8_lossy(&starter.stderr)
    );

    // Erst jetzt, nach dem Ende des Starters, entstehen Protokoll und Ergebnis
    let frist = Instant::now() + Duration::from_secs(60);
    while runs
        .result("ARC-0011-x")
        .expect("Ergebnis lesbar")
        .is_none()
        && Instant::now() < frist
    {
        std::thread::sleep(Duration::from_millis(100));
    }
    let end = runs
        .result("ARC-0011-x")
        .expect("Ergebnis lesbar")
        .expect("Ergebnis vorhanden");
    assert_eq!(end.result.code, Some(0));
    let log = std::fs::read_to_string(&log_file).expect("Protokoll lesbar");
    assert!(log.contains("junior-ARC-0011-x startet"), "{log}");
    assert!(log.contains("Ende: Exit-Code 0"), "{log}");
    assert!(
        end.result.output.contains("ticket=ARC-0011"),
        "die Umgebung des Starters ging mit: {}",
        end.result.output
    );
}

/// Der Lauf, den der losgelöste Worker ausführt
fn lauf(id: &str, cwd: &Path, log_file: &Path) -> RunRecord {
    RunRecord {
        id: id.to_string(),
        kind: RunKind::Work,
        ticket_id: "ARC-0011".to_string(),
        board_id: 1,
        title: "T".to_string(),
        agent: format!("junior-{id}"),
        cwd: cwd.to_string_lossy().into_owned(),
        branch: "b".to_string(),
        target: None,
        run: RunSettings {
            model: Some("opus".to_string()),
            effort: Some("high".to_string()),
            max_turns: 80,
            timeout_minutes: 1,
            profile: "standard".to_string(),
            why: "Standardprofil standard".to_string(),
            reason: None,
            config_hash: None,
            rule_index: None,
            escalated_from: None,
        },
        attempt: None,
        command: fake_claude().to_string_lossy().into_owned(),
        args: vec!["-p".to_string()],
        prompt: "Arbeitsanweisung".to_string(),
        log_file: log_file.to_string_lossy().into_owned(),
        started_at: "2026-10-29T09:00:00.000Z".to_string(),
        pid: None,
        setup: None,
        setup_timeout_minutes: None,
        question_id: None,
        research_before: None,
        planned_before: None,
        repo_root: None,
    }
}
