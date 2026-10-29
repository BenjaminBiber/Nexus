//! Feldweiser Vergleich zweier JSON-Objekte für den Schattenbetrieb.
//!
//! Verglichen wird immer nur, was ausdrücklich genannt ist: So fällt ein Feld, das Rust noch
//! gar nicht bestimmen kann, nicht als Abweichung auf, und ein neues Feld in der Lauf-Datei
//! macht den Vergleich nicht stillschweigend ungenau.

use serde_json::Value;

use crate::dispatch::shadow::types::ShadowDiff;

/// Holt einen Wert über einen Pfad mit Punkten (`run.profile`); fehlt ein Stück, kommt
/// `Value::Null`
pub fn at_path<'a>(value: &'a Value, path: &str) -> &'a Value {
    let mut current = value;
    for part in path.split('.') {
        current = match current.get(part) {
            Some(next) => next,
            None => return &Value::Null,
        };
    }
    current
}

/// Ein Wert, wie ihn Zeile und Bericht zeigen: Texte unverändert, alles andere als JSON;
/// `null` und fehlende Felder als `None`
pub fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

/// Vergleicht die genannten Felder zweier Objekte und liefert die Abweichungen in der
/// Reihenfolge der Felder
pub fn diffs(node: &Value, rust: &Value, fields: &[&str]) -> Vec<ShadowDiff> {
    let mut found = Vec::new();
    for field in fields {
        let left = at_path(node, field);
        let right = at_path(rust, field);
        if left == right {
            continue;
        }
        found.push(ShadowDiff::new(*field, text_of(left), text_of(right)));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gleiche_felder_ergeben_keine_abweichung() {
        let node = json!({ "run": { "profile": "leicht", "maxTurns": 60 } });
        assert!(diffs(&node, &node.clone(), &["run.profile", "run.maxTurns"]).is_empty());
    }

    #[test]
    fn eine_abweichung_nennt_feld_und_beide_werte() {
        let node = json!({ "run": { "profile": "leicht", "maxTurns": 60 } });
        let rust = json!({ "run": { "profile": "schwer", "maxTurns": 60 } });
        assert_eq!(
            diffs(&node, &rust, &["run.profile", "run.maxTurns"]),
            vec![ShadowDiff::new(
                "run.profile",
                Some("leicht".into()),
                Some("schwer".into())
            )]
        );
    }

    #[test]
    fn ein_fehlendes_feld_zaehlt_wie_null() {
        let node = json!({ "attempt": 2 });
        let rust = json!({});
        assert_eq!(
            diffs(&node, &rust, &["attempt"]),
            vec![ShadowDiff::new("attempt", Some("2".into()), None)]
        );
    }

    #[test]
    fn nicht_genannte_felder_bleiben_aussen_vor() {
        let node = json!({ "run": { "profile": "leicht" }, "pid": 1 });
        let rust = json!({ "run": { "profile": "leicht" }, "pid": 2 });
        assert!(diffs(&node, &rust, &["run.profile"]).is_empty());
    }

    #[test]
    fn listen_und_zahlen_stehen_als_json_da() {
        let node = json!({ "args": ["-p", "--model", "opus"] });
        let rust = json!({ "args": ["-p"] });
        assert_eq!(
            diffs(&node, &rust, &["args"]),
            vec![ShadowDiff::new(
                "args",
                Some(r#"["-p","--model","opus"]"#.into()),
                Some(r#"["-p"]"#.into())
            )]
        );
    }
}
