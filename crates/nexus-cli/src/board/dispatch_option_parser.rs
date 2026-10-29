//! Prüft die Optionen der Dispatcher-Befehle.
//! Portiert aus `packages/cli/src/board/DispatchOptionParser.ts`.

use nexus_core::registry::DEFAULT_SERVER_PORT;
use nexus_core::{HarnessError, Result};

/// Prüft die Optionen der Dispatcher-Befehle.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchOptionParser;

impl DispatchOptionParser {
    /// Board-ID aus der Option
    pub fn board_id(&self, value: Option<&str>) -> Result<Option<i32>> {
        let Some(value) = value else {
            return Ok(None);
        };
        match value.parse::<i32>() {
            Ok(id) if id >= 1 => Ok(Some(id)),
            _ => Err(
                HarnessError::new("id-invalid", format!("„{value}“ ist keine Board-ID.")).into(),
            ),
        }
    }

    /// Dauer in Minuten, ganze Zahl von 1 bis 1440
    pub fn minutes(&self, value: &str) -> Result<u32> {
        let ziffern = !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit());
        match value.parse::<u32>() {
            Ok(minutes) if ziffern && (1..=1440).contains(&minutes) => Ok(minutes),
            _ => Err(HarnessError::new(
                "input-invalid",
                "minutes muss eine ganze Zahl von 1 bis 1440 sein.",
            )
            .into()),
        }
    }

    /// Port: Option, sonst `HARNESS_PORT`, sonst Standard
    pub fn port(&self, option: Option<&str>, env: Option<&str>) -> Result<u16> {
        let default = DEFAULT_SERVER_PORT.to_string();
        let raw = option.or(env).unwrap_or(&default);
        match raw.parse::<u16>() {
            Ok(port) if port >= 1 => Ok(port),
            _ => Err(
                HarnessError::new("port-invalid", format!("Der Port „{raw}“ ist ungültig.")).into(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARSER: DispatchOptionParser = DispatchOptionParser;

    #[test]
    fn akzeptiert_gueltige_minuten() {
        assert_eq!(PARSER.minutes("15").unwrap(), 15);
        assert_eq!(PARSER.minutes("1440").unwrap(), 1440);
    }

    #[test]
    fn weist_kaputte_minuten_ab() {
        for value in ["abc", "0", "1441", "1.5", "-3", ""] {
            let error = PARSER.minutes(value).unwrap_err();
            assert_eq!(error.code(), Some("input-invalid"), "{value}");
            assert_eq!(
                error.to_string(),
                "minutes muss eine ganze Zahl von 1 bis 1440 sein."
            );
        }
    }

    #[test]
    fn weist_eine_kaputte_board_nummer_ab() {
        let error = PARSER.board_id(Some("x")).unwrap_err();
        assert_eq!(error.code(), Some("id-invalid"));
        assert_eq!(error.to_string(), "„x“ ist keine Board-ID.");
        assert_eq!(PARSER.board_id(Some("3")).unwrap(), Some(3));
        assert_eq!(PARSER.board_id(None).unwrap(), None);
        assert_eq!(
            PARSER.board_id(Some("0")).unwrap_err().code(),
            Some("id-invalid")
        );
    }

    #[test]
    fn bevorzugt_option_vor_umgebung_vor_standard() {
        assert_eq!(PARSER.port(Some("4800"), Some("4900")).unwrap(), 4800);
        assert_eq!(PARSER.port(None, Some("4900")).unwrap(), 4900);
        assert_eq!(PARSER.port(None, None).unwrap(), DEFAULT_SERVER_PORT);
        let error = PARSER.port(Some("70000"), None).unwrap_err();
        assert_eq!(error.code(), Some("port-invalid"));
        assert_eq!(error.to_string(), "Der Port „70000“ ist ungültig.");
    }
}
