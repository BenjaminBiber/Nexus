//! `nexus theme pack …` von außen: Paket bauen, Fehler mit Datei und Grund.

use std::path::Path;

use assert_cmd::cargo::CommandCargoExt;
use nexus_core::NexusHome;
use tempfile::TempDir;

/// Ergebnis eines Aufrufs: Rückgabecode und beide Ausgaben
struct Lauf {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Ruft `nexus theme pack …` im Ordner `cwd` auf
fn pack(cwd: &Path, args: &[&str]) -> Lauf {
    let output = std::process::Command::cargo_bin("nexus")
        .expect("Binary nexus")
        .args(["theme", "pack"])
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

/// Eine Datei samt Ordnern darüber
fn datei(file: &Path, inhalt: &str) {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, inhalt).unwrap();
}

/// Ein gültiger Theme-Ordner `nacht` unter `root`
fn theme(root: &Path, id: &str) -> std::path::PathBuf {
    let dir = root.join("nacht");
    datei(
        &dir.join("manifest.json"),
        &format!(
            r#"{{"id":"{id}","name":"Nacht","version":"1.2.3","author":"Ada","sdkVersion":"^1.0","colorScheme":"dark"}}"#
        ),
    );
    datei(&dir.join("theme.css"), ":root { --bg: url(assets/a.png); }");
    datei(&dir.join("main.js"), "export default {};");
    datei(&dir.join("assets/a.png"), "png");
    dir
}

/// Alle `.nexus`-Dateien unter `dir` (nicht rekursiv)
fn pakete(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".nexus"))
        .collect()
}

#[test]
fn mit_out_entsteht_ein_installierbares_paket() {
    let tmp = TempDir::new().unwrap();
    theme(tmp.path(), "nacht");
    let lauf = pack(tmp.path(), &["nacht", "--out", "dist/x.nexus"]);
    assert_eq!(lauf.code, 0, "{}{}", lauf.stdout, lauf.stderr);
    assert!(lauf.stdout.contains("x.nexus"), "{}", lauf.stdout);
    assert!(lauf.stdout.contains("nacht 1.2.3"), "{}", lauf.stdout);

    let bytes = std::fs::read(tmp.path().join("dist/x.nexus")).unwrap();
    let home = NexusHome::at(tmp.path().join("home"));
    nexus_core::themes::install_theme(&home, &bytes).unwrap();
    assert!(home.themes_dir().join("nacht/main.js").is_file());
    assert!(home.themes_dir().join("nacht/assets/a.png").is_file());
}

#[test]
fn ohne_out_entsteht_id_version_im_aktuellen_ordner() {
    let tmp = TempDir::new().unwrap();
    theme(tmp.path(), "nacht");
    let lauf = pack(tmp.path(), &["nacht"]);
    assert_eq!(lauf.code, 0, "{}{}", lauf.stdout, lauf.stderr);
    assert_eq!(pakete(tmp.path()), vec!["nacht-1.2.3.nexus".to_string()]);
}

#[test]
fn ungueltiges_manifest_bricht_ohne_ausgabedatei_ab() {
    let tmp = TempDir::new().unwrap();
    theme(tmp.path(), "Nacht");
    let lauf = pack(tmp.path(), &["nacht"]);
    assert_ne!(lauf.code, 0);
    assert!(lauf.stderr.contains("manifest.json"), "{}", lauf.stderr);
    assert!(lauf.stderr.contains("id"), "{}", lauf.stderr);
    assert!(pakete(tmp.path()).is_empty());
}

#[test]
fn unerlaubte_datei_bricht_ohne_ausgabedatei_ab() {
    let tmp = TempDir::new().unwrap();
    let dir = theme(tmp.path(), "nacht");
    datei(&dir.join("assets/notes.exe"), "MZ");
    let lauf = pack(tmp.path(), &["nacht", "--out", "x.nexus"]);
    assert_ne!(lauf.code, 0);
    assert!(
        lauf.stderr
            .contains("Dateityp nicht erlaubt: assets/notes.exe"),
        "{}",
        lauf.stderr
    );
    assert!(pakete(tmp.path()).is_empty());
}

#[test]
fn fehlender_ordner_wird_verstaendlich_gemeldet() {
    let tmp = TempDir::new().unwrap();
    let lauf = pack(tmp.path(), &["gibts-nicht"]);
    assert_ne!(lauf.code, 0);
    assert!(
        lauf.stderr.contains("Ordner nicht gefunden: gibts-nicht"),
        "{}",
        lauf.stderr
    );
    assert!(!lauf.stderr.contains("panicked"), "{}", lauf.stderr);
}

/// Ordner der Vorlage `themes/_template` im Repo
fn vorlage() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/_template")
}

#[test]
fn manifest_und_css_der_vorlage_sind_pack_und_installierbar() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("vorlage");
    for name in ["manifest.json", "theme.css"] {
        datei(&dir.join(name), &std::fs::read_to_string(vorlage().join(name)).unwrap());
    }
    let paket = nexus_core::themes::pack_theme(&dir).unwrap();
    assert_eq!(paket.manifest.id, "my-theme");

    let home = NexusHome::at(tmp.path().join("home"));
    let manifest = nexus_core::themes::install_theme(&home, &paket.bytes).unwrap();
    assert_eq!(manifest.slots, vec!["Shell".to_string()]);
    assert!(home.themes_dir().join("my-theme/theme.css").is_file());
}
