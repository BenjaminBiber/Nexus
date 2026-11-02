//! Zustand des Claude-Code-Plugins: installiert, fehlt oder älter als das mitgelieferte.
//! Gelesen aus `plugins/installed_plugins.json`, ohne `claude` aufzurufen.

use std::cmp::Ordering;
use std::fs;
use std::path::Path;

use nexus_core::install::{PluginBundle, PLUGIN_ID};
use serde::Serialize;
use serde_json::Value;

/// Zustand des Plugins bei Claude Code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginState {
    /// Eingetragen, mindestens so neu wie das mitgelieferte
    Installed,
    /// Kein Eintrag
    Missing,
    /// Eingetragen, aber älter als das mitgelieferte
    Outdated,
}

/// Antwort von `GET /api/setup/plugin`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatusReport {
    /// Die App bringt ein Plugin mit
    pub bundled: bool,
    /// `claude --version` lief
    pub claude: bool,
    /// Zustand bei Claude Code
    pub state: PluginState,
    /// Höchste eingetragene Version
    pub installed_version: Option<String>,
    /// Version des mitgelieferten Plugins
    pub bundled_version: Option<String>,
}

/// Liest den Zustand; `claude_dir` ist der Ordner mit `plugins/installed_plugins.json`
pub fn read_status(
    claude_dir: &Path,
    bundle: Option<&PluginBundle>,
    claude: bool,
) -> PluginStatusReport {
    let installed_version = installed_version(claude_dir);
    let bundled_version = bundle.and_then(PluginBundle::version);
    let state = match (&installed_version, &bundled_version) {
        (None, _) => PluginState::Missing,
        (Some(installed), Some(bundled))
            if compare_versions(installed, bundled) == Ordering::Less =>
        {
            PluginState::Outdated
        }
        (Some(_), _) => PluginState::Installed,
    };
    PluginStatusReport {
        bundled: bundle.is_some(),
        claude,
        state,
        installed_version,
        bundled_version,
    }
}

/// Höchste Version aller Einträge des Plugins; fehlende oder unlesbare Datei: `None`
fn installed_version(claude_dir: &Path) -> Option<String> {
    let text =
        fs::read_to_string(claude_dir.join("plugins").join("installed_plugins.json")).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let entries = value.get("plugins")?.get(PLUGIN_ID)?.as_array()?;
    entries
        .iter()
        .map(|entry| entry.get("version").and_then(Value::as_str).unwrap_or("0"))
        .max_by(|a, b| compare_versions(a, b))
        .map(ToString::to_string)
}

/// Vergleicht Versionen Teil für Teil als Zahlen (`0.1.10` > `0.1.9`); Unlesbares zählt als 0
fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |v: &str| -> Vec<u64> { v.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (a, b) = (parts(a), parts(b));
    for index in 0..a.len().max(b.len()) {
        let order = a.get(index).unwrap_or(&0).cmp(b.get(index).unwrap_or(&0));
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("nexus-status-")
            .tempdir()
            .unwrap()
    }

    /// Schreibt `plugins/installed_plugins.json` mit Einträgen der angegebenen Versionen
    fn installed(dir: &Path, versions: &[&str]) {
        let entries: Vec<_> = versions
            .iter()
            .map(|version| serde_json::json!({ "scope": "user", "version": version }))
            .collect();
        fs::create_dir_all(dir.join("plugins")).unwrap();
        fs::write(
            dir.join("plugins/installed_plugins.json"),
            serde_json::json!({ "version": 2, "plugins": { PLUGIN_ID: entries } }).to_string(),
        )
        .unwrap();
    }

    fn bundle(dir: &Path, version: &str) -> PluginBundle {
        let root: PathBuf = dir.join("bundle");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin/plugin.json"),
            format!(r#"{{"version":"{version}"}}"#),
        )
        .unwrap();
        PluginBundle::open(&root).unwrap()
    }

    #[test]
    fn meldet_ein_fehlendes_plugin() {
        let dir = temp();
        let bundle = bundle(dir.path(), "0.1.2");
        let report = read_status(&dir.path().join("claude"), Some(&bundle), true);
        assert_eq!(report.state, PluginState::Missing);
        assert!(report.bundled);
        assert!(report.claude);
        assert_eq!(report.installed_version, None);
        assert_eq!(report.bundled_version.as_deref(), Some("0.1.2"));
    }

    #[test]
    fn meldet_ein_aelteres_plugin_als_veraltet() {
        let dir = temp();
        installed(dir.path(), &["0.1.1793607011"]);
        let bundle = bundle(dir.path(), "0.1.1793700000");
        let report = read_status(dir.path(), Some(&bundle), true);
        assert_eq!(report.state, PluginState::Outdated);
        assert_eq!(report.installed_version.as_deref(), Some("0.1.1793607011"));
    }

    #[test]
    fn gleiche_oder_neuere_version_gilt_als_installiert() {
        let dir = temp();
        installed(dir.path(), &["0.1.9", "0.1.10"]);
        let bundle = bundle(dir.path(), "0.1.10");
        let report = read_status(dir.path(), Some(&bundle), false);
        assert_eq!(report.state, PluginState::Installed);
        assert_eq!(report.installed_version.as_deref(), Some("0.1.10"));
    }

    #[test]
    fn ohne_bundle_zaehlt_nur_da_oder_nicht_da() {
        let dir = temp();
        installed(dir.path(), &["0.0.1"]);
        let report = read_status(dir.path(), None, true);
        assert_eq!(report.state, PluginState::Installed);
        assert!(!report.bundled);
        assert_eq!(report.bundled_version, None);
    }

    #[test]
    fn unlesbare_liste_gilt_als_fehlend() {
        let dir = temp();
        fs::create_dir_all(dir.path().join("plugins")).unwrap();
        fs::write(dir.path().join("plugins/installed_plugins.json"), "{kaputt").unwrap();
        assert_eq!(
            read_status(dir.path(), None, true).state,
            PluginState::Missing
        );
    }

    #[test]
    fn serialisiert_wie_die_oberflaeche_es_erwartet() {
        let dir = temp();
        let report = read_status(dir.path(), None, false);
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::json!({
                "bundled": false, "claude": false, "state": "missing",
                "installedVersion": null, "bundledVersion": null,
            })
        );
    }
}
