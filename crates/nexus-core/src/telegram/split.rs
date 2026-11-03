//! Teilt eine Telegram-HTML-Nachricht, die die Höchstlänge überschreitet, in mehrere
//! Nachrichten. Getrennt wird nur an Zeilenumbrüchen, weil die erzeugten Texte je Zeile
//! höchstens ein einfaches Tag nutzen – ein Schnitt mitten in einer Zeile könnte sonst ein
//! Tag zerreißen. Nur eine für sich allein zu lange Zeile wird ohne Tags hart geschnitten.
//!
//! Die Länge zählt in UTF-16-Einheiten (siehe [`crate::text`]), weil Telegrams Grenze von
//! 4096 Zeichen dort genauso gezählt wird wie in JavaScript.

use crate::text::{utf16_len, utf16_prefix};

/// Vorgabe für die Höchstlänge einer Telegram-Nachricht (UTF-16-Einheiten)
pub const MAX_MESSAGE_LEN: usize = 4096;

/// Teilt `html` in Stücke von höchstens `max` UTF-16-Einheiten
pub fn split_message(html: &str, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in html.split('\n') {
        if utf16_len(line) > max {
            lines.extend(hard_split(&strip_tags(line), max));
        } else {
            lines.push(line.to_string());
        }
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in lines {
        let zusaetzlich = if current.is_empty() { 0 } else { 1 };
        if !current.is_empty() && utf16_len(&current) + zusaetzlich + utf16_len(&line) > max {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(&line);
    }
    chunks.push(current);
    chunks
}

/// Schneidet `text` hart in Stücke von höchstens `max` UTF-16-Einheiten, ohne auf Wörter
/// oder Tags zu achten – nur für eine bereits von Tags befreite, überlange Zeile gedacht
fn hard_split(text: &str, max: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while utf16_len(rest) > max {
        let piece = utf16_prefix(rest, max);
        let used = piece.len();
        pieces.push(piece.to_string());
        rest = &rest[used..];
    }
    pieces.push(rest.to_string());
    pieces
}

/// Entfernt alle `<…>`-Tags; der restliche Text (samt Entitäten wie `&amp;`) bleibt
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for zeichen in html.chars() {
        match zeichen {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(zeichen),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laesst_kurze_texte_unangetastet() {
        assert_eq!(split_message("kurzer Text", 4096), vec!["kurzer Text"]);
        assert_eq!(split_message("", 4096), vec![""]);
    }

    #[test]
    fn teilt_an_zeilengrenzen_ohne_eine_zeile_zu_zerreissen() {
        let html = "<b>Zeile 1</b>\n<b>Zeile 2</b>\n<b>Zeile 3</b>";
        let teile = split_message(html, 16);

        assert_eq!(
            teile,
            vec!["<b>Zeile 1</b>", "<b>Zeile 2</b>", "<b>Zeile 3</b>"]
        );
        for teil in &teile {
            assert!(utf16_len(teil) <= 16, "{teil}");
        }
    }

    #[test]
    fn fasst_mehrere_kurze_zeilen_in_ein_stueck() {
        let html = "a\nb\nc";
        assert_eq!(split_message(html, 16), vec!["a\nb\nc"]);
    }

    #[test]
    fn schneidet_eine_einzelne_ueberlange_zeile_hart_und_ohne_tags() {
        let html = "<b>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa</b>";
        let teile = split_message(html, 10);

        // Keine Tags mehr, da die Zeile allein schon zu lang war
        for teil in &teile {
            assert!(!teil.contains('<') && !teil.contains('>'), "{teil}");
            assert!(utf16_len(teil) <= 10, "{teil}");
        }
        assert_eq!(teile.concat(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    }

    #[test]
    fn teilt_eine_ueberlange_zeile_mitten_in_sonst_normalen_zeilen() {
        let lang = "x".repeat(25);
        let html = format!("kurz\n<b>{lang}</b>\nkurz");
        let teile = split_message(&html, 10);

        for teil in &teile {
            assert!(utf16_len(teil) <= 10, "{teil}");
        }
        // Die überlange Zeile verliert ihre Tags, die kurzen Zeilen bleiben unverändert
        assert!(teile.iter().any(|teil| teil == "kurz"));
        assert!(!teile.iter().any(|teil| teil.contains('<')));
    }

    #[test]
    fn zaehlt_in_utf16_einheiten_nicht_in_bytes() {
        // „ä“ ist eine UTF-16-Einheit, aber zwei Byte
        let html = "ä".repeat(20);
        let teile = split_message(&html, 10);

        assert_eq!(teile.len(), 2);
        for teil in &teile {
            assert_eq!(utf16_len(teil), 10);
        }
    }
}
