//! Kleine Helfer, die JavaScript-Semantik auf `serde_json::Value` nachbilden.
//!
//! Die rohe Einstellung bleibt ein `Value` (Reihenfolge der Schlüssel bleibt erhalten, siehe
//! `GET /api/dispatch/config`). Damit die Prüfungen Zeichen für Zeichen dieselben Fehler
//! melden wie der TypeScript-Code, zählen hier dieselben Umwandlungen wie dort: `Number(x)`
//! und `String(x)`.

use serde_json::Value;

/// `Number(value)` aus JavaScript; `None` steht für `NaN`
pub fn js_number(value: &Value) -> Option<f64> {
    match value {
        Value::Null => Some(0.0),
        Value::Bool(true) => Some(1.0),
        Value::Bool(false) => Some(0.0),
        Value::Number(number) => number.as_f64(),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Some(0.0)
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        // Objekte und Listen ergeben in JavaScript NaN (die leere Liste wäre 0, kommt hier
        // aber nirgends als Zahl vor)
        Value::Array(_) | Value::Object(_) => None,
    }
}

/// `String(value)` aus JavaScript – für Meldungen und Tool-Listen
pub fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Null => String::new(),
                other => js_string(other),
            })
            .collect::<Vec<String>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// Ist der Wert eine ganze Zahl (`Number.isInteger`)?
pub fn is_integer(number: f64) -> bool {
    number.is_finite() && number.fract() == 0.0
}

/// Der Wert unter diesem Schlüssel, falls das Elternteil ein Objekt ist
pub fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object().and_then(|object| object.get(key))
}

/// Wie `field`, aber `Value::Null` statt `None`, wenn der Schlüssel fehlt
pub fn field_or_null<'a>(value: &'a Value, key: &str) -> &'a Value {
    field(value, key).unwrap_or(&Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn number_folgt_javascript() {
        assert_eq!(js_number(&json!(5)), Some(5.0));
        assert_eq!(js_number(&json!("7")), Some(7.0));
        assert_eq!(js_number(&json!("")), Some(0.0));
        assert_eq!(js_number(&Value::Null), Some(0.0));
        assert_eq!(js_number(&json!(true)), Some(1.0));
        assert_eq!(js_number(&json!("x")), None);
        assert_eq!(js_number(&json!({})), None);
    }

    #[test]
    fn string_folgt_javascript() {
        assert_eq!(js_string(&json!("a")), "a");
        assert_eq!(js_string(&json!(3)), "3");
        assert_eq!(js_string(&Value::Null), "null");
        assert_eq!(js_string(&json!(true)), "true");
        assert_eq!(js_string(&json!(["a", "b"])), "a,b");
        assert_eq!(js_string(&json!({ "a": 1 })), "[object Object]");
    }

    #[test]
    fn ganze_zahlen_werden_erkannt() {
        assert!(is_integer(5.0));
        assert!(!is_integer(5.5));
        assert!(!is_integer(f64::INFINITY));
    }
}
