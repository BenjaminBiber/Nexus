//! Gegenprobe zu den gemeinsamen Dateien: `board.yaml`, `board-repos.yaml` und
//! `settings.json` werden von TypeScript geschrieben, von Rust gelesen und verändert
//! und wieder von TypeScript gelesen. Die Dateien gehören beiden Seiten, deshalb muss
//! jede lesen können, was die andere schreibt.
//!
//! Der Test braucht `node` und das gebaute `packages/core/dist`
//! (`npx tsc -b packages/core`). Fehlt eines davon, wird er mit einem Hinweis
//! übersprungen statt zu scheitern.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use nexus_core::board::{BoardConfigStore, BoardProvider, BoardRepoEntries, BoardRepoFile};
use nexus_core::registry::HarnessHome;
use nexus_core::settings::{NotificationMode, ThemeMode, UserSettingsStore};

use common::make_temp_dir;

#[test]
fn typescript_und_rust_lesen_und_schreiben_dieselben_dateien() {
    let Some(index) = gebautes_paket() else {
        println!(
            "übersprungen: node oder packages/core/dist fehlt – für die Gegenprobe zuerst `npx tsc -b packages/core` ausführen"
        );
        return;
    };
    let dir = make_temp_dir("harness-dateien-ts-");
    let home = HarnessHome::at(dir.path());

    // 1. TypeScript schreibt die drei Dateien
    node(&["schreiben", &index, &text(dir.path())]);

    // 2. Rust liest sie und findet dieselben Werte
    let config = BoardConfigStore::new(home.clone()).require().unwrap();
    assert_eq!(config.provider, BoardProvider::Mssql);
    assert_eq!(config.server, "dev-sql-5");
    assert_eq!(config.password, "geheim 1234");
    assert_eq!(config.user_name, "Florian Müller");
    assert_eq!(config.active_board, Some(7));
    // Ohne eigenen Pfad gilt die Standarddatei unter HARNESS_HOME
    assert_eq!(config.file, text(&dir.path().join("board.db")));

    let repos = BoardRepoFile::new(home.clone());
    assert_eq!(
        repos.read().unwrap(),
        BoardRepoEntries::from([
            (7, Some("C:\\Projects\\Arches ä".to_string())),
            (8, None),
            (12, Some("/home/x/mein repo".to_string())),
        ])
    );

    let settings_store = UserSettingsStore::new(home.clone());
    let settings = settings_store.read();
    assert_eq!(settings.notifications, NotificationMode::Windows);
    assert_eq!(settings.theme, ThemeMode::Dark);
    assert_eq!(settings.ntfy_topic, "harness-x");
    assert!(settings.hero_images);
    assert!(settings.hide_in_planning);

    // 3. Rust schreibt die Dateien verändert zurück
    let mut geandert = config.clone();
    geandert.server = "rust-sql ä".into();
    geandert.active_board = Some(9);
    BoardConfigStore::new(home.clone())
        .write(&geandert)
        .unwrap();

    repos
        .write(&BoardRepoEntries::from([
            (7, Some("D:\\Neu\\Pfad ö".to_string())),
            (13, None),
        ]))
        .unwrap();

    let mut neue_einstellungen = settings;
    neue_einstellungen.theme = ThemeMode::Win95;
    neue_einstellungen.ntfy_topic = "rust-topic".into();
    settings_store.write(&neue_einstellungen).unwrap();

    // 4. TypeScript liest, was Rust geschrieben hat
    let gelesen = node(&["lesen", &index, &text(dir.path())]);
    let gelesen: serde_json::Value = serde_json::from_str(&gelesen).unwrap();

    assert_eq!(gelesen["config"]["server"], serde_json::json!("rust-sql ä"));
    assert_eq!(gelesen["config"]["activeBoard"], serde_json::json!(9));
    assert_eq!(
        gelesen["config"]["password"],
        serde_json::json!("geheim 1234")
    );
    assert_eq!(
        gelesen["config"]["userName"],
        serde_json::json!("Florian Müller")
    );
    assert_eq!(
        gelesen["repos"],
        serde_json::json!({ "7": "D:\\Neu\\Pfad ö", "13": null })
    );
    assert_eq!(gelesen["settings"]["theme"], serde_json::json!("win95"));
    assert_eq!(
        gelesen["settings"]["ntfyTopic"],
        serde_json::json!("rust-topic")
    );
    assert_eq!(gelesen["settings"]["heroImages"], serde_json::json!(true));
}

/// Pfad des gebauten TypeScript-Pakets, falls es ihn, seine Abhängigkeiten und `node` gibt
fn gebautes_paket() -> Option<String> {
    let datei = repo_root().join("packages/core/dist/index.js");
    // Der Board-Konfig-Speicher liest YAML über das Paket `yaml`; ohne node_modules geht nichts
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
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/dateien_ts.mjs");
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
