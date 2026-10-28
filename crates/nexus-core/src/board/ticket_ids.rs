//! Projektkey prüfen und Ticket-IDs der Form „ARC-0042“ bilden und zerlegen.

use std::sync::LazyLock;

use regex::Regex;

use crate::error::{HarnessError, Result};

/// Großbuchstaben und Ziffern, 2–6 Zeichen, beginnt mit einem Buchstaben
static PROJECT_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Z][A-Z0-9]{1,5}$").unwrap());

/// Ticket-ID: Projektkey, Bindestrich, Nummer. `[0-9]` statt `\d`, weil Rust sonst auch
/// Ziffern anderer Schriften nimmt, JavaScript aber nur die ASCII-Ziffern
static TICKET_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z][A-Za-z0-9]{1,5})-([0-9]+)$").unwrap());

/// Projektkey und Nummer einer Ticket-ID
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketId {
    pub key: String,
    pub number: u32,
}

/// Bildet und zerlegt Ticket-IDs.
pub struct TicketIds;

impl TicketIds {
    /// Großbuchstaben und Ziffern, 2–6 Zeichen, beginnt mit einem Buchstaben
    pub fn normalize_key(input: &str) -> Result<String> {
        let key: String = input
            .to_uppercase()
            .chars()
            .filter(|zeichen| zeichen.is_ascii_uppercase() || zeichen.is_ascii_digit())
            .collect();
        if !PROJECT_KEY.is_match(&key) {
            return Err(HarnessError::new(
                "project-key-invalid",
                format!("Projektkey „{input}“ ist ungültig: 2–6 Buchstaben oder Ziffern, beginnend mit einem Buchstaben."),
            )
            .into());
        }
        Ok(key)
    }

    /// Bildet die ID mit mindestens vier Ziffern
    pub fn format(key: &str, n: u32) -> String {
        format!("{key}-{n:04}")
    }

    /// Zerlegt eine ID in Key und Nummer
    pub fn parse(id: &str) -> Result<TicketId> {
        let ungultig = || {
            HarnessError::new(
                "ticket-id-invalid",
                format!("„{id}“ ist keine Ticket-ID (erwartet z. B. ARC-0042)."),
            )
        };
        let gefunden = TICKET_ID.captures(id.trim()).ok_or_else(ungultig)?;
        // Nummern jenseits von u32 kommen in keiner Spalte vor und gelten als ungültige ID
        let number = gefunden[2].parse::<u32>().map_err(|_| ungultig())?;
        Ok(TicketId {
            key: gefunden[1].to_uppercase(),
            number,
        })
    }

    /// Schreibweise vereinheitlichen (Großbuchstaben, vier Ziffern)
    pub fn canonical(id: &str) -> Result<String> {
        let TicketId { key, number } = TicketIds::parse(id)?;
        Ok(TicketIds::format(&key, number))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisiert_den_projektkey() {
        assert_eq!(TicketIds::normalize_key(" arc ").unwrap(), "ARC");
        assert_eq!(TicketIds::normalize_key("Inc-1").unwrap(), "INC1");
    }

    #[test]
    fn lehnt_ungultige_keys_ab() {
        for key in ["1AB", "A", "ABCDEFG", ""] {
            let error = TicketIds::normalize_key(key).unwrap_err();
            assert_eq!(
                error.code(),
                Some("project-key-invalid"),
                "Key „{key}“ müsste abgelehnt werden"
            );
        }
    }

    #[test]
    fn nennt_die_regel_im_fehler_zum_projektkey() {
        let error = TicketIds::normalize_key("1AB").unwrap_err();
        assert_eq!(
            error.to_string(),
            "Projektkey „1AB“ ist ungültig: 2–6 Buchstaben oder Ziffern, beginnend mit einem Buchstaben."
        );
    }

    #[test]
    fn formatiert_vierstellig_und_wachst_daruber_hinaus() {
        assert_eq!(TicketIds::format("ARC", 42), "ARC-0042");
        assert_eq!(TicketIds::format("ARC", 12345), "ARC-12345");
    }

    #[test]
    fn zerlegt_eine_id_und_akzeptiert_kleinschreibung() {
        assert_eq!(
            TicketIds::parse("arc-0042").unwrap(),
            TicketId {
                key: "ARC".into(),
                number: 42
            }
        );
        assert_eq!(TicketIds::parse(" ARC-0042 ").unwrap().number, 42);
        assert_eq!(TicketIds::canonical("arc-42").unwrap(), "ARC-0042");
    }

    #[test]
    fn lehnt_eine_ungultige_id_ab() {
        let error = TicketIds::parse("42").unwrap_err();
        assert_eq!(error.code(), Some("ticket-id-invalid"));
        assert_eq!(
            error.to_string(),
            "„42“ ist keine Ticket-ID (erwartet z. B. ARC-0042)."
        );
    }

    #[test]
    fn lehnt_eine_masslos_grosse_nummer_ab() {
        let error = TicketIds::parse("ARC-99999999999").unwrap_err();
        assert_eq!(error.code(), Some("ticket-id-invalid"));
    }
}
