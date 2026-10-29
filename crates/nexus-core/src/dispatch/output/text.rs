//! Kürzen von Texten wie in JavaScript (`shorten`, `clip`, `one` der TypeScript-Klassen).
//!
//! JavaScript zählt UTF-16-Einheiten, Rust zählt hier Zeichen (`chars()`). Für die Ausgaben
//! von Claude Code – ASCII bis Latin-1 – ist das dasselbe; erst Zeichen außerhalb der
//! Basic Multilingual Plane (z. B. Emoji) würden abweichen. Die Grenzen dienen nur der
//! Lesbarkeit, deshalb ist der Unterschied gewollt.

use std::sync::LazyLock;

use regex::Regex;

/// Leerraum, der zu einem Leerzeichen zusammengezogen wird (`/\s+/g`)
static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("festes Muster"));

/// Eine Zeile, Leerraum zusammengezogen, höchstens `max` Zeichen
pub fn one_line(text: &str, max: usize) -> String {
    let one = WHITESPACE.replace_all(text, " ");
    let one = one.trim();
    if one.chars().count() > max {
        format!("{}…", take_chars(one, max.saturating_sub(1)))
    } else {
        one.to_string()
    }
}

/// Mehrzeilig, höchstens `max` Zeichen; `tail` behält das Ende
pub fn clip(text: &str, max: usize, tail: bool) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    if tail {
        format!("…{}", last_chars(text, max.saturating_sub(1)))
    } else {
        format!("{}…", take_chars(text, max.saturating_sub(1)))
    }
}

/// Die ersten `count` Zeichen
pub fn take_chars(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}

/// Die letzten `count` Zeichen
pub fn last_chars(text: &str, count: usize) -> String {
    let total = text.chars().count();
    text.chars().skip(total.saturating_sub(count)).collect()
}

/// Erste Zeile eines Texts, auf `max` Zeichen gekürzt
pub fn first_line(text: &str, max: usize) -> String {
    let line = text
        .trim()
        .split('\n')
        .next()
        .unwrap_or("")
        .trim_end_matches('\r');
    if line.chars().count() > max {
        format!("{}…", take_chars(line, max))
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zieht_leerraum_zusammen_und_kurzt_mit_auslassung() {
        assert_eq!(one_line(" a\n  b ", 10), "a b");
        assert_eq!(one_line("abcdef", 3), "ab…");
        assert_eq!(one_line("äöüß", 3), "äö…");
    }

    #[test]
    fn clip_behalt_anfang_oder_ende() {
        assert_eq!(clip("abcdef", 10, false), "abcdef");
        assert_eq!(clip("abcdef", 3, false), "ab…");
        assert_eq!(clip("abcdef", 3, true), "…ef");
    }

    #[test]
    fn erste_zeile_ohne_wagenrucklauf() {
        assert_eq!(first_line("eins\r\nzwei", 100), "eins");
        assert_eq!(first_line("langezeile", 4), "lang…");
    }
}
