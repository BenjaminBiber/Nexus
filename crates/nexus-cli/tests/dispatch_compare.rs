//! `nexus dispatch-compare`: der Bericht des Schattenbetriebs als Markdown und als JSON.
//!
//! Gelesen werden allein die JSONL-Dateien unter `<Benutzerordner>/dispatch/shadow/`; der
//! Test schreibt sie von Hand, wie der Schattenbetrieb es täte, und ruft den Befehl auf.

use std::path::PathBuf;

use nexus_core::dispatch::shadow::{ShadowDecision, ShadowDiff, ShadowStore};
use nexus_core::HarnessHome;

/// Programmdatei der Kommandozeile
fn nexus() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_nexus"))
}

/// Ein Benutzerordner mit zwei verglichenen Läufen: einer gleich, einer abweichend
fn home() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Ordner");
    let store = ShadowStore::new(HarnessHome::at(dir.path()));
    store
        .append_decision(&ShadowDecision {
            at: "2026-11-01T10:00:00.000+01:00".into(),
            run_id: "ARC-0001-work-20261101-100000".into(),
            ticket_id: "ARC-0001".into(),
            kind: "work".into(),
            equal: true,
            diffs: Vec::new(),
            prompt_diff: None,
            skipped: None,
        })
        .expect("Entscheidung schreiben");
    store
        .append_decision(&ShadowDecision {
            at: "2026-11-05T10:00:00.000+01:00".into(),
            run_id: "ARC-0002-work-20261105-100000".into(),
            ticket_id: "ARC-0002".into(),
            kind: "work".into(),
            equal: false,
            diffs: vec![ShadowDiff::new(
                "run.profile",
                Some("leicht".into()),
                Some("schwer".into()),
            )],
            prompt_diff: Some("@@ -1,1 +1,1 @@\n-alt\n+neu\n".into()),
            skipped: None,
        })
        .expect("Entscheidung schreiben");
    dir
}

/// Ruft den Befehl im Benutzerordner auf und liefert die Ausgabe
fn run(dir: &tempfile::TempDir, args: &[&str]) -> std::process::Output {
    assert_cmd::Command::new(nexus())
        .arg("dispatch-compare")
        .args(args)
        .env("NEXUS_HOME", dir.path())
        .env_remove("HARNESS_HOME")
        .output()
        .expect("Bericht")
}

#[test]
fn der_versteckte_befehl_steht_nicht_in_der_hilfe() {
    let hilfe = assert_cmd::Command::new(nexus())
        .arg("--help")
        .output()
        .expect("Hilfe");
    let text = String::from_utf8_lossy(&hilfe.stdout);
    assert!(text.contains("serve"), "die Hilfe erscheint: {text}");
    assert!(!text.contains("dispatch-compare"), "{text}");
}

#[test]
fn das_markdown_nennt_zahlen_abweichungen_und_den_prompt_diff() {
    let dir = home();
    let ausgabe = run(&dir, &[]);
    assert!(ausgabe.status.success(), "{ausgabe:?}");
    let text = String::from_utf8_lossy(&ausgabe.stdout);
    assert!(
        text.contains("# Schattenbetrieb: Node gegen Rust"),
        "{text}"
    );
    assert!(text.contains("Zeitraum: alle Tage"), "{text}");
    assert!(text.contains("- Läufe: 2"), "{text}");
    assert!(
        text.contains("- Entscheidungen: 1 gleich, 1 abweichend, 0 ohne Urteil"),
        "{text}"
    );
    assert!(text.contains("### 2026-11-05 ARC-0002 (work)"), "{text}");
    assert!(text.contains("| run.profile | leicht | schwer |"), "{text}");
    assert!(
        text.contains("<details><summary>Arbeitsanweisung</summary>"),
        "{text}"
    );
    assert!(text.contains("-alt"), "{text}");
}

#[test]
fn der_zeitraum_grenzt_die_tage_ein() {
    let dir = home();
    let text = String::from_utf8_lossy(&run(&dir, &["--since", "2026-11-03"]).stdout).into_owned();
    assert!(text.contains("Zeitraum: ab 2026-11-03"), "{text}");
    assert!(text.contains("- Läufe: 1"), "{text}");

    let eng = run(&dir, &["--since", "2026-11-01", "--until", "2026-11-02"]);
    let text = String::from_utf8_lossy(&eng.stdout);
    assert!(
        text.contains("Zeitraum: 2026-11-01 bis 2026-11-02"),
        "{text}"
    );
    assert!(text.contains("- Läufe: 1"), "{text}");
    assert!(!text.contains("## Abweichende"), "{text}");
}

#[test]
fn mit_json_kommen_dieselben_zahlen_als_json() {
    let dir = home();
    let ausgabe = run(&dir, &["--json"]);
    assert!(ausgabe.status.success(), "{ausgabe:?}");
    let bericht: serde_json::Value =
        serde_json::from_slice(&ausgabe.stdout).expect("JSON der Ausgabe");
    assert_eq!(bericht["summary"]["runs"], 2);
    assert_eq!(bericht["summary"]["decisionsEqual"], 1);
    assert_eq!(bericht["summary"]["decisionsDifferent"], 1);
    // Nur die Abweichungen stehen in der Liste
    assert_eq!(bericht["decisions"].as_array().expect("Liste").len(), 1);
    assert_eq!(bericht["decisions"][0]["diffs"][0]["field"], "run.profile");
    assert_eq!(bericht["decisions"][0]["diffs"][0]["node"], "leicht");
    assert_eq!(bericht["decisions"][0]["diffs"][0]["rust"], "schwer");
}

#[test]
fn ein_tag_ohne_jjjj_mm_tt_wird_mit_date_invalid_abgewiesen() {
    let dir = home();
    let ausgabe = run(&dir, &["--since", "1.11.2026", "--json"]);
    let fehler: serde_json::Value =
        serde_json::from_slice(&ausgabe.stdout).expect("JSON des Fehlers");
    assert_eq!(fehler["error"]["code"], "date-invalid");
    assert_eq!(
        fehler["error"]["message"],
        "„--since“ muss ein Datum JJJJ-MM-TT sein."
    );
}

#[test]
fn ohne_schattenbetrieb_bleibt_der_bericht_bei_den_zahlen() {
    let dir = tempfile::tempdir().expect("Ordner");
    let ausgabe = run(&dir, &[]);
    assert!(ausgabe.status.success(), "{ausgabe:?}");
    let text = String::from_utf8_lossy(&ausgabe.stdout);
    assert!(text.contains("- Läufe: 0"), "{text}");
    assert!(!text.contains("## Abweichende"), "{text}");
}
