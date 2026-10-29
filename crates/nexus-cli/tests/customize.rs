//! `nexus customize …` von außen: prüfen, kopieren, zurücksetzen, anwenden und anzeigen.
//! Entspricht `packages/plugin/tests/customizeScript.test.ts`, nur gegen das Binary.

use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::cargo::CommandCargoExt;
use tempfile::TempDir;

/// Ergebnis eines Aufrufs: Rückgabecode und beide Ausgaben
struct Lauf {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Lauf {
    /// Alles, was der Aufruf geschrieben hat
    fn ausgabe(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Ein kleiner Clone: `packages/plugin/manifest` mit einem Skill und einem Output-Style
struct Clone {
    dir: TempDir,
}

impl Clone {
    fn neu() -> Self {
        let clone = Self {
            dir: TempDir::new().expect("Temp-Ordner"),
        };
        let manifest = clone.manifest();
        datei(
            &manifest
                .join("skills")
                .join("harness-grill")
                .join("SKILL.md"),
            "---\nname: harness-grill\ndescription: Standard\n---\nText\n",
        );
        datei(
            &manifest.join("output-styles").join("software-engineer.md"),
            "---\nname: software-engineer\ndescription: Standard\n---\n",
        );
        datei(
            &clone
                .root()
                .join("packages")
                .join("server")
                .join("package.json"),
            "{ \"version\": \"0.2.0\" }",
        );
        clone
    }

    fn root(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }

    fn manifest(&self) -> PathBuf {
        self.root().join("packages").join("plugin").join("manifest")
    }

    fn custom(&self) -> PathBuf {
        self.root().join("custom")
    }

    /// Ruft `nexus customize …` mit diesem Clone und seinem Ordner der Anpassungen auf
    fn run(&self, args: &[&str]) -> Lauf {
        let mut befehl = std::process::Command::cargo_bin("nexus").expect("Binary nexus");
        befehl
            .arg("customize")
            .arg("--clone")
            .arg(self.root())
            .arg("--custom")
            .arg(self.custom())
            .args(args)
            .current_dir(self.dir.path())
            // Eigener Benutzerordner, damit der Lauf den echten nicht anfasst
            .env("NEXUS_HOME", self.root().join("home"))
            .env_remove("HARNESS_HOME")
            .env_remove("HARNESS_CUSTOM_DIR")
            .env_remove("NEXUS_CUSTOM_DIR")
            .env_remove("HARNESS_SESSION");
        let output: Output = befehl.output().expect("nexus startet");
        Lauf {
            code: output.status.code().expect("Rückgabecode"),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

/// Eine Datei samt Ordnern darüber
fn datei(file: &Path, inhalt: &str) {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, inhalt).unwrap();
}

#[test]
fn pruft_ohne_anpassungen() {
    let clone = Clone::neu();
    let lauf = clone.run(&["check"]);
    assert_eq!(lauf.code, 0, "{}", lauf.ausgabe());
    assert!(
        lauf.stdout
            .contains("Keine Anpassungen, überall gilt der Standard."),
        "{}",
        lauf.stdout
    );
}

#[test]
fn kopiert_pruft_und_setzt_zuruck() {
    let clone = Clone::neu();
    assert_eq!(clone.run(&["copy", "skill", "harness-grill"]).code, 0);
    assert!(clone
        .custom()
        .join("skills")
        .join("harness-grill")
        .join("SKILL.md")
        .exists());
    assert!(clone
        .run(&["check"])
        .stdout
        .contains("harness-grill (ersetzt)"));
    assert_eq!(clone.run(&["reset", "skill", "harness-grill"]).code, 0);
    assert!(!clone.custom().join("skills").join("harness-grill").exists());
}

#[test]
fn nennt_bei_falschem_aufruf_die_verwendung() {
    let clone = Clone::neu();
    let lauf = clone.run(&["löschen"]);
    assert_eq!(lauf.code, 1);
    assert!(
        lauf.ausgabe().contains("Aufruf: nexus customize check |"),
        "{}",
        lauf.ausgabe()
    );
    // Ohne Unterbefehl gilt dasselbe
    let leer = clone.run(&[]);
    assert_eq!(leer.code, 1);
    assert!(leer.ausgabe().contains("Aufruf: nexus customize check |"));
}

#[test]
fn kopiert_bei_einer_unbekannten_option_nichts() {
    let clone = Clone::neu();
    let lauf = clone.run(&["copy", "skill", "harness-grill", "--rebas"]);
    assert_eq!(lauf.code, 1);
    assert!(lauf.ausgabe().contains("Aufruf: nexus customize check |"));
    assert!(!clone.custom().join("skills").join("harness-grill").exists());
}

#[test]
fn der_aufruftext_lasst_sich_austauschen() {
    let clone = Clone::neu();
    let lauf = clone.run(&["--aufruf", "npm run customize --", "löschen"]);
    assert_eq!(lauf.code, 1);
    assert!(
        lauf.ausgabe()
            .contains("Aufruf: npm run customize -- check |"),
        "{}",
        lauf.ausgabe()
    );
}

#[test]
fn wendet_die_anpassungen_auf_ein_gebautes_plugin_an() {
    let clone = Clone::neu();
    datei(
        &clone
            .custom()
            .join("skills")
            .join("mein-review")
            .join("SKILL.md"),
        "---\nname: mein-review\ndescription: Neu\n---\nText\n",
    );
    let out = clone.root().join("plugin");
    datei(
        &out.join("skills").join("harness-grill").join("SKILL.md"),
        "---\nname: harness-grill\ndescription: Standard\n---\nText\n",
    );
    let lauf = clone.run(&[
        "apply",
        "--standard",
        &out.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
    ]);
    assert_eq!(lauf.code, 0, "{}", lauf.ausgabe());
    assert!(out
        .join("skills")
        .join("mein-review")
        .join("SKILL.md")
        .exists());
    let overview: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("customizations").join("overview.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(overview["version"], serde_json::json!(1));
    assert_eq!(overview["workflow"], serde_json::json!("standard"));
    assert_eq!(overview["harnessVersion"], serde_json::json!("0.2.0"));
    assert_eq!(
        overview["entries"][0]["name"],
        serde_json::json!("mein-review")
    );

    // Dieselbe Datei liest `overview` wieder und zeigt sie als Text
    let gezeigt = clone.run(&[
        "overview",
        "--file",
        &out.join("customizations")
            .join("overview.json")
            .to_string_lossy(),
    ]);
    assert_eq!(gezeigt.code, 0, "{}", gezeigt.ausgabe());
    assert!(
        gezeigt.stdout.contains("mein-review (neu)"),
        "{}",
        gezeigt.stdout
    );
}

#[test]
fn ohne_ubersicht_warnt_overview_und_endet_mit_null() {
    let clone = Clone::neu();
    let lauf = clone.run(&[
        "overview",
        "--file",
        &clone.root().join("gibt-es-nicht.json").to_string_lossy(),
    ]);
    assert_eq!(lauf.code, 0);
    assert!(
        lauf.stderr
            .contains("Keine Übersicht der Anpassungen gefunden"),
        "{}",
        lauf.stderr
    );
}

#[test]
fn die_hilfe_nennt_die_unterbefehle() {
    let mut befehl = std::process::Command::cargo_bin("nexus").expect("Binary nexus");
    let output = befehl
        .args(["customize", "--help"])
        .output()
        .expect("nexus startet");
    let hilfe = String::from_utf8_lossy(&output.stdout);
    assert!(
        hilfe.starts_with("Aufruf: nexus customize [options] [command]\n"),
        "{hilfe}"
    );
    for teil in [
        "apply",
        "overview",
        "check",
        "copy",
        "reset",
        "--clone <ordner>",
    ] {
        assert!(hilfe.contains(teil), "{teil} fehlt in:\n{hilfe}");
    }
}
