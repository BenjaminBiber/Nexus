//! Das Gerüst von außen: Hilfe, Rückgabecodes und Fehlerformate des Binaries `nexus`.
//! Portiert aus `packages/cli/tests/integration/skeleton.test.ts` und den Gerüst-Fällen aus
//! `packages/cli/tests/integration/help.test.ts`.

use std::process::Output;

use assert_cmd::cargo::CommandCargoExt;
use tempfile::TempDir;

/// Ergebnis eines Aufrufs: Rückgabecode und beide Ausgaben als Text
struct Lauf {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Ruft `nexus` in einem leeren Ordner auf, ohne Session in der Umgebung
fn run(args: &[&str]) -> Lauf {
    let dir = TempDir::new().expect("Temp-Ordner");
    let output: Output = std::process::Command::cargo_bin("nexus")
        .expect("Binary nexus")
        .args(args)
        .current_dir(dir.path())
        .env_remove("HARNESS_SESSION")
        .output()
        .expect("nexus startet");
    Lauf {
        code: output.status.code().expect("Rückgabecode"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn zeigt_die_hilfe_auf_deutsch_und_endet_mit_code_0() {
    let lauf = run(&["--help"]);
    assert_eq!(lauf.stderr, "");
    assert_eq!(lauf.code, 0);
    assert!(lauf.stdout.contains("AI Harness"), "{}", lauf.stdout);
    assert!(lauf.stdout.contains("--json"), "{}", lauf.stdout);
}

#[test]
fn die_hauptseite_hat_den_aufrufkopf_und_deutsche_uberschriften() {
    let lauf = run(&["--help"]);
    assert!(
        lauf.stdout
            .starts_with("Aufruf: harness [options] [command]\n"),
        "{}",
        lauf.stdout
    );
    assert!(lauf.stdout.contains("Optionen:"), "{}", lauf.stdout);
    assert!(lauf.stdout.contains("Befehle:"), "{}", lauf.stdout);
    assert!(
        lauf.stdout.contains("-h, --help     Hilfe anzeigen"),
        "{}",
        lauf.stdout
    );
    for englisch in [
        "Usage:",
        "Options:",
        "Commands:",
        "display help",
        "Print help",
    ] {
        assert!(
            !lauf.stdout.contains(englisch),
            "„{englisch}“ steht noch in der Hilfe"
        );
    }
}

#[test]
fn kurzform_h_zeigt_dieselbe_hilfe() {
    assert_eq!(run(&["-h"]).stdout, run(&["--help"]).stdout);
}

#[test]
fn der_hilfe_befehl_zeigt_die_hauptseite_und_einzelne_befehle() {
    assert_eq!(run(&["help"]).stdout, run(&["--help"]).stdout);
    let einzeln = run(&["help", "help"]);
    assert_eq!(einzeln.code, 0);
    assert!(
        einzeln
            .stdout
            .starts_with("Aufruf: harness help [options] [befehl]\n"),
        "{}",
        einzeln.stdout
    );
}

#[test]
fn ohne_befehl_zeigt_das_programm_die_hilfe_auf_stderr_und_endet_mit_code_1() {
    let lauf = run(&[]);
    assert_eq!(lauf.code, 1);
    assert_eq!(lauf.stdout, "");
    assert!(
        lauf.stderr
            .starts_with("Aufruf: harness [options] [command]\n"),
        "{}",
        lauf.stderr
    );
}

#[test]
fn unbekannter_befehl_endet_mit_code_1_und_deutschem_text() {
    let lauf = run(&["gibtesnicht"]);
    assert_eq!(lauf.code, 1);
    assert_eq!(lauf.stdout, "");
    assert_eq!(lauf.stderr, "Fehler: Unbekannter Befehl 'gibtesnicht'.\n");
}

#[test]
fn unbekannter_befehl_mit_ahnlichem_namen_bekommt_einen_vorschlag() {
    assert_eq!(
        run(&["hepl"]).stderr,
        "Fehler: Unbekannter Befehl 'hepl'.\n(Meinten Sie help?)\n"
    );
}

#[test]
fn unbekannte_option_endet_mit_code_1_und_deutschem_text() {
    let lauf = run(&["--quatsch"]);
    assert_eq!(lauf.code, 1);
    assert_eq!(lauf.stderr, "Fehler: Unbekannte Option '--quatsch'.\n");
}

#[test]
fn eine_option_ohne_wert_wird_auf_deutsch_gemeldet() {
    let lauf = run(&["--dir"]);
    assert_eq!(lauf.code, 1);
    assert_eq!(
        lauf.stderr,
        "Fehler: Zur Option '--dir <pfad>' fehlt ein Wert.\n"
    );
}

#[test]
fn ein_fehlendes_pflichtargument_wird_auf_deutsch_gemeldet() {
    let lauf = run(&["selbsttest-fehler"]);
    assert_eq!(lauf.code, 1);
    assert_eq!(lauf.stderr, "Fehler: Das Pflichtargument 'code' fehlt.\n");
}

#[test]
fn zu_viele_argumente_werden_auf_deutsch_gemeldet() {
    let lauf = run(&[
        "selbsttest-fehler",
        "conflict",
        "Bitte neu lesen.",
        "zuviel",
    ]);
    assert_eq!(lauf.code, 1);
    assert_eq!(lauf.stderr, "Fehler: Zu viele Argumente.\n");
}

#[test]
fn ein_erwartbarer_fehler_endet_mit_code_1_und_text_auf_stderr() {
    let lauf = run(&["selbsttest-fehler", "conflict", "Bitte neu lesen."]);
    assert_eq!(lauf.code, 1);
    assert_eq!(lauf.stdout, "");
    assert_eq!(lauf.stderr, "Bitte neu lesen. [conflict]\n");
}

#[test]
fn ein_erwartbarer_fehler_kommt_mit_json_als_objekt_auf_stdout() {
    let lauf = run(&[
        "--json",
        "selbsttest-fehler",
        "conflict",
        "Bitte neu lesen.",
    ]);
    assert_eq!(lauf.code, 1);
    assert_eq!(lauf.stderr, "");
    assert_eq!(
        lauf.stdout,
        "{\n  \"error\": {\n    \"code\": \"conflict\",\n    \"message\": \"Bitte neu lesen.\"\n  }\n}\n"
    );
}

#[test]
fn der_hinweis_auf_force_steht_auch_im_json() {
    let lauf = run(&[
        "--json",
        "selbsttest-fehler",
        "ticket-duplicate",
        "Gibt es schon: ARC-0012.",
    ]);
    assert_eq!(lauf.code, 1);
    assert!(
        lauf.stdout
            .contains("Gibt es schon: ARC-0012. Mit --force trotzdem anlegen."),
        "{}",
        lauf.stdout
    );
}

#[test]
fn versteckte_befehle_stehen_nicht_in_der_hilfe() {
    assert!(!run(&["--help"]).stdout.contains("selbsttest"));
}
