//! Kleine Texthelfer für Meldungen und Längen. Wichtig: JavaScript zählt `string.length`
//! und schneidet mit `slice` in UTF-16-Einheiten, `nvarchar(n)` in SQL Server ebenso.
//! Rust zählt sonst Bytes oder Zeichen – deshalb die beiden Funktionen hier.

/// Länge in UTF-16-Einheiten, also die Zahl, die JavaScript als `value.length` liefert
pub fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Erste `max` UTF-16-Einheiten wie JavaScript `value.slice(0, max)`; ein Zeichen, das
/// nur halb hineinpasst (halbes Surrogatpaar), entfällt, damit kein gültiger Text zerfällt
pub fn utf16_prefix(value: &str, max: usize) -> &str {
    let mut used = 0;
    for (index, zeichen) in value.char_indices() {
        let breite = zeichen.len_utf16();
        if used + breite > max {
            return &value[..index];
        }
        used += breite;
    }
    value
}

/// Ausschnitt ab `start` mit höchstens `len` UTF-16-Einheiten, wie JavaScript
/// `value.slice(start, start + len)`; ein Zeichen, das nur halb hineinpasst
/// (halbes Surrogatpaar), entfällt an beiden Enden
pub fn utf16_slice(value: &str, start: usize, len: usize) -> &str {
    let mut used = 0;
    let mut from: Option<usize> = None;
    let mut to = value.len();
    for (index, zeichen) in value.char_indices() {
        if from.is_none() && used >= start {
            from = Some(index);
        }
        let breite = zeichen.len_utf16();
        if from.is_some() && used + breite > start + len {
            to = index;
            break;
        }
        used += breite;
    }
    let from = from.unwrap_or(value.len());
    &value[from..to.max(from)]
}

/// Position des ersten Vorkommens in UTF-16-Einheiten, wie JavaScript `value.indexOf(needle)`;
/// `None` statt -1, wenn der Text nicht vorkommt
pub fn utf16_index_of(value: &str, needle: &str) -> Option<usize> {
    value.find(needle).map(|byte| utf16_len(&value[..byte]))
}

/// Zählt erlaubte Werte in Meldungen auf, wie JavaScript `array.join(', ')`
pub fn join_names(names: impl IntoIterator<Item = &'static str>) -> String {
    names.into_iter().collect::<Vec<_>>().join(", ")
}

/// Höchstlänge eines Kurztitels
const SLUG_MAX_LENGTH: usize = 40;

/// Baut aus einem Titel einen Kurztitel, der als Git-Branch und Ordnername taugt.
/// Portiert aus `packages/core/src/text/SlugBuilder.ts`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SlugBuilder;

impl SlugBuilder {
    /// Neuer Baumeister (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Kleinschreibung, Umlaute umschreiben, Sonderzeichen zu Bindestrichen, höchstens
    /// 40 Zeichen
    pub fn build(&self, text: &str) -> String {
        let mut dashed = String::new();
        for zeichen in text.chars().flat_map(|zeichen| zeichen.to_lowercase()) {
            match zeichen {
                'ä' => dashed.push_str("ae"),
                'ö' => dashed.push_str("oe"),
                'ü' => dashed.push_str("ue"),
                'ß' => dashed.push_str("ss"),
                _ if zeichen.is_ascii_lowercase() || zeichen.is_ascii_digit() => {
                    dashed.push(zeichen)
                }
                // Alles andere wird ein Bindestrich; mehrere in Folge zählen als einer
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
    if slug.len() <= SLUG_MAX_LENGTH {
        return slug.to_string();
    }
    let cut = &slug[..SLUG_MAX_LENGTH];
    let shortened = match cut.rfind('-') {
        Some(position) if position > 0 => &cut[..position],
        _ => cut,
    };
    shortened.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zahlt_erlaubte_werte_mit_komma_auf() {
        assert_eq!(join_names(["a", "b", "c"]), "a, b, c");
        assert_eq!(join_names(["a"]), "a");
    }

    #[test]
    fn zahlt_wie_javascript_in_utf16_einheiten() {
        assert_eq!(utf16_len(""), 0);
        assert_eq!(utf16_len("abc"), 3);
        // Umlaut: ein Zeichen, zwei Bytes, eine UTF-16-Einheit
        assert_eq!(utf16_len("ä"), 1);
        // Emoji außerhalb der BMP: ein Zeichen, vier Bytes, zwei UTF-16-Einheiten
        assert_eq!(utf16_len("🙂"), 2);
        assert_eq!(utf16_len("a🙂"), 3);
    }

    #[test]
    fn schneidet_in_utf16_einheiten_ab() {
        assert_eq!(utf16_prefix("abcdef", 3), "abc");
        assert_eq!(utf16_prefix("abc", 10), "abc");
        assert_eq!(utf16_prefix("", 3), "");
        assert_eq!(utf16_prefix("ä🙂b", 3), "ä🙂");
    }

    #[test]
    fn schneidet_ausschnitte_in_utf16_einheiten() {
        assert_eq!(utf16_slice("abcdef", 2, 3), "cde");
        assert_eq!(utf16_slice("abcdef", 4, 10), "ef");
        assert_eq!(utf16_slice("abcdef", 10, 3), "");
        assert_eq!(utf16_slice("abcdef", 0, 0), "");
        // „🙂“ belegt zwei Einheiten: das Fenster [1, 3) deckt seine zweite Hälfte und „a“ ab,
        // die Hälfte entfällt – an beiden Enden
        assert_eq!(utf16_slice("🙂ab", 1, 2), "a");
        assert_eq!(utf16_slice("a🙂b", 0, 2), "a");
        assert_eq!(utf16_slice("🙂ab", 0, 3), "🙂a");
    }

    #[test]
    fn findet_die_fundstelle_in_utf16_einheiten() {
        assert_eq!(utf16_index_of("abcdef", "cd"), Some(2));
        assert_eq!(utf16_index_of("abc", "x"), None);
        // vor der Fundstelle steht ein Emoji: zwei UTF-16-Einheiten, vier Bytes
        assert_eq!(utf16_index_of("🙂abc", "abc"), Some(2));
        assert_eq!(utf16_index_of("", ""), Some(0));
    }

    /// Kurztitel eines Titels
    fn slug(text: &str) -> String {
        SlugBuilder::new().build(text)
    }

    #[test]
    fn schreibt_klein_und_ersetzt_leerzeichen_durch_bindestriche() {
        assert_eq!(slug("Login Formular bauen"), "login-formular-bauen");
    }

    #[test]
    fn schreibt_umlaute_und_scharfes_s_um() {
        assert_eq!(
            slug("Größe Änderung für Übung"),
            "groesse-aenderung-fuer-uebung"
        );
    }

    #[test]
    fn entfernt_sonderzeichen_und_fasst_bindestriche_zusammen() {
        assert_eq!(slug("  Token: prüfen!!! (v2) "), "token-pruefen-v2");
    }

    #[test]
    fn kurzt_auf_vierzig_zeichen_am_wortende() {
        let result = slug("eine sehr lange Überschrift die weit über vierzig Zeichen hinausgeht");
        assert!(result.len() <= 40, "{result}");
        assert!(!result.ends_with('-'), "{result}");
        assert_eq!(result, "eine-sehr-lange-ueberschrift-die-weit");
    }

    #[test]
    fn liefert_ticket_bei_leerem_ergebnis() {
        assert_eq!(slug("???"), "ticket");
    }

    #[test]
    fn lasst_ein_halbes_surrogatpaar_weg() {
        // „a🙂“ sind drei UTF-16-Einheiten; bei zwei bleibt nur „a“ übrig, nie ein halbes Emoji
        assert_eq!(utf16_prefix("a🙂", 2), "a");
    }
}
