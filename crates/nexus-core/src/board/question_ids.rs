//! Fragenummern der Form „ARC-F0001“ bilden und erkennen; Verweise auf Fragen oder
//! Tickets vereinheitlichen.

use std::sync::LazyLock;

use regex::Regex;

use crate::board::ticket_ids::TicketIds;
use crate::error::{HarnessError, Result};

/// Fragenummer: Projektkey, Bindestrich, „F“, Nummer (Schreibweise egal)
static QUESTION_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z][A-Za-z0-9]{1,5})-[Ff]([0-9]+)$").unwrap());

/// Bildet und erkennt Fragenummern.
pub struct QuestionIds;

impl QuestionIds {
    /// Bildet die Nummer mit mindestens vier Ziffern
    pub fn format(key: &str, n: u32) -> String {
        format!("{key}-F{n:04}")
    }

    /// Ist der Text eine Fragenummer (Schreibweise egal)?
    pub fn is_question(id: &str) -> bool {
        QUESTION_ID.is_match(id.trim())
    }

    /// Schreibweise vereinheitlichen (Großbuchstaben, vier Ziffern)
    pub fn canonical(id: &str) -> Result<String> {
        let ungultig = || {
            HarnessError::new(
                "question-id-invalid",
                format!("„{id}“ ist keine Fragenummer (erwartet z. B. ARC-F0001)."),
            )
        };
        let gefunden = QUESTION_ID.captures(id.trim()).ok_or_else(ungultig)?;
        let number = gefunden[2].parse::<u32>().map_err(|_| ungultig())?;
        Ok(QuestionIds::format(&gefunden[1].to_uppercase(), number))
    }

    /// Projektkey einer Fragenummer
    pub fn key_of(id: &str) -> Result<String> {
        let canonical = QuestionIds::canonical(id)?;
        Ok(canonical.split('-').next().unwrap_or_default().to_string())
    }

    /// Verweis auf eine Frage oder ein Ticket in einheitlicher Schreibweise
    pub fn reference(id: &str) -> Result<String> {
        if QuestionIds::is_question(id) {
            return QuestionIds::canonical(id);
        }
        TicketIds::canonical(id).map_err(|_| {
            HarnessError::new(
                "reference-invalid",
                format!("„{id}“ ist weder eine Frage (z. B. ARC-F0001) noch ein Ticket (z. B. ARC-0001)."),
            )
            .into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bildet_und_vereinheitlicht_fragenummern() {
        assert_eq!(QuestionIds::format("ARC", 7), "ARC-F0007");
        assert_eq!(QuestionIds::format("ARC", 12345), "ARC-F12345");
        assert_eq!(QuestionIds::canonical(" arc-f7 ").unwrap(), "ARC-F0007");
        assert!(QuestionIds::is_question("ARC-F0001"));
        assert!(!QuestionIds::is_question("ARC-0001"));
        assert_eq!(QuestionIds::key_of("arc-f0003").unwrap(), "ARC");

        let error = QuestionIds::canonical("ARC-0001").unwrap_err();
        assert_eq!(error.code(), Some("question-id-invalid"));
        assert_eq!(
            error.to_string(),
            "„ARC-0001“ ist keine Fragenummer (erwartet z. B. ARC-F0001)."
        );
    }

    #[test]
    fn vereinheitlicht_verweise_auf_fragen_oder_tickets() {
        assert_eq!(QuestionIds::reference("arc-f2").unwrap(), "ARC-F0002");
        assert_eq!(QuestionIds::reference("arc-3").unwrap(), "ARC-0003");

        let error = QuestionIds::reference("quatsch").unwrap_err();
        assert_eq!(error.code(), Some("reference-invalid"));
        assert_eq!(
            error.to_string(),
            "„quatsch“ ist weder eine Frage (z. B. ARC-F0001) noch ein Ticket (z. B. ARC-0001)."
        );
    }
}
