//! Der Slot des Claude-Themes: ein Plugin-Ordner `<Claude-Konfiguration>/skills/nexus-theme/`,
//! den Claude Code als `nexus-theme@skills-dir` lädt und beobachtet.
//!
//! Nexus legt dort den Mod des aktiven Themes ab (alles aus `<NEXUS_HOME>/themes/<id>/claude/`),
//! ergänzt `.claude-plugin/plugin.json` und schreibt `hooks/nexus-settings.ts` mit den gewählten
//! Theme-Einstellungen. Die Markierung `.nexus-theme.json` weist den Ordner als von Nexus
//! verwaltet aus; ein Ordner ohne gültige Markierung gilt als fremd und wird weder geschrieben
//! noch gelöscht.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use super::manifest::{ThemeManifest, ThemeSettingSpec};
use super::package::CLAUDE_DIR;

/// Name des Plugin-Ordners unter `skills/` und Name des Plugins
pub const SLOT_NAME: &str = "nexus-theme";
/// Markierung im Slot
pub const MARKER_FILE: &str = ".nexus-theme.json";
/// Wert von `managedBy` in der Markierung
const MANAGED_BY: &str = "nexus";
/// Manifest des Plugins im Slot
const PLUGIN_FILE: &str = ".claude-plugin/plugin.json";
/// Einstellungen des Themes im Slot (überschreibt die Standardwerte aus dem Theme)
pub const SETTINGS_FILE: &str = "hooks/nexus-settings.ts";
/// Kopfzeile von `nexus-settings.ts`; gleich in `scripts/themes/sync-claude-kit.mjs`
const SETTINGS_HEADER: &str =
    "// Von Nexus erzeugt – nicht bearbeiten. Im Quellordner des Themes stehen hier die Standardwerte.";

/// Werte der Theme-Einstellungen, Schlüssel sortiert
pub type ClaudeSettings = BTreeMap<String, Value>;

/// Markierung `.nexus-theme.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SlotMarker {
    managed_by: String,
    theme_id: String,
    theme_version: String,
    #[serde(default)]
    settings: ClaudeSettings,
}

/// Zustand des Slot-Ordners
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotState {
    /// Kein Ordner
    None,
    /// Ordner von Nexus mit dem Claude-Theme dieses Themes
    Nexus {
        theme_id: String,
        theme_version: String,
    },
    /// Ordner ohne gültige Markierung (oder ein symbolischer Link): gehört nicht Nexus
    Foreign,
}

/// Fehler beim Schreiben oder Löschen des Slots
#[derive(Debug, thiserror::Error)]
pub enum ClaudeSlotError {
    #[error("{path} gehört nicht Nexus – umbenennen oder löschen, damit Nexus das Claude-Theme ablegen kann.")]
    Foreign { path: String },
    #[error("Das Theme {id} bringt kein Claude-Theme mit.")]
    NoClaudeTheme { id: String },
    #[error("Dateifehler im Claude-Theme: {0}")]
    Io(#[from] io::Error),
}

/// Zustand des Claude-Themes, wie ihn die Oberfläche zeigt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "themes.ts")]
pub enum ClaudeThemeState {
    /// Der Slot trägt das Claude-Theme des aktiven Themes
    Active,
    /// Kein Claude-Theme: Schalter aus oder das aktive Theme bringt keins mit
    None,
    /// Der Slot-Ordner gehört nicht Nexus; Nexus lässt ihn in Ruhe
    Foreign,
    /// Der letzte Abgleich ist gescheitert (`message`)
    Error,
}

/// Antwort von `GET /api/claude-theme`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "themes.ts")]
pub struct ClaudeThemeStatus {
    /// Einstellung `claudeTheme`
    pub enabled: bool,
    pub state: ClaudeThemeState,
    /// Aktives Theme, wenn es installiert ist
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub theme_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub theme_name: Option<String>,
    /// Pfad des Slots
    pub path: String,
    /// Meldung bei `foreign` und `error`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub message: Option<String>,
}

/// Was ein `apply` am Slot geändert hat
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyOutcome {
    /// Neu geschriebene oder geänderte Dateien
    pub written: usize,
    /// Gelöschte Dateien, die nicht mehr dazugehören
    pub removed: usize,
}

/// Der Slot-Ordner des Claude-Themes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeThemeSlot {
    dir: PathBuf,
}

impl ClaudeThemeSlot {
    /// Slot unter dem Konfigurationsordner von Claude Code (`<config_dir>/skills/nexus-theme`)
    pub fn for_config_dir(config_dir: &Path) -> Self {
        Self {
            dir: config_dir.join("skills").join(SLOT_NAME),
        }
    }

    /// Pfad des Slots
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Liest die Markierung: kein Slot, Slot von Nexus oder fremder Ordner
    pub fn status(&self) -> io::Result<SlotState> {
        let metadata = match fs::symlink_metadata(&self.dir) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(SlotState::None),
            Err(error) => return Err(error),
        };
        if !metadata.is_dir() {
            return Ok(SlotState::Foreign);
        }
        Ok(match self.marker() {
            Some(marker) => SlotState::Nexus {
                theme_id: marker.theme_id,
                theme_version: marker.theme_version,
            },
            None => SlotState::Foreign,
        })
    }

    /// Gültige Markierung des Slots, falls vorhanden
    fn marker(&self) -> Option<SlotMarker> {
        let path = self.dir.join(MARKER_FILE);
        if fs::symlink_metadata(&path).ok()?.is_symlink() {
            return None;
        }
        let marker: SlotMarker = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
        (marker.managed_by == MANAGED_BY).then_some(marker)
    }

    /// Legt das Claude-Theme eines installierten Themes in den Slot. `values` sind die
    /// aufgelösten Einstellungen (siehe [`resolve_settings`]). Ein neuer Slot bekommt zuerst die
    /// Markierung; ein vorhandener nur die geänderten Dateien, überzählige werden gelöscht.
    pub fn apply(
        &self,
        theme_dir: &Path,
        manifest: &ThemeManifest,
        values: &ClaudeSettings,
    ) -> Result<ApplyOutcome, ClaudeSlotError> {
        if !manifest.claude_theme {
            return Err(ClaudeSlotError::NoClaudeTheme {
                id: manifest.id.clone(),
            });
        }
        let state = self.status()?;
        if state == SlotState::Foreign {
            return Err(self.foreign());
        }
        let mut files = desired_files(theme_dir, manifest, values)?;
        let marker = files
            .remove(Path::new(MARKER_FILE))
            .expect("die Markierung gehört immer dazu");

        let mut outcome = ApplyOutcome::default();
        if state == SlotState::None {
            fs::create_dir_all(&self.dir)?;
            self.write_file(Path::new(MARKER_FILE), &marker)?;
            outcome.written += 1;
            for (path, data) in &files {
                self.write_file(path, data)?;
                outcome.written += 1;
            }
            return Ok(outcome);
        }

        for (path, data) in &files {
            if self.write_if_changed(path, data)? {
                outcome.written += 1;
            }
        }
        let mut keep: BTreeSet<PathBuf> = files.into_keys().collect();
        keep.insert(PathBuf::from(MARKER_FILE));
        outcome.removed = self.remove_others(&self.dir, Path::new(""), &keep)?;
        // Die Markierung zuletzt: sie nennt erst dann das neue Theme, wenn alles dafür liegt
        if self.write_if_changed(Path::new(MARKER_FILE), &marker)? {
            outcome.written += 1;
        }
        Ok(outcome)
    }

    /// Löscht den Slot, aber nur mit gültiger Markierung. `true`, wenn etwas gelöscht wurde.
    pub fn clear(&self) -> Result<bool, ClaudeSlotError> {
        match self.status()? {
            SlotState::None => Ok(false),
            SlotState::Foreign => Err(self.foreign()),
            SlotState::Nexus { .. } => {
                fs::remove_dir_all(&self.dir)?;
                Ok(true)
            }
        }
    }

    fn foreign(&self) -> ClaudeSlotError {
        ClaudeSlotError::Foreign {
            path: self.dir.display().to_string(),
        }
    }

    /// Schreibt eine Datei unterhalb des Slots; symbolische Links auf dem Weg werden durch
    /// echte Ordner ersetzt, damit nichts außerhalb des Slots getroffen wird
    fn write_file(&self, relative: &Path, data: &[u8]) -> io::Result<()> {
        let target = self.dir.join(relative);
        let mut current = self.dir.clone();
        let parents: Vec<_> = relative
            .parent()
            .map(|parent| parent.components().collect())
            .unwrap_or_default();
        for component in parents {
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    remove_entry(&current)?;
                    fs::create_dir(&current)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(&current)?;
                }
                Err(error) => return Err(error),
            }
        }
        if let Ok(metadata) = fs::symlink_metadata(&target) {
            if !metadata.is_file() {
                remove_entry(&target)?;
            }
        }
        fs::write(target, data)
    }

    /// Schreibt nur, wenn die Datei fehlt oder anders ist; `true`, wenn geschrieben wurde
    fn write_if_changed(&self, relative: &Path, data: &[u8]) -> io::Result<bool> {
        let target = self.dir.join(relative);
        let unchanged = fs::symlink_metadata(&target).is_ok_and(|metadata| metadata.is_file())
            && fs::read(&target).is_ok_and(|current| current == data);
        if unchanged {
            return Ok(false);
        }
        self.write_file(relative, data)?;
        Ok(true)
    }

    /// Löscht alles unter `dir`, was nicht in `keep` steht, und danach leere Ordner; folgt
    /// keinen symbolischen Links (sie werden selbst entfernt)
    fn remove_others(
        &self,
        dir: &Path,
        relative: &Path,
        keep: &BTreeSet<PathBuf>,
    ) -> io::Result<usize> {
        let mut removed = 0;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                removed += self.remove_others(&entry.path(), &path, keep)?;
                if fs::read_dir(entry.path())?.next().is_none() {
                    fs::remove_dir(entry.path())?;
                }
            } else if !keep.contains(&path) {
                remove_entry(&entry.path())?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// Entfernt eine Datei oder einen Link (auch einen Ordner-Link unter Windows), ohne ihm zu folgen
fn remove_entry(path: &Path) -> io::Result<()> {
    fs::remove_file(path).or_else(|_| fs::remove_dir(path))
}

/// Soll-Inhalt des Slots: relativer Pfad → Bytes
fn desired_files(
    theme_dir: &Path,
    manifest: &ThemeManifest,
    values: &ClaudeSettings,
) -> io::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();
    collect_theme_files(&theme_dir.join(CLAUDE_DIR), Path::new(""), &mut files)?;
    let marker = SlotMarker {
        managed_by: MANAGED_BY.to_string(),
        theme_id: manifest.id.clone(),
        theme_version: manifest.version.clone(),
        settings: values.clone(),
    };
    files.insert(PathBuf::from(MARKER_FILE), pretty_json(&marker));
    files.insert(
        PathBuf::from(PLUGIN_FILE),
        pretty_json(&plugin_json(manifest)),
    );
    files.insert(
        PathBuf::from(SETTINGS_FILE),
        settings_module(values).into_bytes(),
    );
    Ok(files)
}

/// Dateien unter `claude/` des installierten Themes; symbolische Links und versteckte Einträge
/// werden übersprungen
fn collect_theme_files(
    dir: &Path,
    relative: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = relative.join(&name);
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_theme_files(&entry.path(), &path, files)?;
        } else if file_type.is_file() {
            files.insert(path, fs::read(entry.path())?);
        }
    }
    Ok(())
}

/// `.claude-plugin/plugin.json` des Slots
fn plugin_json(manifest: &ThemeManifest) -> Value {
    serde_json::json!({
        "name": SLOT_NAME,
        "version": manifest.version,
        "description": format!(
            "Claude-Theme „{}“ aus Nexus – wird von Nexus verwaltet",
            manifest.name
        ),
        "author": { "name": manifest.author },
    })
}

fn pretty_json(value: &impl Serialize) -> Vec<u8> {
    let mut text = serde_json::to_string_pretty(value).expect("JSON aus festen Typen");
    text.push('\n');
    text.into_bytes()
}

/// Löst die gespeicherten Werte gegen das `settingsSchema` auf, wie `resolveThemeSettings` in
/// der Oberfläche: je Schema-Schlüssel der gespeicherte Wert, wenn Typ (und bei Auswahl die
/// Option) passt, sonst der Standard; unbekannte Schlüssel fallen weg.
pub fn resolve_settings(
    schema: Option<&BTreeMap<String, ThemeSettingSpec>>,
    stored: &Map<String, Value>,
) -> ClaudeSettings {
    let mut values = ClaudeSettings::new();
    for (key, spec) in schema.into_iter().flatten() {
        let value = stored.get(key);
        let resolved = match spec {
            ThemeSettingSpec::Boolean { default, .. } => {
                Value::Bool(value.and_then(Value::as_bool).unwrap_or(*default))
            }
            ThemeSettingSpec::Select {
                options, default, ..
            } => {
                let chosen = value
                    .and_then(Value::as_str)
                    .filter(|chosen| options.iter().any(|option| option.value == *chosen));
                Value::String(chosen.unwrap_or(default).to_string())
            }
        };
        values.insert(key.clone(), resolved);
    }
    values
}

/// Inhalt von `hooks/nexus-settings.ts` für diese Werte; byte-gleich mit `settingsModule` in
/// `scripts/themes/sync-claude-kit.mjs` (Schlüssel sortiert, JSON ohne Leerzeichen)
pub fn settings_module(values: &ClaudeSettings) -> String {
    let json = serde_json::to_string(values).expect("JSON aus Texten und Schaltern");
    format!(
        "{SETTINGS_HEADER}\nexport const settings: Readonly<Record<string, string | boolean>> = {json}\n"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::themes::manifest::parse_manifest;

    fn manifest(claude: bool) -> ThemeManifest {
        parse_manifest(
            &json!({
                "id": "nacht", "name": "Nacht", "version": "1.2.0", "author": "Ada",
                "sdkVersion": "^1.3", "colorScheme": "dark", "claudeTheme": claude,
                "settingsSchema": {
                    "palette": { "type": "select", "label": "Palette", "default": "blau",
                        "options": [ { "value": "blau", "label": "Blau" }, { "value": "rot", "label": "Rot" } ] },
                    "crt": { "type": "boolean", "label": "CRT", "default": true }
                }
            })
            .to_string(),
        )
        .unwrap()
    }

    /// Installiertes Theme mit einem kleinen Claude-Theme
    fn theme() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let hooks = dir.path().join("claude").join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        fs::write(
            hooks.join("hooks.json"),
            r#"{ "modules": ["./register.tsx"] }"#,
        )
        .unwrap();
        fs::write(
            hooks.join("register.tsx"),
            "export const register = () => {}",
        )
        .unwrap();
        fs::write(hooks.join("nexus-settings.ts"), "// Standardwerte").unwrap();
        dir
    }

    fn slot() -> (tempfile::TempDir, ClaudeThemeSlot) {
        let config = tempfile::tempdir().unwrap();
        let slot = ClaudeThemeSlot::for_config_dir(config.path());
        (config, slot)
    }

    fn standard() -> ClaudeSettings {
        resolve_settings(manifest(true).settings_schema.as_ref(), &Map::new())
    }

    fn lesen(slot: &ClaudeThemeSlot, datei: &str) -> String {
        fs::read_to_string(slot.dir().join(datei)).unwrap()
    }

    #[test]
    fn slot_liegt_unter_skills() {
        let slot = ClaudeThemeSlot::for_config_dir(Path::new("/c"));
        assert_eq!(
            slot.dir(),
            Path::new("/c").join("skills").join("nexus-theme")
        );
    }

    #[test]
    fn apply_legt_den_slot_mit_markierung_plugin_und_einstellungen_an() {
        let (_config, slot) = slot();
        let theme = theme();
        assert_eq!(slot.status().unwrap(), SlotState::None);
        let outcome = slot
            .apply(theme.path(), &manifest(true), &standard())
            .unwrap();
        assert_eq!(
            outcome,
            ApplyOutcome {
                written: 5,
                removed: 0
            }
        );
        assert_eq!(
            slot.status().unwrap(),
            SlotState::Nexus {
                theme_id: "nacht".into(),
                theme_version: "1.2.0".into()
            }
        );
        let plugin: Value =
            serde_json::from_str(&lesen(&slot, ".claude-plugin/plugin.json")).unwrap();
        assert_eq!(plugin["name"], "nexus-theme");
        assert_eq!(plugin["version"], "1.2.0");
        assert_eq!(
            plugin["description"],
            "Claude-Theme „Nacht“ aus Nexus – wird von Nexus verwaltet"
        );
        assert_eq!(plugin["author"]["name"], "Ada");
        assert_eq!(
            lesen(&slot, "hooks/register.tsx"),
            "export const register = () => {}"
        );
        assert_eq!(
            lesen(&slot, "hooks/nexus-settings.ts"),
            format!("{SETTINGS_HEADER}\nexport const settings: Readonly<Record<string, string | boolean>> = {{\"crt\":true,\"palette\":\"blau\"}}\n")
        );
        let marker: Value = serde_json::from_str(&lesen(&slot, MARKER_FILE)).unwrap();
        assert_eq!(marker["managedBy"], "nexus");
        assert_eq!(
            marker["settings"],
            json!({ "crt": true, "palette": "blau" })
        );
    }

    #[test]
    fn zweites_apply_ohne_aenderung_schreibt_nichts() {
        let (_config, slot) = slot();
        let theme = theme();
        slot.apply(theme.path(), &manifest(true), &standard())
            .unwrap();
        let outcome = slot
            .apply(theme.path(), &manifest(true), &standard())
            .unwrap();
        assert_eq!(outcome, ApplyOutcome::default());
    }

    #[test]
    fn apply_schreibt_geaenderte_und_loescht_ueberzaehlige_dateien() {
        let (_config, slot) = slot();
        let theme = theme();
        slot.apply(theme.path(), &manifest(true), &standard())
            .unwrap();
        fs::write(slot.dir().join("hooks").join("alt.ts"), "weg").unwrap();
        fs::create_dir_all(slot.dir().join("leer").join("tiefer")).unwrap();
        fs::write(theme.path().join("claude/hooks/register.tsx"), "neu").unwrap();

        let mut values = standard();
        values.insert("palette".into(), json!("rot"));
        let outcome = slot.apply(theme.path(), &manifest(true), &values).unwrap();
        // register.tsx, nexus-settings.ts und die Markierung (neue Einstellungen)
        assert_eq!(
            outcome,
            ApplyOutcome {
                written: 3,
                removed: 1
            }
        );
        assert_eq!(lesen(&slot, "hooks/register.tsx"), "neu");
        assert!(lesen(&slot, "hooks/nexus-settings.ts").contains(r#""palette":"rot""#));
        assert!(!slot.dir().join("hooks/alt.ts").exists());
        assert!(!slot.dir().join("leer").exists());
    }

    #[test]
    fn fremder_ordner_wird_weder_geschrieben_noch_geloescht() {
        let (_config, slot) = slot();
        fs::create_dir_all(slot.dir()).unwrap();
        fs::write(slot.dir().join("mein.txt"), "meins").unwrap();
        assert_eq!(slot.status().unwrap(), SlotState::Foreign);
        assert!(matches!(
            slot.apply(theme().path(), &manifest(true), &standard()),
            Err(ClaudeSlotError::Foreign { .. })
        ));
        assert!(matches!(slot.clear(), Err(ClaudeSlotError::Foreign { .. })));
        assert_eq!(lesen(&slot, "mein.txt"), "meins");
        assert!(!slot.dir().join(MARKER_FILE).exists());
    }

    #[test]
    fn markierung_eines_anderen_besitzers_gilt_als_fremd() {
        let (_config, slot) = slot();
        fs::create_dir_all(slot.dir()).unwrap();
        fs::write(
            slot.dir().join(MARKER_FILE),
            r#"{"managedBy":"jemand","themeId":"x","themeVersion":"1.0.0"}"#,
        )
        .unwrap();
        assert_eq!(slot.status().unwrap(), SlotState::Foreign);
        fs::write(slot.dir().join(MARKER_FILE), "{kaputt").unwrap();
        assert_eq!(slot.status().unwrap(), SlotState::Foreign);
    }

    #[test]
    fn clear_loescht_nur_den_eigenen_slot() {
        let (_config, slot) = slot();
        assert!(!slot.clear().unwrap());
        slot.apply(theme().path(), &manifest(true), &standard())
            .unwrap();
        assert!(slot.clear().unwrap());
        assert_eq!(slot.status().unwrap(), SlotState::None);
    }

    #[test]
    fn theme_ohne_claude_theme_wird_nicht_angewandt() {
        let (_config, slot) = slot();
        assert!(matches!(
            slot.apply(theme().path(), &manifest(false), &standard()),
            Err(ClaudeSlotError::NoClaudeTheme { .. })
        ));
        assert_eq!(slot.status().unwrap(), SlotState::None);
    }

    #[test]
    fn einstellungen_mit_standardwerten_ungueltigen_werten_und_unbekannten_schluesseln() {
        let schema = manifest(true).settings_schema;
        assert_eq!(
            resolve_settings(schema.as_ref(), &Map::new()),
            BTreeMap::from([
                ("crt".into(), json!(true)),
                ("palette".into(), json!("blau"))
            ])
        );
        let gespeichert = json!({ "crt": "ja", "palette": "gruen", "fremd": 1 });
        assert_eq!(
            resolve_settings(schema.as_ref(), gespeichert.as_object().unwrap()),
            BTreeMap::from([
                ("crt".into(), json!(true)),
                ("palette".into(), json!("blau"))
            ])
        );
        let gespeichert = json!({ "crt": false, "palette": "rot" });
        assert_eq!(
            resolve_settings(schema.as_ref(), gespeichert.as_object().unwrap()),
            BTreeMap::from([
                ("crt".into(), json!(false)),
                ("palette".into(), json!("rot"))
            ])
        );
        assert!(resolve_settings(None, gespeichert.as_object().unwrap()).is_empty());
    }

    #[test]
    fn settings_modul_ohne_einstellungen() {
        assert_eq!(
            settings_module(&ClaudeSettings::new()),
            format!("{SETTINGS_HEADER}\nexport const settings: Readonly<Record<string, string | boolean>> = {{}}\n")
        );
    }

    #[cfg(unix)]
    #[test]
    fn symbolischer_link_im_slot_wird_ersetzt_ohne_ihm_zu_folgen() {
        let (_config, slot) = slot();
        let theme = theme();
        slot.apply(theme.path(), &manifest(true), &standard())
            .unwrap();
        let draussen = tempfile::tempdir().unwrap();
        fs::remove_dir_all(slot.dir().join("hooks")).unwrap();
        std::os::unix::fs::symlink(draussen.path(), slot.dir().join("hooks")).unwrap();
        slot.apply(theme.path(), &manifest(true), &standard())
            .unwrap();
        assert!(fs::read_dir(draussen.path()).unwrap().next().is_none());
        assert!(slot.dir().join("hooks/register.tsx").is_file());
    }
}
