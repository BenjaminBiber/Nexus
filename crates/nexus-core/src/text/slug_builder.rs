//! Baut aus einem Titel einen Kurztitel, der als Git-Branch und Ordnername taugt.
//! Portiert aus `packages/core/src/text/SlugBuilder.ts`.

/// Höchstlänge eines Kurztitels
const MAX_LENGTH: usize = 40;

/// Baut Kurztitel.
#[derive(Debug, Clone, Copy, Default)]
pub struct SlugBuilder;

impl SlugBuilder {
    /// Kleinschreibung, Umlaute umschreiben, Sonderzeichen zu Bindestrichen, höchstens
    /// 40 Zeichen
    pub fn build(text: &str) -> String {
        let mut dashed = String::with_capacity(text.len());
        for zeichen in text.to_lowercase().chars() {
            match zeichen {
                'ä' => dashed.push_str("ae"),
                'ö' => dashed.push_str("oe"),
                'ü' => dashed.push_str("ue"),
                'ß' => dashed.push_str("ss"),
                'a'..='z' | '0'..='9' => dashed.push(zeichen),
                // Jede Folge anderer Zeichen wird ein Bindestrich
                _ if !dashed.ends_with('-') => dashed.push('-'),
                _ => {}
            }
        }
        let slug = shorten(dashed.trim_matches('-'));
        if slug.is_empty() {
            "ticket".to_string()
        } else {
            slug
        }
    }
}

/// Kürzt am letzten Bindestrich innerhalb der Höchstlänge, damit kein Wort zerschnitten wird
fn shorten(slug: &str) -> String {
    if slug.len() <= MAX_LENGTH {
        return slug.to_string();
    }
    let cut = &slug[..MAX_LENGTH];
    let kurz = match cut.rfind('-') {
        Some(dash) if dash > 0 => &cut[..dash],
        _ => cut,
    };
    kurz.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schreibt_klein_und_ersetzt_leerzeichen_durch_bindestriche() {
        assert_eq!(
            SlugBuilder::build("Login Formular bauen"),
            "login-formular-bauen"
        );
    }

    #[test]
    fn schreibt_umlaute_und_scharfes_s_um() {
        assert_eq!(
            SlugBuilder::build("Größe Änderung für Übung"),
            "groesse-aenderung-fuer-uebung"
        );
    }

    #[test]
    fn entfernt_sonderzeichen_und_fasst_bindestriche_zusammen() {
        assert_eq!(
            SlugBuilder::build("  Token: prüfen!!! (v2) "),
            "token-pruefen-v2"
        );
    }

    #[test]
    fn kurzt_auf_40_zeichen_am_wortende() {
        let result = SlugBuilder::build(
            "eine sehr lange Überschrift die weit über vierzig Zeichen hinausgeht",
        );
        assert!(result.len() <= 40);
        assert!(!result.ends_with('-'));
        assert_eq!(result, "eine-sehr-lange-ueberschrift-die-weit");
    }

    #[test]
    fn liefert_ticket_bei_leerem_ergebnis() {
        assert_eq!(SlugBuilder::build("???"), "ticket");
    }
}
