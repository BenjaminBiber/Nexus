//! Was aus „Erneut mergen“, „Abnehmen“ bzw. „Zurück an den Agenten“ geworden ist; portiert aus
//! `packages/server/src/dispatch/MergeResultReader.ts`.
//!
//! Die JSON-Form ist Vertrag: `outcome` und `text` kommen so aus den Routen und der
//! Kommandozeile.

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::TicketDetail;
use nexus_core::dispatch::{markers, MergeEntry, MergePhase, MERGE_REASON_PREFIX};
use serde::{Deserialize, Serialize};

/// Ausgang für das angefragte Ticket (`accepted`: nur erledigt, kein Merge angestoßen;
/// `returned`: zurück nach Geplant)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MergeRetryOutcome {
    Merged,
    Blocked,
    Waiting,
    Session,
    Pending,
    Accepted,
    Returned,
}

/// Ergebnis von „Erneut mergen“, „Abnehmen“ bzw. „Zurück an den Agenten“
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRetryResult {
    pub outcome: MergeRetryOutcome,
    /// Eine Zeile für Kommandozeile und Oberfläche, z. B. „ARC-0012 gemergt in main (abc1234).“
    pub text: String,
}

/// Liest nach „Erneut mergen“ aus Bericht, Ticket (Status, Grund, letzte Dispatcher-Kommentare)
/// und Merge-Auftrag, was mit dem Merge dieses Tickets geschehen ist.
#[derive(Debug, Clone, Copy, Default)]
pub struct MergeResultReader;

impl MergeResultReader {
    /// Leser ohne Zustand
    pub fn new() -> Self {
        Self
    }

    /// Ergebnis für das Ticket `id`; `detail` `None` = Ticket nicht mehr lesbar
    pub fn read(
        &self,
        id: &str,
        merged: &[String],
        errors: &[String],
        detail: Option<&TicketDetail>,
        entry: Option<&MergeEntry>,
    ) -> MergeRetryResult {
        if merged.iter().any(|ticket| ticket == id) {
            return MergeRetryResult {
                outcome: MergeRetryOutcome::Merged,
                text: self.merged_text(id, detail),
            };
        }
        let reason = detail
            .and_then(|detail| detail.ticket.blocked_reason.clone())
            .unwrap_or_default();
        if detail.is_some_and(|detail| detail.ticket.status == TicketStatus::Blocked)
            && reason.starts_with(MERGE_REASON_PREFIX)
        {
            return MergeRetryResult {
                outcome: MergeRetryOutcome::Blocked,
                text: format!(
                    "{id}: Merge blockiert – {}",
                    reason[MERGE_REASON_PREFIX.len()..].trim()
                ),
            };
        }
        if entry.is_some_and(|entry| entry.phase == MergePhase::Session) {
            return MergeRetryResult {
                outcome: MergeRetryOutcome::Session,
                text: format!("{id}: Merge-Konflikt – Merge-Session gestartet."),
            };
        }
        if entry.is_some_and(|entry| entry.phase == MergePhase::Waiting) {
            let wait = self.last_comment(detail, markers::MERGE_WAIT);
            let rest = match &wait {
                Some(body) => body[markers::MERGE_WAIT.len()..].trim().to_string(),
                None => "auf weitere Tickets des Worktrees.".to_string(),
            };
            return MergeRetryResult {
                outcome: MergeRetryOutcome::Waiting,
                text: format!("{id}: Merge wartet {rest}"),
            };
        }
        // Fachlicher Fehler des Durchlaufs zu diesem Ticket: „<ID>, <ID>: <Meldung>“
        if let Some(error) = errors.iter().find(|error| {
            error
                .split(':')
                .next()
                .unwrap_or_default()
                .split(", ")
                .any(|ticket| ticket == id)
        }) {
            let rest = match error.find(':') {
                Some(at) => error[at + 1..].trim(),
                None => error.as_str(),
            };
            return MergeRetryResult {
                outcome: MergeRetryOutcome::Pending,
                text: format!("{id}: Merge nicht ausgeführt – {rest}"),
            };
        }
        if entry.is_some_and(|entry| entry.conflict_seen == Some(true)) {
            return MergeRetryResult {
                outcome: MergeRetryOutcome::Pending,
                text: format!(
                    "{id}: Merge-Konflikt – die Merge-Session wartet (Dispatcher ausgeschaltet, \
Pause oder außerhalb des Zeitfensters)."
                ),
            };
        }
        MergeRetryResult {
            outcome: MergeRetryOutcome::Pending,
            text: format!("{id}: Merge wartet auf den nächsten Durchlauf des Dispatchers."),
        }
    }

    /// „<ID> gemergt in <Ziel> (<Commit>).“ aus dem Kommentar des Dispatchers, samt Hinweis auf
    /// unvollständiges Aufräumen
    fn merged_text(&self, id: &str, detail: Option<&TicketDetail>) -> String {
        let Some(comment) = self.last_comment(detail, markers::MERGED) else {
            return format!("{id} gemergt.");
        };
        let Some((target, commit)) = merged_head(&comment) else {
            return format!("{id} gemergt.");
        };
        let lines: Vec<&str> = comment.split('\n').collect();
        let incomplete = if lines[0].contains("Aufräumen unvollständig") {
            format!(" Aufräumen unvollständig: {}", lines[1..].join(" "))
        } else {
            String::new()
        };
        format!("{id} gemergt in {target} ({commit}).{incomplete}")
    }

    /// Letzter Kommentar mit dem Marker oder `None`
    fn last_comment(&self, detail: Option<&TicketDetail>, marker: &str) -> Option<String> {
        detail?
            .comments
            .iter()
            .rfind(|comment| comment.body.starts_with(marker))
            .map(|comment| comment.body.clone())
    }
}

/// Ziel und Commit aus „[dispatch] Gemergt in <Ziel> (<Commit>).“ der ersten Zeile
fn merged_head(comment: &str) -> Option<(String, String)> {
    let head = comment.split('\n').next()?;
    let rest = head.strip_prefix("[dispatch] Gemergt in ")?;
    // Ziel kann Schrägstriche enthalten, der Commit steht in der ersten Klammer vor „).“
    let open = rest.find(" (")?;
    let target = &rest[..open];
    let after = &rest[open + 2..];
    let close = after.find(')')?;
    if !after[close..].starts_with(").") || target.is_empty() {
        return None;
    }
    let commit = &after[..close];
    if commit.is_empty() {
        return None;
    }
    Some((target.to_string(), commit.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_ziel_und_commit_aus_dem_gemergt_kommentar() {
        assert_eq!(
            merged_head("[dispatch] Gemergt in release/1 (abc1234). Branch aufgeräumt."),
            Some(("release/1".to_string(), "abc1234".to_string()))
        );
        assert_eq!(merged_head("[dispatch] Gemergt ohne Ziel"), None);
    }
}
