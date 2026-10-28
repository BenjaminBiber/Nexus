//! Liefert den angewendeten Workflow aus `customizations/workflow.json`, sonst den Standard.
//! Portiert aus `packages/core/src/workflow/WorkflowSource.ts`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::registry::HarnessHome;

use super::phases::workflow_phases;

/// Herkunft des gelieferten Workflows
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowOrigin {
    Standard,
    Custom,
}

/// Der angewendete Workflow mit seiner Herkunft, wie `GET /api/workflow` ihn meldet. `phases`
/// bleibt rohes JSON: ein eigener Workflow wird unverändert weitergegeben, nicht neu
/// eingelesen und typgeprüft (das prüft schon das Anwenden).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowSelection {
    pub source: WorkflowOrigin,
    pub phases: Value,
}

/// Liest den angewendeten Workflow eines Benutzerordners
#[derive(Debug, Clone)]
pub struct WorkflowSource {
    home: HarnessHome,
}

impl WorkflowSource {
    /// Leser für diesen Harness-Ordner
    pub fn new(home: HarnessHome) -> Self {
        Self { home }
    }

    /// Fehlende, unlesbare oder leere Datei ergibt den Standard (geprüft wurde schon beim
    /// Anwenden)
    pub fn read(&self) -> WorkflowSelection {
        match self.read_custom() {
            Some(phases) => WorkflowSelection {
                source: WorkflowOrigin::Custom,
                phases,
            },
            None => WorkflowSelection {
                source: WorkflowOrigin::Standard,
                phases: serde_json::to_value(workflow_phases())
                    .unwrap_or_else(|_| Value::Array(Vec::new())),
            },
        }
    }

    /// Der eigene Workflow, falls die Datei eine nichtleere Liste enthält
    fn read_custom(&self) -> Option<Value> {
        let file = self.home.customizations_dir().join("workflow.json");
        let text = std::fs::read_to_string(file).ok()?;
        let phases: Value = serde_json::from_str(&text).ok()?;
        match &phases {
            Value::Array(items) if !items.is_empty() => Some(phases),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn home(dir: &Path) -> HarnessHome {
        HarnessHome::at(dir)
    }

    #[test]
    fn liefert_ohne_anpassung_den_standard() {
        let dir = tempfile::tempdir().unwrap();
        let selection = WorkflowSource::new(home(dir.path())).read();
        assert_eq!(selection.source, WorkflowOrigin::Standard);
        let ids: Vec<&str> = selection
            .phases
            .as_array()
            .unwrap()
            .iter()
            .map(|phase| phase["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["planen", "umsetzen"]);
    }

    #[test]
    fn liefert_den_angewendeten_workflow() {
        let dir = tempfile::tempdir().unwrap();
        let home = home(dir.path());
        std::fs::create_dir_all(home.customizations_dir()).unwrap();
        let phases = serde_json::json!([
            { "id": "planen", "title": "P", "hint": "h", "steps": [] },
            { "id": "umsetzen", "title": "U", "hint": "h", "steps": [] },
        ]);
        std::fs::write(
            home.customizations_dir().join("workflow.json"),
            serde_json::to_string(&phases).unwrap(),
        )
        .unwrap();
        let selection = WorkflowSource::new(home).read();
        assert_eq!(selection.source, WorkflowOrigin::Custom);
        assert_eq!(selection.phases, phases);
    }

    #[test]
    fn eine_leere_liste_oder_kaputte_datei_ergibt_den_standard() {
        let dir = tempfile::tempdir().unwrap();
        let home = home(dir.path());
        std::fs::create_dir_all(home.customizations_dir()).unwrap();
        let file = home.customizations_dir().join("workflow.json");
        for inhalt in ["[]", "kaputt", "{}"] {
            std::fs::write(&file, inhalt).unwrap();
            assert_eq!(
                WorkflowSource::new(home.clone()).read().source,
                WorkflowOrigin::Standard,
                "{inhalt}"
            );
        }
    }
}
