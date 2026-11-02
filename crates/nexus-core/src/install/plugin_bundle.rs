//! Das mit der Desktop-App gelieferte Plugin: erkennen, Version lesen und als lokalen
//! Marktplatz unter `~/.nexus/plugin` ablegen.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::install::claude_plugin::MARKETPLACE;
use crate::install::plugin_files::{copy_fresh, copy_home_data};
use crate::registry::HarnessHome;

/// Unterordner des Plugins im lokalen Marktplatz
pub const BUNDLE_PLUGIN_DIR: &str = "nexus";

/// Ein gebautes Plugin (Ordner mit `.claude-plugin/plugin.json`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginBundle {
    root: PathBuf,
}

impl PluginBundle {
    /// Das Plugin in diesem Ordner; ohne `plugin.json` `None`
    pub fn open(root: &Path) -> Option<Self> {
        manifest(root).is_file().then(|| Self {
            root: root.to_path_buf(),
        })
    }

    /// Ordner des Plugins
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Version aus `plugin.json`; unlesbar oder ohne Eintrag `None`
    pub fn version(&self) -> Option<String> {
        let text = fs::read_to_string(manifest(&self.root)).ok()?;
        let value: Value = serde_json::from_str(&text).ok()?;
        value.get("version")?.as_str().map(ToString::to_string)
    }

    /// Kopiert das Plugin nach `~/.nexus/plugin/nexus`, schreibt den Marktplatz daneben und legt
    /// Regeln und Anpassungen im Benutzerordner ab; liefert den Marktplatz-Ordner
    pub fn stage(&self, home: &HarnessHome) -> Result<PathBuf> {
        let marketplace = home.plugin_dir();
        copy_fresh(&self.root, &marketplace.join(BUNDLE_PLUGIN_DIR))?;
        let manifest_dir = marketplace.join(".claude-plugin");
        fs::create_dir_all(&manifest_dir).map_err(Error::unexpected)?;
        let content = json!({
            "name": MARKETPLACE,
            "description": "Lokaler Marktplatz von Nexus: stellt das mit der App gelieferte Plugin bereit",
            "owner": { "name": "Nexus" },
            "plugins": [{
                "name": "nexus",
                "version": self.version().unwrap_or_default(),
                "source": format!("./{BUNDLE_PLUGIN_DIR}"),
                "defaultEnabled": false,
            }],
        });
        let text = serde_json::to_string_pretty(&content).map_err(Error::unexpected)?;
        fs::write(manifest_dir.join("marketplace.json"), text).map_err(Error::unexpected)?;
        copy_home_data(&self.root, home)?;
        Ok(marketplace)
    }
}

/// `plugin.json` eines Plugin-Ordners
fn manifest(root: &Path) -> PathBuf {
    root.join(".claude-plugin").join("plugin.json")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Gebautes Plugin mit Version, einer Regel und einer Anpassung
    fn bundle(dir: &Path, version: &str) -> PathBuf {
        let root = dir.join("bundle");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin").join("plugin.json"),
            format!(r#"{{"name":"nexus","version":"{version}"}}"#),
        )
        .unwrap();
        fs::create_dir_all(root.join("rules")).unwrap();
        fs::write(root.join("rules").join("docs.md"), "regel").unwrap();
        fs::create_dir_all(root.join("customizations")).unwrap();
        fs::write(root.join("customizations").join("overview.json"), "{}").unwrap();
        root
    }

    fn temp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("nexus-bundle-")
            .tempdir()
            .unwrap()
    }

    #[test]
    fn erkennt_ein_bundle_nur_mit_plugin_json() {
        let dir = temp();
        assert!(PluginBundle::open(&dir.path().join("fehlt")).is_none());
        let root = bundle(dir.path(), "0.1.2");
        assert_eq!(PluginBundle::open(&root).unwrap().root(), root.as_path());
    }

    #[test]
    fn liest_die_version_aus_plugin_json() {
        let dir = temp();
        let root = bundle(dir.path(), "0.1.1793607011");
        assert_eq!(
            PluginBundle::open(&root).unwrap().version().as_deref(),
            Some("0.1.1793607011")
        );
    }

    #[test]
    fn legt_plugin_marktplatz_regeln_und_anpassungen_im_benutzerordner_ab() {
        let dir = temp();
        let home = HarnessHome::at(dir.path().join("home"));
        let root = bundle(dir.path(), "0.1.5");

        let marketplace = PluginBundle::open(&root).unwrap().stage(&home).unwrap();

        assert_eq!(marketplace, home.plugin_dir());
        assert!(home
            .plugin_dir()
            .join("nexus/.claude-plugin/plugin.json")
            .is_file());
        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(home.plugin_dir().join(".claude-plugin/marketplace.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["name"], "nexus-local");
        assert_eq!(manifest["plugins"][0]["name"], "nexus");
        assert_eq!(manifest["plugins"][0]["source"], "./nexus");
        assert_eq!(manifest["plugins"][0]["version"], "0.1.5");
        assert_eq!(manifest["plugins"][0]["defaultEnabled"], false);
        assert_eq!(
            fs::read_to_string(home.rules_dir().join("docs.md")).unwrap(),
            "regel"
        );
        assert!(home.customizations_dir().join("overview.json").is_file());
    }

    #[test]
    fn ersetzt_eine_alte_kopie_vollstaendig() {
        let dir = temp();
        let home = HarnessHome::at(dir.path().join("home"));
        let alt = home.plugin_dir().join("nexus").join("alt.txt");
        fs::create_dir_all(alt.parent().unwrap()).unwrap();
        fs::write(&alt, "alt").unwrap();
        let root = bundle(dir.path(), "0.1.5");

        PluginBundle::open(&root).unwrap().stage(&home).unwrap();

        assert!(!alt.exists());
    }
}
