//! Gegenprobe zum Verzeichnis der Worktrees: `harness/worktrees.json` wird von TypeScript
//! geschrieben, von Rust gelesen und verändert und wieder von TypeScript gelesen. Die Datei
//! gehört beiden Seiten (CLI in Rust, Server in Node), deshalb muss jede lesen können, was
//! die andere schreibt.
//!
//! Der Test braucht `node` und das gebaute `packages/core/dist`
//! (`npx tsc -b packages/core`). Fehlt eines davon, wird er mit einem Hinweis übersprungen
//! statt zu scheitern.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use nexus_core::project::HarnessLayout;
use nexus_core::worktrees::{SetupState, WorktreeRegistry};

use common::make_temp_dir;

#[test]
fn registry_gegenprobe() {
    let Some(index) = gebautes_paket() else {
        println!(
            "übersprungen: node oder packages/core/dist fehlt – für die Gegenprobe zuerst `npx tsc -b packages/core` ausführen"
        );
        return;
    };
    let dir = make_temp_dir("harness-worktrees-ts-");
    let common_dir = dir.path().join(".git");
    fs::create_dir_all(common_dir.join("harness")).unwrap();
    let registry = WorktreeRegistry::new(HarnessLayout::new(&common_dir));

    // 1. TypeScript schreibt das Verzeichnis
    node(&["schreiben", &index, &text(&common_dir)]);

    // 2. Rust liest es und findet dieselben Werte
    let map = registry.read().unwrap();
    assert_eq!(map.len(), 2);
    let erster = map.get("ticket/ARC-0001-von-ts").expect("Eintrag aus TS");
    assert_eq!(erster.path, "/pfad/wt ä");
    assert_eq!(erster.tickets, ["ARC-0001", "ARC-0002"]);
    assert!(erster.ready_to_merge);
    assert_eq!(erster.setup, Some(SetupState::Pending));
    assert_eq!(erster.removed, None);
    let zweiter = map.get("ticket/ARC-0003-zweiter").expect("Eintrag aus TS");
    assert!(zweiter.is_removed());
    assert!(!zweiter.ready_to_merge);

    // 3. Rust schreibt das Verzeichnis verändert zurück
    registry
        .add_ticket("ticket/ARC-0001-von-ts", "ARC-0004")
        .unwrap();
    registry.set_ready("ticket/ARC-0001-von-ts", false).unwrap();
    registry
        .set_setup("ticket/ARC-0001-von-ts", SetupState::Done)
        .unwrap();
    registry.mark_restored("ticket/ARC-0003-zweiter").unwrap();
    registry
        .add("ticket/ARC-0005-von-rust", "/pfad/drei ö", "ARC-0005")
        .unwrap();

    // 4. TypeScript liest, was Rust geschrieben hat
    let gelesen: serde_json::Value =
        serde_json::from_str(&node(&["lesen", &index, &text(&common_dir)])).unwrap();

    assert_eq!(
        gelesen,
        serde_json::json!({
            "ticket/ARC-0001-von-ts": {
                "path": "/pfad/wt ä",
                "tickets": ["ARC-0001", "ARC-0002", "ARC-0004"],
                "readyToMerge": false,
                "setup": "done",
            },
            "ticket/ARC-0003-zweiter": {
                "path": "/pfad/zwei",
                "tickets": ["ARC-0003"],
                "readyToMerge": false,
                "removed": false,
            },
            "ticket/ARC-0005-von-rust": {
                "path": "/pfad/drei ö",
                "tickets": ["ARC-0005"],
                "readyToMerge": false,
            },
        })
    );

    // Dieselbe Form wie in TypeScript: zwei Leerzeichen Einrückung, kein Zeilenende am Schluss
    let datei = fs::read_to_string(common_dir.join("harness").join("worktrees.json")).unwrap();
    assert!(
        datei.starts_with("{\n  \"ticket/ARC-0001-von-ts\": {\n    \"path\""),
        "{datei}"
    );
    assert!(datei.ends_with('}'), "{datei}");
}

/// Pfad des gebauten TypeScript-Pakets, falls es ihn und `node` gibt
fn gebautes_paket() -> Option<String> {
    let datei = repo_root().join("packages/core/dist/index.js");
    let node_da = Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    (datei.is_file() && node_da).then(|| text(&datei))
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
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/worktrees_ts.mjs");
    let output = Command::new("node")
        .arg(script)
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .expect("node startet");
    assert!(output.status.success(), "node endete mit {}", output.status);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Pfad als Text für die Übergabe an Node
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
