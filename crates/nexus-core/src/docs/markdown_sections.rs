//! Zerlegt ein Markdown-Dokument in Abschnitte und ersetzt Zeilenbereiche, ohne den Rest
//! anzurühren; portiert aus `packages/core/src/docs/MarkdownSections.ts`.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{HarnessError, Result};

/// Ein Abschnitt eines Markdown-Dokuments. Zeilennummern sind 1-basiert und einschließlich,
/// passend zu `lines` in [`crate::docs::DocComment`] – so lassen sich Kommentare und
/// Abschnitte vergleichen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownSection {
    pub heading: String,
    pub level: usize,
    /// Zeile der Überschrift
    pub start_line: usize,
    /// Letzte Zeile vor der nächsten Überschrift gleicher oder höherer Ebene, sonst das Dateiende
    pub end_line: usize,
}

/// Überschrift einer Ebene 1–6
fn heading_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^(#{1,6})\s+(.*)$").expect("festes Muster"))
}

/// Beginn oder Ende eines Code-Zauns
fn fence_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^(`{3,}|~{3,})").expect("festes Muster"))
}

/// Zerlegt ein Markdown-Dokument in Abschnitte und ersetzt Zeilenbereiche.
#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownSections;

impl MarkdownSections {
    /// Neue Zerlegung (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Alle Überschriften mit ihrem Zeilenbereich; Text vor der ersten Überschrift wird kein
    /// Abschnitt
    pub fn parse(&self, text: &str) -> Vec<MarkdownSection> {
        let lines = split_lines(text);
        let mut sections: Vec<MarkdownSection> = Vec::new();
        let mut fence: Option<String> = None;
        for (index, line) in lines.iter().enumerate() {
            if let Some(marker) = fence_pattern()
                .captures(line.trim())
                .and_then(|found| found.get(1))
                .map(|found| found.as_str().to_string())
            {
                // Ein Zaun endet nur durch denselben Zeichentyp in mindestens derselben Länge
                // – sonst schließt ``` einen ~~~-Block
                match &fence {
                    None => fence = Some(marker),
                    Some(open) => {
                        if marker.as_bytes().first() == open.as_bytes().first()
                            && marker.len() >= open.len()
                        {
                            fence = None;
                        }
                    }
                }
                continue;
            }
            if fence.is_some() {
                continue;
            }
            let Some(found) = heading_pattern().captures(line) else {
                continue;
            };
            let level = found[1].len();
            // Alle offenen Abschnitte gleicher oder tieferer Ebene enden in der Zeile davor
            for open in sections.iter_mut() {
                if open.end_line == 0 && open.level >= level {
                    open.end_line = index;
                }
            }
            sections.push(MarkdownSection {
                heading: found[2].trim().to_string(),
                level,
                start_line: index + 1,
                end_line: 0,
            });
        }
        for open in sections.iter_mut() {
            if open.end_line == 0 {
                open.end_line = lines.len();
            }
        }
        sections.sort_by_key(|section| section.start_line);
        sections
    }

    /// Der erste Abschnitt mit genau dieser Überschrift, sonst `None`
    pub fn find(&self, text: &str, heading: &str) -> Option<MarkdownSection> {
        let wanted = heading.trim();
        self.parse(text)
            .into_iter()
            .find(|section| section.heading == wanted)
    }

    /// Ersetzt den 1-basierten, einschließenden Zeilenbereich; Zeilenende-Stil und
    /// abschließender Umbruch bleiben erhalten
    pub fn replace_lines(
        &self,
        text: &str,
        start_line: i64,
        end_line: i64,
        replacement: &str,
    ) -> Result<String> {
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut lines = split_lines(text);
        let count = lines.len() as i64;
        if start_line < 1 || end_line < start_line || end_line > count {
            return Err(HarnessError::new(
                "doc-range-invalid",
                format!(
                    "Zeilenbereich {start_line}–{end_line} liegt nicht im Dokument (1–{count}). \
                     Lies das Dokument neu und nimm die Zeilen daraus."
                ),
            )
            .into());
        }
        let from = (start_line - 1) as usize;
        let to = end_line as usize;
        let replaced = split_lines(replacement);
        lines.splice(from..to, replaced);
        Ok(lines.join(eol))
    }
}

/// Zeilen eines Textes wie `text.split(/\r?\n/)` in JavaScript
pub fn split_lines(text: &str) -> Vec<String> {
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dasselbe Dokument wie in `MarkdownSections.test.ts`
    const DOC: &str = concat!(
        "Vorspann ohne Überschrift\n",
        "# Titel\n",
        "eins\n",
        "## Ziel\n",
        "zwei\n",
        "### Teilziel\n",
        "drei\n",
        "## Schritte\n",
        "vier"
    );

    /// Ein Abschnitt als Tupel, damit sich Erwartungen kurz schreiben lassen
    fn tuples(sections: &[MarkdownSection]) -> Vec<(&str, usize, usize, usize)> {
        sections
            .iter()
            .map(|section| {
                (
                    section.heading.as_str(),
                    section.level,
                    section.start_line,
                    section.end_line,
                )
            })
            .collect()
    }

    #[test]
    fn erkennt_ebenen_und_grenzen() {
        let sections = MarkdownSections::new().parse(DOC);
        assert_eq!(
            tuples(&sections),
            [
                ("Titel", 1, 2, 9),
                ("Ziel", 2, 4, 7),
                ("Teilziel", 3, 6, 7),
                ("Schritte", 2, 8, 9),
            ]
        );
    }

    #[test]
    fn lasst_die_letzte_section_bis_zum_dateiende_laufen() {
        let sections = MarkdownSections::new().parse("# A\neins\nzwei\n");
        assert_eq!(tuples(&sections), [("A", 1, 1, 4)]);
    }

    #[test]
    fn ubergeht_uberschriften_in_code_zaunen() {
        let text = [
            "# Echt",
            "```",
            "# Unecht",
            "```",
            "~~~",
            "```",
            "## Auch unecht",
            "~~~",
            "## Wieder echt",
        ]
        .join("\n");
        let headings: Vec<String> = MarkdownSections::new()
            .parse(&text)
            .into_iter()
            .map(|section| section.heading)
            .collect();
        assert_eq!(headings, ["Echt", "Wieder echt"]);
    }

    #[test]
    fn findet_eine_section_uber_ihre_uberschrift() {
        let sections = MarkdownSections::new();
        assert_eq!(sections.find(DOC, "Ziel").map(|s| s.start_line), Some(4));
        assert_eq!(sections.find(DOC, "Fehlt"), None);
    }

    #[test]
    fn ersetzt_genau_den_zeilenbereich() {
        let out = MarkdownSections::new()
            .replace_lines(DOC, 4, 7, "## Ziel\nneu\nnoch neu")
            .unwrap();
        assert_eq!(
            out.split('\n').collect::<Vec<_>>(),
            [
                "Vorspann ohne Überschrift",
                "# Titel",
                "eins",
                "## Ziel",
                "neu",
                "noch neu",
                "## Schritte",
                "vier",
            ]
        );
    }

    #[test]
    fn behalt_crlf_und_den_abschliessenden_umbruch() {
        let crlf = "# A\r\neins\r\n";
        assert_eq!(
            MarkdownSections::new()
                .replace_lines(crlf, 2, 2, "zwei")
                .unwrap(),
            "# A\r\nzwei\r\n"
        );
    }

    #[test]
    fn wirft_bei_ungultigem_bereich() {
        let sections = MarkdownSections::new();
        for (start, end) in [(0, 2), (3, 2), (1, 99)] {
            let error = sections.replace_lines(DOC, start, end, "x").unwrap_err();
            assert_eq!(error.code(), Some("doc-range-invalid"));
        }
        assert_eq!(
            sections
                .replace_lines(DOC, 1, 99, "x")
                .unwrap_err()
                .to_string(),
            "Zeilenbereich 1–99 liegt nicht im Dokument (1–9). Lies das Dokument neu und nimm die Zeilen daraus."
        );
    }
}
