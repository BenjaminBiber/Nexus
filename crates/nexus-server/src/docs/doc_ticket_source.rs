//! Die Quellenzeile in der Beschreibung eines Tickets: schreiben und lesen; portiert aus
//! `packages/server/src/docs/DocTicketSource.ts`.

use std::sync::OnceLock;

use nexus_core::docs::markdown_sections::split_lines;
use regex::Regex;

/// Stelle eines Dokuments, aus der ein Ticket entstanden ist
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocTicketSourceRef {
    pub path: String,
    pub heading: String,
    pub start_line: i64,
    pub end_line: i64,
}

/// Trenner zwischen Pfad und Überschrift sowie Gedankenstrich im Zeilenbereich (U+2013) –
/// beide Zeichen stehen so in den Beschreibungen
fn line_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^Quelle: (.+?) › (.+) \(Z\. (\d+)–(\d+)\)$").expect("festes Muster")
    })
}

/// Quellenzeile für die Beschreibung eines Tickets; Schreiben und Lesen hängen an diesem einen
/// Format
pub fn format_source(doc_path: &str, heading: &str, start_line: i64, end_line: i64) -> String {
    format!("Quelle: {doc_path} › {heading} (Z. {start_line}–{end_line})")
}

/// Liest die Quellenzeile aus einer Beschreibung; ohne passende Zeile `None`.
/// Gelesen wird die letzte Zeile, damit eine nachträglich angehängte Quelle gilt.
pub fn parse_source(description: &str) -> Option<DocTicketSourceRef> {
    split_lines(description).iter().rev().find_map(|line| {
        let found = line_pattern().captures(line.trim())?;
        Some(DocTicketSourceRef {
            path: found[1].to_string(),
            heading: found[2].to_string(),
            start_line: found[3].parse().ok()?,
            end_line: found[4].parse().ok()?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = "docs/superpowers/specs/2026-10-22-docs-rework-design.md";

    #[test]
    fn liest_zuruck_was_format_source_geschrieben_hat() {
        let line = format_source(SPEC, "Ziel › Teil", 3, 12);

        assert_eq!(
            parse_source(&format!("Beschreibung\n\n{line}")),
            Some(DocTicketSourceRef {
                path: SPEC.into(),
                heading: "Ziel › Teil".into(),
                start_line: 3,
                end_line: 12,
            })
        );
    }

    #[test]
    fn nimmt_die_letzte_quellenzeile_und_meldet_ohne_eine_nichts() {
        let text = format!(
            "{}\n{}",
            format_source("a.md", "Alt", 1, 2),
            format_source("b.md", "Neu", 4, 5)
        );

        assert_eq!(
            parse_source(&text).map(|found| found.heading),
            Some("Neu".into())
        );
        assert_eq!(parse_source("Nur Beschreibung"), None);
    }
}
