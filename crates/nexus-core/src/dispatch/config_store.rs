//! Liest und schreibt `~/.nexus/dispatch.yaml`; portiert aus
//! `packages/core/src/dispatch/DispatchConfigStore.ts`. Geschrieben wird nur, was die Prüfung
//! besteht.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::dispatch::config::{DispatchConfig, PromptProblem};
use crate::dispatch::config_normalizer::DispatchConfigNormalizer;
use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Die Einstellung, wie die Oberfläche sie braucht
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchConfigRead {
    /// Inhalt der Datei (ohne Datei: das Beispiel) – für die Oberfläche
    pub raw: Value,
    pub config: DispatchConfig,
    pub exists: bool,
    /// Befunde zu eigenen Vorlagen in der Datei; leer = alles in Ordnung.
    /// Diese Vorlagen laufen auf dem Standard.
    pub prompt_problems: Vec<PromptProblem>,
}

/// Liest und schreibt die Einstellung des Dispatchers.
#[derive(Debug, Clone)]
pub struct DispatchConfigStore {
    home: HarnessHome,
    writer: AtomicWriter,
    normalizer: DispatchConfigNormalizer,
}

impl DispatchConfigStore {
    /// Store im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
            normalizer: DispatchConfigNormalizer::new(),
        }
    }

    /// Pfad der Datei (für Meldungen)
    pub fn file(&self) -> PathBuf {
        self.home.dispatch_config_file()
    }

    /// Liest die Einstellung; ohne Datei gilt das Beispiel (ausgeschaltet)
    pub fn read(&self) -> Result<DispatchConfigRead> {
        let file = self.file();
        if !file.exists() {
            let raw = self.normalizer.example();
            return Ok(DispatchConfigRead {
                config: self.normalizer.normalize(&raw)?,
                prompt_problems: self.normalizer.prompt_problems(&raw)?,
                raw,
                exists: false,
            });
        }
        let text = fs::read_to_string(&file).map_err(Error::unexpected)?;
        let raw: Value = serde_yaml_ng::from_str(&text).map_err(|_| {
            HarnessError::new(
                "dispatch-config-invalid",
                format!("{} ist kein gültiges YAML.", file.display()),
            )
        })?;
        // Eine leere Datei ergibt null; das zählt wie eine leere Einstellung
        let raw = if raw.is_null() {
            Value::Object(Default::default())
        } else {
            raw
        };
        if !raw.is_object() {
            return Err(HarnessError::new(
                "dispatch-config-invalid",
                format!("{} enthält keine Einstellungen.", file.display()),
            )
            .into());
        }
        Ok(DispatchConfigRead {
            config: self.normalizer.normalize(&raw)?,
            prompt_problems: self.normalizer.prompt_problems(&raw)?,
            raw,
            exists: true,
        })
    }

    /// Prüft und schreibt atomar; liefert die aufgelöste Einstellung
    pub fn write(&self, raw: &Value) -> Result<DispatchConfig> {
        // Beim Speichern ist eine fehlerhafte Vorlage ein Fehler; beim Lesen fällt sie still auf
        // den Standard zurück
        let problems = self.normalizer.prompt_problems(raw)?;
        if !problems.is_empty() {
            return Err(DispatchConfigNormalizer::prompt_error(&problems).into());
        }
        let config = self.normalizer.normalize(raw)?;
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let yaml = serde_yaml_ng::to_string(raw).map_err(Error::unexpected)?;
        self.writer.write(&self.file(), &yaml)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Benutzerordner und Store darin
    fn store() -> (tempfile::TempDir, DispatchConfigStore) {
        let dir = tempfile::Builder::new()
            .prefix("harness-dispatch-")
            .tempdir()
            .unwrap();
        let store = DispatchConfigStore::new(HarnessHome::at(dir.path()));
        (dir, store)
    }

    /// Knappe eigene Vorlage für work, die alle Pflichtteile erfüllt
    fn work_template() -> String {
        [
            "{{agent}} setzt {{ticketId}} („{{title}}“) um.",
            "Fertig: nexus board done {{ticketId}} --by {{agent}}",
            "Blockiert: nexus board set {{ticketId}} --status blocked",
            "Frage: nexus board question add \"…\" --ticket {{ticketId}}",
        ]
        .join("\n")
    }

    #[test]
    fn liefert_ohne_datei_das_beispiel_ausgeschaltet() {
        let (_dir, store) = store();
        let read = store.read().unwrap();
        assert!(!read.exists);
        assert!(!read.config.enabled);
        assert_eq!(
            read.config.profiles.keys().collect::<Vec<&String>>(),
            ["klein", "standard", "schwer"]
        );
        assert!(read.raw.get("profiles").is_some());
    }

    #[test]
    fn schreibt_gepruft_und_liest_wieder() {
        let (dir, store) = store();
        let saved = store.write(&json!({ "enabled": true, "wip": 2 })).unwrap();
        assert!(saved.enabled);
        assert_eq!(saved.wip, 2);
        let text = fs::read_to_string(dir.path().join("dispatch.yaml")).unwrap();
        assert!(text.contains("enabled: true"), "{text}");
        let read = store.read().unwrap();
        assert!(read.exists);
        assert_eq!(read.config.wip, 2);
    }

    #[test]
    fn schreibt_ungultiges_nicht() {
        let (_dir, store) = store();
        let error = store.write(&json!({ "wip": -3 })).unwrap_err();
        assert_eq!(error.code(), Some("dispatch-config-invalid"));
        assert!(!store.read().unwrap().exists);
    }

    #[test]
    fn meldet_kaputtes_yaml() {
        let (dir, store) = store();
        fs::write(dir.path().join("dispatch.yaml"), "wip: [").unwrap();
        let error = store.read().unwrap_err();
        assert_eq!(error.code(), Some("dispatch-config-invalid"));
    }

    #[test]
    fn speichert_eine_gultige_eigene_vorlage() {
        let (dir, store) = store();
        let template = work_template();
        let saved = store
            .write(&json!({ "prompts": { "work": template } }))
            .unwrap();
        assert_eq!(saved.prompts.work.as_deref(), Some(template.as_str()));
        let text = fs::read_to_string(dir.path().join("dispatch.yaml")).unwrap();
        assert!(text.contains("nexus board done"), "{text}");
        let read = store.read().unwrap();
        assert_eq!(read.config.prompts.work.as_deref(), Some(template.as_str()));
        assert_eq!(read.prompt_problems, vec![]);
    }

    #[test]
    fn lehnt_eine_vorlage_mit_unbekanntem_platzhalter_beim_speichern_ab() {
        let (_dir, store) = store();
        let template = format!("{}\n{{{{unbekannt}}}}", work_template());
        let error = store
            .write(&json!({ "prompts": { "work": template } }))
            .unwrap_err();
        assert_eq!(error.code(), Some("dispatch-prompt-invalid"));
        let message = error.to_string();
        assert!(message.contains("Vorlage „work“"), "{message}");
        assert!(message.contains("{{unbekannt}}"), "{message}");
        assert!(!store.read().unwrap().exists);
    }

    #[test]
    fn lehnt_eine_vorlage_mit_fehlendem_pflichtteil_beim_speichern_ab_und_nennt_ihn() {
        let (_dir, store) = store();
        let error = store
            .write(&json!({ "prompts": { "plan": "Hallo {{agent}}, plane {{ticketId}}." } }))
            .unwrap_err();
        assert_eq!(error.code(), Some("dispatch-prompt-invalid"));
        let message = error.to_string();
        assert!(message.contains("Vorlage „plan“"), "{message}");
        assert!(message.contains("plan.json"), "{message}");
    }

    #[test]
    fn bricht_beim_lesen_nicht_an_einer_von_hand_verfalschten_vorlage() {
        let (dir, store) = store();
        fs::write(
            dir.path().join("dispatch.yaml"),
            "prompts:\n  plan: \"kaputt {{agent}}\"\n",
        )
        .unwrap();
        let read = store.read().unwrap();
        assert_eq!(read.config.prompts.plan, None);
        assert_eq!(
            read.prompt_problems
                .iter()
                .map(|problem| problem.kind.as_str())
                .collect::<Vec<&str>>(),
            ["plan"]
        );
        assert!(!read.prompt_problems[0].missing_parts.is_empty());
    }

    /// Der Plan verlangt die Gegenprobe: eine von TypeScript geschriebene Datei lesen, mit Rust
    /// schreiben und wieder lesen ergibt dieselbe Einstellung
    #[test]
    fn lesen_schreiben_lesen_ergibt_dieselbe_einstellung() {
        let (_dir, store) = store();
        let normalizer = DispatchConfigNormalizer::new();
        let raw = normalizer.example();
        let erst = store.write(&raw).unwrap();
        let gelesen = store.read().unwrap();
        assert_eq!(gelesen.config, erst);
        let nochmal = store.write(&gelesen.raw).unwrap();
        assert_eq!(store.read().unwrap().config, nochmal);
        assert_eq!(nochmal, erst);
    }
}
