//! Zerlegt den Inhalt einer JSONL-Datei in vollständige Zeilen mit ihrem Byte-Offset.
//! Eine letzte Zeile ohne Zeilenumbruch schreibt Claude Code gerade noch und gilt als nicht da.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde_json::Value;

/// Eine vollständige Zeile: Beginn in der Datei, Ende hinter dem Zeilenumbruch, Inhalt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line<'a> {
    pub start: u64,
    pub end: u64,
    pub text: &'a str,
}

/// Die vollständigen Zeilen von `bytes`, das ab Offset `base` der Datei gelesen wurde.
/// Leere Zeilen entfallen, `\r` am Ende wird abgeschnitten, ungültiges UTF-8 ergibt eine
/// leere Zeile mit `text == "\u{FFFD}"`, damit sie als kaputt gezählt wird.
pub fn lines(bytes: &[u8], base: u64) -> Vec<Line<'_>> {
    let mut list = Vec::new();
    let mut start = 0usize;
    while let Some(found) = bytes[start..].iter().position(|byte| *byte == b'\n') {
        let end = start + found;
        let raw = &bytes[start..end];
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        let text = std::str::from_utf8(raw).unwrap_or("\u{FFFD}");
        let text = text.trim_start_matches('\u{FEFF}');
        if !text.trim().is_empty() {
            list.push(Line {
                start: base + start as u64,
                end: base + end as u64 + 1,
                text,
            });
        }
        start = end + 1;
    }
    list
}

/// Offset hinter der letzten vollständigen Zeile
pub fn complete_end(bytes: &[u8], base: u64) -> u64 {
    match bytes.iter().rposition(|byte| *byte == b'\n') {
        Some(index) => base + index as u64 + 1,
        None => base,
    }
}

/// Liest die Datei ab `offset` bis zum Ende (ein Offset hinter dem Ende liest nichts)
pub fn read_from(path: &Path, offset: u64) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if offset >= len {
        return Ok(Vec::new());
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::with_capacity((len - offset) as usize);
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Ein Textfeld eines JSON-Objekts, leere Texte zählen nicht
pub fn text<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
}

/// Ein Wahrheitswert eines JSON-Objekts, fehlend = falsch
pub fn flag(value: &Value, name: &str) -> bool {
    value.get(name).and_then(Value::as_bool).unwrap_or(false)
}

/// Eine Zahl eines JSON-Objekts als `i64` (auch aus Fließkommazahlen)
pub fn number(value: &Value, name: &str) -> Option<i64> {
    let field = value.get(name)?;
    field
        .as_i64()
        .or_else(|| field.as_f64().map(|number| number.round() as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liefert_nur_vollstaendige_zeilen_mit_offset() {
        let bytes = b"{\"a\":1}\r\n\n{\"b\":2}\n{\"halb";
        let found = lines(bytes, 10);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].text, "{\"a\":1}");
        assert_eq!((found[0].start, found[0].end), (10, 19));
        assert_eq!(found[1].text, "{\"b\":2}");
        assert_eq!((found[1].start, found[1].end), (20, 28));
        assert_eq!(complete_end(bytes, 10), 28);
    }

    #[test]
    fn ohne_zeilenumbruch_ist_nichts_vollstaendig() {
        assert!(lines(b"{\"a\":1}", 0).is_empty());
        assert_eq!(complete_end(b"{\"a\":1}", 5), 5);
    }
}
