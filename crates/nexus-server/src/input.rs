//! Eingabeprüfung der `/api/db`-Routen; portiert aus
//! `packages/server/src/dbboard/DbInput.ts`. Die Meldungen sind wortgleich zu heute.

use nexus_core::board::basics::{Priority, TicketStatus};
use nexus_core::board::status_parser::StatusParser;
use nexus_core::{HarnessError, Result};
use serde_json::{Map, Value};

/// Der Körper einer Anfrage als Objekt
pub type Body = Map<String, Value>;

/// Ein Feld, das fehlen, `null` sein oder einen Wert tragen kann (Feld leeren)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nullable<T> {
    /// Das Feld fehlt: nichts ändern
    Missing,
    /// Das Feld ist `null`: Wert löschen
    Null,
    /// Das Feld trägt einen Wert
    Value(T),
}

/// Liest und prüft Eingaben der `/api/db`-Routen; Fehler sind `input-invalid` (422) mit
/// deutscher Meldung.
#[derive(Debug, Clone, Copy, Default)]
pub struct DbInput;

impl DbInput {
    /// Körper einer Anfrage als Objekt (fehlend oder kein Objekt = leer)
    pub fn body(raw: &Value) -> Body {
        raw.as_object().cloned().unwrap_or_default()
    }

    /// Pflichttext
    pub fn text(body: &Body, key: &str) -> Result<String> {
        match body.get(key).and_then(Value::as_str) {
            Some(value) => Ok(value.to_string()),
            None => Err(invalid(format!("Feld „{key}“ fehlt oder ist kein Text."))),
        }
    }

    /// Optionaler Text
    pub fn optional_text(body: &Body, key: &str) -> Result<Option<String>> {
        match body.get(key) {
            None => Ok(None),
            Some(_) => Ok(Some(Self::text(body, key)?)),
        }
    }

    /// Optionaler Text, der auch `null` sein darf (Feld leeren)
    pub fn nullable_text(body: &Body, key: &str) -> Result<Nullable<String>> {
        match body.get(key) {
            None => Ok(Nullable::Missing),
            Some(Value::Null) => Ok(Nullable::Null),
            Some(_) => Ok(Nullable::Value(Self::text(body, key)?)),
        }
    }

    /// Optionaler Schalter
    pub fn optional_bool(body: &Body, key: &str) -> Result<Option<bool>> {
        match body.get(key) {
            None => Ok(None),
            Some(Value::Bool(value)) => Ok(Some(*value)),
            Some(_) => Err(invalid(format!("Feld „{key}“ muss true oder false sein."))),
        }
    }

    /// Optionale Liste von Texten
    pub fn optional_list(body: &Body, key: &str) -> Result<Option<Vec<String>>> {
        let Some(value) = body.get(key) else {
            return Ok(None);
        };
        let texts = value.as_array().and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_string))
                .collect::<Option<Vec<String>>>()
        });
        texts
            .map(Some)
            .ok_or_else(|| invalid(format!("Feld „{key}“ muss eine Liste von Texten sein.")))
    }

    /// Pflichtliste von Texten mit mindestens einem und höchstens `max` Einträgen
    pub fn required_list(body: &Body, key: &str, max: usize) -> Result<Vec<String>> {
        let too_long = |list: &Vec<String>| list.is_empty() || list.len() > max;
        match Self::optional_list(body, key) {
            Ok(Some(list)) if !too_long(&list) => Ok(list),
            _ => Err(invalid(format!(
                "Feld „{key}“ muss eine Liste mit 1 bis {max} Einträgen sein."
            ))),
        }
    }

    /// Optionale ganze Zahl
    pub fn optional_int(body: &Body, key: &str) -> Result<Option<i64>> {
        match body.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_i64()
                .map(Some)
                .ok_or_else(|| invalid(format!("Feld „{key}“ muss eine ganze Zahl sein."))),
        }
    }

    /// Optionaler Status (deutsche Aliase erlaubt, „Wartet“ abgelehnt)
    pub fn optional_status(body: &Body, key: &str) -> Result<Option<TicketStatus>> {
        match Self::optional_text(body, key)? {
            None => Ok(None),
            Some(value) => StatusParser::parse(&value).map(Some),
        }
    }

    /// Optionale Priorität. In TypeScript reicht die Eingabe ungeprüft weiter und erst der
    /// Dienst meldet `priority-invalid`; in Rust gibt es keinen „Text als Priorität“, also
    /// prüft der Lader selbst – mit derselben Meldung wie der Dienst.
    pub fn optional_priority(body: &Body, key: &str) -> Result<Option<Priority>> {
        match Self::optional_text(body, key)? {
            None => Ok(None),
            Some(value) => value.parse::<Priority>().map(Some),
        }
    }

    /// Positive Nummer aus der Adresse (Board, Notiz)
    pub fn id(raw: &str, what: &str) -> Result<i32> {
        raw.parse::<i32>()
            .ok()
            .filter(|id| *id >= 1)
            .ok_or_else(|| {
                HarnessError::new(
                    "id-invalid",
                    format!("„{raw}“ ist keine gültige {what}-Nummer."),
                )
                .into()
            })
    }
}

/// Fehler einer ungültigen Eingabe
fn invalid(message: String) -> nexus_core::Error {
    HarnessError::new("input-invalid", message).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Körper aus JSON
    fn body(value: serde_json::Value) -> Body {
        DbInput::body(&value)
    }

    /// Code und Meldung eines Fehlers
    fn error(result: Result<impl std::fmt::Debug>) -> (String, String) {
        let error = result.unwrap_err();
        let harness = error.harness().expect("erwartbarer Fehler").clone();
        (harness.code, harness.message)
    }

    #[test]
    fn liest_den_korper_als_objekt() {
        assert_eq!(body(serde_json::json!({ "a": 1 })).len(), 1);
        assert!(body(serde_json::Value::Null).is_empty());
        assert!(body(serde_json::json!([1, 2])).is_empty());
        assert!(body(serde_json::json!("text")).is_empty());
    }

    #[test]
    fn prufft_pflicht_und_optionale_texte() {
        let full = body(serde_json::json!({ "title": "Titel", "note": null, "zahl": 3 }));
        assert_eq!(DbInput::text(&full, "title").unwrap(), "Titel");
        assert_eq!(
            error(DbInput::text(&full, "zahl")),
            (
                "input-invalid".into(),
                "Feld „zahl“ fehlt oder ist kein Text.".into()
            )
        );
        assert_eq!(
            error(DbInput::text(&full, "fehlt")).1,
            "Feld „fehlt“ fehlt oder ist kein Text."
        );
        assert_eq!(
            DbInput::optional_text(&full, "title").unwrap().as_deref(),
            Some("Titel")
        );
        assert_eq!(DbInput::optional_text(&full, "fehlt").unwrap(), None);
        assert!(DbInput::optional_text(&full, "note").is_err());

        assert_eq!(
            DbInput::nullable_text(&full, "note").unwrap(),
            Nullable::Null
        );
        assert_eq!(
            DbInput::nullable_text(&full, "fehlt").unwrap(),
            Nullable::Missing
        );
        assert_eq!(
            DbInput::nullable_text(&full, "title").unwrap(),
            Nullable::Value("Titel".into())
        );
    }

    #[test]
    fn prufft_schalter_zahlen_und_listen() {
        let full = body(serde_json::json!({
            "flag": true, "text": "x", "zahl": 5, "komma": 1.5,
            "liste": ["a", "b"], "gemischt": ["a", 1], "leer": [],
        }));
        assert_eq!(DbInput::optional_bool(&full, "flag").unwrap(), Some(true));
        assert_eq!(DbInput::optional_bool(&full, "fehlt").unwrap(), None);
        assert_eq!(
            error(DbInput::optional_bool(&full, "text")).1,
            "Feld „text“ muss true oder false sein."
        );

        assert_eq!(DbInput::optional_int(&full, "zahl").unwrap(), Some(5));
        assert_eq!(DbInput::optional_int(&full, "fehlt").unwrap(), None);
        assert_eq!(
            error(DbInput::optional_int(&full, "komma")).1,
            "Feld „komma“ muss eine ganze Zahl sein."
        );

        assert_eq!(
            DbInput::optional_list(&full, "liste").unwrap(),
            Some(vec!["a".to_string(), "b".to_string()])
        );
        assert_eq!(DbInput::optional_list(&full, "fehlt").unwrap(), None);
        assert_eq!(
            error(DbInput::optional_list(&full, "gemischt")).1,
            "Feld „gemischt“ muss eine Liste von Texten sein."
        );

        assert_eq!(DbInput::required_list(&full, "liste", 5).unwrap().len(), 2);
        assert_eq!(
            error(DbInput::required_list(&full, "leer", 5)).1,
            "Feld „leer“ muss eine Liste mit 1 bis 5 Einträgen sein."
        );
        assert_eq!(
            error(DbInput::required_list(&full, "liste", 1)).1,
            "Feld „liste“ muss eine Liste mit 1 bis 1 Einträgen sein."
        );
        assert_eq!(
            error(DbInput::required_list(&full, "fehlt", 3)).1,
            "Feld „fehlt“ muss eine Liste mit 1 bis 3 Einträgen sein."
        );
    }

    #[test]
    fn liest_status_und_prioritat() {
        let full = body(serde_json::json!({
            "status": "In Arbeit", "wartet": "wartet", "prio": "high", "fremd": "sofort",
        }));
        assert_eq!(
            DbInput::optional_status(&full, "status").unwrap(),
            Some(TicketStatus::InProgress)
        );
        assert_eq!(DbInput::optional_status(&full, "fehlt").unwrap(), None);
        assert_eq!(
            error(DbInput::optional_status(&full, "wartet")).0,
            "status-invalid"
        );

        assert_eq!(
            DbInput::optional_priority(&full, "prio").unwrap(),
            Some(Priority::High)
        );
        assert_eq!(DbInput::optional_priority(&full, "fehlt").unwrap(), None);
        assert_eq!(
            error(DbInput::optional_priority(&full, "fremd")).0,
            "priority-invalid"
        );
    }

    #[test]
    fn liest_nummern_aus_der_adresse() {
        assert_eq!(DbInput::id("15", "Board").unwrap(), 15);
        for raw in ["0", "-1", "abc", "1.5", ""] {
            assert_eq!(
                error(DbInput::id(raw, "Board")),
                (
                    "id-invalid".into(),
                    format!("„{raw}“ ist keine gültige Board-Nummer.")
                )
            );
        }
    }
}
