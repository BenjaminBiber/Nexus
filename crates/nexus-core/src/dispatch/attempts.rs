//! Zählt gescheiterte automatische Versuche an den Abbruch-Kommentaren des Dispatchers;
//! portiert aus `packages/core/src/dispatch/AttemptCounter.ts`.

use std::sync::LazyLock;

use regex::Regex;

use crate::dispatch::comments::CommentLike;
use crate::dispatch::config::{limit_pause, DispatchConfig};
use crate::dispatch::markers;

/// Anfang einer Limit-Pause mit Account: „[dispatch] Pause: Nutzungslimit von Account <id>“
static LIMIT_ACCOUNT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\S+ Pause: Nutzungslimit von Account ([a-z0-9-]+)").unwrap());

/// Zählt Abbrüche und Limit-Pausen an den Kommentaren eines Tickets.
#[derive(Debug, Clone, Copy)]
pub struct AttemptCounter<'a> {
    cfg: &'a DispatchConfig,
}

impl<'a> AttemptCounter<'a> {
    /// Zähler für diese Einstellung (der Name des Dispatchers steht darin)
    pub fn new(cfg: &'a DispatchConfig) -> Self {
        Self { cfg }
    }

    /// Anzahl der Kommentare des Dispatchers, die mit „[dispatch] Abbruch“ beginnen
    pub fn failed<C: CommentLike>(&self, comments: &[C]) -> usize {
        comments
            .iter()
            .filter(|comment| {
                comment.author_name() == self.cfg.dispatcher_name
                    && comment.body().starts_with(markers::ABORT)
            })
            .count()
    }

    /// Wie viele Limit-Pausen („[dispatch] Pause: Nutzungslimit …“) zuletzt in Folge kamen;
    /// Start-Kommentare dazwischen zählen nicht, jeder andere Kommentar des Dispatchers
    /// (Abbruch, Frage, …) beendet die Folge. Mit `account` zählen nur Pausen dieses Accounts;
    /// Pausen anderer Accounts (Wechsel auf den nächsten) überspringt die Zählung
    pub fn limit_pauses_in_row<C: CommentLike>(
        &self,
        comments: &[C],
        account: Option<&str>,
    ) -> usize {
        let pause = limit_pause();
        let mut count = 0;
        for comment in comments.iter().rev() {
            if comment.author_name() != self.cfg.dispatcher_name
                || comment.body().starts_with(markers::START)
            {
                continue;
            }
            if !comment.body().starts_with(&pause) {
                break;
            }
            let of = limit_account(comment.body());
            if let (Some(account), Some(of)) = (account, of.as_deref()) {
                if of != account {
                    continue;
                }
            }
            count += 1;
        }
        count
    }
}

/// Account einer Limit-Pause („… Nutzungslimit von Account <id> …“) oder `None`
/// (ältere Kommentare, ein einziger Account)
pub fn limit_account(body: &str) -> Option<String> {
    LIMIT_ACCOUNT
        .captures(body)
        .map(|found| found[1].to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;

    /// Die aufgelöste Beispiel-Einstellung
    fn cfg() -> DispatchConfig {
        let normalizer = DispatchConfigNormalizer::new();
        normalizer.normalize(&normalizer.example()).unwrap()
    }

    #[test]
    fn zahlt_nur_abbruch_kommentare_des_dispatchers() {
        let cfg = cfg();
        let counter = AttemptCounter::new(&cfg);
        assert_eq!(
            counter.failed(&[
                ("dispatcher", "[dispatch] Abbruch Versuch 1/2: x"),
                ("dispatcher", "[dispatch] Start: an junior-ARC-0001"),
                ("Florian", "[dispatch] Abbruch nachgemacht"),
                ("dispatcher", "[dispatch] Abbruch Versuch 2/2: y"),
            ]),
            2
        );
    }

    #[test]
    fn zahlt_limit_pausen_in_folge_und_ubergeht_starts() {
        let cfg = cfg();
        let counter = AttemptCounter::new(&cfg);
        assert_eq!(
            counter.limit_pauses_in_row(
                &[
                    ("dispatcher", "[dispatch] Abbruch Versuch 1/2: x"),
                    ("dispatcher", "[dispatch] Pause: Nutzungslimit bis 10:00"),
                    ("dispatcher", "[dispatch] Start: an junior-ARC-0001"),
                    ("dispatcher", "[dispatch] Pause: Nutzungslimit bis 11:00"),
                ],
                None
            ),
            2
        );
        assert_eq!(
            counter.limit_pauses_in_row(
                &[
                    ("dispatcher", "[dispatch] Pause: Nutzungslimit bis 10:00"),
                    ("dispatcher", "[dispatch] Abbruch Versuch 1/2: x"),
                ],
                None
            ),
            0
        );
    }

    #[test]
    fn zahlt_limit_pausen_je_account_und_ubergeht_die_anderen() {
        let cfg = cfg();
        let counter = AttemptCounter::new(&cfg);
        let pause = limit_pause();
        let von = |body: String| ("dispatcher".to_string(), body);
        let comments: Vec<(String, String)> = vec![
            von(format!(
                "{pause} von Account max erreicht, Ticket zurück nach Geplant."
            )),
            von("[dispatch] Start: an junior-ARC-1".to_string()),
            von(format!(
                "{pause} von Account pro-1 erreicht, Ticket zurück nach Geplant."
            )),
            von(format!(
                "{pause} von Account max erreicht, Ticket zurück nach Geplant."
            )),
        ];
        assert_eq!(counter.limit_pauses_in_row(&comments, Some("max")), 2);
        assert_eq!(counter.limit_pauses_in_row(&comments, Some("pro-1")), 1);
        assert_eq!(counter.limit_pauses_in_row(&comments, None), 3);
        assert_eq!(limit_account(&format!("{pause} erreicht")), None);
        assert_eq!(
            limit_account(&format!("{pause} von Account pro-1 erreicht")),
            Some("pro-1".to_string())
        );
    }

    #[test]
    fn der_name_des_dispatchers_kommt_aus_der_einstellung() {
        let cfg = DispatchConfigNormalizer::new()
            .normalize(&json!({ "dispatcherName": "robot" }))
            .unwrap();
        let counter = AttemptCounter::new(&cfg);
        assert_eq!(
            counter.failed(&[
                ("robot", "[dispatch] Abbruch x"),
                ("dispatcher", "[dispatch] Abbruch y")
            ]),
            1
        );
    }
}
