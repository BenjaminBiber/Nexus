//! `nexus theme install`, `list` und `remove` von außen: Pakete einspielen, auflisten, entfernen
//! und verständliche Fehler, ohne dass unter `themes/` etwas zurückbleibt.

use std::io::Write;
use std::path::{Path, PathBuf};

use assert_cmd::cargo::CommandCargoExt;
use tempfile::TempDir;

/// Ergebnis eines Aufrufs: Rückgabecode und beide Ausgaben
struct Lauf {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Ruft `nexus theme …` im Ordner `cwd` auf; `NEXUS_HOME` ist `<cwd>/home`
fn theme(cwd: &Path, args: &[&str]) -> Lauf {
    let output = std::process::Command::cargo_bin("nexus")
        .expect("Binary nexus")
        .arg("theme")
        .args(args)
        .current_dir(cwd)
        .env("NEXUS_HOME", cwd.join("home"))
        .env_remove("HARNESS_HOME")
        .env_remove("HARNESS_SESSION")
        .output()
        .expect("nexus startet");
    Lauf {
        code: output.status.code().expect("Rückgabecode"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn manifest(id: &str, version: &str) -> String {
    format!(
        r#"{{"id":"{id}","name":"Nacht","version":"{version}","author":"Ada","sdkVersion":"^1.0","colorScheme":"dark"}}"#
    )
}

/// Schreibt ein ZIP mit den gegebenen Einträgen nach `ziel`
fn zip(ziel: &Path, eintraege: &[(&str, &str)]) {
    let mut writer = zip::ZipWriter::new(std::fs::File::create(ziel).unwrap());
    for (name, inhalt) in eintraege {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(inhalt.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
}

/// Ein gültiges Paket `<name>` unter `root`, mit `main.js`, wenn `mit_code` gesetzt ist
fn paket(root: &Path, name: &str, id: &str, version: &str, mit_code: bool) -> PathBuf {
    let ziel = root.join(name);
    let manifest = manifest(id, version);
    let mut eintraege = vec![
        ("manifest.json", manifest.as_str()),
        ("theme.css", ":root { --bg: #000; }"),
    ];
    if mit_code {
        eintraege.push(("main.js", "export default {};"));
    }
    zip(&ziel, &eintraege);
    ziel
}

/// Namen der Einträge unter `<home>/themes` (leer, wenn der Ordner fehlt)
fn themes_ordner(root: &Path) -> Vec<String> {
    match std::fs::read_dir(root.join("home/themes")) {
        Ok(entries) => {
            let mut namen: Vec<String> = entries
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            namen.sort();
            namen
        }
        Err(_) => Vec::new(),
    }
}

#[test]
fn list_ohne_themes_meldet_keine() {
    let tmp = TempDir::new().unwrap();
    let lauf = theme(tmp.path(), &["list"]);
    assert_eq!(lauf.code, 0, "{}{}", lauf.stdout, lauf.stderr);
    assert!(
        lauf.stdout.contains("Keine Themes installiert."),
        "{}",
        lauf.stdout
    );
}

#[test]
fn install_legt_das_theme_ab_und_list_zeigt_es() {
    let tmp = TempDir::new().unwrap();
    let datei = paket(tmp.path(), "nacht.nexus", "nacht", "1.2.3", false);
    let lauf = theme(tmp.path(), &["install", datei.to_str().unwrap()]);
    assert_eq!(lauf.code, 0, "{}{}", lauf.stdout, lauf.stderr);
    assert!(lauf.stdout.contains("nacht"), "{}", lauf.stdout);
    assert!(lauf.stdout.contains("1.2.3"), "{}", lauf.stdout);
    assert!(!lauf.stdout.contains("Code"), "{}", lauf.stdout);
    assert!(tmp.path().join("home/themes/nacht/manifest.json").is_file());
    assert!(tmp.path().join("home/themes/nacht/theme.css").is_file());

    let liste = theme(tmp.path(), &["list"]);
    assert_eq!(liste.code, 0, "{}{}", liste.stdout, liste.stderr);
    assert!(liste.stdout.contains("nacht"), "{}", liste.stdout);
    assert!(liste.stdout.contains("1.2.3"), "{}", liste.stdout);
    assert!(liste.stdout.contains("Nacht"), "{}", liste.stdout);
    assert!(liste.stdout.contains("dark"), "{}", liste.stdout);
}

#[test]
fn paket_mit_main_js_weist_auf_den_code_hin() {
    let tmp = TempDir::new().unwrap();
    let datei = paket(tmp.path(), "nacht.nexus", "nacht", "1.0.0", true);
    let lauf = theme(tmp.path(), &["install", datei.to_str().unwrap()]);
    assert_eq!(lauf.code, 0, "{}{}", lauf.stdout, lauf.stderr);
    assert!(
        lauf.stdout.contains("bringt Code mit (main.js)"),
        "{}",
        lauf.stdout
    );
}

#[test]
fn erneutes_installieren_ersetzt_die_vorhandene_version() {
    let tmp = TempDir::new().unwrap();
    let alt = paket(tmp.path(), "alt.nexus", "nacht", "1.0.0", true);
    let neu = paket(tmp.path(), "neu.nexus", "nacht", "2.0.0", false);
    assert_eq!(
        theme(tmp.path(), &["install", alt.to_str().unwrap()]).code,
        0
    );
    let lauf = theme(tmp.path(), &["install", neu.to_str().unwrap()]);
    assert_eq!(lauf.code, 0, "{}{}", lauf.stdout, lauf.stderr);

    let liste = theme(tmp.path(), &["list"]);
    assert!(liste.stdout.contains("2.0.0"), "{}", liste.stdout);
    assert!(!liste.stdout.contains("1.0.0"), "{}", liste.stdout);
    assert!(!tmp.path().join("home/themes/nacht/main.js").exists());
    assert_eq!(themes_ordner(tmp.path()), vec!["nacht".to_string()]);
}

#[test]
fn list_ist_nach_id_sortiert() {
    let tmp = TempDir::new().unwrap();
    for id in ["zebra", "apfel", "mitte"] {
        let datei = paket(tmp.path(), &format!("{id}.nexus"), id, "1.0.0", false);
        assert_eq!(
            theme(tmp.path(), &["install", datei.to_str().unwrap()]).code,
            0
        );
    }
    let liste = theme(tmp.path(), &["list"]);
    let apfel = liste.stdout.find("apfel").expect("apfel");
    let mitte = liste.stdout.find("mitte").expect("mitte");
    let zebra = liste.stdout.find("zebra").expect("zebra");
    assert!(apfel < mitte && mitte < zebra, "{}", liste.stdout);
}

#[test]
fn datei_ohne_zip_wird_abgelehnt_und_hinterlaesst_nichts() {
    let tmp = TempDir::new().unwrap();
    let datei = tmp.path().join("kaputt.nexus");
    std::fs::write(&datei, "das ist kein zip").unwrap();
    let lauf = theme(tmp.path(), &["install", datei.to_str().unwrap()]);
    assert_ne!(lauf.code, 0);
    assert!(lauf.stderr.contains("kein gültiges ZIP"), "{}", lauf.stderr);
    assert!(themes_ordner(tmp.path()).is_empty());
}

#[test]
fn paket_ohne_theme_css_wird_abgelehnt_und_hinterlaesst_nichts() {
    let tmp = TempDir::new().unwrap();
    let datei = tmp.path().join("x.nexus");
    zip(&datei, &[("manifest.json", &manifest("nacht", "1.0.0"))]);
    let lauf = theme(tmp.path(), &["install", datei.to_str().unwrap()]);
    assert_ne!(lauf.code, 0);
    assert!(
        lauf.stderr.contains("Im Paket fehlt theme.css"),
        "{}",
        lauf.stderr
    );
    assert!(themes_ordner(tmp.path()).is_empty());
}

#[test]
fn ungueltiges_manifest_nennt_das_feld_und_hinterlaesst_nichts() {
    let tmp = TempDir::new().unwrap();
    let datei = paket(tmp.path(), "x.nexus", "Nacht", "1.0.0", false);
    let lauf = theme(tmp.path(), &["install", datei.to_str().unwrap()]);
    assert_ne!(lauf.code, 0);
    assert!(lauf.stderr.contains("id"), "{}", lauf.stderr);
    assert!(themes_ordner(tmp.path()).is_empty());
}

#[test]
fn fehlende_datei_wird_verstaendlich_gemeldet() {
    let tmp = TempDir::new().unwrap();
    let lauf = theme(tmp.path(), &["install", "gibts-nicht.nexus"]);
    assert_ne!(lauf.code, 0);
    assert!(
        lauf.stderr
            .contains("Datei nicht gefunden: gibts-nicht.nexus"),
        "{}",
        lauf.stderr
    );
    assert!(!lauf.stderr.contains("panicked"), "{}", lauf.stderr);
    assert!(themes_ordner(tmp.path()).is_empty());
}

#[test]
fn remove_entfernt_das_theme() {
    let tmp = TempDir::new().unwrap();
    let datei = paket(tmp.path(), "nacht.nexus", "nacht", "1.0.0", false);
    assert_eq!(
        theme(tmp.path(), &["install", datei.to_str().unwrap()]).code,
        0
    );
    let lauf = theme(tmp.path(), &["remove", "nacht"]);
    assert_eq!(lauf.code, 0, "{}{}", lauf.stdout, lauf.stderr);
    assert!(lauf.stdout.contains("nacht"), "{}", lauf.stdout);
    assert!(!tmp.path().join("home/themes/nacht").exists());
    let liste = theme(tmp.path(), &["list"]);
    assert!(
        liste.stdout.contains("Keine Themes installiert."),
        "{}",
        liste.stdout
    );
}

#[test]
fn remove_einer_unbekannten_id_scheitert() {
    let tmp = TempDir::new().unwrap();
    let lauf = theme(tmp.path(), &["remove", "gibts-nicht"]);
    assert_ne!(lauf.code, 0);
    assert!(
        lauf.stderr.contains("ist nicht installiert"),
        "{}",
        lauf.stderr
    );
}

#[test]
fn remove_mit_ungueltiger_id_loescht_nichts() {
    let tmp = TempDir::new().unwrap();
    let datei = paket(tmp.path(), "nacht.nexus", "nacht", "1.0.0", false);
    assert_eq!(
        theme(tmp.path(), &["install", datei.to_str().unwrap()]).code,
        0
    );
    let lauf = theme(tmp.path(), &["remove", "../x"]);
    assert_ne!(lauf.code, 0);
    assert!(
        lauf.stderr.contains("Ungültige Theme-ID"),
        "{}",
        lauf.stderr
    );
    assert!(tmp.path().join("home/themes/nacht/manifest.json").is_file());
}

#[test]
fn hilfe_listet_alle_befehle() {
    let tmp = TempDir::new().unwrap();
    let lauf = theme(tmp.path(), &["help"]);
    assert_eq!(lauf.code, 0, "{}{}", lauf.stdout, lauf.stderr);
    for name in ["pack", "install", "list", "remove"] {
        assert!(lauf.stdout.contains(name), "{name}: {}", lauf.stdout);
    }
}
