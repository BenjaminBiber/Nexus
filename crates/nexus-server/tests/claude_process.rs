//! Prozess-Tests von `ClaudeProcess`: portiert aus den Claude-Fällen in
//! `packages/server/tests/unit/dispatch/DispatchWorker.test.ts`. Statt des Skripts aus
//! `FakeClaude.ts` kommt das Binary `nexus-fake-claude` zum Einsatz.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nexus_server::dispatch::{ClaudeProcess, ClaudeRunInput, OutputSource, RunResult};
use tokio_util::sync::CancellationToken;

/// Der Ersatz für Claude Code
fn fake_claude() -> String {
    env!("CARGO_BIN_EXE_nexus-fake-claude").to_string()
}

/// Umgebung aus Paaren (die Umgebung wird vollständig vorgegeben)
fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Lauf mit Vorgaben, die jeder Test überschreiben kann
fn lauf(command: &str, cwd: &std::path::Path) -> ClaudeRunInput {
    ClaudeRunInput {
        command: command.to_string(),
        args: Vec::new(),
        prompt: String::new(),
        cwd: cwd.to_path_buf(),
        env: BTreeMap::new(),
        timeout: Duration::from_secs(20),
        cancel: None,
    }
}

/// Führt den Lauf aus und liefert Ergebnis und alles, was `on_output` gesehen hat
async fn fuehre_aus(input: ClaudeRunInput, process: ClaudeProcess) -> (RunResult, String) {
    let gesehen = Arc::new(Mutex::new(String::new()));
    let mitschrift = Arc::clone(&gesehen);
    let result = process
        .run(input, move |chunk, _source| {
            mitschrift.lock().expect("Mitschrift").push_str(chunk);
        })
        .await;
    let gesehen = gesehen.lock().expect("Mitschrift").clone();
    (result, gesehen)
}

#[tokio::test]
async fn gibt_anweisung_ueber_stdin_argumente_und_umgebung_weiter() {
    let dir = tempfile::tempdir().expect("Ordner");
    let mut input = lauf(&fake_claude(), dir.path());
    input.args = vec![
        "-p".to_string(),
        "--allowedTools".to_string(),
        "Bash(harness:*)".to_string(),
    ];
    input.prompt = "Hallo Welt".to_string();
    input.env = env(&[("HARNESS_TICKET", "ARC-0001")]);
    let (result, gesehen) = fuehre_aus(input, ClaudeProcess::new()).await;
    assert_eq!(result.code, Some(0));
    assert!(!result.timed_out);
    assert!(
        result.output.contains(
            "ticket=ARC-0001 key=- args=-p --allowedTools Bash(harness:*) prompt=Hallo Welt"
        ),
        "Ausgabe: {}",
        result.output
    );
    assert!(
        gesehen.contains("\"type\":\"result\""),
        "Mitschrift: {gesehen}"
    );
}

#[tokio::test]
async fn kuerzt_lange_ausgabe_auf_ganze_zeilen() {
    let dir = tempfile::tempdir().expect("Ordner");
    // Der Modus „riesig“ schreibt eine Zeile mit 250.000 Zeichen und danach das Ergebnis;
    // das Bruchstück der ersten Zeile darf nicht als Nutzungslimit durchgehen
    let mut input = lauf(&fake_claude(), dir.path());
    input.env = env(&[("FAKE_CLAUDE_MODE", "riesig")]);
    let (result, _) = fuehre_aus(input, ClaudeProcess::new()).await;
    assert!(
        !result.output.contains("rate_limit"),
        "das Bruchstück der ersten Zeile zählt nicht"
    );
    assert!(
        result.output.starts_with("{\"type\":\"result\""),
        "Ausgabe beginnt mit einer ganzen Zeile: {}",
        &result.output[..result.output.len().min(80)]
    );
}

#[tokio::test]
async fn beendet_nach_dem_zeitlimit() {
    let dir = tempfile::tempdir().expect("Ordner");
    let mut input = lauf(&fake_claude(), dir.path());
    input.prompt = "x".to_string();
    input.env = env(&[("FAKE_CLAUDE_MODE", "sleep")]);
    input.timeout = Duration::from_millis(1500);
    let (result, gesehen) = fuehre_aus(input, ClaudeProcess::new()).await;
    assert!(result.timed_out);
    assert!(
        gesehen.contains("[dispatch] Zeitlimit erreicht – Claude wird beendet"),
        "Meldung im Protokoll: {gesehen}"
    );
}

#[tokio::test]
async fn ein_abbruch_von_aussen_beendet_claude() {
    let dir = tempfile::tempdir().expect("Ordner");
    let cancel = CancellationToken::new();
    let mut input = lauf(&fake_claude(), dir.path());
    input.env = env(&[("FAKE_CLAUDE_MODE", "sleep")]);
    input.timeout = Duration::from_secs(60);
    input.cancel = Some(cancel.clone());
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        cancel.cancel();
    });
    let begonnen = Instant::now();
    let (result, gesehen) = fuehre_aus(input, ClaudeProcess::new()).await;
    assert!(!result.timed_out, "Abbruch ist kein Zeitlimit");
    assert!(begonnen.elapsed() < Duration::from_secs(30));
    assert!(
        gesehen.contains("[dispatch] Abgebrochen – Claude wird beendet"),
        "Meldung im Protokoll: {gesehen}"
    );
}

#[tokio::test]
async fn meldet_einen_fehlenden_befehl_mit_exit_code_127() {
    let dir = tempfile::tempdir().expect("Ordner");
    let fehlt = dir.path().join("gibt-es-nicht.exe");
    let input = lauf(&fehlt.to_string_lossy(), dir.path());
    let (result, _) = fuehre_aus(input, ClaudeProcess::new()).await;
    assert_eq!(result.code, Some(127));
    assert!(result.output.contains("Claude Code nicht gefunden"));
}

/// Unter Windows kann ein Prozess dem Job-Objekt nicht entkommen; dort greift der Abbruch
/// des Baums immer, und der harte Abbruch ist nur die zweite Sicherung.
#[tokio::test]
#[cfg(unix)]
async fn liefert_auch_dann_ein_ergebnis_wenn_das_beenden_des_baums_nicht_greift() {
    let dir = tempfile::tempdir().expect("Ordner");
    let mut input = lauf(&fake_claude(), dir.path());
    // Der Ersatz legt sich im Modus „entkommt“ in eine eigene Prozessgruppe; killpg greift nicht
    input.env = env(&[("FAKE_CLAUDE_MODE", "entkommt")]);
    input.timeout = Duration::from_millis(500);
    let begonnen = Instant::now();
    let (result, _) = fuehre_aus(
        input,
        ClaudeProcess::with_hard_kill(Duration::from_millis(300)),
    )
    .await;
    assert!(result.timed_out);
    assert!(
        begonnen.elapsed() < Duration::from_secs(20),
        "das Ergebnis kommt, ohne auf den Prozess zu warten"
    );
}

#[tokio::test]
async fn dekodiert_utf8_zeichen_die_ueber_zwei_bloecke_verteilt_ankommen() {
    let dir = tempfile::tempdir().expect("Ordner");
    let mut input = lauf(&fake_claude(), dir.path());
    input.env = env(&[("FAKE_CLAUDE_MODE", "geteilt")]);
    let (result, gesehen) = fuehre_aus(input, ClaudeProcess::new()).await;
    assert_eq!(result.output, "ä");
    assert_eq!(gesehen, "ä");
}

#[tokio::test]
#[cfg(windows)]
async fn sucht_befehle_zuerst_nach_pfad_reihenfolge_dann_nach_endung() {
    use nexus_server::dispatch::claude_process::resolve_command;

    let dir = tempfile::tempdir().expect("Ordner");
    let erster = dir.path().join("a");
    let zweiter = dir.path().join("b");
    std::fs::create_dir(&erster).expect("Ordner a");
    std::fs::create_dir(&zweiter).expect("Ordner b");
    std::fs::write(erster.join("claude.cmd"), "").expect("claude.cmd");
    std::fs::write(zweiter.join("claude.exe"), "").expect("claude.exe");
    let pfad = format!("{};{}", erster.display(), zweiter.display());
    let aufgeloest = resolve_command("claude", &env(&[("PATH", &pfad)])).expect("aufgelöst");
    assert_eq!(aufgeloest.file, erster.join("claude.cmd").to_string_lossy());
    assert!(aufgeloest.shell, "eine .cmd startet über cmd.exe");
}

#[tokio::test]
async fn trennt_stdout_und_stderr_in_der_mitschrift() {
    let dir = tempfile::tempdir().expect("Ordner");
    let mut input = lauf(&fake_claude(), dir.path());
    input.env = env(&[("FAKE_CLAUDE_MODE", "fail")]);
    let quellen: Arc<Mutex<Vec<(String, OutputSource)>>> = Arc::new(Mutex::new(Vec::new()));
    let mitschrift = Arc::clone(&quellen);
    let result = ClaudeProcess::new()
        .run(input, move |chunk, source| {
            mitschrift
                .lock()
                .expect("Mitschrift")
                .push((chunk.to_string(), source));
        })
        .await;
    assert_eq!(result.code, Some(3));
    assert!(result.output.contains("kaputt"));
    let quellen = quellen.lock().expect("Mitschrift");
    assert!(quellen
        .iter()
        .any(|(chunk, source)| *source == OutputSource::Stderr && chunk.contains("kaputt")));
}

#[tokio::test]
async fn der_arbeitsordner_ist_der_worktree() {
    let dir = tempfile::tempdir().expect("Ordner");
    let cwd: PathBuf = dir.path().to_path_buf();
    let mut input = lauf(&fake_claude(), &cwd);
    input.env = env(&[("FAKE_CLAUDE_MODE", "cwd")]);
    let (result, _) = fuehre_aus(input, ClaudeProcess::new()).await;
    let erwartet = cwd.canonicalize().expect("Pfad");
    assert!(
        result
            .output
            .contains(&erwartet.to_string_lossy().to_string()),
        "Arbeitsordner in der Ausgabe: {}",
        result.output
    );
}
