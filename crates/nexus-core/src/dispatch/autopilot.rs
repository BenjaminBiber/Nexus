//! Einzige Entscheidungsstelle, ob der Autopilot für ein Ticket gilt, und die Regeln für
//! Fragen, die der Autopilot selbst entscheidet

use crate::actors::AuthorKind;
use crate::board::question_types::{BoardQuestion, BoardQuestionDecision};
use crate::dispatch::config::{DispatchConfig, AUTOPILOT_PREFIX};
use crate::questions::state::QuestionState;

/// Vorsilbe der Planungs-Agents des Dispatchers (`planner-<Ticket>`)
pub const PLAN_AGENT_PREFIX: &str = "planner-";

/// So viele Recherchen ohne Empfehlung versucht der Autopilot je Frage, bevor er die Tickets
/// blockiert
pub const AUTOPILOT_RESEARCH_ATTEMPTS: u32 = 2;

/// Gilt der Autopilot für ein Ticket mit diesen Labels? Schalter an und kein Ausnahme-Label
/// (Vergleich getrimmt, ohne Groß-/Kleinschreibung)
pub fn autopilot_for(cfg: &DispatchConfig, labels: &[String]) -> bool {
    if !cfg.autopilot.enabled {
        return false;
    }
    match cfg.autopilot.except_label.as_deref().map(str::trim) {
        Some(except) if !except.is_empty() => !labels
            .iter()
            .any(|label| label.trim().to_lowercase() == except.to_lowercase()),
        _ => true,
    }
}

/// Hat ein Agent des Dispatchers die Frage gestellt? Kein Mensch und Name mit `agentPrefix`
/// (Umsetzung) oder `planner-` (Planung); interaktive Sessions zählen nicht
pub fn asked_by_dispatch_agent(cfg: &DispatchConfig, kind: AuthorKind, name: &str) -> bool {
    if kind == AuthorKind::Human {
        return false;
    }
    let work = !cfg.agent_prefix.is_empty() && name.starts_with(&cfg.agent_prefix);
    work || name.starts_with(PLAN_AGENT_PREFIX)
}

/// Ist die Frage eine Autopilot-Frage? Offen, von einem Agent des Dispatchers gestellt und
/// mindestens ein betroffenes Ticket mit Autopilot (`ticket_labels`: die Labels je betroffenem
/// Ticket)
pub fn is_autopilot_question(
    cfg: &DispatchConfig,
    question: &BoardQuestion,
    ticket_labels: &[Vec<String>],
) -> bool {
    question.state == QuestionState::Open
        && asked_by_dispatch_agent(cfg, question.origin_kind, &question.origin_name)
        && ticket_labels
            .iter()
            .any(|labels| autopilot_for(cfg, labels))
}

/// Hat der Autopilot die Frage entschieden? Die Begründung beginnt mit [`AUTOPILOT_PREFIX`]
pub fn is_autopilot_decision(decision: &BoardQuestionDecision) -> bool {
    decision.reason.starts_with(AUTOPILOT_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::question_types::BoardQuestionDecisionOutcome;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;
    use crate::testing::question_fixture;
    use serde_json::json;

    fn cfg(enabled: bool) -> DispatchConfig {
        DispatchConfigNormalizer::new()
            .normalize(&json!({ "autopilot": { "enabled": enabled } }))
            .unwrap()
    }

    fn labels(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    fn question(kind: AuthorKind, name: &str) -> BoardQuestion {
        BoardQuestion {
            origin_kind: kind,
            origin_name: name.into(),
            ..question_fixture("ARC-F0001", 1, 1)
        }
    }

    fn decision(reason: &str) -> BoardQuestionDecision {
        BoardQuestionDecision {
            outcome: BoardQuestionDecisionOutcome::Answered,
            option: Some("O1".into()),
            text: None,
            reason: reason.into(),
            by_id: "u".into(),
            by_name: "dispatcher".into(),
            at: "jetzt".into(),
        }
    }

    #[test]
    fn aus_gilt_nie() {
        assert!(!autopilot_for(&cfg(false), &labels(&["auto"])));
    }

    #[test]
    fn an_ohne_ausnahme_label_gilt() {
        assert!(autopilot_for(&cfg(true), &labels(&[])));
        assert!(autopilot_for(&cfg(true), &labels(&["auto", "backend"])));
    }

    #[test]
    fn ausnahme_label_in_anderer_schreibweise_gilt_nicht() {
        assert!(!autopilot_for(
            &cfg(true),
            &labels(&["auto", " Interaktiv "])
        ));
    }

    #[test]
    fn ohne_ausnahme_label_gilt_immer() {
        let mut cfg = cfg(true);
        cfg.autopilot.except_label = None;
        assert!(autopilot_for(&cfg, &labels(&["interaktiv"])));
    }

    #[test]
    fn fragen_von_umsetzungs_und_planungs_agents_sind_autopilot_fragen() {
        let cfg = cfg(true);
        let tickets = vec![labels(&["auto"])];
        for name in ["junior-ARC-0001", "planner-ARC-0001"] {
            assert!(
                is_autopilot_question(&cfg, &question(AuthorKind::Session, name), &tickets),
                "{name}"
            );
        }
    }

    #[test]
    fn fragen_von_menschen_und_interaktiven_sessions_bleiben_beim_menschen() {
        let cfg = cfg(true);
        let tickets = vec![labels(&["auto"])];
        assert!(!is_autopilot_question(
            &cfg,
            &question(AuthorKind::Human, "junior-ARC-0001"),
            &tickets
        ));
        for name in ["claude", "research-ARC-F0001", "juniorARC-0001"] {
            assert!(
                !is_autopilot_question(&cfg, &question(AuthorKind::Session, name), &tickets),
                "{name}"
            );
        }
    }

    #[test]
    fn autopilot_frage_braucht_offene_frage_und_ein_ticket_mit_autopilot() {
        let cfg = cfg(true);
        let agent = question(AuthorKind::Session, "junior-ARC-0001");
        assert!(!is_autopilot_question(&cfg, &agent, &[]));
        assert!(!is_autopilot_question(
            &cfg,
            &agent,
            &[labels(&["interaktiv"])]
        ));
        assert!(is_autopilot_question(
            &cfg,
            &agent,
            &[labels(&["interaktiv"]), labels(&[])]
        ));
        let answered = BoardQuestion {
            state: QuestionState::Answered,
            ..agent.clone()
        };
        assert!(!is_autopilot_question(&cfg, &answered, &[labels(&[])]));
        assert!(!is_autopilot_question(
            &self::cfg(false),
            &agent,
            &[labels(&[])]
        ));
    }

    #[test]
    fn ohne_agent_prefix_zaehlen_nur_planungs_agents() {
        let mut cfg = cfg(true);
        cfg.agent_prefix = String::new();
        assert!(!asked_by_dispatch_agent(
            &cfg,
            AuthorKind::Session,
            "claude"
        ));
        assert!(asked_by_dispatch_agent(
            &cfg,
            AuthorKind::Session,
            "planner-ARC-0001"
        ));
    }

    #[test]
    fn autopilot_entscheidung_an_der_vorsilbe_der_begruendung() {
        assert!(is_autopilot_decision(&decision(
            "Autopilot: Empfehlung der Recherche übernommen"
        )));
        assert!(!is_autopilot_decision(&decision("passt so")));
        assert!(!is_autopilot_decision(&decision("")));
    }
}
