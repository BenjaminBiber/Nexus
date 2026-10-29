//! Liest die Eingabe einer Recherche aus einer JSON-Datei, der Standardeingabe oder einem
//! Kontexttext. Portiert aus `packages/core/src/questions/ResearchInputReader.ts`.

use std::io::Read;
use std::path::Path;

use crate::error::{HarnessError, Result};
use crate::questions::research_validator::ResearchInput;

/// Liest Recherche-Eingaben.
pub struct ResearchInputReader;

impl ResearchInputReader {
    /// Genau eine Quelle ist erlaubt: `from` (Datei oder „-“ für die Standardeingabe) oder
    /// `context`
    pub fn read(from: Option<&str>, context: Option<&str>) -> Result<ResearchInput> {
        let from = from.filter(|value| !value.is_empty());
        let context = context.filter(|value| !value.is_empty());
        if from.is_some() && context.is_some() {
            return Err(HarnessError::new(
                "research-input",
                "Bitte entweder --from oder --context angeben, nicht beides.",
            )
            .into());
        }
        if let Some(context) = context {
            return Ok(ResearchInput::context(context));
        }
        let Some(from) = from else {
            return Err(HarnessError::new(
                "research-input",
                "Bitte --from <datei|-> oder --context <text> angeben.",
            )
            .into());
        };
        let text = Self::read_source(from)?;
        serde_json::from_str(&text).map_err(|_| {
            HarnessError::new("research-input", "Die Eingabe ist kein gültiges JSON.").into()
        })
    }

    /// Liest eine Datei oder die Standardeingabe
    fn read_source(source: &str) -> Result<String> {
        let read = if source == "-" {
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text).map(|_| text)
        } else {
            std::fs::read_to_string(Path::new(source))
        };
        read.map_err(|_| {
            HarnessError::new(
                "research-input",
                format!(
                    "Die Eingabe {} lässt sich nicht lesen.",
                    if source == "-" {
                        "von der Standardeingabe".to_string()
                    } else {
                        source.to_string()
                    }
                ),
            )
            .into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_den_kontext_unverandert() {
        let input = ResearchInputReader::read(None, Some("Heute: Datei")).unwrap();
        assert_eq!(input.context.as_deref(), Some("Heute: Datei"));
        assert!(input.options.is_none());
    }

    #[test]
    fn lehnt_beide_quellen_und_keine_quelle_ab() {
        let beides = ResearchInputReader::read(Some("a.json"), Some("x")).unwrap_err();
        assert_eq!(beides.code(), Some("research-input"));
        assert_eq!(
            beides.to_string(),
            "Bitte entweder --from oder --context angeben, nicht beides."
        );

        let keines = ResearchInputReader::read(None, None).unwrap_err();
        assert_eq!(keines.code(), Some("research-input"));
        assert_eq!(
            keines.to_string(),
            "Bitte --from <datei|-> oder --context <text> angeben."
        );
    }

    #[test]
    fn liest_eine_json_datei_und_meldet_fehler_mit_dem_pfad() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("recherche.json");
        std::fs::write(
            &file,
            r#"{ "context": "Heute", "options": [{ "title": "A", "text": "x" }] }"#,
        )
        .unwrap();

        let input = ResearchInputReader::read(Some(file.to_str().unwrap()), None).unwrap();
        assert_eq!(input.context.as_deref(), Some("Heute"));
        assert_eq!(input.options.unwrap().len(), 1);

        let fehlt = dir.path().join("fehlt.json");
        let error = ResearchInputReader::read(Some(fehlt.to_str().unwrap()), None).unwrap_err();
        assert_eq!(error.code(), Some("research-input"));
        assert!(error.to_string().contains("lässt sich nicht lesen"));
    }

    #[test]
    fn lehnt_ungultiges_json_ab() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kaputt.json");
        std::fs::write(&file, "kein json").unwrap();

        let error = ResearchInputReader::read(Some(file.to_str().unwrap()), None).unwrap_err();
        assert_eq!(error.code(), Some("research-input"));
        assert_eq!(error.to_string(), "Die Eingabe ist kein gültiges JSON.");
    }
}
