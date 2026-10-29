//! Liest und schreibt `~/.nexus/dispatch-suggestions.json` (Ausblendungen und letzte
//! Claude-Analyse); portiert aus
//! `packages/core/src/dispatch/optimization/SuggestionStateStore.ts`.
//!
//! Eine kaputte Datei gilt als leer; der Benutzer bekommt dazu eine Warnung.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::dispatch::optimization::types::Suggestion;
use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Letztes Ergebnis der Claude-Analyse
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct StoredAnalysis {
    pub at: String,
    /// Hash der Einstellung, gegen die analysiert wurde
    /// ([`crate::dispatch::DispatchConfigFingerprint`])
    pub config_hash: String,
    pub cost_usd: Option<f64>,
    pub suggestions: Vec<Suggestion>,
    /// Antworten, die Claude geliefert hat, die aber nicht als Vorschlag taugten (kein
    /// Titel); fehlt in älteren Dateien
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub skipped: Option<i64>,
}

/// Ausblendungen und letzte Analyse
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct SuggestionState {
    /// IDs ausgeblendeter Vorschläge
    pub dismissed: Vec<String>,
    pub analysis: Option<StoredAnalysis>,
}

/// Liest und schreibt die Datei der Vorschläge.
#[derive(Clone)]
pub struct SuggestionStateStore {
    home: HarnessHome,
    writer: AtomicWriter,
    warn: Arc<dyn Fn(&str) + Send + Sync>,
}

impl std::fmt::Debug for SuggestionStateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SuggestionStateStore")
            .field("file", &self.file())
            .finish()
    }
}

impl SuggestionStateStore {
    /// Store im angegebenen Benutzerordner, der Warnungen verwirft
    pub fn new(home: HarnessHome) -> Self {
        Self::with_warn(home, |_| {})
    }

    /// Store, dessen Warnungen an `warn` gehen (im Server das Protokoll)
    pub fn with_warn(home: HarnessHome, warn: impl Fn(&str) + Send + Sync + 'static) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
            warn: Arc::new(warn),
        }
    }

    /// Pfad der Datei
    pub fn file(&self) -> PathBuf {
        self.home.dispatch_suggestions_file()
    }

    /// Liest den Zustand; ohne Datei oder bei kaputtem JSON ist er leer
    pub fn read(&self) -> SuggestionState {
        let file = self.file();
        if !file.exists() {
            return SuggestionState::default();
        }
        let Ok(text) = fs::read_to_string(&file) else {
            return SuggestionState::default();
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
            (self.warn)(&format!(
                "{} ist kein gültiges JSON und wird als leer behandelt.",
                file.display()
            ));
            return SuggestionState::default();
        };
        SuggestionState {
            dismissed: parsed
                .get("dismissed")
                .and_then(Value::as_array)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| id.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            analysis: parsed
                .get("analysis")
                .filter(|analysis| analysis.is_object())
                .and_then(|analysis| serde_json::from_value(analysis.clone()).ok()),
        }
    }

    /// Schreibt den Zustand atomar
    pub fn write(&self, state: &SuggestionState) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let text = serde_json::to_string_pretty(state).map_err(Error::unexpected)?;
        self.writer.write(&self.file(), &text)
    }

    /// Liest, ändert und schreibt; liefert den neuen Zustand
    pub fn update(
        &self,
        change: impl FnOnce(SuggestionState) -> SuggestionState,
    ) -> Result<SuggestionState> {
        let next = change(self.read());
        self.write(&next)?;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Benutzerordner und Store darin
    fn store() -> (tempfile::TempDir, SuggestionStateStore) {
        let dir = tempfile::Builder::new()
            .prefix("harness-sugg-")
            .tempdir()
            .unwrap();
        let store = SuggestionStateStore::new(HarnessHome::at(dir.path()));
        (dir, store)
    }

    #[test]
    fn liefert_ohne_datei_einen_leeren_zustand_und_speichert_anderungen() {
        let (dir, store) = store();
        assert_eq!(store.file(), dir.path().join("dispatch-suggestions.json"));
        assert_eq!(store.read(), SuggestionState::default());
        store
            .update(|mut state| {
                state.dismissed = vec!["abcdef0123456789".to_string()];
                state
            })
            .unwrap();
        let analysis = StoredAnalysis {
            at: "2026-10-22T10:00:00.000Z".to_string(),
            config_hash: "h".to_string(),
            cost_usd: Some(0.04),
            suggestions: vec![],
            skipped: None,
        };
        store
            .update(|mut state| {
                state.analysis = Some(analysis.clone());
                state
            })
            .unwrap();
        let gelesen = SuggestionStateStore::new(HarnessHome::at(dir.path())).read();
        assert_eq!(
            gelesen,
            SuggestionState {
                dismissed: vec!["abcdef0123456789".to_string()],
                analysis: Some(analysis),
            }
        );
    }

    #[test]
    fn behandelt_eine_kaputte_datei_als_leer_und_warnt() {
        let (dir, _) = store();
        let home = HarnessHome::at(dir.path());
        fs::write(home.dispatch_suggestions_file(), "{kaputt").unwrap();
        let warnungen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
        let gesammelt = warnungen.clone();
        let store = SuggestionStateStore::with_warn(home, move |message| {
            gesammelt.lock().unwrap().push(message.to_string());
        });
        assert_eq!(store.read(), SuggestionState::default());
        let warnungen = warnungen.lock().unwrap();
        assert_eq!(warnungen.len(), 1);
        assert!(
            warnungen[0].contains("dispatch-suggestions.json"),
            "{}",
            warnungen[0]
        );
    }
}
