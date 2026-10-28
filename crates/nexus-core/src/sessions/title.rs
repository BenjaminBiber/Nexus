//! Sitzungsnamen „SE-001: Titel“ bilden und aus einem Prompt einen kurzen Titel ableiten.

use std::sync::LazyLock;

use regex::Regex;

use crate::text::{utf16_len, utf16_prefix};

/// Ein Titel, der schon mit „SE-<Nummer>“ beginnt, wird nicht erneut nummeriert
static PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^SE-\d+").unwrap());

/// Geschlossene Blöcke, die nicht vom Nutzer stammen, sondern von Claude Code oder der IDE
const INJECTED_NAMES: [&str; 2] = ["system-reminder", "browser_instruction"];

/// Dieselben Blöcke mit wechselndem Namensteil: `ide_…`, `command-…`, `local-command-…`
const INJECTED_PREFIXES: [&str; 3] = ["ide_", "command-", "local-command-"];

/// Wie viele Wörter und wie viele UTF-16-Einheiten ein Titel aus dem Prompt höchstens hat
const MAX_WORDS: usize = 6;
const MAX_LENGTH: usize = 50;

/// Bildet den Sitzungsnamen „SE-001: Titel“ (nur „SE-001“, wenn es noch keinen Titel gibt).
pub struct SessionTitle;

impl SessionTitle {
    /// Setzt die Nummer (dreistellig) vor den Titel; ein bereits nummerierter Titel bleibt unverändert
    pub fn format(number: i64, title: Option<&str>) -> String {
        let text = title.unwrap_or_default().trim();
        if PREFIX.is_match(text) {
            return text.to_string();
        }
        let label = format!("SE-{number:03}");
        if text.is_empty() {
            label
        } else {
            format!("{label}: {text}")
        }
    }

    /// Kurzer Titel aus dem Prompt: erste Zeile, die ersten Wörter, ohne Zeilenumbrüche;
    /// leer, wenn nach den eingeschleusten Blöcken nichts übrig bleibt
    pub fn from_prompt(prompt: Option<&str>) -> String {
        let ohne_blocke = strip_injected(prompt.unwrap_or_default());
        let erste_zeile = ohne_blocke
            .lines()
            .find(|zeile| !zeile.trim().is_empty())
            .unwrap_or_default();
        let worter: Vec<&str> = erste_zeile.split_whitespace().collect();
        let text = worter
            .iter()
            .take(MAX_WORDS)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        // JavaScript schneidet mit `slice` in UTF-16-Einheiten, deshalb hier dieselbe Zählung
        let gekurzt = if utf16_len(&text) > MAX_LENGTH {
            utf16_prefix(&text, MAX_LENGTH).trim_end()
        } else {
            text.as_str()
        };
        let gekurzt_wurde = worter.len() > MAX_WORDS || utf16_len(gekurzt) < utf16_len(&text);
        if !gekurzt.is_empty() && gekurzt_wurde {
            format!("{gekurzt} …")
        } else {
            gekurzt.to_string()
        }
    }
}

/// Entfernt die eingeschleusten Blöcke mitsamt Inhalt. Den Rückverweis `\1` des JavaScript-Musters
/// kennt `regex` nicht, deshalb der Blick Stelle für Stelle: zu einem passenden Anfangs-Tag muss
/// dasselbe Ende-Tag folgen, sonst bleibt das `<` stehen – genau wie in TypeScript.
fn strip_injected(prompt: &str) -> String {
    let mut rest = prompt;
    let mut ubrig = String::with_capacity(prompt.len());
    while let Some(start) = rest.find('<') {
        ubrig.push_str(&rest[..start]);
        let block = &rest[start..];
        match block_len(block) {
            Some(lange) => rest = &block[lange..],
            None => {
                ubrig.push('<');
                rest = &block[1..];
            }
        }
    }
    ubrig.push_str(rest);
    ubrig
}

/// Länge des Blocks ab `<`, wenn hier ein eingeschleuster Block mit passendem Ende-Tag beginnt
fn block_len(block: &str) -> Option<usize> {
    let name_lange = block[1..]
        .find(|zeichen: char| !is_name_char(zeichen))
        .unwrap_or(block.len() - 1);
    let name = &block[1..1 + name_lange];
    if !is_injected(name) {
        return None;
    }
    // Nach dem Namen folgt entweder sofort „>“ oder Leerraum und dann Angaben bis zum „>“
    let nach_name = &block[1 + name_lange..];
    let nach_offen = match nach_name.chars().next()? {
        '>' => 1 + name_lange + 1,
        zeichen if zeichen.is_whitespace() => 1 + name_lange + nach_name.find('>')? + 1,
        _ => return None,
    };
    let ende_tag = format!("</{name}>");
    let ende = block[nach_offen..].find(&ende_tag)?;
    Some(nach_offen + ende + ende_tag.len())
}

/// Zeichen, die in JavaScript als `[\w-]` zu einem Blocknamen gehören
fn is_name_char(zeichen: char) -> bool {
    zeichen.is_ascii_alphanumeric() || zeichen == '_' || zeichen == '-'
}

/// Gehört der Blockname zu den eingeschleusten Blöcken?
fn is_injected(name: &str) -> bool {
    INJECTED_NAMES.contains(&name)
        || INJECTED_PREFIXES.iter().any(|prefix| {
            name.strip_prefix(prefix)
                .is_some_and(|rest| !rest.is_empty())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stellt_die_dreistellige_nummer_vor_den_titel() {
        assert_eq!(
            SessionTitle::format(1, Some("Login bauen")),
            "SE-001: Login bauen"
        );
    }

    #[test]
    fn nutzt_nur_die_nummer_wenn_es_keinen_titel_gibt() {
        assert_eq!(SessionTitle::format(12, None), "SE-012");
        assert_eq!(SessionTitle::format(12, Some("  ")), "SE-012");
    }

    #[test]
    fn nummeriert_einen_bereits_nummerierten_titel_nicht_doppelt() {
        assert_eq!(
            SessionTitle::format(3, Some("SE-001: Login bauen")),
            "SE-001: Login bauen"
        );
    }

    #[test]
    fn leitet_den_titel_aus_den_ersten_wortern_des_prompts_ab() {
        assert_eq!(
            SessionTitle::from_prompt(Some("\n  Baue   bitte einen Login\nmit Passwort")),
            "Baue bitte einen Login"
        );
        assert_eq!(
            SessionTitle::from_prompt(Some("eins zwei drei vier fünf sechs sieben")),
            "eins zwei drei vier fünf sechs …"
        );
        assert_eq!(SessionTitle::from_prompt(Some("   ")), "");
        assert_eq!(SessionTitle::from_prompt(None), "");
    }

    #[test]
    fn uberspringt_eingeschleuste_blocke_wie_system_reminder_und_browser_instruction() {
        let prompt = [
            "<system-reminder>",
            "Codebase und Regeln",
            "</system-reminder>",
            "<browser_instruction># Claude in Chrome",
            "Nutze die Tools.",
            "</browser_instruction>Das Setzen der Session Namen funktioniert nicht",
        ]
        .join("\n");

        assert_eq!(
            SessionTitle::from_prompt(Some(&prompt)),
            "Das Setzen der Session Namen funktioniert …"
        );
    }

    #[test]
    fn liefert_leer_wenn_der_prompt_nur_aus_eingeschleusten_blocken_besteht() {
        assert_eq!(
            SessionTitle::from_prompt(Some("<system-reminder>\nnur Kontext\n</system-reminder>")),
            ""
        );
    }

    #[test]
    fn entfernt_auch_blocke_der_ide_und_der_befehle_samt_angaben() {
        let prompt = concat!(
            "<ide_selection file=\"a.rs\">Zeile 1</ide_selection>",
            "<local-command-stdout>Ausgabe</local-command-stdout>",
            "<command-name>/foo</command-name>Titel danach"
        );

        assert_eq!(SessionTitle::from_prompt(Some(prompt)), "Titel danach");
    }

    #[test]
    fn lasst_ein_tag_ohne_passendes_ende_stehen() {
        // Das JavaScript-Muster verlangt dasselbe Ende-Tag; fehlt es, bleibt der Text, wie er ist
        assert_eq!(
            SessionTitle::from_prompt(Some("<system-reminder>ohne Ende")),
            "<system-reminder>ohne Ende"
        );
        assert_eq!(
            SessionTitle::from_prompt(Some("<nur ein Tag>")),
            "<nur ein Tag>"
        );
    }

    #[test]
    fn schneidet_bei_funfzig_einheiten_ab_wie_javascript() {
        // Fünf Wörter, zusammen 54 Einheiten: der Schnitt liegt mitten im letzten Wort
        assert_eq!(
            SessionTitle::from_prompt(Some(
                "aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd eeeeeeeeee"
            )),
            "aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd eeeeee …"
        );
        // Liegt der Schnitt auf einem Leerzeichen, fällt es weg (JavaScript `trimEnd`)
        assert_eq!(
            SessionTitle::from_prompt(Some(
                "aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddddddddd eeeee"
            )),
            "aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddddddddd …"
        );
    }
}
