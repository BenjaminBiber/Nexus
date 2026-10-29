//! Sagt, warum der Dispatcher ein Ticket (noch) nicht nimmt; portiert aus
//! `packages/core/src/dispatch/DispatchReadiness.ts`.

use crate::board::{TicketStatus, TicketView};
use crate::dispatch::config::DispatchConfig;
use crate::dispatch::profile_chooser::ProfileChooser;

/// Die Felder eines Tickets, die für die Bereitschaft zählen – in TypeScript `TicketView`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessTicket {
    pub status: TicketStatus,
    pub assignee: Option<String>,
    pub needs_planning: bool,
    pub labels: Vec<String>,
    pub plan: String,
    pub handbook: String,
    pub acceptance: String,
    pub depends_open: Vec<String>,
    pub depends_unmerged: Vec<String>,
    /// Kennungen der offenen Fragen, die dieses Ticket betreffen
    pub waiting_on_questions: Vec<String>,
}

impl From<&TicketView> for ReadinessTicket {
    fn from(view: &TicketView) -> Self {
        Self {
            status: view.ticket.status,
            assignee: view.ticket.assignee.clone(),
            needs_planning: view.ticket.needs_planning,
            labels: view.ticket.labels.clone(),
            plan: view.ticket.plan.clone(),
            handbook: view.ticket.handbook.clone(),
            acceptance: view.ticket.acceptance.clone(),
            depends_open: view.depends_open.clone(),
            depends_unmerged: view.depends_unmerged.clone(),
            waiting_on_questions: view
                .waiting_on_questions
                .iter()
                .map(|question| question.id.clone())
                .collect(),
        }
    }
}

/// Das Board, soweit die Bereitschaft es braucht
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessBoard {
    pub owner_id: String,
    pub repo_path: Option<String>,
}

/// Prüft, ob ein Ticket vom Dispatcher genommen werden darf.
#[derive(Debug, Clone)]
pub struct DispatchReadiness<'a> {
    cfg: &'a DispatchConfig,
    chooser: ProfileChooser<'a>,
}

impl<'a> DispatchReadiness<'a> {
    /// Bereitschaftsprüfung für diese Einstellung
    pub fn new(cfg: &'a DispatchConfig) -> Self {
        Self {
            cfg,
            chooser: ProfileChooser::new(cfg),
        }
    }

    /// Erster fehlender Punkt in fester Reihenfolge; `repo_exists` prüft den Repo-Pfad des Boards
    pub fn reason(
        &self,
        ticket: &ReadinessTicket,
        board: &ReadinessBoard,
        user_id: &str,
        repo_exists: &dyn Fn(&str) -> bool,
    ) -> Option<String> {
        if ticket.status != TicketStatus::Planned {
            return Some("Status ist nicht Geplant".to_string());
        }
        if let Some(assignee) = &ticket.assignee {
            return Some(format!("bereits {assignee} zugewiesen"));
        }
        if ticket.needs_planning {
            return Some("Planungs-Flag ist noch gesetzt".to_string());
        }
        if let Some(label) = &self.cfg.opt_out_label {
            if ticket.labels.contains(label) {
                return Some(format!("Label „{label}“"));
            }
        }
        if let Some(label) = &self.cfg.opt_in_label {
            if !ticket.labels.contains(label) {
                return Some(format!("Label „{label}“ fehlt"));
            }
        }
        if ticket.plan.trim().is_empty() {
            return Some("Kein Plan".to_string());
        }
        if ticket.handbook.trim().is_empty() {
            return Some("Kein Handbuch".to_string());
        }
        if ticket.acceptance.trim().is_empty() {
            return Some("Keine Abnahme".to_string());
        }
        if !ticket.depends_open.is_empty() {
            return Some(format!("Wartet auf {}", ticket.depends_open.join(", ")));
        }
        // Erledigt reicht nicht: erst gemergt (kein Branch mehr) steckt der Code der
        // Voraussetzung im neuen Branch
        if !ticket.depends_unmerged.is_empty() {
            return Some(format!(
                "wartet auf Merge von {}",
                ticket.depends_unmerged.join(", ")
            ));
        }
        if !ticket.waiting_on_questions.is_empty() {
            return Some(format!(
                "Wartet auf Frage {}",
                ticket.waiting_on_questions.join(", ")
            ));
        }
        if let Some(problem) = self.chooser.label_problem(&ticket.labels) {
            return Some(problem);
        }
        if board.owner_id != user_id {
            return Some("Board gehört nicht dir".to_string());
        }
        let Some(repo_path) = board.repo_path.as_deref().filter(|path| !path.is_empty()) else {
            return Some(
                "Board hat keinen Repo-Pfad (harness board boards repo <id> <pfad>)".to_string(),
            );
        };
        if !repo_exists(repo_path) {
            return Some(format!("Repo-Pfad {repo_path} gibt es nicht"));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;

    const ME: &str = "11111111-1111-1111-1111-111111111111";

    /// Die aufgelöste Beispiel-Einstellung
    fn example() -> DispatchConfig {
        let normalizer = DispatchConfigNormalizer::new();
        normalizer.normalize(&normalizer.example()).unwrap()
    }

    /// Bereites Ticket; Felder lassen sich danach überschreiben
    fn ready() -> ReadinessTicket {
        ReadinessTicket {
            status: TicketStatus::Planned,
            assignee: None,
            needs_planning: false,
            labels: vec!["auto".to_string()],
            plan: "Plan".to_string(),
            handbook: "Hb".to_string(),
            acceptance: "Ok".to_string(),
            depends_open: Vec::new(),
            depends_unmerged: Vec::new(),
            waiting_on_questions: Vec::new(),
        }
    }

    /// Board mit Besitzer und Repo-Pfad
    fn board() -> ReadinessBoard {
        ReadinessBoard {
            owner_id: ME.to_string(),
            repo_path: Some("C:/repo".to_string()),
        }
    }

    /// Texte der Labels
    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn nimmt_ein_vollstandiges_ticket_mit_label_auto() {
        let cfg = example();
        assert_eq!(
            DispatchReadiness::new(&cfg).reason(&ready(), &board(), ME, &|_| true),
            None
        );
    }

    #[test]
    fn nennt_den_ersten_fehlenden_punkt() {
        let cfg = example();
        let readiness = DispatchReadiness::new(&cfg);
        /// Eine Änderung am bereiten Ticket samt erwartetem Grund
        type Case = (Box<dyn Fn(&mut ReadinessTicket)>, &'static str);
        let cases: Vec<Case> = vec![
            (
                Box::new(|t: &mut ReadinessTicket| t.status = TicketStatus::Backlog),
                "Status ist nicht Geplant",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.assignee = Some("claude".to_string())),
                "bereits claude zugewiesen",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.needs_planning = true),
                "Planungs-Flag ist noch gesetzt",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.labels = labels(&["auto", "manuell"])),
                "Label „manuell“",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.labels = Vec::new()),
                "Label „auto“ fehlt",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.plan = " ".to_string()),
                "Kein Plan",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.handbook = String::new()),
                "Kein Handbuch",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.acceptance = String::new()),
                "Keine Abnahme",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.depends_open = labels(&["ARC-0002"])),
                "Wartet auf ARC-0002",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.depends_unmerged = labels(&["ARC-0002"])),
                "wartet auf Merge von ARC-0002",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| {
                    t.depends_unmerged = labels(&["ARC-0002", "ARC-0003"])
                }),
                "wartet auf Merge von ARC-0002, ARC-0003",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.waiting_on_questions = labels(&["ARC-F0001"])),
                "Wartet auf Frage ARC-F0001",
            ),
            (
                Box::new(|t: &mut ReadinessTicket| t.labels = labels(&["auto", "profil:gross"])),
                "Label profil:gross – dieses Profil gibt es nicht (klein, standard, schwer)",
            ),
        ];
        for (change, expected) in cases {
            let mut ticket = ready();
            change(&mut ticket);
            assert_eq!(
                readiness
                    .reason(&ticket, &board(), ME, &|_| true)
                    .as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn pruft_besitzer_und_repo_pfad_des_boards() {
        let cfg = example();
        let readiness = DispatchReadiness::new(&cfg);
        let fremd = ReadinessBoard {
            owner_id: "x".to_string(),
            ..board()
        };
        assert_eq!(
            readiness.reason(&ready(), &fremd, ME, &|_| true).as_deref(),
            Some("Board gehört nicht dir")
        );
        let ohne_pfad = ReadinessBoard {
            repo_path: None,
            ..board()
        };
        assert_eq!(
            readiness
                .reason(&ready(), &ohne_pfad, ME, &|_| true)
                .as_deref(),
            Some("Board hat keinen Repo-Pfad (harness board boards repo <id> <pfad>)")
        );
        assert_eq!(
            readiness
                .reason(&ready(), &board(), ME, &|_| false)
                .as_deref(),
            Some("Repo-Pfad C:/repo gibt es nicht")
        );
    }

    #[test]
    fn ohne_opt_in_label_zahlt_jedes_ticket() {
        let normalizer = DispatchConfigNormalizer::new();
        let mut raw = normalizer.example();
        raw["optInLabel"] = json!(null);
        let cfg = normalizer.normalize(&raw).unwrap();
        let ohne_labels = ReadinessTicket {
            labels: Vec::new(),
            ..ready()
        };
        assert_eq!(
            DispatchReadiness::new(&cfg).reason(&ohne_labels, &board(), ME, &|_| true),
            None
        );
    }
}
