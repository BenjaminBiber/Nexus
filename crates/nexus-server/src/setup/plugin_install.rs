//! Installiert das mitgelieferte Plugin: in den Benutzerordner legen, Regeln kopieren und bei
//! Claude Code anmelden.

use nexus_core::install::{ClaudePluginInstall, PluginBundle};
use nexus_core::{HarnessHome, Result};
use serde::Serialize;

use crate::setup::SetupContext;

/// Antwort von `POST /api/setup/plugin`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSetupResult {
    /// Alles ging durch
    pub ok: bool,
    /// Plugin bei Claude Code installiert oder aktualisiert
    pub installed: bool,
    /// Erledigte Schritte in Klartext
    pub steps: Vec<String>,
    /// Hinweise mit Befehlen zum Abtippen, wenn etwas nicht ging
    pub hints: Vec<String>,
}

/// Die drei Schritte der Installation; Dateifehler brechen ab, Claude-Fehler werden Hinweise
pub fn install_plugin(
    setup: &SetupContext,
    bundle: &PluginBundle,
    home: &HarnessHome,
) -> Result<PluginSetupResult> {
    let marketplace = bundle.stage(home)?;
    let mut steps = vec![format!(
        "Plugin, Regeln und Anpassungen nach {} gelegt",
        home.dir().display()
    )];
    let result =
        ClaudePluginInstall::new(setup.claude(), Some(setup.claude_dir())).install(&marketplace);
    if result.marketplace_added {
        steps.push("Marktplatz nexus-local bei Claude Code angemeldet".into());
    }
    if result.installed {
        steps.push("Plugin nexus@nexus-local installiert".into());
    }
    if !result.updated_projects.is_empty() {
        steps.push(format!(
            "{} Projekt(e) aktualisiert",
            result.updated_projects.len()
        ));
    }
    if result.legacy_disabled {
        steps.push("Altes Plugin ai-harness ausgeschaltet".into());
    }
    Ok(PluginSetupResult {
        ok: result.available && result.installed,
        installed: result.installed,
        steps,
        hints: result.hints,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use nexus_core::install::{ClaudeCli, ClaudeOutput};

    use super::*;
    use crate::setup::NoClaude;

    /// Claude, das jeden Aufruf mit Erfolg beantwortet und ihn sich merkt
    #[derive(Debug, Default)]
    struct Recorder {
        calls: Mutex<Vec<String>>,
    }

    impl ClaudeCli for Recorder {
        fn run(&self, args: &[&str], _cwd: Option<&Path>) -> ClaudeOutput {
            self.calls.lock().unwrap().push(args.join(" "));
            ClaudeOutput {
                ok: true,
                out: String::new(),
            }
        }
    }

    fn bundle(dir: &Path) -> PluginBundle {
        let root = dir.join("bundle");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin/plugin.json"),
            r#"{"version":"0.1.5"}"#,
        )
        .unwrap();
        fs::create_dir_all(root.join("rules")).unwrap();
        fs::write(root.join("rules/board.md"), "regel").unwrap();
        PluginBundle::open(&root).unwrap()
    }

    fn temp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("nexus-inst-")
            .tempdir()
            .unwrap()
    }

    #[test]
    fn legt_das_plugin_ab_und_meldet_den_marktplatz_bei_claude_an() {
        let dir = temp();
        let home = HarnessHome::at(dir.path().join("home"));
        let claude = Arc::new(Recorder::default());
        let setup = SetupContext::new(
            claude.clone(),
            Some(dir.path().join("claude")),
            dir.path().into(),
        );

        let result = install_plugin(&setup, &bundle(dir.path()), &home).unwrap();

        assert!(result.ok);
        assert!(result.installed);
        assert!(home.rules_dir().join("board.md").is_file());
        let marketplace = home.plugin_dir().to_string_lossy().into_owned();
        let calls = claude.calls.lock().unwrap().clone();
        assert!(
            calls.contains(&format!("plugin marketplace add {marketplace}")),
            "{calls:?}"
        );
        assert!(
            calls.contains(&"plugin install nexus@nexus-local".to_string()),
            "{calls:?}"
        );
    }

    #[test]
    fn ohne_claude_bleiben_hinweise_mit_den_befehlen() {
        let dir = temp();
        let home = HarnessHome::at(dir.path().join("home"));
        let setup = SetupContext::new(Arc::new(NoClaude), None, dir.path().into());

        let result = install_plugin(&setup, &bundle(dir.path()), &home).unwrap();

        assert!(!result.ok);
        assert!(result
            .hints
            .iter()
            .any(|hint| hint.contains("claude plugin install")));
        // Die Dateien liegen trotzdem bereit
        assert!(home
            .plugin_dir()
            .join(".claude-plugin/marketplace.json")
            .is_file());
    }
}
