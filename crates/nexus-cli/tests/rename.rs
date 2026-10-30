//! Die Übergangszeit der Umbenennung von außen: der Alias `harness` über die Startdatei,
//! `nexus migrate` und der Umzug beim ersten Start.

mod common;

use std::fs;
// Die Startdateien und der Umzug aus dem Benutzerordner werden nur unter Unix geprüft
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command;

#[cfg(unix)]
use assert_cmd::cargo::CommandCargoExt;
#[cfg(unix)]
use common::temp_project::Lauf;
use common::temp_project::TempProject;
#[cfg(unix)]
use nexus_core::install::LauncherWriter;

/// Hinweis, den die Startdatei `harness` auslöst
#[cfg(unix)]
const ALIAS_HINWEIS: &str =
    "Hinweis: `harness` heißt jetzt `nexus`. Der alte Name funktioniert noch eine Weile.";

/// Legt einen Startordner mit einer Kopie des Binaries und den Startdateien an und liefert den
/// Pfad der Startdatei `harness`
#[cfg(unix)]
fn startordner(base: &Path) -> PathBuf {
    let bin = base.join("bin");
    fs::create_dir_all(&bin).expect("Startordner");
    let binary = Command::cargo_bin("nexus").expect("Binary nexus");
    fs::copy(binary.get_program(), bin.join("nexus")).expect("Binary kopieren");
    LauncherWriter::new().write(&bin).expect("Startdateien");
    bin.join("harness")
}

/// Ruft die Startdatei `harness` mit dem Home des Projekts auf
#[cfg(unix)]
fn ueber_startdatei(projekt: &TempProject, launcher: &Path, args: &[&str]) -> Lauf {
    let output = Command::new(launcher)
        .args(args)
        .current_dir(&projekt.path)
        .env("NEXUS_HOME", projekt.home())
        .env_remove("HARNESS_HOME")
        .env_remove("NEXUS_SESSION")
        .env_remove("HARNESS_SESSION")
        .output()
        .expect("harness startet");
    Lauf {
        code: output.status.code().expect("Rückgabecode"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[cfg(unix)]
#[test]
fn der_alias_harness_liefert_dieselbe_ausgabe_mit_einem_hinweis_auf_stderr() {
    let projekt = TempProject::new();
    projekt.seed();
    let launcher = startordner(&projekt.base());

    let ueber_alias = ueber_startdatei(&projekt, &launcher, &["board", "list"]);
    let direkt = projekt.run(&["board", "list"]);

    assert_eq!(ueber_alias.code, 0, "{}", ueber_alias.stderr);
    assert_eq!(ueber_alias.stdout, direkt.stdout);
    assert_eq!(ueber_alias.stderr, format!("{ALIAS_HINWEIS}\n"));
    assert_eq!(direkt.stderr, "");
}

#[cfg(unix)]
#[test]
fn der_hinweis_zum_alten_namen_steht_nur_einmal_je_aufruf() {
    let projekt = TempProject::new();
    projekt.seed();
    let launcher = startordner(&projekt.base());

    let lauf = ueber_startdatei(&projekt, &launcher, &["board", "list"]);

    assert_eq!(
        lauf.stderr.matches(ALIAS_HINWEIS).count(),
        1,
        "{}",
        lauf.stderr
    );
}

#[test]
fn migrate_dry_run_listet_die_dateien_als_json_und_schreibt_nichts() {
    let projekt = TempProject::new();
    let alt = projekt.base().join("alt");
    fs::create_dir_all(alt.join("dispatch")).expect("alter Ordner");
    fs::write(alt.join("projects.json"), "[]\n").expect("projects.json");
    fs::write(alt.join("dispatch").join("state.json"), "{}\n").expect("state.json");
    let neu = projekt.base().join("neu");

    let lauf = projekt.run_in_home(
        &neu,
        &[
            "--json",
            "migrate",
            "--dry-run",
            "--from",
            &alt.display().to_string(),
        ],
    );

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    let json = lauf.json();
    assert_eq!(json["status"], "dry-run");
    assert_eq!(json["from"], alt.display().to_string());
    assert_eq!(json["to"], neu.display().to_string());
    assert_eq!(
        json["files"],
        serde_json::json!(["dispatch/state.json", "projects.json"])
    );
    assert!(!neu.exists(), "--dry-run hat den Zielordner angelegt");
}

#[test]
fn migrate_holt_die_daten_und_laesst_den_alten_ordner_liegen() {
    let projekt = TempProject::new();
    let alt = projekt.base().join("alt");
    fs::create_dir_all(&alt).expect("alter Ordner");
    fs::write(alt.join("board.db"), "db").expect("board.db");
    fs::write(alt.join("board.db-wal"), "wal").expect("board.db-wal");
    let neu = projekt.base().join("neu");

    let lauf = projekt.run_in_home(&neu, &["migrate", "--from", &alt.display().to_string()]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(lauf.stdout.contains("2 Dateien"), "{}", lauf.stdout);
    assert_eq!(fs::read_to_string(neu.join("board.db")).unwrap(), "db");
    assert_eq!(fs::read_to_string(neu.join("board.db-wal")).unwrap(), "wal");
    assert!(neu.join("migrated-from.json").is_file());
    // Der alte Ordner bleibt, wie er war
    assert_eq!(fs::read_to_string(alt.join("board.db")).unwrap(), "db");
}

#[test]
fn ein_zweiter_aufruf_meldet_schon_migriert() {
    let projekt = TempProject::new();
    let alt = projekt.base().join("alt");
    fs::create_dir_all(&alt).expect("alter Ordner");
    fs::write(alt.join("projects.json"), "[]\n").expect("projects.json");
    let neu = projekt.base().join("neu");
    let von = alt.display().to_string();
    projekt.run_in_home(&neu, &["migrate", "--from", &von]);

    let lauf = projekt.run_in_home(&neu, &["migrate", "--from", &von]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(
        lauf.stdout
            .contains(&format!("Schon migriert: {}.", neu.display())),
        "{}",
        lauf.stdout
    );
}

#[cfg(unix)]
#[test]
fn der_erste_start_holt_die_daten_aus_dem_alten_benutzerordner() {
    let projekt = TempProject::new();
    let benutzer = projekt.base().join("benutzer");
    let alt = benutzer.join(".harness");
    fs::create_dir_all(&alt).expect("alter Ordner");
    fs::write(alt.join("projects.json"), "[]\n").expect("projects.json");

    // Ohne NEXUS_HOME und HARNESS_HOME fällt die Auflösung auf `~/.harness` zurück; genau
    // dann holt der erste Aufruf die Daten nach `~/.nexus`
    let lauf = im_benutzerordner(&projekt, &benutzer, &["stop"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    let neu = benutzer.join(".nexus");
    assert_eq!(
        fs::read_to_string(neu.join("projects.json")).unwrap(),
        "[]\n"
    );
    assert!(neu.join("migrated-from.json").is_file());
    assert!(lauf.stderr.contains("umgezogen"), "{}", lauf.stderr);
    // Der alte Ordner bleibt liegen
    assert!(alt.join("projects.json").is_file());
}

#[cfg(unix)]
#[test]
fn der_hook_migriert_nie() {
    let projekt = TempProject::new();
    let benutzer = projekt.base().join("benutzer");
    fs::create_dir_all(benutzer.join(".harness")).expect("alter Ordner");

    im_benutzerordner(&projekt, &benutzer, &["hook", "--help"]);

    assert!(!benutzer.join(".nexus").exists());
}

/// Ruft das Binary ohne `NEXUS_HOME` und `HARNESS_HOME` mit einem eigenen Benutzerordner auf
#[cfg(unix)]
fn im_benutzerordner(projekt: &TempProject, benutzer: &Path, args: &[&str]) -> Lauf {
    let output = Command::cargo_bin("nexus")
        .expect("Binary nexus")
        .args(args)
        .current_dir(&projekt.path)
        .env("HOME", benutzer)
        .env_remove("NEXUS_HOME")
        .env_remove("HARNESS_HOME")
        .env_remove("NEXUS_SESSION")
        .env_remove("HARNESS_SESSION")
        .output()
        .expect("nexus startet");
    Lauf {
        code: output.status.code().expect("Rückgabecode"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}
