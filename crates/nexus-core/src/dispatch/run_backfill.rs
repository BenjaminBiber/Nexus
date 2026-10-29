//! Rekonstruiert Läufe aus den `[dispatch]`-Kommentaren (für die Zeit vor der Erfassung in
//! `board.DispatchRuns`): je Start-Kommentar ein Lauf mit Profil, Modell-Alias, Effort,
//! Versuch und Eskalation; das Ergebnis aus dem nächsten Dispatcher-Kommentar bzw.
//! Statuswechsel. Kosten und tatsächliches Modell sind nicht bekannt.
//! Portiert aus `packages/core/src/dispatch/DispatchRunBackfill.ts`.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::board::basics::is_human_interaction;
use crate::board::dispatch_run_types::{
    DispatchRun, DispatchRunKind, DispatchRunOutcome, DispatchRunSource,
};
use crate::board::types::{Comment, HistoryEntry, Ticket};
use crate::clock::parse_millis;
use crate::dispatch::abort_reason::AbortReasonClassifier;
use crate::dispatch::markers;
use crate::text::utf16_prefix;

/// „[dispatch] Start: an \<agent\> übergeben – Branch …, Profil \<p\> (Modell \<m\>, Effort
/// \<e\>, max. n Schritte, n min). Grund: \<why\>.“
static START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\[dispatch\] Start: an (\S+) übergeben – Branch .*?, Profil (\S+) \(Modell ([^,]+), Effort ([^,]+), max\. \d+ Schritte, \d+ min\)\. Grund: ([^\n]*?)\.?(?:\n|$)",
    )
    .expect("festes Muster")
});

/// Basisprofil am Anfang der Begründung (`ProfileChooser.choose`)
static BASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:Standardprofil (\S+?)|Label profil:(\S+?)|Aufwand laut Plan: (\S+?)|Regel \d+ → (\S+?))(?:,| |$)",
    )
    .expect("festes Muster")
});

/// Eskalation in der Begründung
static ESCALATED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Versuch (\d+): eskaliert auf (\S+?)(?:,|$)").expect("festes Muster")
});

/// „[dispatch] Abbruch Versuch n/m: \<Grund\>.“
static ABORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[dispatch\] Abbruch Versuch (\d+)/\d+: ([^\n]*?)\.?(?:\n|$)")
        .expect("festes Muster")
});

/// Höchstlänge des abgelegten Abbruchgrunds
const MAX_REASON: usize = 500;

/// Was das Nachtragen braucht
#[derive(Debug, Clone, Default)]
pub struct BackfillInput {
    pub project_id: i32,
    /// Kommentare „[dispatch] …“ der Tickets des Projekts, nach ID
    pub comments: Vec<Comment>,
    /// Statuswechsel und Blockiert-Gründe dieser Tickets
    pub history: Vec<HistoryEntry>,
    pub tickets: Vec<Ticket>,
}

/// Das Ergebnis eines Laufs, wie es aus Kommentaren und Verlauf hervorgeht
#[derive(Debug, Clone)]
struct Event {
    at: String,
    outcome: DispatchRunOutcome,
    reason: Option<String>,
    /// Blockiert-Grund laut Verlauf
    blocked: Option<String>,
}

impl Event {
    /// Ein Ergebnis ohne Grund
    fn new(at: &str, outcome: DispatchRunOutcome) -> Self {
        Self {
            at: at.to_string(),
            outcome,
            reason: None,
            blocked: None,
        }
    }
}

/// Trägt Läufe aus den `[dispatch]`-Kommentaren nach.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchRunBackfill;

impl DispatchRunBackfill {
    /// Alle rekonstruierten Läufe, ältester zuerst
    pub fn runs(input: &BackfillInput) -> Vec<DispatchRun> {
        let tickets: HashMap<&str, &Ticket> = input
            .tickets
            .iter()
            .map(|ticket| (ticket.id.as_str(), ticket))
            .collect();
        let mut out: Vec<DispatchRun> = Vec::new();
        // Kommentare je Ticket in der Reihenfolge ihres ersten Auftretens
        let mut by_ticket: Vec<(&str, Vec<&Comment>)> = Vec::new();
        for comment in &input.comments {
            match by_ticket
                .iter_mut()
                .find(|(id, _)| *id == comment.ticket_id.as_str())
            {
                Some((_, list)) => list.push(comment),
                None => by_ticket.push((comment.ticket_id.as_str(), vec![comment])),
            }
        }

        for (ticket_id, comments) in by_ticket {
            let ticket = tickets.get(ticket_id).copied();
            let mut history: Vec<&HistoryEntry> = input
                .history
                .iter()
                .filter(|entry| entry.ticket_id == ticket_id)
                .collect();
            history.sort_by(|a, b| a.at.cmp(&b.at));
            let mut sorted = comments;
            sorted.sort_by_key(|comment| comment.id);

            for (index, comment) in sorted.iter().enumerate() {
                let Some(start) = START.captures(&comment.body) else {
                    continue;
                };
                let next_start = sorted
                    .iter()
                    .skip(index + 1)
                    .find(|next| next.body.starts_with(markers::START));
                let until = next_start.map(|next| next.created_at.clone());
                let after: Vec<&&Comment> = sorted
                    .iter()
                    .skip(index + 1)
                    .filter(|next| next_start.is_none_or(|start| next.id < start.id))
                    .collect();
                let end = end_of_run(&after, &history, &comment.created_at, until.as_deref());
                // Letzter Start ohne Ergebnis an einem Ticket in Arbeit: der Lauf läuft
                // vermutlich noch
                let läuft_noch = end.is_none()
                    && next_start.is_none()
                    && ticket.is_some_and(|ticket| {
                        ticket.status == crate::board::basics::TicketStatus::InProgress
                    });
                if läuft_noch {
                    continue;
                }
                let agent = start[1].to_string();
                let profile = start[2].to_string();
                let model = start[3].to_string();
                let effort = start[4].to_string();
                let why = start[5].to_string();
                let escalated = ESCALATED.is_match(&why);
                let base = BASE.captures(&why).and_then(|found| {
                    found
                        .iter()
                        .skip(1)
                        .flatten()
                        .map(|group| group.as_str().to_string())
                        .next()
                });
                let aborts = sorted
                    .iter()
                    .take(index)
                    .filter(|earlier| earlier.body.starts_with(markers::ABORT))
                    .count();
                let event = end.unwrap_or_else(|| {
                    Event::new(
                        until.as_deref().unwrap_or(&comment.created_at),
                        DispatchRunOutcome::Other,
                    )
                });
                let duration = parse_millis(&event.at)
                    .zip(parse_millis(&comment.created_at))
                    .map_or(0, |(end, start)| (end - start).max(0));
                out.push(DispatchRun {
                    id: format!("c{}", comment.id),
                    kind: DispatchRunKind::Work,
                    subject_id: ticket_id.to_string(),
                    project_id: input.project_id,
                    board_id: ticket.map(|ticket| ticket.board_id),
                    agent,
                    attempt: Some(aborts as i32 + 1),
                    profile,
                    escalated_from: if escalated { base } else { None },
                    requested_model: (model != "Standard").then_some(model),
                    model: None,
                    effort: (effort != "Standard").then_some(effort),
                    started_at: comment.created_at.clone(),
                    ended_at: Some(event.at.clone()),
                    duration_ms: Some(duration as i32),
                    num_turns: None,
                    exit_code: None,
                    timed_out: false,
                    outcome: event.outcome,
                    abort_category: event
                        .reason
                        .as_deref()
                        .map(|reason| AbortReasonClassifier::category(reason).as_str().to_string()),
                    abort_reason: event
                        .reason
                        .as_deref()
                        .or(event.blocked.as_deref())
                        .map(|text| utf16_prefix(text, MAX_REASON).to_string()),
                    cost_usd: None,
                    input_tokens: None,
                    output_tokens: None,
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                    model_usage: None,
                    source: DispatchRunSource::Comments,
                    // Rekonstruierte Läufe kennen weder Limits noch Konfig-Version oder
                    // abgelehnte Werkzeuge
                    max_turns: None,
                    timeout_minutes: None,
                    profile_reason: None,
                    rule_index: None,
                    config_hash: None,
                    denied_count: None,
                    denied_tools: None,
                });
            }
        }
        out.sort_by(|a, b| {
            a.started_at
                .cmp(&b.started_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        out
    }
}

/// Ergebnis eines Laufs: erster aussagekräftiger Dispatcher-Kommentar, sonst der erste
/// Statuswechsel weg von In Arbeit
fn end_of_run(
    after: &[&&Comment],
    history: &[&HistoryEntry],
    from: &str,
    until: Option<&str>,
) -> Option<Event> {
    for comment in after {
        if let Some(abort) = ABORT.captures(&comment.body) {
            let outcome = if comment.body.contains("letzte automatische Versuch") {
                DispatchRunOutcome::AbortFinal
            } else {
                DispatchRunOutcome::Abort
            };
            return Some(Event {
                reason: Some(abort[2].to_string()),
                ..Event::new(&comment.created_at, outcome)
            });
        }
        let pause = format!("{}: ", markers::PAUSE);
        if comment.body.starts_with(&format!("{pause}Nutzungslimit")) {
            return Some(Event::new(
                &comment.created_at,
                DispatchRunOutcome::PauseLimit,
            ));
        }
        if comment
            .body
            .starts_with(&format!("{pause}Claude Code wurde nicht gefunden"))
        {
            return Some(Event::new(
                &comment.created_at,
                DispatchRunOutcome::PauseClaudeMissing,
            ));
        }
        if comment.body.starts_with(markers::QUESTION) {
            return Some(Event::new(
                &comment.created_at,
                DispatchRunOutcome::Question,
            ));
        }
        if comment.body.starts_with(markers::SETUP) {
            return Some(Event::new(
                &comment.created_at,
                DispatchRunOutcome::SetupFailed,
            ));
        }
        if comment.body.starts_with(&format!("{}:", markers::REVIEW)) {
            return Some(Event::new(&comment.created_at, DispatchRunOutcome::Review));
        }
    }
    let change = history.iter().find(|entry| {
        entry.field == "status"
            && entry.at.as_str() >= from
            && until.is_none_or(|until| entry.at.as_str() <= until)
            && entry.to.as_deref() != Some("in_progress")
    })?;
    match change.to.as_deref() {
        Some("done") => Some(Event::new(&change.at, DispatchRunOutcome::Done)),
        Some("blocked") => {
            // Der Grund steht im selben Verlaufsschritt; „Human Interaction: …“ ist fertig,
            // nur vom Menschen abzunehmen
            let reason = history
                .iter()
                .find(|entry| entry.field == "blockedReason" && entry.at == change.at)
                .and_then(|entry| entry.to.clone());
            let outcome = if is_human_interaction(reason.as_deref()) {
                DispatchRunOutcome::Human
            } else {
                DispatchRunOutcome::Blocked
            };
            Some(Event {
                blocked: reason.filter(|reason| !reason.is_empty()),
                ..Event::new(&change.at, outcome)
            })
        }
        _ => Some(Event::new(&change.at, DispatchRunOutcome::Other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::basics::TicketStatus;
    use crate::testing::ticket_fixture;

    /// Der Start-Kommentar, den der Dispatcher schreibt
    fn start(agent: &str, profile: &str, why: &str) -> String {
        format!(
            "[dispatch] Start: an {agent} übergeben – Branch ticket/ARC-0001-x, Profil {profile} (Modell opus, Effort high, max. 80 Schritte, 90 min). Grund: {why}."
        )
    }

    /// Ein Kommentar an ARC-0001
    fn comment(id: i64, at: &str, body: &str) -> Comment {
        Comment {
            id,
            ticket_id: "ARC-0001".into(),
            author_id: "u".into(),
            author_name: "dispatcher".into(),
            body: body.to_string(),
            created_at: at.to_string(),
            read_by_agent_at: None,
        }
    }

    /// Ein Verlaufseintrag an ARC-0001
    fn history(at: &str, field: &str, to: &str) -> HistoryEntry {
        HistoryEntry {
            ticket_id: "ARC-0001".into(),
            at: at.to_string(),
            by_id: "u".into(),
            by_name: "dispatcher".into(),
            field: field.to_string(),
            from: None,
            to: Some(to.to_string()),
            note: None,
        }
    }

    /// Das Ticket ARC-0001 in diesem Status
    fn ticket(status: TicketStatus) -> Ticket {
        Ticket {
            status,
            ..ticket_fixture("ARC-0001", 7, 3, 1)
        }
    }

    #[test]
    fn liest_agent_profil_modell_effort_und_versuch_aus_dem_start_kommentar() {
        let runs = DispatchRunBackfill::runs(&BackfillInput {
            project_id: 7,
            comments: vec![
                comment(
                    1,
                    "2026-10-13T10:00:00.000Z",
                    &start("junior-ARC-0001", "standard", "Standardprofil standard"),
                ),
                comment(
                    2,
                    "2026-10-13T10:30:00.000Z",
                    "[dispatch] Abbruch Versuch 1/3: Zeitlimit von 90 min überschritten.",
                ),
                comment(
                    3,
                    "2026-10-13T11:00:00.000Z",
                    &start(
                        "junior-ARC-0001",
                        "schwer",
                        "Standardprofil standard, Versuch 2: eskaliert auf schwer",
                    ),
                ),
            ],
            history: vec![history("2026-10-13T12:00:00.000Z", "status", "done")],
            tickets: vec![ticket(TicketStatus::Done)],
        });

        assert_eq!(
            runs.iter().map(|run| run.id.as_str()).collect::<Vec<_>>(),
            ["c1", "c3"]
        );
        let erster = &runs[0];
        assert_eq!(erster.agent, "junior-ARC-0001");
        assert_eq!(erster.profile, "standard");
        assert_eq!(erster.requested_model.as_deref(), Some("opus"));
        assert_eq!(erster.effort.as_deref(), Some("high"));
        assert_eq!(erster.attempt, Some(1));
        assert_eq!(erster.escalated_from, None);
        assert_eq!(erster.outcome, DispatchRunOutcome::Abort);
        assert_eq!(erster.abort_category.as_deref(), Some("timeout"));
        assert_eq!(
            erster.abort_reason.as_deref(),
            Some("Zeitlimit von 90 min überschritten")
        );
        assert_eq!(erster.duration_ms, Some(1_800_000));
        assert_eq!(erster.board_id, Some(3));
        assert_eq!(erster.project_id, 7);
        assert_eq!(erster.source, DispatchRunSource::Comments);

        let zweiter = &runs[1];
        assert_eq!(zweiter.profile, "schwer");
        assert_eq!(zweiter.attempt, Some(2));
        assert_eq!(zweiter.escalated_from.as_deref(), Some("standard"));
        // Das Ergebnis kommt aus dem Statuswechsel nach Erledigt
        assert_eq!(zweiter.outcome, DispatchRunOutcome::Done);
        assert_eq!(
            zweiter.ended_at.as_deref(),
            Some("2026-10-13T12:00:00.000Z")
        );
    }

    #[test]
    fn erkennt_den_letzten_automatischen_versuch_als_endgultigen_abbruch() {
        let runs = DispatchRunBackfill::runs(&BackfillInput {
            project_id: 7,
            comments: vec![
                comment(
                    1,
                    "2026-10-13T10:00:00.000Z",
                    &start("junior-ARC-0001", "schwer", "Label profil:schwer"),
                ),
                comment(
                    2,
                    "2026-10-13T10:30:00.000Z",
                    "[dispatch] Abbruch Versuch 3/3: Kontext voll: Prompt is too long. Das war der letzte automatische Versuch.",
                ),
            ],
            history: Vec::new(),
            tickets: vec![ticket(TicketStatus::Blocked)],
        });
        assert_eq!(runs[0].outcome, DispatchRunOutcome::AbortFinal);
        assert_eq!(runs[0].abort_category.as_deref(), Some("context"));
    }

    #[test]
    fn nimmt_den_blockiert_grund_aus_dem_verlauf_und_erkennt_human_interaction() {
        let runs = DispatchRunBackfill::runs(&BackfillInput {
            project_id: 7,
            comments: vec![comment(
                1,
                "2026-10-13T10:00:00.000Z",
                &start("junior-ARC-0001", "standard", "Regel 2 → standard"),
            )],
            history: vec![
                history("2026-10-13T11:00:00.000Z", "status", "blocked"),
                history(
                    "2026-10-13T11:00:00.000Z",
                    "blockedReason",
                    "Human Interaction: Review vor dem Merge",
                ),
            ],
            tickets: vec![ticket(TicketStatus::Blocked)],
        });
        assert_eq!(runs[0].outcome, DispatchRunOutcome::Human);
        assert_eq!(
            runs[0].abort_reason.as_deref(),
            Some("Human Interaction: Review vor dem Merge")
        );
        assert_eq!(runs[0].abort_category, None);
    }

    #[test]
    fn lasst_den_letzten_start_eines_laufenden_tickets_weg() {
        let input = BackfillInput {
            project_id: 7,
            comments: vec![comment(
                1,
                "2026-10-13T10:00:00.000Z",
                &start("junior-ARC-0001", "standard", "Standardprofil standard"),
            )],
            history: Vec::new(),
            tickets: vec![ticket(TicketStatus::InProgress)],
        };
        assert!(DispatchRunBackfill::runs(&input).is_empty());
        // Dasselbe Ticket blockiert: der Lauf endete ohne erkennbares Ergebnis
        let fertig = DispatchRunBackfill::runs(&BackfillInput {
            tickets: vec![ticket(TicketStatus::Blocked)],
            ..input
        });
        assert_eq!(fertig[0].outcome, DispatchRunOutcome::Other);
    }

    #[test]
    fn erkennt_frage_einrichtung_review_und_pausen_als_ergebnis() {
        let fälle = [
            (
                "[dispatch] Frage: wartet auf ARC-F0001",
                DispatchRunOutcome::Question,
            ),
            (
                "[dispatch] Einrichtung fehlgeschlagen: npm ci",
                DispatchRunOutcome::SetupFailed,
            ),
            (
                "[dispatch] Review: wartet auf Freigabe",
                DispatchRunOutcome::Review,
            ),
            (
                "[dispatch] Pause: Nutzungslimit erreicht",
                DispatchRunOutcome::PauseLimit,
            ),
            (
                "[dispatch] Pause: Claude Code wurde nicht gefunden",
                DispatchRunOutcome::PauseClaudeMissing,
            ),
        ];
        for (body, outcome) in fälle {
            let runs = DispatchRunBackfill::runs(&BackfillInput {
                project_id: 7,
                comments: vec![
                    comment(
                        1,
                        "2026-10-13T10:00:00.000Z",
                        &start("junior-ARC-0001", "standard", "Standardprofil standard"),
                    ),
                    comment(2, "2026-10-13T10:30:00.000Z", body),
                ],
                history: Vec::new(),
                tickets: vec![ticket(TicketStatus::Blocked)],
            });
            assert_eq!(runs[0].outcome, outcome, "{body}");
        }
    }

    #[test]
    fn ubergeht_kommentare_ohne_start_und_sortiert_nach_startzeit() {
        let runs = DispatchRunBackfill::runs(&BackfillInput {
            project_id: 7,
            comments: vec![
                comment(5, "2026-10-13T12:00:00.000Z", "Ein normaler Kommentar"),
                Comment {
                    ticket_id: "ARC-0002".into(),
                    ..comment(
                        6,
                        "2026-10-13T09:00:00.000Z",
                        &start("junior-ARC-0002", "klein", "Standardprofil klein"),
                    )
                },
                comment(
                    7,
                    "2026-10-13T10:00:00.000Z",
                    &start("junior-ARC-0001", "standard", "Standardprofil standard"),
                ),
            ],
            history: Vec::new(),
            tickets: vec![
                ticket(TicketStatus::Blocked),
                Ticket {
                    status: TicketStatus::Blocked,
                    ..ticket_fixture("ARC-0002", 7, 3, 2)
                },
            ],
        });
        assert_eq!(
            runs.iter()
                .map(|run| (run.id.as_str(), run.subject_id.as_str()))
                .collect::<Vec<_>>(),
            [("c6", "ARC-0002"), ("c7", "ARC-0001")]
        );
    }
}
