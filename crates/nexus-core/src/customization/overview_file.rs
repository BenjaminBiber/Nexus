//! Portiert `packages/core/src/customization/OverviewFile.ts`.

use std::path::Path;

use serde_json::Value;

use crate::registry::HarnessHome;

use super::overview::CustomizationOverview;

/// Liest die Übersicht des letzten Anwendens aus `~/.harness/customizations/overview.json`.
#[derive(Debug, Clone)]
pub struct OverviewFile {
    home: HarnessHome,
}

impl OverviewFile {
    /// Leser für diesen Harness-Ordner
    pub fn new(home: HarnessHome) -> Self {
        Self { home }
    }

    /// None, wenn die Datei fehlt, unlesbar ist oder nicht die erwartete Form hat
    /// (Harness wurde noch nicht eingerichtet)
    pub fn read(&self) -> Option<CustomizationOverview> {
        Self::read_file(&self.home.customizations_dir().join("overview.json"))
    }

    /// Wie `read`, aber für eine ausdrücklich genannte Datei (`customize overview --file`)
    pub fn read_file(file: &Path) -> Option<CustomizationOverview> {
        let text = std::fs::read_to_string(file).ok()?;
        let parsed: Value = serde_json::from_str(&text).ok()?;
        // Die erwartete Form ist ein Objekt mit einer Liste `entries`; fehlende weitere
        // Felder füllt der Standard, damit auch ältere Dateien lesbar bleiben
        if !parsed.is_object() || !parsed.get("entries").is_some_and(Value::is_array) {
            return None;
        }
        serde_json::from_value(parsed).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(dir: &Path) -> HarnessHome {
        HarnessHome::at(dir)
    }

    #[test]
    fn liest_die_ubersicht_oder_none() {
        let dir = tempfile::tempdir().unwrap();
        let home = home(dir.path());
        assert!(OverviewFile::new(home.clone()).read().is_none());

        std::fs::create_dir_all(home.customizations_dir()).unwrap();
        let file = home.customizations_dir().join("overview.json");
        std::fs::write(&file, "kaputt").unwrap();
        assert!(OverviewFile::new(home.clone()).read().is_none());

        std::fs::write(&file, "{\"version\":1,\"entries\":[]}").unwrap();
        let overview = OverviewFile::new(home).read().expect("Übersicht");
        assert_eq!(overview.version, 1);
        assert!(overview.entries.is_empty());
    }

    #[test]
    fn liefert_none_bei_gultigem_json_mit_falscher_form() {
        let dir = tempfile::tempdir().unwrap();
        let home = home(dir.path());
        std::fs::create_dir_all(home.customizations_dir()).unwrap();
        let file = home.customizations_dir().join("overview.json");
        for content in [
            "{}",
            "[]",
            "{\"version\":1,\"entries\":null}",
            "{\"version\":1}",
        ] {
            std::fs::write(&file, content).unwrap();
            assert!(
                OverviewFile::new(home.clone()).read().is_none(),
                "{content}"
            );
        }
    }
}
