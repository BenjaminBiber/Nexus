//! Speichert die Einstellungen der Themes (`settingsSchema`) je Theme in
//! `<NEXUS_HOME>/theme-settings.json`: `{ "<themeId>": { "<key>": bool|string } }`.

use std::collections::BTreeMap;
use std::fs;
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};

use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;
use crate::themes::manifest::is_valid_theme_id;

/// Fehlercode für ungültige Theme-ID oder Werte (HTTP 400)
pub const THEME_SETTINGS_INVALID: &str = "theme-settings-invalid";

/// Höchstzahl der Einstellungen eines Themes
pub const MAX_THEME_SETTINGS: usize = 50;

/// Werte der Einstellungen eines Themes
pub type ThemeSettingValues = Map<String, Value>;

/// Liest und schreibt die Einstellungen der Themes
#[derive(Debug, Clone)]
pub struct ThemeSettingsStore {
    home: HarnessHome,
    writer: AtomicWriter,
    /// Hält Lesen-Ändern-Schreiben der Datei zusammen
    lock: Arc<Mutex<()>>,
}

impl ThemeSettingsStore {
    /// Einstellungen im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    /// Gespeicherte Werte eines Themes; ohne Eintrag ein leeres Objekt
    pub fn get(&self, id: &str) -> Result<ThemeSettingValues> {
        validate_id(id)?;
        Ok(self.read().remove(id).unwrap_or_default())
    }

    /// Ersetzt die Werte eines Themes vollständig; andere Themes bleiben
    pub fn put(&self, id: &str, values: ThemeSettingValues) -> Result<ThemeSettingValues> {
        validate_id(id)?;
        validate_values(&values)?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.read();
        all.insert(id.to_string(), values.clone());
        self.write(&all)?;
        Ok(values)
    }

    /// Löscht die Werte eines Themes (z. B. nach dem Deinstallieren)
    pub fn remove(&self, id: &str) -> Result<()> {
        validate_id(id)?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.read();
        if all.remove(id).is_some() {
            self.write(&all)?;
        }
        Ok(())
    }

    /// Alle Einträge; fehlende oder beschädigte Datei ergibt keine
    fn read(&self) -> BTreeMap<String, ThemeSettingValues> {
        fs::read_to_string(self.home.theme_settings_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn write(&self, all: &BTreeMap<String, ThemeSettingValues>) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let json = serde_json::to_string_pretty(all).map_err(Error::unexpected)?;
        self.writer.write(&self.home.theme_settings_file(), &json)
    }
}

/// `^[a-zA-Z][a-zA-Z0-9_-]{0,39}$`, Schlüssel einer Theme-Einstellung
pub fn is_valid_setting_key(key: &str) -> bool {
    let mut chars = key.chars();
    key.len() <= 40
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn validate_id(id: &str) -> Result<()> {
    match is_valid_theme_id(id) {
        true => Ok(()),
        false => {
            Err(invalid("Die Theme-ID muss aus 2 bis 40 Zeichen a-z, 0-9 oder - bestehen.").into())
        }
    }
}

fn validate_values(values: &ThemeSettingValues) -> Result<()> {
    if values.len() > MAX_THEME_SETTINGS {
        return Err(invalid(format!(
            "Ein Theme hat höchstens {MAX_THEME_SETTINGS} Einstellungen."
        ))
        .into());
    }
    for (key, value) in values {
        if !is_valid_setting_key(key) {
            return Err(invalid(format!("Ungültiger Schlüssel „{key}“.")).into());
        }
        if !(value.is_boolean() || value.is_string()) {
            return Err(invalid(format!("„{key}“ muss ein Schalter oder Text sein.")).into());
        }
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> HarnessError {
    HarnessError::new(THEME_SETTINGS_INVALID, message)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn values(value: Value) -> ThemeSettingValues {
        value.as_object().cloned().expect("Objekt")
    }

    fn store() -> (ThemeSettingsStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("Temp-Ordner");
        (ThemeSettingsStore::new(HarnessHome::at(dir.path())), dir)
    }

    #[test]
    fn liest_ohne_eintrag_ein_leeres_objekt() {
        let (store, _dir) = store();
        assert!(store.get("min-craft").unwrap().is_empty());
    }

    #[test]
    fn neuer_store_liest_die_gespeicherten_werte() {
        let (store, dir) = store();
        let saved = values(json!({ "sounds": false, "style": "pixel" }));
        store.put("min-craft", saved.clone()).unwrap();
        let again = ThemeSettingsStore::new(HarnessHome::at(dir.path()));
        assert_eq!(again.get("min-craft").unwrap(), saved);
        assert!(dir.path().join("theme-settings.json").exists());
    }

    #[test]
    fn put_ersetzt_nur_das_eigene_theme() {
        let (store, _dir) = store();
        store
            .put("eins", values(json!({ "a": true, "b": "x" })))
            .unwrap();
        store.put("zwei", values(json!({ "a": false }))).unwrap();
        store.put("eins", values(json!({ "b": "y" }))).unwrap();
        assert_eq!(store.get("eins").unwrap(), values(json!({ "b": "y" })));
        assert_eq!(store.get("zwei").unwrap(), values(json!({ "a": false })));
        store.remove("eins").unwrap();
        assert!(store.get("eins").unwrap().is_empty());
        assert_eq!(store.get("zwei").unwrap(), values(json!({ "a": false })));
    }

    #[test]
    fn lehnt_ungueltige_eingaben_ab_und_speichert_nichts() {
        let (store, dir) = store();
        let ok = values(json!({ "sounds": true }));
        assert_eq!(
            store.put("A", ok.clone()).unwrap_err().code(),
            Some(THEME_SETTINGS_INVALID)
        );
        assert_eq!(
            store.get("x").unwrap_err().code(),
            Some(THEME_SETTINGS_INVALID)
        );
        for bad in [
            json!({ "zahl": 1 }),
            json!({ "liste": [] }),
            json!({ "leer": null }),
            json!({ "1abc": true }),
            json!({ "mit punkt": true }),
        ] {
            let error = store.put("theme", values(bad.clone())).unwrap_err();
            assert_eq!(error.code(), Some(THEME_SETTINGS_INVALID), "{bad}");
        }
        let many: ThemeSettingValues = (0..=MAX_THEME_SETTINGS)
            .map(|i| (format!("k{i}"), Value::Bool(true)))
            .collect();
        assert_eq!(
            store.put("theme", many).unwrap_err().code(),
            Some(THEME_SETTINGS_INVALID)
        );
        assert!(!dir.path().join("theme-settings.json").exists());
    }

    #[test]
    fn beschaedigte_datei_gilt_als_leer() {
        let (store, dir) = store();
        fs::write(dir.path().join("theme-settings.json"), "{kaputt").unwrap();
        assert!(store.get("theme").unwrap().is_empty());
        store.put("theme", values(json!({ "a": true }))).unwrap();
        assert_eq!(store.get("theme").unwrap(), values(json!({ "a": true })));
    }
}
