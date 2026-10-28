//! Gegenprobe der Anpassungen: Der Standard-Workflow als JSON, die Übersicht
//! `customizations/overview.json` und der Text von `check` müssen in Rust und in
//! TypeScript gleich aussehen. Beide Seiten schreiben dieselben Dateien, deshalb darf sich
//! kein Zeichen unterscheiden.
//!
//! Der Test braucht `node` und das gebaute `packages/core/dist` (`npm run build`).
//! Fehlt eines davon, wird er mit einem Hinweis übersprungen statt zu scheitern.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use nexus_core::customization::{
    BaseSnapshot, CloneWorkflowTools, CustomizationApplier, CustomizationInput,
    CustomizationOverview, OverviewFormatter,
};
use nexus_core::workflow::workflow_phases;
use nexus_core::Clock;

use common::make_temp_dir;

#[test]
fn der_standard_workflow_ist_in_rust_und_typescript_dasselbe_json() {
    let Some(index) = gebautes_paket() else {
        println!(
            "übersprungen: node oder packages/core/dist fehlt – zuerst `npm run build` ausführen"
        );
        return;
    };
    let aus_ts = node(&["workflow-json", &index]);
    let aus_rust = BaseSnapshot::format_json(&workflow_phases()).unwrap();
    assert_eq!(aus_rust, aus_ts);
}

#[test]
fn uebersicht_und_prueftext_sind_in_rust_und_typescript_gleich() {
    let Some(index) = gebautes_paket() else {
        println!(
            "übersprungen: node oder packages/core/dist fehlt – zuerst `npm run build` ausführen"
        );
        return;
    };
    let dir = make_temp_dir("harness-anpassungen-ts-");
    let root = dir.path().to_path_buf();
    let custom = root.join("custom");
    let standard = root.join("standard");
    // Standard mit zwei Skills und einem Output-Style, Anpassungen mit je einer Änderung
    skill(
        &standard.join("skills").join("harness-grill"),
        "harness-grill",
        "Standard",
    );
    skill(
        &standard.join("skills").join("harness-implement"),
        "harness-implement",
        "Standard",
    );
    datei(
        &standard.join("output-styles").join("software-engineer.md"),
        "---\nname: software-engineer\ndescription: Standard\n---\n",
    );
    skill(
        &custom.join("skills").join("harness-grill"),
        "harness-grill",
        "Eigen",
    );
    skill(
        &custom.join("skills").join("mein-review"),
        "mein-review",
        "Neu",
    );
    datei(
        &custom.join("output-styles").join("kurz.md"),
        "---\ndescription: Kurz\n---\n",
    );
    // Ein Skill mit Fehlern (Rückfall) und eine veraltete Basis (Warnung mit Diff)
    skill(&custom.join("skills").join("ohne-text"), "ohne-text", "");
    datei(
        &custom.join("skills").join("ohne-text").join("SKILL.md"),
        "---\nname: ohne-text\n---\nohne description",
    );
    datei(
        &custom
            .join(".base")
            .join("skills")
            .join("harness-grill")
            .join("SKILL.md"),
        "---\nname: harness-grill\ndescription: Alt\n---\nText\n",
    );

    // TypeScript wendet an und schreibt customizations/overview.json
    let plugin_ts = root.join("plugin-ts");
    kopie(&standard, &plugin_ts);
    node(&[
        "apply",
        &index,
        &text(&root),
        &text(&custom),
        &text(&standard),
        &text(&plugin_ts),
    ]);
    let aus_ts: CustomizationOverview = serde_json::from_str(
        &std::fs::read_to_string(plugin_ts.join("customizations").join("overview.json")).unwrap(),
    )
    .unwrap();

    // Rust wendet dieselben Anpassungen an
    let plugin_rust = root.join("plugin-rust");
    kopie(&standard, &plugin_rust);
    let tools = CloneWorkflowTools::new(&root);
    let input = CustomizationInput {
        clone_dir: root.clone(),
        custom_dir: custom.clone(),
        standard_dir: standard.clone(),
        harness_version: "0.2.0".to_string(),
        tools: &tools,
        clock: Clock::fixed(
            chrono::DateTime::parse_from_rfc3339("2026-10-14T10:00:00Z")
                .unwrap()
                .into(),
        ),
    };
    let aus_rust = CustomizationApplier::new()
        .apply(&input, &plugin_rust)
        .unwrap();

    assert_eq!(aus_rust, aus_ts);
    // Dieselbe Datei, Zeichen für Zeichen
    assert_eq!(
        std::fs::read_to_string(plugin_rust.join("customizations").join("overview.json")).unwrap(),
        std::fs::read_to_string(plugin_ts.join("customizations").join("overview.json")).unwrap()
    );

    // Und derselbe Text für `check`
    let text_ts = node(&[
        "check",
        &index,
        &text(&root),
        &text(&custom),
        &text(&standard),
    ]);
    let text_rust = OverviewFormatter::new().format(&aus_rust, None);
    assert_eq!(text_rust, text_ts);
    assert!(text_rust.contains("harness-grill (ersetzt)"));
    assert!(text_rust.contains("Rückfälle auf den Standard"));
}

/// Ein Skill mit Kopf und Text
fn skill(dir: &Path, name: &str, inhalt: &str) {
    datei(
        &dir.join("SKILL.md"),
        &format!("---\nname: {name}\ndescription: {inhalt}\n---\nText\n"),
    );
}

/// Eine Datei samt Ordnern darüber
fn datei(file: &Path, inhalt: &str) {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, inhalt).unwrap();
}

/// Kopiert einen Ordner mit allem darin (das gebaute Plugin wird überlagert)
fn kopie(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let ziel = to.join(entry.file_name());
        if entry.path().is_dir() {
            kopie(&entry.path(), &ziel);
        } else {
            std::fs::copy(entry.path(), ziel).unwrap();
        }
    }
}

/// Pfad des gebauten TypeScript-Pakets, falls es ihn, seine Abhängigkeiten und `node` gibt
fn gebautes_paket() -> Option<String> {
    let datei = repo_root().join("packages/core/dist/index.js");
    // Der Front-Matter-Leser braucht das Paket `yaml`; ohne node_modules geht nichts
    let yaml_da = repo_root().join("node_modules/yaml").is_dir();
    let node_da = Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    (datei.is_file() && yaml_da && node_da).then(|| text(&datei))
}

/// Wurzel des Arbeitsbereichs (zwei Ebenen über `crates/nexus-core`)
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/nexus-core liegt im Arbeitsbereich")
        .to_path_buf()
}

/// Führt das Gegenstück in Node aus und liefert dessen Ausgabe
fn node(args: &[&str]) -> String {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/anpassungen_ts.mjs");
    let output = Command::new("node")
        .arg(script)
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .expect("node startet");
    assert!(output.status.success(), "node endete mit {}", output.status);
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Pfad als Text für die Übergabe an Node
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
