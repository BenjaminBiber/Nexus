//! Prüft den Test-Ersatz `nexus-fake-claude` in allen Modi des Vertrags (ok, sleep, fail,
//! stream, limit) – er ersetzt `packages/server/tests/helpers/FakeClaude.ts`.

use std::collections::BTreeMap;
use std::time::Duration;

use nexus_server::dispatch::{ClaudeProcess, ClaudeResultEnvelope, ClaudeRunInput, RunResult};

/// Lauf des Ersatzes in einem Modus
async fn lauf(modus: &str, prompt: &str) -> RunResult {
    let dir = tempfile::tempdir().expect("Ordner");
    let mut env = BTreeMap::new();
    env.insert("FAKE_CLAUDE_MODE".to_string(), modus.to_string());
    env.insert("HARNESS_TICKET".to_string(), "ARC-0004".to_string());
    let input = ClaudeRunInput {
        command: env!("CARGO_BIN_EXE_nexus-fake-claude").to_string(),
        args: vec!["-p".to_string()],
        prompt: prompt.to_string(),
        cwd: dir.path().to_path_buf(),
        env,
        timeout: Duration::from_secs(20),
        cancel: None,
    };
    ClaudeProcess::new().run(input, |_, _| {}).await
}

#[tokio::test]
async fn modus_ok_antwortet_mit_ticket_schluessel_argumenten_und_anweisung() {
    let result = lauf("ok", "Hallo").await;
    assert_eq!(result.code, Some(0));
    let text = ClaudeResultEnvelope::new()
        .parse(&result.output)
        .expect("Ergebnis");
    assert_eq!(text.text, "ticket=ARC-0004 key=- args=-p prompt=Hallo");
}

#[tokio::test]
async fn modus_fail_schreibt_auf_die_fehlerausgabe_und_endet_mit_drei() {
    let result = lauf("fail", "").await;
    assert_eq!(result.code, Some(3));
    assert!(result.output.contains("kaputt"));
}

#[tokio::test]
async fn modus_limit_meldet_das_nutzungslimit() {
    let result = lauf("limit", "").await;
    let text = ClaudeResultEnvelope::new()
        .parse(&result.output)
        .expect("Ergebnis");
    assert_eq!(text.text, "Claude usage limit reached");
}

#[tokio::test]
async fn modus_stream_liefert_ganze_ereigniszeilen_obwohl_mitten_in_der_zeile_geteilt_wird() {
    let result = lauf("stream", "").await;
    assert_eq!(result.code, Some(0));
    let zeilen: Vec<&str> = result.output.lines().filter(|z| !z.is_empty()).collect();
    assert_eq!(zeilen.len(), 4, "vier Ereignisse: {}", result.output);
    for zeile in &zeilen {
        serde_json::from_str::<serde_json::Value>(zeile)
            .unwrap_or_else(|error| panic!("Zeile ist kein JSON ({error}): {zeile}"));
    }
    assert!(zeilen[0].contains("\"model\":\"fake-opus\""));
    assert!(zeilen[1].contains("harness board show ARC-0004"));
    let text = ClaudeResultEnvelope::new()
        .parse(&result.output)
        .expect("Ergebnis");
    assert_eq!(text.text, "erledigt");
}

#[tokio::test]
async fn modus_sleep_laeuft_bis_zum_zeitlimit() {
    let dir = tempfile::tempdir().expect("Ordner");
    let mut env = BTreeMap::new();
    env.insert("FAKE_CLAUDE_MODE".to_string(), "sleep".to_string());
    let input = ClaudeRunInput {
        command: env!("CARGO_BIN_EXE_nexus-fake-claude").to_string(),
        args: Vec::new(),
        prompt: String::new(),
        cwd: dir.path().to_path_buf(),
        env,
        timeout: Duration::from_millis(800),
        cancel: None,
    };
    let result = ClaudeProcess::new().run(input, |_, _| {}).await;
    assert!(result.timed_out);
}
