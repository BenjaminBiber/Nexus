//! Portiert `packages/core/src/customization/FrontMatter.ts`.

use std::collections::BTreeMap;

/// Felder des Kopfes und, falls das YAML nicht lesbar ist, die erste Zeile des Fehlers
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FrontMatterResult {
    /// Felder des Kopfes; ohne oder mit kaputtem Kopf leer
    pub fields: BTreeMap<String, serde_yaml_ng::Value>,
    /// Erste Zeile der YAML-Fehlermeldung, wenn der Kopf vorhanden, aber nicht lesbar ist
    pub error: Option<String>,
}

/// Liest den YAML-Kopf einer Markdown-Datei (zwischen den ersten beiden Zeilen „---“).
#[derive(Debug, Clone, Copy, Default)]
pub struct FrontMatter;

impl FrontMatter {
    /// Neuer Leser (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Felder des Kopfes; ohne oder mit kaputtem Kopf leer
    pub fn read(&self, text: &str) -> BTreeMap<String, serde_yaml_ng::Value> {
        self.parse(text).fields
    }

    /// Felder des Kopfes und, falls das YAML nicht lesbar ist, die erste Zeile des Fehlers
    pub fn parse(&self, text: &str) -> FrontMatterResult {
        let Some(head) = Self::head(text) else {
            return FrontMatterResult::default();
        };
        match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&head) {
            // Nur eine Abbildung zählt; eine Liste oder ein einzelner Wert gilt als leerer Kopf
            Ok(serde_yaml_ng::Value::Mapping(mapping)) => FrontMatterResult {
                fields: mapping
                    .into_iter()
                    .filter_map(|(key, value)| key.as_str().map(|key| (key.to_string(), value)))
                    .collect(),
                error: None,
            },
            Ok(_) => FrontMatterResult::default(),
            Err(error) => FrontMatterResult {
                fields: BTreeMap::new(),
                error: Some(first_line(&error.to_string())),
            },
        }
    }

    /// Textfeld des Kopfes getrimmt; leer, fehlend oder kein Text ergibt None
    pub fn text(
        &self,
        fields: &BTreeMap<String, serde_yaml_ng::Value>,
        key: &str,
    ) -> Option<String> {
        let value = fields.get(key)?.as_str()?.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    /// Text zwischen den ersten beiden Zeilen „---“; wie /^---\r?\n([\s\S]*?)\r?\n---(\r?\n|$)/
    fn head(text: &str) -> Option<String> {
        let rest = text
            .strip_prefix("---\n")
            .or_else(|| text.strip_prefix("---\r\n"))?;
        // Erste Zeile, die nur aus „---“ besteht, beendet den Kopf
        let mut head: Vec<&str> = Vec::new();
        for line in rest.split_inclusive('\n') {
            let ohne_ende = line.trim_end_matches('\n').trim_end_matches('\r');
            if ohne_ende == "---" {
                return Some(head.join("\n"));
            }
            head.push(ohne_ende);
        }
        None
    }
}

/// Erste Zeile einer Meldung (Meldungen bleiben einzeilig)
fn first_line(message: &str) -> String {
    message
        .split(['\n', '\r'])
        .next()
        .unwrap_or(message)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feld(fields: &BTreeMap<String, serde_yaml_ng::Value>, key: &str) -> Option<String> {
        fields.get(key)?.as_str().map(ToString::to_string)
    }

    #[test]
    fn liest_die_felder_des_kopfes_auch_mit_crlf() {
        let front = FrontMatter::new();
        let fields = front.read("---\nname: x\ndescription: Kurz\n---\nText");
        assert_eq!(fields.len(), 2);
        assert_eq!(feld(&fields, "name").as_deref(), Some("x"));
        assert_eq!(feld(&fields, "description").as_deref(), Some("Kurz"));

        let crlf = front.read("---\r\nname: y\r\n---\r\n");
        assert_eq!(crlf.len(), 1);
        assert_eq!(feld(&crlf, "name").as_deref(), Some("y"));
    }

    #[test]
    fn liefert_ohne_oder_mit_kaputtem_kopf_ein_leeres_objekt() {
        let front = FrontMatter::new();
        assert!(front.read("# Nur Text").is_empty());
        assert!(front.read("---\nname: [offen\n---\n").is_empty());
    }

    #[test]
    fn meldet_einen_yaml_fehler_im_kopf_statt_ihn_zu_verschweigen() {
        let front = FrontMatter::new();
        let broken = front.parse("---\ndescription: Review: prüft vor dem Abschluss\n---\n");
        assert!(broken.fields.is_empty());
        let message = broken.error.expect("Fehler erwartet");
        assert!(!message.is_empty());
        assert!(!message.contains('\n'));

        assert_eq!(front.parse("# Nur Text"), FrontMatterResult::default());
        assert_eq!(front.parse("---\n\n---\n"), FrontMatterResult::default());
        let gut = front.parse("---\nname: x\n---\n");
        assert_eq!(gut.error, None);
        assert_eq!(feld(&gut.fields, "name").as_deref(), Some("x"));
    }

    #[test]
    fn liefert_textfelder_getrimmt_leere_als_none() {
        let front = FrontMatter::new();
        let mut fields = BTreeMap::new();
        fields.insert("description".to_string(), " Kurz ".into());
        assert_eq!(front.text(&fields, "description").as_deref(), Some("Kurz"));

        fields.insert("description".to_string(), "  ".into());
        assert_eq!(front.text(&fields, "description"), None);

        fields.insert("description".to_string(), 3.into());
        assert_eq!(front.text(&fields, "description"), None);

        assert_eq!(front.text(&BTreeMap::new(), "name"), None);
    }
}
