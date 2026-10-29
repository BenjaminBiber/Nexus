//! `nexus init` von außen: Projekt einrichten, Plugin einschalten, Rules anlegen, Import in
//! Claude Code freigeben und auf die Board-Datenbank hinweisen. Portiert aus
//! `packages/cli/tests/integration/init.test.ts` und `pluginSwitch.test.ts`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use assert_cmd::cargo::CommandCargoExt;
use tempfile::TempDir;

/// Ergebnis eines Aufrufs
struct Lauf {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Temporäres Git-Projekt mit eigenem `HARNESS_HOME` und eigener Claude-Konfiguration
struct TempProject {
    base: TempDir,
    path: PathBuf,
}

impl TempProject {
    /// Legt das Projekt mit einem ersten Commit an (Ordnername mit Leerzeichen und Umlaut)
    fn new() -> Self {
        let base = tempfile::Builder::new()
            .prefix("harness-init-")
            .tempdir()
            .unwrap();
        let path = fs::canonicalize(base.path())
            .unwrap()
            .join("Demo Projekt ä");
        fs::create_dir(&path).unwrap();
        fs::create_dir(fs::canonicalize(base.path()).unwrap().join("home")).unwrap();
        fs::create_dir(fs::canonicalize(base.path()).unwrap().join("claude")).unwrap();
        let project = Self { base, path };
        project.git(&["init", "-q", "-b", "main"]);
        project.git(&["config", "user.email", "test@example.invalid"]);
        project.git(&["config", "user.name", "Test"]);
        fs::write(project.path.join("a.txt"), "x\n").unwrap();
        project.git(&["add", "-A"]);
        project.git(&["commit", "-qm", "erster"]);
        project
    }

    /// Ordner, in dem Projekt, Home und Claude-Konfiguration liegen
    fn base(&self) -> PathBuf {
        fs::canonicalize(self.base.path()).unwrap()
    }

    /// Führt Git im Projekt aus und liefert die getrimmte Ausgabe
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Ruft `nexus` im Projekt auf, mit eigenem Home und eigener Claude-Konfiguration
    fn run(&self, args: &[&str]) -> Lauf {
        self.run_in(&self.path, args)
    }

    /// Wie `run`, aber in einem anderen Arbeitsordner
    fn run_in(&self, cwd: &Path, args: &[&str]) -> Lauf {
        let output = Command::cargo_bin("nexus")
            .expect("Binary nexus")
            .args(args)
            .current_dir(cwd)
            .env("NEXUS_HOME", self.base().join("home"))
            .env_remove("HARNESS_HOME")
            .env("CLAUDE_CONFIG_DIR", self.base().join("claude"))
            .env_remove("HARNESS_SESSION")
            .output()
            .expect("nexus startet");
        Lauf {
            code: output.status.code().expect("Rückgabecode"),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Konfigurationsdatei von Claude Code dieses Projekts
    fn claude_config(&self) -> PathBuf {
        self.base().join("claude").join(".claude.json")
    }
}

/// Die Ausgabe von `--json` als Wert
fn json(lauf: &Lauf) -> serde_json::Value {
    serde_json::from_str(&lauf.stdout).unwrap_or_else(|error| panic!("{error}: {}", lauf.stdout))
}

#[test]
fn richtet_das_projekt_ohne_ticket_ordner_ein_und_weist_auf_die_fehlende_board_datenbank_hin() {
    let projekt = TempProject::new();

    let lauf = projekt.run(&["init"]);

    assert_eq!(lauf.stderr, "");
    assert_eq!(lauf.code, 0);
    assert!(
        lauf.stdout.contains("Nexus eingerichtet"),
        "{}",
        lauf.stdout
    );
    assert!(
        lauf.stdout
            .contains("Board-Datenbank ist noch nicht eingerichtet"),
        "{}",
        lauf.stdout
    );
    assert!(projekt
        .path
        .join(".git")
        .join("harness")
        .join("token")
        .exists());
    assert!(!projekt
        .path
        .join(".git")
        .join("harness")
        .join("tickets")
        .exists());
}

#[test]
fn liefert_mit_json_ordner_plugin_schalter_und_board_hinweis() {
    let projekt = TempProject::new();

    let wert = json(&projekt.run(&["--json", "init"]));

    assert_eq!(wert["pluginEnabled"], serde_json::Value::Bool(true));
    assert_eq!(wert["board"], serde_json::Value::Null);
    assert!(
        wert["boardHint"]
            .as_str()
            .unwrap()
            .contains("Board-Datenbank"),
        "{wert}"
    );
}

#[test]
fn ist_wiederholbar_und_schnell() {
    let projekt = TempProject::new();
    projekt.run(&["init"]);

    let start = Instant::now();
    assert_eq!(projekt.run(&["init"]).code, 0);
    assert!(start.elapsed().as_millis() < 5000, "{:?}", start.elapsed());
}

#[test]
fn init_ausserhalb_eines_git_projekts_meldet_das_und_legt_nichts_an() {
    let projekt = TempProject::new();
    let ohne_git = tempfile::Builder::new()
        .prefix("harness-nogit-")
        .tempdir()
        .unwrap();

    let lauf = projekt.run_in(&fs::canonicalize(ohne_git.path()).unwrap(), &["init"]);

    assert_eq!(lauf.code, 1);
    assert!(lauf.stderr.contains("kein Git-Projekt"), "{}", lauf.stderr);
}

#[test]
fn schaltet_das_plugin_fur_das_projekt_ein_und_weist_auf_das_neuladen_hin() {
    let projekt = TempProject::new();

    let lauf = projekt.run(&["init"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(lauf.stdout.contains("reload-plugins"), "{}", lauf.stdout);
    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(projekt.path.join(".claude").join("settings.local.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        settings["enabledPlugins"]["nexus@nexus-local"],
        serde_json::Value::Bool(true)
    );
    // Wiederholung bleibt ohne Hinweis
    assert!(!projekt.run(&["init"]).stdout.contains("reload-plugins"));
}

#[test]
fn stellt_ein_projekt_vom_alten_plugin_auf_nexus_um() {
    let projekt = TempProject::new();
    // Ein Projekt, wie `harness init` es hinterlassen hat
    projekt.run(&["init"]);
    let claude = projekt.path.join(".claude");
    let rules = claude.join("rules");
    fs::write(
        claude.join("settings.local.json"),
        "{\n  \"enabledPlugins\": {\n    \"ai-harness@ai-harness-local\": true,\n    \"fremd@markt\": true\n  }\n}\n",
    )
    .unwrap();
    for (name, neu) in [
        ("ai-harness-docs.md", "nexus-docs.md"),
        ("ai-harness-board.md", "nexus-board.md"),
    ] {
        let alt = fs::read_to_string(rules.join(neu)).unwrap();
        fs::write(
            rules.join(name),
            alt.replace("`nexus init`", "`harness init`"),
        )
        .unwrap();
        fs::remove_file(rules.join(neu)).unwrap();
    }

    let wert = json(&projekt.run(&["--json", "init"]));

    assert_eq!(wert["migratedFrom"], "ai-harness@ai-harness-local");
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(claude.join("settings.local.json")).unwrap())
            .unwrap();
    let plugins = &settings["enabledPlugins"];
    assert_eq!(plugins["nexus@nexus-local"], serde_json::Value::Bool(true));
    assert!(
        plugins.get("ai-harness@ai-harness-local").is_none(),
        "{plugins}"
    );
    // Fremde Einträge bleiben
    assert_eq!(plugins["fremd@markt"], serde_json::Value::Bool(true));
    // Die alten Rules sind abgelöst, die neuen da
    assert!(!rules.join("ai-harness-docs.md").exists());
    assert!(!rules.join("ai-harness-board.md").exists());
    assert!(rules.join("nexus-docs.md").exists());
    assert!(rules.join("nexus-board.md").exists());
}

#[test]
fn eine_von_hand_geanderte_alte_rule_bleibt_liegen() {
    let projekt = TempProject::new();
    projekt.run(&["init"]);
    let rules = projekt.path.join(".claude").join("rules");
    fs::write(rules.join("ai-harness-board.md"), "# Meine eigene Regel\n").unwrap();

    let lauf = projekt.run(&["init"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(rules.join("ai-harness-board.md").exists());
    assert!(lauf.stdout.contains("bleiben liegen"), "{}", lauf.stdout);
    assert_eq!(
        fs::read_to_string(rules.join("ai-harness-board.md")).unwrap(),
        "# Meine eigene Regel\n"
    );
}

#[test]
fn legt_die_rule_fur_die_doku_regeln_an_ohne_sie_zu_versionieren() {
    let projekt = TempProject::new();

    let wert = json(&projekt.run(&["--json", "init"]));

    let docs_rule = wert["docsRule"].as_str().unwrap().replace('\\', "/");
    assert!(
        docs_rule.ends_with("/Demo Projekt ä/.claude/rules/nexus-docs.md"),
        "{docs_rule}"
    );
    let text = fs::read_to_string(
        projekt
            .path
            .join(".claude")
            .join("rules")
            .join("nexus-docs.md"),
    )
    .unwrap();
    assert!(text.contains('@'), "{text}");
    assert!(!projekt
        .git(&["status", "--porcelain", "--untracked-files=all"])
        .contains("nexus-docs.md"));
}

#[test]
fn legt_die_immer_geladene_board_regel_an_und_erganzt_sie_in_schon_eingerichteten_projekten() {
    let projekt = TempProject::new();
    projekt.run(&["init"]);
    let board_rule = projekt
        .path
        .join(".claude")
        .join("rules")
        .join("nexus-board.md");
    fs::remove_file(&board_rule).unwrap();

    let lauf = projekt.run(&["init"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(
        lauf.stdout
            .contains("Board-Regel eingebunden: .claude/rules/nexus-board.md"),
        "{}",
        lauf.stdout
    );
    let text = fs::read_to_string(&board_rule).unwrap();
    assert!(!text.contains("paths:"), "{text}");
    assert!(
        text.contains("\n@") && text.trim_end().ends_with("rules/board.md"),
        "{text}"
    );
    assert!(!projekt
        .git(&["status", "--porcelain", "--untracked-files=all"])
        .contains("nexus-board.md"));
    let wert = json(&projekt.run(&["--json", "init"]));
    assert!(wert["boardRule"]
        .as_str()
        .unwrap()
        .replace('\\', "/")
        .ends_with("/.claude/rules/nexus-board.md"));
    assert!(!projekt
        .run(&["init"])
        .stdout
        .contains("Board-Regel eingebunden"));
}

#[test]
fn gibt_den_import_der_doku_regeln_in_der_konfiguration_von_claude_code_frei() {
    let projekt = TempProject::new();
    fs::write(projekt.claude_config(), "{\"projects\":{}}").unwrap();

    let lauf = projekt.run(&["--json", "init"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert_eq!(json(&lauf)["importApproval"]["status"], "approved");
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(projekt.claude_config()).unwrap()).unwrap();
    let projects = config["projects"].as_object().unwrap();
    assert!(!projects.is_empty(), "{config}");
    for (key, entry) in projects {
        assert_eq!(
            entry["hasClaudeMdExternalIncludesApproved"],
            serde_json::Value::Bool(true),
            "{key}"
        );
    }
}

#[test]
fn ohne_konfiguration_von_claude_code_weist_init_auf_den_ersten_start_hin() {
    let projekt = TempProject::new();

    let lauf = projekt.run(&["init"]);

    assert_eq!(
        json(&projekt.run(&["--json", "init"]))["importApproval"]["status"],
        "no-config"
    );
    assert!(
        lauf.stdout
            .contains("Claude Code lief auf diesem Rechner noch nie"),
        "{}",
        lauf.stdout
    );
    assert!(!projekt.claude_config().exists());
}

#[test]
fn meldet_eine_nicht_erreichbare_board_datenbank_nur_als_hinweis_und_endet_mit_0() {
    let projekt = TempProject::new();
    // SQL Server auf Port 1: die Verbindung scheitert sofort
    fs::write(
        projekt.base().join("home").join("board.yaml"),
        "provider: mssql\nserver: 127.0.0.1\nport: 1\ndatabase: AiHarness_Nirgends\nuser: x\npassword: x\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: Test\n",
    )
    .unwrap();

    let lauf = projekt.run(&["--json", "init"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    let wert = json(&lauf);
    assert_eq!(wert["board"], serde_json::Value::Null);
    assert!(
        wert["boardHint"]
            .as_str()
            .unwrap()
            .contains("Board-Datenbank"),
        "{wert}"
    );
}

#[test]
fn wartet_hochstens_kurz_auf_die_board_datenbank_und_endet_mit_0() {
    let projekt = TempProject::new();
    // Nicht routbare Adresse: die Verbindung hängt, bis das Zeitlimit greift
    fs::write(
        projekt.base().join("home").join("board.yaml"),
        "provider: mssql\nserver: 10.255.255.1\nport: 1433\ndatabase: AiHarness_Nirgends\nuser: x\npassword: x\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: Test\n",
    )
    .unwrap();

    let start = Instant::now();
    let lauf = projekt.run(&["init"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(
        lauf.stdout.contains("Nexus eingerichtet"),
        "{}",
        lauf.stdout
    );
    assert!(lauf.stdout.contains("Board-Datenbank"), "{}", lauf.stdout);
    assert!(start.elapsed().as_millis() < 7000, "{:?}", start.elapsed());
}
