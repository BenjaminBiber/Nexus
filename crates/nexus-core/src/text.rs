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

/// Zählt erlaubte Werte in Meldungen auf, wie JavaScript `array.join(', ')`
pub fn join_names(names: impl IntoIterator<Item = &'static str>) -> String {
    names.into_iter().collect::<Vec<_>>().join(", ")
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
    fn lasst_ein_halbes_surrogatpaar_weg() {
        // „a🙂“ sind drei UTF-16-Einheiten; bei zwei bleibt nur „a“ übrig, nie ein halbes Emoji
        assert_eq!(utf16_prefix("a🙂", 2), "a");
    }
}
