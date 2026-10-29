//! Meldet das Plugin über den lokalen Marktplatz bei Claude Code an und schaltet es auf
//! Benutzerebene wieder aus (eingeschaltet wird es je Projekt von `nexus init`).
//! Portiert aus `installPlugin()` in `scripts/setup.mjs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;
use serde_json::Value;

use crate::install::plugin_switch::{LEGACY_PLUGIN_ID, PLUGIN_ID};
use crate::process::hide_console;

/// Name des lokalen Marktplatzes in Claude Code
pub const MARKETPLACE: &str = "nexus-local";

/// Datei, an der ein Marktplatz-Ordner zu erkennen ist
const MARKETPLACE_MANIFEST: [&str; 2] = [".claude-plugin", "marketplace.json"];

/// Ergebnis eines Aufrufs von `claude …`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeOutput {
    /// true, wenn der Befehl mit Code 0 endete
    pub ok: bool,
    /// Standardausgabe und Standardfehler zusammen
    pub out: String,
}

/// Ruft die Kommandozeile von Claude Code auf; in Tests ein Fake.
pub trait ClaudeCli {
    /// Führt `claude <args>` aus, bei `cwd` in einem anderen Ordner
    fn run(&self, args: &[&str], cwd: Option<&Path>) -> ClaudeOutput;
}

/// Die echte Kommandozeile von Claude Code.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClaudeCli;

impl SystemClaudeCli {
    /// Neuer Aufrufer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }
}

impl ClaudeCli for SystemClaudeCli {
    fn run(&self, args: &[&str], cwd: Option<&Path>) -> ClaudeOutput {
        // Unter Windows ist `claude` eine Batchdatei, die nur `cmd` findet
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.arg("/C").arg("claude").args(args);
            command
        } else {
            let mut command = Command::new("claude");
            command.args(args);
            command
        };
        command.stdin(Stdio::null());
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
        hide_console(&mut command);
        match command.output() {
            Ok(output) => ClaudeOutput {
                ok: output.status.success(),
                out: format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
            },
            Err(error) => ClaudeOutput {
                ok: false,
                out: error.to_string(),
            },
        }
    }
}

/// Ergebnis der Einrichtung; die Schlüssel im JSON sind dieselben wie die Felder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallResult {
    /// true, wenn `claude` auf diesem Rechner gefunden wurde
    pub available: bool,
    /// true, wenn der Marktplatz neu hinzugefügt wurde
    pub marketplace_added: bool,
    /// true, wenn das Plugin installiert oder aktualisiert wurde
    pub installed: bool,
    /// Projektordner, deren Plugin-Eintrag aktualisiert wurde
    pub updated_projects: Vec<String>,
    /// true, wenn das Plugin auf Benutzerebene ausgeschaltet wurde
    pub disabled_for_user: bool,
    /// true, wenn das alte Plugin auf Benutzerebene ausgeschaltet wurde
    pub legacy_disabled: bool,
    /// Hinweise für Menschen, wenn etwas nicht ging
    pub hints: Vec<String>,
}

/// Richtet das Plugin bei Claude Code ein: Marktplatz anmelden, Plugin installieren oder
/// aktualisieren, Projekt-Einträge auffrischen und es auf Benutzerebene ausschalten.
pub struct ClaudePluginInstall<'a> {
    claude: &'a dyn ClaudeCli,
    config_dir: Option<PathBuf>,
}

impl<'a> ClaudePluginInstall<'a> {
    /// Einrichtung mit der echten oder einer gefälschten Kommandozeile; `config_dir` ist der
    /// Ordner mit `plugins/installed_plugins.json` (`CLAUDE_CONFIG_DIR`, sonst `~/.claude`)
    pub fn new(claude: &'a dyn ClaudeCli, config_dir: Option<PathBuf>) -> Self {
        Self { claude, config_dir }
    }

    /// Einrichtung aus der Umgebung des laufenden Prozesses
    pub fn from_process_env(claude: &'a dyn ClaudeCli) -> Self {
        let config_dir = std::env::var("CLAUDE_CONFIG_DIR")
            .ok()
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| Some(home_dir().join(".claude")));
        Self::new(claude, config_dir)
    }

    /// Der Ordner mit `.claude-plugin/marketplace.json` über dem gebauten Plugin (Wurzel des
    /// Clones); gibt es ihn nicht, `None`
    pub fn marketplace_dir(plugin_root: &Path) -> Option<PathBuf> {
        plugin_root
            .ancestors()
            .find(|dir| {
                dir.join(MARKETPLACE_MANIFEST[0])
                    .join(MARKETPLACE_MANIFEST[1])
                    .exists()
            })
            .map(Path::to_path_buf)
    }

    /// Richtet das Plugin ein; ohne `claude` bleiben nur Hinweise mit den Befehlen
    pub fn install(&self, marketplace_dir: &Path) -> PluginInstallResult {
        let mut result = PluginInstallResult::default();
        let dir = marketplace_dir.to_string_lossy().into_owned();
        if !self.claude.run(&["--version"], None).ok {
            result.hints.push(
                "Claude Code (Befehl `claude`) wurde nicht gefunden. Das Plugin muss später selbst installiert werden:"
                    .to_string(),
            );
            result
                .hints
                .push(format!("  claude plugin marketplace add \"{dir}\""));
            result
                .hints
                .push(format!("  claude plugin install {PLUGIN_ID}"));
            return result;
        }
        result.available = true;
        if !self
            .claude
            .run(&["plugin", "marketplace", "list"], None)
            .out
            .contains(MARKETPLACE)
        {
            let added = self
                .claude
                .run(&["plugin", "marketplace", "add", &dir], None);
            if !added.ok {
                result.hints.push(format!(
                    "Der Marktplatz ließ sich nicht hinzufügen: claude plugin marketplace add \"{dir}\""
                ));
                return result;
            }
            result.marketplace_added = true;
        }
        // Marktplatz neu einlesen, sonst sieht `plugin update` den frisch gebauten Stand nicht
        self.claude
            .run(&["plugin", "marketplace", "update", MARKETPLACE], None);
        let list = self.claude.run(&["plugin", "list"], None).out;
        let step = if list.contains(PLUGIN_ID) {
            "update"
        } else {
            "install"
        };
        if !self.claude.run(&["plugin", step, PLUGIN_ID], None).ok {
            result
                .hints
                .push(format!("claude plugin {step} {PLUGIN_ID} ist gescheitert."));
            return result;
        }
        result.installed = true;
        result.updated_projects = self.update_projects();
        // Auf Benutzerebene ausschalten: `nexus init` schaltet es je Projekt ein, sonst kostet
        // es in jedem Projekt Zeit und Kontext
        result.disabled_for_user = self
            .claude
            .run(&["plugin", "disable", PLUGIN_ID, "--scope", "user"], None)
            .ok;
        if list.contains(LEGACY_PLUGIN_ID) {
            // Das alte Plugin wird nur ausgeschaltet, nicht deinstalliert
            result.legacy_disabled = self
                .claude
                .run(
                    &["plugin", "disable", LEGACY_PLUGIN_ID, "--scope", "user"],
                    None,
                )
                .ok;
        }
        result
    }

    /// Frischt die Projekt-Einträge des Plugins auf (die `nexus init` eingeschaltet hat);
    /// sonst bleiben sie auf dem alten Stand
    fn update_projects(&self) -> Vec<String> {
        let mut updated = Vec::new();
        for (scope, path) in self.project_entries() {
            if !Path::new(&path).exists() {
                continue;
            }
            if self
                .claude
                .run(
                    &["plugin", "update", PLUGIN_ID, "--scope", &scope],
                    Some(Path::new(&path)),
                )
                .ok
            {
                updated.push(path);
            }
        }
        updated
    }

    /// Projekt-Einträge des Plugins aus `plugins/installed_plugins.json`; Benutzerebene und
    /// Einträge ohne Pfad zählen nicht
    fn project_entries(&self) -> Vec<(String, String)> {
        let Some(dir) = &self.config_dir else {
            return Vec::new();
        };
        let file = dir.join("plugins").join("installed_plugins.json");
        let Ok(text) = fs::read_to_string(file) else {
            return Vec::new();
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return Vec::new();
        };
        let Some(entries) = value
            .get("plugins")
            .and_then(|plugins| plugins.get(PLUGIN_ID))
            .and_then(Value::as_array)
        else {
            return Vec::new();
        };
        entries
            .iter()
            .filter_map(|entry| {
                let scope = entry.get("scope").and_then(Value::as_str)?;
                let path = entry.get("projectPath").and_then(Value::as_str)?;
                (scope != "user").then(|| (scope.to_string(), path.to_string()))
            })
            .collect()
    }
}

/// Benutzerordner des angemeldeten Benutzers, wie `os.homedir()` in Node
fn home_dir() -> PathBuf {
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Gefälschte Kommandozeile: merkt sich die Aufrufe und antwortet nach Vorgabe
    #[derive(Debug, Default)]
    struct FakeClaude {
        calls: RefCell<Vec<String>>,
        dirs: RefCell<Vec<Option<PathBuf>>>,
        /// Ausgaben je Aufruf-Anfang (erster Treffer gilt)
        answers: Vec<(&'static str, bool, &'static str)>,
    }

    impl FakeClaude {
        fn new(answers: Vec<(&'static str, bool, &'static str)>) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                dirs: RefCell::new(Vec::new()),
                answers,
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    impl ClaudeCli for FakeClaude {
        fn run(&self, args: &[&str], cwd: Option<&Path>) -> ClaudeOutput {
            let line = args.join(" ");
            self.calls.borrow_mut().push(line.clone());
            self.dirs.borrow_mut().push(cwd.map(Path::to_path_buf));
            for (prefix, ok, out) in &self.answers {
                if line.starts_with(prefix) {
                    return ClaudeOutput {
                        ok: *ok,
                        out: out.to_string(),
                    };
                }
            }
            ClaudeOutput {
                ok: true,
                out: String::new(),
            }
        }
    }

    /// Temporärer Ordner
    fn temp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("nexus-claude-plugin-")
            .tempdir()
            .unwrap()
    }

    #[test]
    fn ohne_claude_bleiben_nur_hinweise_mit_den_befehlen() {
        let claude = FakeClaude::new(vec![("--version", false, "")]);
        let install = ClaudePluginInstall::new(&claude, None);

        let result = install.install(Path::new("/clone"));

        assert!(!result.available);
        assert!(!result.installed);
        assert_eq!(claude.calls(), ["--version".to_string()]);
        assert!(result.hints[1].contains("marketplace add \"/clone\""));
        assert!(result.hints[2].contains("claude plugin install nexus@nexus-local"));
    }

    #[test]
    fn meldet_den_marktplatz_an_installiert_und_schaltet_auf_benutzerebene_aus() {
        let claude = FakeClaude::new(vec![("plugin marketplace list", true, "andere")]);
        let install = ClaudePluginInstall::new(&claude, None);

        let result = install.install(Path::new("/clone"));

        assert!(result.available);
        assert!(result.marketplace_added);
        assert!(result.installed);
        assert!(result.disabled_for_user);
        assert!(!result.legacy_disabled);
        assert_eq!(
            claude.calls(),
            [
                "--version",
                "plugin marketplace list",
                "plugin marketplace add /clone",
                "plugin marketplace update nexus-local",
                "plugin list",
                "plugin install nexus@nexus-local",
                "plugin disable nexus@nexus-local --scope user",
            ]
        );
    }

    #[test]
    fn aktualisiert_ein_schon_installiertes_plugin_und_schaltet_das_alte_aus() {
        let claude = FakeClaude::new(vec![
            ("plugin marketplace list", true, "nexus-local"),
            (
                "plugin list",
                true,
                "nexus@nexus-local\nai-harness@ai-harness-local",
            ),
        ]);
        let install = ClaudePluginInstall::new(&claude, None);

        let result = install.install(Path::new("/clone"));

        assert!(!result.marketplace_added);
        assert!(result.installed);
        assert!(result.legacy_disabled);
        assert!(claude
            .calls()
            .contains(&"plugin update nexus@nexus-local".to_string()));
        assert!(claude
            .calls()
            .contains(&"plugin disable ai-harness@ai-harness-local --scope user".to_string()));
    }

    #[test]
    fn frischt_die_projekt_eintrage_auf_und_lasst_die_benutzerebene_aus() {
        let base = temp();
        let projekt = base.path().join("projekt");
        fs::create_dir_all(&projekt).unwrap();
        let config = base.path().join("claude");
        fs::create_dir_all(config.join("plugins")).unwrap();
        let eintraege = serde_json::json!({
            "plugins": {
                PLUGIN_ID: [
                    { "scope": "local", "projectPath": projekt.to_string_lossy() },
                    { "scope": "user" },
                    { "scope": "local", "projectPath": base.path().join("weg").to_string_lossy() },
                ]
            }
        });
        fs::write(
            config.join("plugins").join("installed_plugins.json"),
            eintraege.to_string(),
        )
        .unwrap();
        let claude = FakeClaude::new(vec![("plugin marketplace list", true, "nexus-local")]);
        let install = ClaudePluginInstall::new(&claude, Some(config));

        let result = install.install(Path::new("/clone"));

        assert_eq!(
            result.updated_projects,
            [projekt.to_string_lossy().into_owned()]
        );
        assert!(claude
            .calls()
            .contains(&"plugin update nexus@nexus-local --scope local".to_string()));
    }

    #[test]
    fn ohne_installierte_plugins_gibt_es_keine_projekt_eintrage() {
        let claude = FakeClaude::new(Vec::new());
        let base = temp();
        let install = ClaudePluginInstall::new(&claude, Some(base.path().to_path_buf()));

        assert!(install.project_entries().is_empty());
    }

    #[test]
    fn findet_den_marktplatz_ordner_uber_dem_gebauten_plugin() {
        let base = temp();
        let plugin = base.path().join("packages/plugin/dist/nexus");
        fs::create_dir_all(&plugin).unwrap();
        fs::create_dir_all(base.path().join(MARKETPLACE_MANIFEST[0])).unwrap();
        fs::write(
            base.path()
                .join(MARKETPLACE_MANIFEST[0])
                .join(MARKETPLACE_MANIFEST[1]),
            "{}",
        )
        .unwrap();

        assert_eq!(
            ClaudePluginInstall::marketplace_dir(&plugin),
            Some(base.path().to_path_buf())
        );
        assert_eq!(
            ClaudePluginInstall::marketplace_dir(Path::new("/nirgendwo/sonst")),
            None
        );
    }
}
