//! Offene Rückmeldungen aus dem Review; portiert aus
//! `packages/core/src/dispatch/ReviewFeedback.ts`.
//!
//! Es zählen die „Zurück an den Agenten“-Kommentare des Dispatchers seit dem letzten Start.

use std::sync::LazyLock;

use regex::Regex;

use crate::dispatch::comments::CommentLike;
use crate::dispatch::markers;

/// Führendes „ (von …)“ und der Doppelpunkt vor dem eigentlichen Text
static LEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\([^)]*\))?\s*:\s*").unwrap());

/// Liest die offenen Rückmeldungen aus den Kommentaren.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReviewFeedback;

impl ReviewFeedback {
    /// Neue Auswertung (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Texte der Rückmeldungen nach dem letzten `[dispatch] Start` des Dispatchers,
    /// älteste zuerst
    pub fn pending<C: CommentLike>(&self, comments: &[C], dispatcher_name: &str) -> Vec<String> {
        let own: Vec<&C> = comments
            .iter()
            .filter(|comment| comment.author_name() == dispatcher_name)
            .collect();
        let last_start = own
            .iter()
            .rposition(|comment| comment.body().starts_with(markers::START));
        let from = last_start.map(|at| at + 1).unwrap_or(0);
        own[from..]
            .iter()
            .filter(|comment| comment.body().starts_with(markers::REVIEW_RETURN))
            .map(|comment| {
                let rest = &comment.body()[markers::REVIEW_RETURN.len()..];
                LEAD.replace(rest, "").trim().to_string()
            })
            .filter(|text| !text.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liefert_nur_ruckmeldungen_des_dispatchers_nach_dem_letzten_start() {
        let comments = [
            (
                "dispatcher",
                "[dispatch] Start: an junior-ARC-0001 übergeben",
            ),
            (
                "dispatcher",
                "[dispatch] Zurück an den Agenten (von Florian): alt, schon erledigt",
            ),
            (
                "dispatcher",
                "[dispatch] Start: an junior-ARC-0001 übergeben",
            ),
            ("dispatcher", "[dispatch] Review: wartet auf Freigabe"),
            (
                "dispatcher",
                "[dispatch] Zurück an den Agenten (von Florian): Bitte auch den Fehlerfall testen",
            ),
            (
                "Florian",
                "[dispatch] Zurück an den Agenten (von Florian): nachgemacht",
            ),
            (
                "dispatcher",
                "[dispatch] Zurück an den Agenten (von Anna): Und\nzweite Zeile",
            ),
        ];
        assert_eq!(
            ReviewFeedback::new().pending(&comments, "dispatcher"),
            ["Bitte auch den Fehlerfall testen", "Und\nzweite Zeile"]
        );
    }

    #[test]
    fn ohne_start_zahlen_alle_ruckmeldungen_ohne_ruckmeldung_ist_die_liste_leer() {
        let feedback = ReviewFeedback::new();
        assert_eq!(
            feedback.pending(
                &[(
                    "dispatcher",
                    "[dispatch] Zurück an den Agenten (von X): eins"
                )],
                "dispatcher"
            ),
            ["eins"]
        );
        assert!(feedback
            .pending(&[("dispatcher", "[dispatch] Start: x")], "dispatcher")
            .is_empty());
    }
}
