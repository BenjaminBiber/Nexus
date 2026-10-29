//! Vorlagen-Engine für die Dispatcher-Prompts; portiert aus
//! `packages/core/src/dispatch/PromptTemplate.ts`.
//!
//! Sie ersetzt `{{name}}`, meldet unbekannte Platzhalter und lässt Zeilen wegfallen, die nur aus
//! einem leeren Block-Platzhalter bestehen. Zeilenenden bleiben je Zeile erhalten.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::{Captures, Regex};

/// Platzhalter-Form: `{{name}}`, keine Leerzeichen innerhalb der Klammern.
static PLACEHOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{([A-Za-z][A-Za-z0-9_]*)\}\}").unwrap());

/// Zeile, deren einziger Inhalt ein Platzhalter ist (Block-Zeile).
static BLOCK_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)\{\{([A-Za-z][A-Za-z0-9_]*)\}\}(\s*)$").unwrap());

/// Wert eines Platzhalters: ein Text oder eine Liste, die mit Zeilenumbruch verbunden wird
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptValue {
    Text(String),
    List(Vec<String>),
}

impl PromptValue {
    /// Der eingesetzte Text; eine leere Liste zählt wie ein leerer Text
    fn text(&self) -> String {
        match self {
            PromptValue::Text(text) => text.clone(),
            PromptValue::List(items) => items.join("\n"),
        }
    }
}

impl From<&str> for PromptValue {
    fn from(text: &str) -> Self {
        PromptValue::Text(text.to_string())
    }
}

impl From<String> for PromptValue {
    fn from(text: String) -> Self {
        PromptValue::Text(text)
    }
}

impl From<Vec<String>> for PromptValue {
    fn from(items: Vec<String>) -> Self {
        PromptValue::List(items)
    }
}

/// Werte einer Vorlage, nach Platzhaltername
pub type PromptValues = HashMap<String, PromptValue>;

/// Ersetzt `{{name}}` in einer Vorlage.
#[derive(Debug, Clone, Copy, Default)]
pub struct PromptTemplate;

impl PromptTemplate {
    /// Neue Engine (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Platzhalternamen der Vorlage in Reihenfolge des ersten Vorkommens, ohne Dubletten
    pub fn names(&self, template: &str) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        for capture in PLACEHOLDER.captures_iter(template) {
            let name = capture[1].to_string();
            if !found.contains(&name) {
                found.push(name);
            }
        }
        found
    }

    /// Setzt die Werte ein; unbekannte Platzhalter führen zu einem Fehler, der alle fehlenden nennt
    pub fn render(&self, template: &str, values: &PromptValues) -> Result<String, String> {
        let unknown: Vec<String> = self
            .names(template)
            .into_iter()
            .filter(|name| !values.contains_key(name))
            .collect();
        if !unknown.is_empty() {
            let names: Vec<String> = unknown
                .iter()
                .map(|name| format!("{{{{{name}}}}}"))
                .collect();
            return Err(format!(
                "Unbekannte Platzhalter in der Vorlage: {}",
                names.join(", ")
            ));
        }

        let mut out = String::new();
        // Jede Zeile mit ihrem ursprünglichen Zeilenende, damit \r\n erhalten bleibt.
        for (line, ending) in lines_with_endings(template) {
            if let Some(block) = BLOCK_LINE.captures(line) {
                if value_of(values, &block[2]).is_empty() {
                    continue;
                }
            }
            out.push_str(
                &PLACEHOLDER.replace_all(line, |capture: &Captures| value_of(values, &capture[1])),
            );
            out.push_str(ending);
        }
        Ok(out)
    }
}

/// Der Wert des Platzhalters; fehlt er, bleibt er leer (render prüft das vorher)
fn value_of(values: &PromptValues, name: &str) -> String {
    values.get(name).map(PromptValue::text).unwrap_or_default()
}

/// Zerlegt den Text in Paare aus Zeileninhalt und zugehörigem Zeilenende (`\r\n`, `\n`, `\r`
/// oder leer am Dateiende) – die Entsprechung des Regex-Splits in TypeScript
fn lines_with_endings(text: &str) -> Vec<(&str, &str)> {
    let mut parts = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find(['\r', '\n']) {
            None => {
                parts.push((rest, ""));
                rest = "";
            }
            Some(at) => {
                let ending_len = if rest[at..].starts_with("\r\n") { 2 } else { 1 };
                parts.push((&rest[..at], &rest[at..at + ending_len]));
                rest = &rest[at + ending_len..];
            }
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Werte aus Paaren, damit die Tests so knapp bleiben wie in TypeScript
    fn values(pairs: &[(&str, PromptValue)]) -> PromptValues {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), value.clone()))
            .collect()
    }

    fn text(value: &str) -> PromptValue {
        PromptValue::Text(value.to_string())
    }

    fn list(items: &[&str]) -> PromptValue {
        PromptValue::List(items.iter().map(|item| (*item).to_string()).collect())
    }

    #[test]
    fn ersetzt_einfache_platzhalter_auch_mehrfach_vorkommende() {
        let out = PromptTemplate::new()
            .render(
                "Ticket {{ticketId}} an {{agent}} ({{ticketId}})",
                &values(&[("ticketId", text("AIH-0086")), ("agent", text("junior"))]),
            )
            .unwrap();
        assert_eq!(out, "Ticket AIH-0086 an junior (AIH-0086)");
    }

    #[test]
    fn setzt_mehrzeilige_werte_und_listen_unverandert_ein() {
        let t = PromptTemplate::new();
        let out = t
            .render(
                "vorher\n{{answers}}\nnachher",
                &values(&[("answers", text("A\nB"))]),
            )
            .unwrap();
        assert_eq!(out, "vorher\nA\nB\nnachher");
        let arr = t
            .render(
                "vorher\n{{answers}}\nnachher",
                &values(&[("answers", list(&["A", "B"]))]),
            )
            .unwrap();
        assert_eq!(arr, "vorher\nA\nB\nnachher");
    }

    #[test]
    fn meldet_unbekannte_platzhalter_mit_namen_in_einer_meldung() {
        let error = PromptTemplate::new()
            .render(
                "{{foo}} und {{bar}} und {{agent}}",
                &values(&[("agent", text("x"))]),
            )
            .unwrap_err();
        assert!(error.contains("{{foo}}, {{bar}}"), "{error}");
    }

    #[test]
    fn lasst_eine_zeile_mit_nur_einem_leeren_block_platzhalter_ganz_wegfallen() {
        let t = PromptTemplate::new();
        assert_eq!(
            t.render("eins\n{{answers}}\nzwei", &values(&[("answers", text(""))]))
                .unwrap(),
            "eins\nzwei"
        );
        assert_eq!(
            t.render(
                "eins\n{{answers}}\nzwei",
                &values(&[("answers", list(&[]))])
            )
            .unwrap(),
            "eins\nzwei"
        );
    }

    #[test]
    fn entfernt_die_zeile_nicht_wenn_sie_neben_dem_leeren_platzhalter_text_enthalt() {
        assert_eq!(
            PromptTemplate::new()
                .render(
                    "eins\nText {{answers}}\nzwei",
                    &values(&[("answers", text(""))])
                )
                .unwrap(),
            "eins\nText \nzwei"
        );
    }

    #[test]
    fn behalt_die_einruckung_einer_nicht_leeren_block_zeile() {
        assert_eq!(
            PromptTemplate::new()
                .render(
                    "eins\n  {{answers}}\nzwei",
                    &values(&[("answers", text("A"))])
                )
                .unwrap(),
            "eins\n  A\nzwei"
        );
    }

    #[test]
    fn lasst_text_ohne_platzhalter_und_fehlerhafte_klammern_unverandert() {
        assert_eq!(
            PromptTemplate::new()
                .render("nur {text} und {{offen", &values(&[]))
                .unwrap(),
            "nur {text} und {{offen"
        );
    }

    #[test]
    fn kommt_mit_crlf_zeilenenden_zurecht() {
        assert_eq!(
            PromptTemplate::new()
                .render(
                    "eins\r\n{{answers}}\r\nzwei",
                    &values(&[("answers", text(""))])
                )
                .unwrap(),
            "eins\r\nzwei"
        );
    }

    #[test]
    fn namen_kommen_in_reihenfolge_ohne_dubletten() {
        assert_eq!(PromptTemplate::new().names("{{b}} {{a}} {{b}}"), ["b", "a"]);
    }

    #[test]
    fn namen_sind_leer_ohne_platzhalter() {
        assert!(PromptTemplate::new()
            .names("kein Platzhalter {hier}")
            .is_empty());
    }
}
