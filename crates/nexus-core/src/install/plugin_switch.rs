//! Schaltet das Plugin für genau ein Projekt ein. Portiert aus
//! `packages/core/src/install/ProjectPluginSwitch.ts`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;

/// Kennung des Plugins in Claude Code
pub const PLUGIN_ID: &str = "nexus@nexus-local";

/// Kennung des alten Plugins; `enable` nimmt sie aus den Einstellungen des Projekts heraus
pub const LEGACY_PLUGIN_ID: &str = "ai-harness@ai-harness-local";

/// Was `enable` getan hat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SwitchResult {
    /// true, wenn das Plugin neu eingetragen wurde (false: war schon eingeschaltet)
    pub enabled: bool,
    /// true, wenn der Eintrag des alten Plugins entfernt wurde
    pub migrated: bool,
}

/// Schaltet das Plugin für genau ein Projekt ein: `enabledPlugins` in
/// `<projekt>/.claude/settings.local.json`. Das Plugin ist sonst ausgeschaltet
/// (Benutzerebene), damit es in Projekten ohne Nexus nichts kostet.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProjectPluginSwitch {
    writer: AtomicWriter,
}

impl ProjectPluginSwitch {
    /// Neuer Schalter (hält keinen Zustand)
    pub fn new() -> Self {
        Self::default()
    }

    /// Datei mit den lokalen Einstellungen des Projekts
    pub fn file(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(".claude").join("settings.local.json")
    }

    /// Trägt `true` ein, nimmt den Eintrag des alten Plugins heraus und lässt alle anderen
    /// Einstellungen unberührt; ohne Änderung wird nichts geschrieben
    pub fn enable(&self, project_dir: &Path) -> Result<SwitchResult> {
        let file = self.file(project_dir);
        let mut settings = self.read(&file)?;
        let mut plugins = plugins(&settings);
        let result = SwitchResult {
            enabled: plugins.get(PLUGIN_ID) != Some(&Value::Bool(true)),
            migrated: plugins.contains_key(LEGACY_PLUGIN_ID),
        };
        if !result.enabled && !result.migrated {
            return Ok(result);
        }
        plugins.insert(PLUGIN_ID.to_string(), Value::Bool(true));
        plugins.remove(LEGACY_PLUGIN_ID);
        settings.insert("enabledPlugins".to_string(), Value::Object(plugins));
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(Error::unexpected)?;
        }
        self.write(&file, settings)?;
        Ok(result)
    }

    /// Entfernt den Eintrag wieder; liefert false, wenn keiner vorhanden war
    pub fn disable(&self, project_dir: &Path) -> Result<bool> {
        let file = self.file(project_dir);
        if !file.exists() {
            return Ok(false);
        }
        let mut settings = self.read(&file)?;
        let mut plugins = plugins(&settings);
        if plugins.remove(PLUGIN_ID).is_none() {
            return Ok(false);
        }
        if plugins.is_empty() {
            settings.remove("enabledPlugins");
        } else {
            settings.insert("enabledPlugins".to_string(), Value::Object(plugins));
        }
        self.write(&file, settings)
    }

    /// Schreibt die Einstellungen wie TypeScript: JSON mit zwei Leerzeichen und `\n` am Ende
    fn write(&self, file: &Path, settings: Map<String, Value>) -> Result<bool> {
        let json =
            serde_json::to_string_pretty(&Value::Object(settings)).map_err(Error::unexpected)?;
        self.writer.write(file, &format!("{json}\n"))?;
        Ok(true)
    }

    /// Liest die Datei; fehlt sie, ist das Ergebnis leer. Kaputtes JSON wird nie überschrieben.
    fn read(&self, file: &Path) -> Result<Map<String, Value>> {
        let text = match fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
            Err(error) => return Err(Error::unexpected(error)),
        };
        if text.trim().is_empty() {
            return Ok(Map::new());
        }
        if let Ok(Value::Object(object)) = serde_json::from_str::<Value>(&text) {
            return Ok(object);
        }
        Err(HarnessError::new(
            "settings-unreadable",
            format!(
                "{} ist kein gültiges JSON. Bitte prüfen; die Datei wurde nicht verändert.",
                file.display()
            ),
        )
        .into())
    }
}

/// Der Abschnitt `enabledPlugins` als Objekt
fn plugins(settings: &Map<String, Value>) -> Map<String, Value> {
    match settings.get("enabledPlugins") {
        Some(Value::Object(object)) => object.clone(),
        _ => Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Temporärer Projektordner
    fn projekt() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("harness-switch-")
            .tempdir()
            .unwrap()
    }

    /// Inhalt der Einstellungen als JSON-Objekt
    fn gelesen(dir: &Path) -> Value {
        let file = ProjectPluginSwitch::new().file(dir);
        serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap()
    }

    /// Schreibt die Einstellungen von Hand
    fn vorgeben(dir: &Path, text: &str) -> PathBuf {
        let file = ProjectPluginSwitch::new().file(dir);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, text).unwrap();
        file
    }

    #[test]
    fn legt_die_einstellungen_mit_dem_eintrag_an() {
        let dir = projekt();
        assert_eq!(
            ProjectPluginSwitch::new().enable(dir.path()).unwrap(),
            SwitchResult {
                enabled: true,
                migrated: false
            }
        );
        assert_eq!(
            gelesen(dir.path()),
            serde_json::json!({ "enabledPlugins": { PLUGIN_ID: true } })
        );
    }

    #[test]
    fn ist_wiederholbar_und_meldet_false_wenn_schon_eingeschaltet() {
        let dir = projekt();
        let switch = ProjectPluginSwitch::new();
        switch.enable(dir.path()).unwrap();
        assert_eq!(switch.enable(dir.path()).unwrap(), SwitchResult::default());
    }

    #[test]
    fn ersetzt_das_alte_plugin_und_meldet_die_umstellung() {
        let dir = projekt();
        vorgeben(
            dir.path(),
            &format!(r#"{{"enabledPlugins":{{"{LEGACY_PLUGIN_ID}":true,"x@y":true}}}}"#),
        );

        let result = ProjectPluginSwitch::new().enable(dir.path()).unwrap();

        assert_eq!(
            result,
            SwitchResult {
                enabled: true,
                migrated: true
            }
        );
        assert_eq!(
            gelesen(dir.path()),
            serde_json::json!({ "enabledPlugins": { "x@y": true, PLUGIN_ID: true } })
        );
    }

    #[test]
    fn nimmt_das_alte_plugin_auch_heraus_wenn_das_neue_schon_eingeschaltet_ist() {
        let dir = projekt();
        vorgeben(
            dir.path(),
            &format!(r#"{{"enabledPlugins":{{"{PLUGIN_ID}":true,"{LEGACY_PLUGIN_ID}":false}}}}"#),
        );

        let result = ProjectPluginSwitch::new().enable(dir.path()).unwrap();

        assert_eq!(
            result,
            SwitchResult {
                enabled: false,
                migrated: true
            }
        );
        assert_eq!(
            gelesen(dir.path()),
            serde_json::json!({ "enabledPlugins": { PLUGIN_ID: true } })
        );
    }

    #[test]
    fn lasst_vorhandene_einstellungen_und_andere_plugins_unberuhrt() {
        let dir = projekt();
        vorgeben(
            dir.path(),
            r#"{"permissions":{"allow":["Bash(ls)"]},"enabledPlugins":{"x@y":true}}"#,
        );
        ProjectPluginSwitch::new().enable(dir.path()).unwrap();
        assert_eq!(
            gelesen(dir.path()),
            serde_json::json!({
                "permissions": { "allow": ["Bash(ls)"] },
                "enabledPlugins": { "x@y": true, PLUGIN_ID: true },
            })
        );
    }

    #[test]
    fn uberschreibt_kaputtes_json_nie() {
        let dir = projekt();
        let file = vorgeben(dir.path(), "{ kaputt");
        let error = ProjectPluginSwitch::new().enable(dir.path()).unwrap_err();
        assert_eq!(error.code(), Some("settings-unreadable"));
        assert!(error.to_string().contains("kein gültiges JSON"), "{error}");
        assert_eq!(fs::read_to_string(file).unwrap(), "{ kaputt");
    }

    #[test]
    fn entfernt_nur_den_eigenen_eintrag() {
        let dir = projekt();
        vorgeben(
            dir.path(),
            &format!(r#"{{"enabledPlugins":{{"x@y":true,"{PLUGIN_ID}":true}}}}"#),
        );
        let switch = ProjectPluginSwitch::new();
        assert!(switch.disable(dir.path()).unwrap());
        assert_eq!(
            gelesen(dir.path()),
            serde_json::json!({ "enabledPlugins": { "x@y": true } })
        );
        assert!(!switch.disable(dir.path()).unwrap());
    }

    #[test]
    fn disable_ohne_datei_tut_nichts() {
        let dir = projekt();
        let switch = ProjectPluginSwitch::new();
        assert!(!switch.disable(dir.path()).unwrap());
        assert!(!switch.file(dir.path()).exists());
    }

    #[test]
    fn entfernt_den_leeren_abschnitt_mit_dem_letzten_eintrag() {
        let dir = projekt();
        let switch = ProjectPluginSwitch::new();
        switch.enable(dir.path()).unwrap();
        assert!(switch.disable(dir.path()).unwrap());
        assert_eq!(gelesen(dir.path()), serde_json::json!({}));
    }
}
