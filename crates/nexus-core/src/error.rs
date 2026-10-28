//! Fehler des Fachkerns: erwartbare Fehler mit stabilem Code und unerwartete Fehler.

use serde::Serialize;

/// Erwartbarer Fehler mit deutscher Meldung und stabilem Code (für Tests, `--json` und HTTP);
/// entspricht `HarnessError` aus TypeScript, das CLI gibt ihn als Rückgabecode 1 aus
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize)]
#[error("{message}")]
pub struct HarnessError {
    pub code: String,
    pub message: String,
    /// Zusätzliche Angaben für Aufrufer (z. B. der Kreis einer Abhängigkeit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl HarnessError {
    /// Erzeugt den Fehler mit einem stabilen Code, der Meldung und ohne weitere Angaben
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Ergänzt die zusätzlichen Angaben
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Alle Fehler des Kerns: erwartbar (`HarnessError`) oder unerwartet (Treiber, E/A …);
/// unerwartete Fehler entsprechen in TypeScript den rohen Fehlern (HTTP 500, Stack im CLI)
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Harness(#[from] HarnessError),
    #[error("{message}")]
    Unexpected {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl Error {
    /// Verpackt einen fremden Fehler (Treiber, E/A …) als unerwarteten Fehler
    pub fn unexpected(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        let source = source.into();
        Error::Unexpected {
            message: source.to_string(),
            source: Some(source),
        }
    }

    /// Unerwarteter Fehler, zu dem es nur eine Meldung gibt
    pub fn unexpected_message(message: impl Into<String>) -> Self {
        Error::Unexpected {
            message: message.into(),
            source: None,
        }
    }

    /// Stabiler Code, falls der Fehler erwartbar ist; unerwartete Fehler haben keinen
    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Harness(error) => Some(&error.code),
            Error::Unexpected { .. } => None,
        }
    }

    /// Der erwartbare Fehler, falls es einer ist (für Tests und die HTTP-Schicht)
    pub fn harness(&self) -> Option<&HarnessError> {
        match self {
            Error::Harness(error) => Some(error),
            Error::Unexpected { .. } => None,
        }
    }
}

/// Ergebnis des Kerns; der Fehlertyp ist standardmäßig [`Error`]
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_fehler_tragt_code_und_meldung() {
        let error = HarnessError::new("ticket-id-invalid", "„42“ ist keine Ticket-ID.");
        assert_eq!(error.code, "ticket-id-invalid");
        assert_eq!(error.to_string(), "„42“ ist keine Ticket-ID.");
        assert_eq!(error.details, None);
    }

    #[test]
    fn details_werden_nur_bei_bedarf_serialisiert() {
        let ohne = HarnessError::new("input-invalid", "Eingabe ungültig.");
        assert_eq!(
            serde_json::to_value(&ohne).unwrap(),
            serde_json::json!({ "code": "input-invalid", "message": "Eingabe ungültig." })
        );

        let mit = ohne
            .clone()
            .with_details(serde_json::json!({ "cycle": ["ARC-0001"] }));
        assert_eq!(
            serde_json::to_value(&mit).unwrap(),
            serde_json::json!({
                "code": "input-invalid",
                "message": "Eingabe ungültig.",
                "details": { "cycle": ["ARC-0001"] },
            })
        );
    }

    #[test]
    fn erwartbarer_fehler_behalt_seinen_code() {
        let error: Error = HarnessError::new("status-invalid", "Status ungültig.").into();
        assert_eq!(error.code(), Some("status-invalid"));
        assert_eq!(error.to_string(), "Status ungültig.");
        assert_eq!(
            error.harness().map(|e| e.code.as_str()),
            Some("status-invalid")
        );
    }

    #[test]
    fn unerwarteter_fehler_hat_keinen_code() {
        let quelle = std::io::Error::other("Verbindung weg");
        let error = Error::unexpected(quelle);
        assert_eq!(error.code(), None);
        assert_eq!(error.to_string(), "Verbindung weg");
        assert!(std::error::Error::source(&error).is_some());

        let ohne_quelle = Error::unexpected_message("Treiber antwortet nicht");
        assert_eq!(ohne_quelle.to_string(), "Treiber antwortet nicht");
        assert!(std::error::Error::source(&ohne_quelle).is_none());
    }
}
