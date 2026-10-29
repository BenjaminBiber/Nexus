//! Fundstelle für die Trefferliste: ein kurzer Ausschnitt um den ersten Treffer, Leerraum
//! zusammengefasst, mit Bereichen aller Treffer (Groß/Klein egal) zum Hervorheben – ohne HTML.
//! Portiert aus `packages/core/src/text/SearchSnippet.ts`.
//!
//! Die Bereiche gehen in die HTTP-API und die Oberfläche schneidet damit den Text in
//! JavaScript: deshalb rechnet alles hier in UTF-16-Einheiten (siehe [`crate::text`]).

use crate::text::utf16_len;

/// Auslassungszeichen am Anfang bzw. Ende eines geschnittenen Ausschnitts
const ELLIPSIS: &str = "…";
/// Zeichen vor der ersten Fundstelle, wenn geschnitten werden muss
const LEAD: usize = 40;
/// Standardlänge eines Ausschnitts
const DEFAULT_SIZE: usize = 160;

/// Ein Ausschnitt samt der Bereiche, die hervorgehoben werden
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub text: String,
    /// Bereiche [Anfang, Ende) der Treffer im Text, in UTF-16-Einheiten
    pub matches: Vec<(u32, u32)>,
}

/// Schnitt eines Feldes, das das Repository schon gekürzt hat (Ausschnitt aus der Mitte)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnippetCut {
    pub cut_before: bool,
    pub cut_after: bool,
}

/// Baut die Ausschnitte der Trefferliste.
#[derive(Debug, Clone, Copy)]
pub struct SearchSnippet {
    size: usize,
}

impl Default for SearchSnippet {
    fn default() -> Self {
        Self::new(DEFAULT_SIZE)
    }
}

impl SearchSnippet {
    /// Ausschnitte dieser Länge
    pub fn new(size: usize) -> Self {
        Self { size }
    }

    /// Ausschnitt um die erste Fundstelle eines der Begriffe
    pub fn build<S: AsRef<str>>(&self, raw: &str, terms: &[S], cut: &SnippetCut) -> Snippet {
        let flat: Vec<u16> = collapse_whitespace(raw).encode_utf16().collect();
        let lower = from_utf16(&flat).to_lowercase();
        // Nur wenn Kleinschreibung die Länge nicht ändert, stimmen Positionen im Original
        let needles: Vec<Vec<u16>> = if utf16_len(&lower) == flat.len() {
            terms
                .iter()
                .map(|term| term.as_ref().to_lowercase())
                .filter(|term| !term.is_empty())
                .map(|term| term.encode_utf16().collect())
                .collect()
        } else {
            Vec::new()
        };
        let lower: Vec<u16> = lower.encode_utf16().collect();
        let first = needles
            .iter()
            .filter_map(|needle| index_of(&lower, needle, 0))
            .min();

        let mut start = 0;
        if let Some(first) = first {
            if first + LEAD > self.size {
                start = first.saturating_sub(LEAD);
                if let Some(space) = last_space(&flat, start) {
                    if start - space < 15 {
                        start = space + 1;
                    }
                }
            }
        }
        let mut end = flat.len().min(start + self.size);
        if end < flat.len() {
            if let Some(space) = last_space(&flat, end) {
                if space > start + self.size / 2 {
                    end = space;
                }
            }
        }

        let before = start > 0 || cut.cut_before;
        let after = end < flat.len() || cut.cut_after;
        let body = from_utf16(&flat[start..end]);
        let offset = if before { utf16_len(ELLIPSIS) } else { 0 };
        Snippet {
            text: format!(
                "{}{body}{}",
                if before { ELLIPSIS } else { "" },
                if after { ELLIPSIS } else { "" }
            ),
            matches: ranges(
                &body.to_lowercase().encode_utf16().collect::<Vec<u16>>(),
                &needles,
                offset,
            ),
        }
    }
}

/// Alle Vorkommen aller Begriffe, sortiert und zusammengefasst
fn ranges(lower: &[u16], needles: &[Vec<u16>], offset: usize) -> Vec<(u32, u32)> {
    let mut found: Vec<(usize, usize)> = Vec::new();
    for needle in needles {
        let mut from = 0;
        while let Some(at) = index_of(lower, needle, from) {
            found.push((at, at + needle.len()));
            from = at + 1;
        }
    }
    found.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for range in found {
        match merged.last_mut() {
            Some(last) if range.0 <= last.1 => last.1 = last.1.max(range.1),
            _ => merged.push(range),
        }
    }
    merged
        .into_iter()
        .map(|(from, to)| ((from + offset) as u32, (to + offset) as u32))
        .collect()
}

/// Fasst jede Folge von Leerraum zu einem Leerzeichen zusammen und schneidet die Ränder ab
/// (`raw.replace(/\s+/g, ' ').trim()` in JavaScript)
fn collapse_whitespace(raw: &str) -> String {
    let mut flat = String::with_capacity(raw.len());
    let mut space = false;
    for zeichen in raw.chars() {
        if zeichen.is_whitespace() {
            space = true;
            continue;
        }
        if space && !flat.is_empty() {
            flat.push(' ');
        }
        space = false;
        flat.push(zeichen);
    }
    flat
}

/// Position des ersten Vorkommens ab `from`, in UTF-16-Einheiten
fn index_of(haystack: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|at| &haystack[*at..at + needle.len()] == needle)
}

/// Letztes Leerzeichen an oder vor `from` (`lastIndexOf(' ', from)` in JavaScript)
fn last_space(haystack: &[u16], from: usize) -> Option<usize> {
    let bis = from.min(haystack.len().saturating_sub(1));
    haystack[..=bis]
        .iter()
        .rposition(|unit| *unit == u16::from(b' '))
}

/// UTF-16-Einheiten als Text; ein halbes Ersatzpaar am Rand entfällt
fn from_utf16(units: &[u16]) -> String {
    String::from_utf16_lossy(units).replace('\u{FFFD}', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Markierte Teile eines Ausschnitts
    fn marked(snippet: &Snippet) -> Vec<String> {
        let text: Vec<u16> = snippet.text.encode_utf16().collect();
        snippet
            .matches
            .iter()
            .map(|(from, to)| from_utf16(&text[*from as usize..*to as usize]))
            .collect()
    }

    /// Ausschnitte von 60 Einheiten wie in TypeScript
    fn snippets() -> SearchSnippet {
        SearchSnippet::new(60)
    }

    #[test]
    fn markiert_treffer_unabhangig_von_gross_klein_und_mit_umlauten() {
        let result = snippets().build(
            "Die Übersicht zeigt übersichtlich alles",
            &["übersicht"],
            &SnippetCut::default(),
        );
        assert_eq!(result.text, "Die Übersicht zeigt übersichtlich alles");
        assert_eq!(marked(&result), ["Übersicht", "übersicht"]);
    }

    #[test]
    fn schneidet_um_die_erste_fundstelle_mit_auslassungspunkten() {
        let text = format!("{}Treffer hier {}", "a ".repeat(100), "b ".repeat(100));
        let result = snippets().build(&text, &["treffer"], &SnippetCut::default());
        assert!(result.text.starts_with('…'));
        assert!(result.text.ends_with('…'));
        assert!(utf16_len(&result.text) <= 62);
        assert_eq!(marked(&result), ["Treffer"]);
    }

    #[test]
    fn markiert_mehrere_begriffe_und_fasst_uberschneidungen_zusammen() {
        let result = snippets().build(
            "Board-Suche im Board",
            &["board", "oard-s"],
            &SnippetCut::default(),
        );
        assert_eq!(marked(&result), ["Board-S", "Board"]);
    }

    #[test]
    fn setzt_auslassungspunkte_bei_einem_ausschnitt_aus_der_mitte() {
        let result = snippets().build(
            "mitten Treffer drin",
            &["treffer"],
            &SnippetCut {
                cut_before: true,
                cut_after: true,
            },
        );
        assert_eq!(result.text, "…mitten Treffer drin…");
        assert_eq!(marked(&result), ["Treffer"]);
    }

    #[test]
    fn fasst_leerraum_zusammen_und_liefert_ohne_treffer_den_anfang() {
        let result = snippets().build("Zeile 1\n\n   Zeile 2", &["fehlt"], &SnippetCut::default());
        assert_eq!(
            result,
            Snippet {
                text: "Zeile 1 Zeile 2".into(),
                matches: Vec::new(),
            }
        );
    }

    #[test]
    fn zahlt_die_bereiche_in_utf16_einheiten() {
        // Das Emoji belegt zwei Einheiten: der Treffer beginnt danach bei 3
        let result =
            SearchSnippet::default().build("🙂 Treffer", &["treffer"], &SnippetCut::default());
        assert_eq!(result.text, "🙂 Treffer");
        assert_eq!(result.matches, [(3, 10)]);
    }
}
