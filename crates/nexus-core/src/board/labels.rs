//! Vereinheitlicht Labels für Tickets und Notizen.

use std::collections::HashSet;

use crate::board::basics::MAX_LABEL_LENGTH;
use crate::board::limits::BoardLimits;
use crate::error::Result;
use crate::text::utf16_prefix;

/// Räumt Label-Listen auf.
pub struct BoardLabels;

impl BoardLabels {
    /// An Kommas trennen, trimmen, Leere entfernen, Doppelte ohne Rücksicht auf
    /// Groß-/Kleinschreibung entfernen (erste Schreibweise bleibt; der SQL-Schlüssel
    /// unterscheidet sie nicht) und Länge prüfen
    pub fn normalize(input: &[String]) -> Result<Vec<String>> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        for label in input.iter().flat_map(|l| l.split(',')).map(str::trim) {
            let key = label.to_lowercase();
            if label.is_empty() || seen.contains(&key) {
                continue;
            }
            BoardLimits::text(
                label,
                MAX_LABEL_LENGTH,
                &format!("Label „{}…“", utf16_prefix(label, 30)),
            )?;
            seen.insert(key);
            result.push(label.to_string());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(input: &[&str]) -> Vec<String> {
        input.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn trennt_an_kommas_und_raumt_leerraum_weg() {
        assert_eq!(
            BoardLabels::normalize(&labels(&[" rust , core ", "", " , ", "auto"])).unwrap(),
            ["rust", "core", "auto"]
        );
        assert_eq!(BoardLabels::normalize(&[]).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn entfernt_doppelte_und_behalt_die_erste_schreibweise() {
        assert_eq!(
            BoardLabels::normalize(&labels(&["Rust", "rust", "RUST,core"])).unwrap(),
            ["Rust", "core"]
        );
    }

    #[test]
    fn lehnt_ein_zu_langes_label_mit_value_too_long_ab() {
        let lang = "l".repeat(101);
        let error = BoardLabels::normalize(&labels(&[&lang])).unwrap_err();
        assert_eq!(error.code(), Some("value-too-long"));
        // in der Meldung stehen nur die ersten 30 Zeichen des Labels
        assert_eq!(
            error.to_string(),
            format!(
                "Label „{}…“ ist zu lang (101 Zeichen, höchstens 100).",
                "l".repeat(30)
            )
        );
    }

    #[test]
    fn genau_hundert_zeichen_sind_erlaubt() {
        let label = "l".repeat(100);
        assert_eq!(BoardLabels::normalize(&labels(&[&label])).unwrap(), [label]);
    }

    #[test]
    fn schneidet_die_meldung_ohne_halbes_emoji_ab() {
        // 60 Emojis sind 120 UTF-16-Einheiten; in den ersten 30 Einheiten stehen 15 Emojis
        let label = "🙂".repeat(60);
        let error = BoardLabels::normalize(&labels(&[&label])).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "Label „{}…“ ist zu lang (120 Zeichen, höchstens 100).",
                "🙂".repeat(15)
            )
        );
    }
}
