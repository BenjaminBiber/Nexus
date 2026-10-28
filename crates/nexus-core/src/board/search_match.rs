//! Treffer und Ausschnitt der Board-Suche ohne Datenbank; portiert aus
//! `packages/core/src/board/BoardSearchMatch.ts`.
//!
//! JavaScript rechnet mit `indexOf`, `slice` und `length` in UTF-16-Einheiten, SQL Server mit
//! `nvarchar` ebenso. `text_start` und `text_length` sind Teil der API (sie landen in
//! [`crate::board::search_types::SearchRow`]), deshalb rechnet auch diese Umsetzung in
//! UTF-16-Einheiten – siehe [`crate::text`].

use crate::board::search_types::SEARCH_WINDOW;
use crate::text::{utf16_index_of, utf16_len, utf16_slice};

/// Ausschnitt eines Feldes: Text ab `text_start` und die Länge des ganzen Feldes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchExcerpt {
    pub text: String,
    /// Position des Ausschnitts im Feld in UTF-16-Einheiten (0 = Anfang)
    pub text_start: i32,
    /// Länge des ganzen Feldes in UTF-16-Einheiten
    pub text_length: i32,
}

/// Treffer und Ausschnitt der Board-Suche ohne Datenbank: Groß/Klein egal (Unicode über
/// `to_lowercase`), Akzente zählen, Teilwörter zählen, Sonderzeichen wörtlich.
/// Gemeinsam für die Suche im Speicher und in SQLite.
#[derive(Debug, Clone)]
pub struct BoardSearchMatch {
    needles: Vec<String>,
}

impl BoardSearchMatch {
    /// Nimmt die Suchbegriffe und legt sie in Kleinschreibung ab.
    ///
    /// `to_lowercase` kann die Länge ändern (etwa „İ“ → „i̇“); TypeScript verhält sich mit
    /// `toLowerCase` genauso. Das ist beabsichtigt und wird hier nicht „verbessert“.
    pub fn new<I, S>(terms: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            needles: terms
                .into_iter()
                .map(|term| term.as_ref().to_lowercase())
                .collect(),
        }
    }

    /// Alle Begriffe kommen im Text vor; ein fehlender Text (`None`) trifft nie
    pub fn matches(&self, text: Option<&str>) -> bool {
        let Some(text) = text else {
            return false;
        };
        let lower = text.to_lowercase();
        self.needles.iter().all(|needle| lower.contains(needle))
    }

    /// Ausschnitt mit dem üblichen Fenster ([`SEARCH_WINDOW`])
    pub fn excerpt(&self, text: &str) -> SearchExcerpt {
        self.excerpt_with_window(text, SEARCH_WINDOW)
    }

    /// Fenster wie `MssqlSearchRepository`: bis `window` Einheiten vor der ersten Fundstelle des
    /// ersten Begriffs, insgesamt `2 * window` plus die Länge des Begriffs
    pub fn excerpt_with_window(&self, text: &str, window: usize) -> SearchExcerpt {
        let first = self
            .needles
            .first()
            .and_then(|needle| utf16_index_of(&text.to_lowercase(), needle));
        let start = match first {
            Some(at) if at > window => at - window,
            _ => 0,
        };
        let length = window * 2 + self.needles.first().map_or(0, |n| utf16_len(n));
        SearchExcerpt {
            text: utf16_slice(text, start, length).to_string(),
            text_start: start as i32,
            text_length: utf16_len(text) as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignoriert_gross_klein_auch_bei_umlauten_achtet_aber_auf_akzente() {
        assert!(BoardSearchMatch::new(["ärger"]).matches(Some("Großer Ärger im Login")));
        assert!(!BoardSearchMatch::new(["arger"]).matches(Some("Großer Ärger im Login")));
        assert!(BoardSearchMatch::new(["ÄRGER"]).matches(Some("ärgerlich")));
    }

    #[test]
    fn verlangt_alle_begriffe_im_selben_text_und_trifft_nichts_nie() {
        assert!(!BoardSearchMatch::new(["login", "passwort"]).matches(Some("Login bauen")));
        assert!(BoardSearchMatch::new(["login", "passwort"]).matches(Some("Login mit Passwort")));
        assert!(!BoardSearchMatch::new(["login"]).matches(None));
    }

    #[test]
    fn nimmt_sonderzeichen_wortlich() {
        let text = Some("Rabatt 100% auf _x_ [a]");
        assert!(BoardSearchMatch::new(["100%"]).matches(text));
        assert!(BoardSearchMatch::new(["_x_"]).matches(text));
        assert!(BoardSearchMatch::new(["[a]"]).matches(text));
        assert!(!BoardSearchMatch::new(["ra%tt"]).matches(text));
    }

    #[test]
    fn liefert_kurze_felder_ganz() {
        assert_eq!(
            BoardSearchMatch::new(["oauth"]).excerpt("Plan: OAuth einbauen"),
            SearchExcerpt {
                text: "Plan: OAuth einbauen".into(),
                text_start: 0,
                text_length: 20,
            }
        );
    }

    #[test]
    fn schneidet_lange_felder_um_die_erste_fundstelle_des_ersten_begriffs_aus() {
        let value = format!("{} Fundstelle am Ende", "x".repeat(9000));
        let excerpt = BoardSearchMatch::new(["fundstelle"]).excerpt(&value);
        assert_eq!(excerpt.text_start, 8851);
        assert_eq!(excerpt.text_length, 9019);
        assert_eq!(excerpt.text, utf16_slice(&value, 8851, 310));
        assert_eq!(utf16_len(&excerpt.text), 168);
        assert!(excerpt.text.ends_with("Fundstelle am Ende"));
    }

    #[test]
    fn beginnt_erst_hinter_dem_fenster_zu_kurzen() {
        let im_fenster = format!("{}treffer", "y".repeat(150));
        let dahinter = format!("{}treffer", "y".repeat(151));
        assert_eq!(
            BoardSearchMatch::new(["treffer"])
                .excerpt(&im_fenster)
                .text_start,
            0
        );
        assert_eq!(
            BoardSearchMatch::new(["treffer"])
                .excerpt(&dahinter)
                .text_start,
            1
        );
    }

    #[test]
    fn zahlt_ein_emoji_vor_der_fundstelle_als_zwei_einheiten() {
        // „🙂“ belegt zwei UTF-16-Einheiten, aber vier Bytes: JavaScript meldet hier 2,
        // eine Byte- oder Zeichenrechnung käme auf 4 bzw. 1
        let text = format!("🙂{}treffer", "y".repeat(200));
        let excerpt = BoardSearchMatch::new(["treffer"]).excerpt(&text);
        assert_eq!(excerpt.text_start, 52);
        assert_eq!(excerpt.text_length, 209);
        assert_eq!(utf16_len(&excerpt.text), 157);
        assert!(excerpt.text.ends_with("treffer"));
    }

    #[test]
    fn ohne_begriffe_beginnt_der_ausschnitt_am_anfang() {
        let leer: [&str; 0] = [];
        let excerpt = BoardSearchMatch::new(leer).excerpt("Irgendein Text");
        assert_eq!(excerpt.text, "Irgendein Text");
        assert_eq!(excerpt.text_start, 0);
        assert_eq!(excerpt.text_length, 14);
    }
}
