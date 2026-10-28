//! Prüft Texte gegen die Höchstlängen der Datenbank-Spalten, bevor SQL sie abschneidet
//! oder ablehnt.

use crate::board::basics::{BoardActor, MAX_PERSON_NAME_LENGTH};
use crate::error::{HarnessError, Result};
use crate::text::utf16_len;

/// Prüft Längen.
pub struct BoardLimits;

impl BoardLimits {
    /// Liefert den Text unverändert oder den Fehler „value-too-long“; gezählt wird in
    /// UTF-16-Einheiten, also wie in JavaScript und wie `nvarchar(n)` in SQL Server
    pub fn text<'a>(value: &'a str, max: usize, what: &str) -> Result<&'a str> {
        let length = utf16_len(value);
        if length > max {
            return Err(HarnessError::new(
                "value-too-long",
                format!("{what} ist zu lang ({length} Zeichen, höchstens {max})."),
            )
            .into());
        }
        Ok(value)
    }

    /// Wie [`BoardLimits::text`], aber für Spalten, die auch leer bleiben dürfen; `None` bleibt `None`
    pub fn text_opt<'a>(value: Option<&'a str>, max: usize, what: &str) -> Result<Option<&'a str>> {
        match value {
            Some(value) => BoardLimits::text(value, max, what).map(Some),
            None => Ok(None),
        }
    }

    /// Name des Handelnden (Verlauf, Kommentare, Notizen, Löschprotokoll) – die eine Stelle
    /// für alle Dienste
    pub fn actor(actor: &BoardActor) -> Result<&BoardActor> {
        BoardLimits::text(
            &actor.name,
            MAX_PERSON_NAME_LENGTH,
            "Der Name des Handelnden",
        )?;
        Ok(actor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lasst_genau_die_hochstlange_durch() {
        let text = "a".repeat(400);
        assert_eq!(BoardLimits::text(&text, 400, "Der Titel").unwrap(), text);
    }

    #[test]
    fn lehnt_ein_zeichen_zu_viel_mit_value_too_long_ab() {
        let text = "a".repeat(401);
        let error = BoardLimits::text(&text, 400, "Der Titel").unwrap_err();
        assert_eq!(error.code(), Some("value-too-long"));
        assert_eq!(
            error.to_string(),
            "Der Titel ist zu lang (401 Zeichen, höchstens 400)."
        );
    }

    #[test]
    fn zahlt_ein_emoji_wie_javascript_als_zwei_zeichen() {
        // „🙂“ ist ein Zeichen, zählt in JavaScript und in nvarchar aber zwei Einheiten
        assert!(BoardLimits::text("🙂", 2, "Der Titel").is_ok());
        let error = BoardLimits::text("🙂", 1, "Der Titel").unwrap_err();
        assert_eq!(
            error.to_string(),
            "Der Titel ist zu lang (2 Zeichen, höchstens 1)."
        );

        // zwei Emojis passen in vier, aber nicht in drei Einheiten
        assert!(BoardLimits::text("🙂🙂", 4, "Der Titel").is_ok());
        assert!(BoardLimits::text("🙂🙂", 3, "Der Titel").is_err());
    }

    #[test]
    fn leere_spalten_bleiben_leer() {
        assert_eq!(BoardLimits::text_opt(None, 10, "Der Grund").unwrap(), None);
        assert_eq!(
            BoardLimits::text_opt(Some("kurz"), 10, "Der Grund").unwrap(),
            Some("kurz")
        );
        assert!(BoardLimits::text_opt(Some("viel zu lang"), 5, "Der Grund").is_err());
    }

    #[test]
    fn pruft_den_namen_des_handelnden() {
        let actor = BoardActor {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            name: "claude".into(),
        };
        assert_eq!(BoardLimits::actor(&actor).unwrap(), &actor);

        let zu_lang = BoardActor {
            name: "n".repeat(101),
            ..actor
        };
        let error = BoardLimits::actor(&zu_lang).unwrap_err();
        assert_eq!(error.code(), Some("value-too-long"));
        assert_eq!(
            error.to_string(),
            "Der Name des Handelnden ist zu lang (101 Zeichen, höchstens 100)."
        );
    }
}
