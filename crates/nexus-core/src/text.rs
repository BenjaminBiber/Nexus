//! Kleine Texthelfer für Meldungen und Längen. Wichtig: JavaScript zählt `string.length`
//! und schneidet mit `slice` in UTF-16-Einheiten, `nvarchar(n)` in SQL Server ebenso.
//! Rust zählt sonst Bytes oder Zeichen – deshalb die beiden Funktionen hier.
//!
//! Daneben die Texthelfer mit eigener Datei: Ausschnitte der Suche, Kurztitel und die
//! Eingabe aus Datei oder Standardeingabe.

pub mod search_snippet;
pub mod slug_builder;
pub mod text_source;

use std::cmp::Ordering;

pub use search_snippet::{SearchSnippet, Snippet, SnippetCut};
pub use slug_builder::SlugBuilder;
pub use text_source::TextSource;

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

/// Vergleicht zwei IDs aufsteigend; Ziffernfolgen zählen als Zahl, alles andere zeichenweise –
/// wie `localeCompare(…, { numeric: true })` in TypeScript (`ARC-0009` steht damit vor
/// `ARC-0010`)
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut left, mut right) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let a_digits = take_digits(&mut left);
                let b_digits = take_digits(&mut right);
                // Führende Nullen zählen nicht: erst die Zahl der Ziffern, dann die Ziffern
                let a_value = a_digits.trim_start_matches('0');
                let b_value = b_digits.trim_start_matches('0');
                let order = a_value
                    .len()
                    .cmp(&b_value.len())
                    .then_with(|| a_value.cmp(b_value));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                left.next();
                right.next();
                if x != y {
                    return x.cmp(&y);
                }
            }
        }
    }
}

/// Die nächste Ziffernfolge
fn take_digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(c) = chars.peek().copied() {
        if !c.is_ascii_digit() {
            break;
        }
        digits.push(c);
        chars.next();
    }
    digits
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

    #[test]
    fn lasst_ein_halbes_surrogatpaar_weg() {
        // „a🙂“ sind drei UTF-16-Einheiten; bei zwei bleibt nur „a“ übrig, nie ein halbes Emoji
        assert_eq!(utf16_prefix("a🙂", 2), "a");
    }
}
