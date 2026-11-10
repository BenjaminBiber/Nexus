//! Die Werte, mit denen die Arbeitsanweisungen gefüllt werden; portiert aus
//! `packages/server/src/dispatch/PromptValues.ts`.
//!
//! Lauf und Vorschau holen sie hier, damit die Vorschau genau das zeigt, was der Agent später
//! bekommt. Die Eingaben der Vorlagen ([`WorkPromptInput`] und Geschwister) leihen sich in
//! Rust ihre Texte; deshalb liefern die Methoden eigene Werte-Typen, aus denen `input(cfg)`
//! die geliehene Eingabe baut.

use nexus_core::board::question_service::BoardQuestionListFilter;
use nexus_core::board::ticket_service::TicketDetail;
use nexus_core::dispatch::{
    AnsweredQuestion, DispatchConfig, MergePromptInput, PlanPromptInput, ResearchPromptInput,
    ReviewFeedback, WorkPromptInput,
};
use nexus_core::questions::state::QuestionState;
use nexus_core::Result;

use crate::dispatch::context::DispatchContext;

/// Zielbranch, wenn keiner bekannt ist – die Vorschau fasst kein Repo an
pub const DEFAULT_TARGET: &str = "main";

/// Name des Planungs-Agents eines Tickets (`PlanRunner.agentName`; der Planungslauf selbst
/// kommt in P4-14b)
pub fn plan_agent_name(ticket_id: &str) -> String {
    format!(
        "{}{ticket_id}",
        nexus_core::dispatch::autopilot::PLAN_AGENT_PREFIX
    )
}

/// Name des Recherche-Agents einer Frage (`ResearchRunner.agentName`; der Recherche-Lauf
/// selbst kommt in P4-14b)
pub fn research_agent_name(question_id: &str) -> String {
    format!("research-{question_id}")
}

/// Werte eines Umsetzungslaufs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkValues {
    pub ticket_id: String,
    pub title: String,
    pub agent: String,
    pub branch: String,
    pub answers: Vec<AnsweredQuestion>,
    pub feedback: Vec<String>,
}

impl WorkValues {
    /// Eingabe der Vorlage auf diesen Werten
    pub fn input<'a>(&'a self, cfg: &'a DispatchConfig) -> WorkPromptInput<'a> {
        WorkPromptInput {
            ticket_id: &self.ticket_id,
            title: &self.title,
            agent: &self.agent,
            branch: &self.branch,
            answers: &self.answers,
            feedback: &self.feedback,
            cfg,
        }
    }
}

/// Werte eines Planungslaufs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanValues {
    pub ticket_id: String,
    pub title: String,
    pub agent: String,
}

impl PlanValues {
    /// Eingabe der Vorlage auf diesen Werten
    pub fn input<'a>(&'a self, cfg: &'a DispatchConfig) -> PlanPromptInput<'a> {
        PlanPromptInput {
            ticket_id: &self.ticket_id,
            title: &self.title,
            agent: &self.agent,
            cfg,
        }
    }
}

/// Werte eines Merge-Laufs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeValues {
    pub ticket_id: String,
    pub title: String,
    pub agent: String,
    pub branch: String,
    pub target: String,
}

impl MergeValues {
    /// Eingabe der Vorlage auf diesen Werten
    pub fn input<'a>(&'a self, cfg: &'a DispatchConfig) -> MergePromptInput<'a> {
        MergePromptInput {
            ticket_id: &self.ticket_id,
            title: &self.title,
            agent: &self.agent,
            branch: &self.branch,
            target: &self.target,
            cfg,
        }
    }
}

/// Werte eines Recherche-Laufs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchValues {
    pub question_id: String,
    pub title: String,
    pub agent: String,
}

impl ResearchValues {
    /// Eingabe der Vorlage auf diesen Werten
    pub fn input<'a>(&'a self, cfg: &'a DispatchConfig) -> ResearchPromptInput<'a> {
        ResearchPromptInput {
            question_id: &self.question_id,
            title: &self.title,
            agent: &self.agent,
            cfg,
        }
    }
}

/// Beispielwerte aller vier Laufarten für die Vorschau
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleValues {
    pub work: WorkValues,
    pub plan: PlanValues,
    pub research: ResearchValues,
    pub merge: MergeValues,
}

/// Holt die Werte der Arbeitsanweisungen aus Board und Einstellung.
#[derive(Debug)]
pub struct PromptValues<'a> {
    ctx: &'a DispatchContext,
    feedback: ReviewFeedback,
}

impl<'a> PromptValues<'a> {
    /// Werte auf diesem Kontext
    pub fn new(ctx: &'a DispatchContext) -> Self {
        Self {
            ctx,
            feedback: ReviewFeedback::new(),
        }
    }

    /// Entschiedene Fragen zum Ticket für die Arbeitsanweisung
    pub async fn answers(&self, ticket_id: &str) -> Result<Vec<AnsweredQuestion>> {
        let key = match ticket_id.rfind('-') {
            Some(at) => &ticket_id[..at],
            None => "",
        };
        let filter = BoardQuestionListFilter {
            state: Some(QuestionState::Answered),
            ticket: Some(ticket_id.to_string()),
            ..BoardQuestionListFilter::default()
        };
        let list = self
            .ctx
            .services()
            .questions
            .list(key, &filter, None)
            .await?;
        Ok(list
            .questions
            .into_iter()
            .filter_map(|item| {
                let question = item.view.question;
                let decision = question.decision?;
                // Variante und Text, sonst nur „entschieden“ – wie `filter(Boolean).join(' – ')`
                let what = [decision.option.as_deref(), decision.text.as_deref()]
                    .into_iter()
                    .flatten()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" – ");
                let what = if what.is_empty() {
                    "entschieden".to_string()
                } else {
                    what
                };
                Some(AnsweredQuestion {
                    id: question.id,
                    title: question.title,
                    decision: format!("{what} (Begründung: {})", decision.reason),
                })
            })
            .collect())
    }

    /// Offene Rückmeldungen aus dem Review seit dem letzten Start
    pub fn pending<C: nexus_core::dispatch::CommentLike>(&self, comments: &[C]) -> Vec<String> {
        self.feedback
            .pending(comments, &self.ctx.cfg.dispatcher_name)
    }

    /// Werte eines Umsetzungslaufs zu einem echten Ticket
    pub async fn work(&self, detail: &TicketDetail) -> Result<WorkValues> {
        Ok(WorkValues {
            ticket_id: detail.ticket.id.clone(),
            title: detail.ticket.title.clone(),
            agent: self.ctx.agent_name(&detail.ticket.id),
            branch: detail
                .ticket
                .branch
                .clone()
                .unwrap_or_else(|| sample_branch(&detail.ticket.id)),
            answers: self.answers(&detail.ticket.id).await?,
            feedback: self.pending(&detail.comments),
        })
    }

    /// Werte eines Planungslaufs zu einem echten Ticket
    pub fn plan(&self, detail: &TicketDetail) -> PlanValues {
        PlanValues {
            ticket_id: detail.ticket.id.clone(),
            title: detail.ticket.title.clone(),
            agent: plan_agent_name(&detail.ticket.id),
        }
    }

    /// Werte eines Merge-Laufs zu einem echten Ticket; der Zielbranch bleibt in der Vorschau
    /// der Standard
    pub fn merge(&self, detail: &TicketDetail) -> MergeValues {
        MergeValues {
            ticket_id: detail.ticket.id.clone(),
            title: detail.ticket.title.clone(),
            agent: self.ctx.agent_name(&detail.ticket.id),
            branch: detail
                .ticket
                .branch
                .clone()
                .unwrap_or_else(|| sample_branch(&detail.ticket.id)),
            target: DEFAULT_TARGET.to_string(),
        }
    }

    /// Werte eines Recherche-Laufs zu einer echten Frage
    pub fn research(&self, question_id: &str, title: &str) -> ResearchValues {
        ResearchValues {
            question_id: question_id.to_string(),
            title: title.to_string(),
            agent: research_agent_name(question_id),
        }
    }
}

/// Branch, den ein ungestartetes Ticket in der Vorschau bekommt
fn sample_branch(ticket_id: &str) -> String {
    format!("ticket/{}", ticket_id.to_lowercase())
}

/// Beispielwerte ohne Board: eine Antwort und eine Review-Rückmeldung sind dabei, damit die
/// optionalen Blöcke der Vorlage in der Vorschau sichtbar werden.
pub fn sample(cfg: &DispatchConfig) -> SampleValues {
    let ticket_id = "AIH-0042";
    let title = "Beispielticket: Vorschau der Arbeitsanweisung";
    let agent = format!("{}{ticket_id}", cfg.agent_prefix);
    let branch = "ticket/AIH-0042-beispiel";
    SampleValues {
        work: WorkValues {
            ticket_id: ticket_id.into(),
            title: title.into(),
            agent: agent.clone(),
            branch: branch.into(),
            answers: vec![AnsweredQuestion {
                id: "AIH-F0007".into(),
                title: "Beispielfrage: Welche Variante?".into(),
                decision: "Variante B (Begründung: weniger Aufwand)".into(),
            }],
            feedback: vec![
                "Beispiel-Rückmeldung aus dem Review: Bitte noch einen Test für den Fehlerfall."
                    .into(),
            ],
        },
        plan: PlanValues {
            ticket_id: ticket_id.into(),
            title: title.into(),
            agent: plan_agent_name(ticket_id),
        },
        research: ResearchValues {
            question_id: "AIH-F0007".into(),
            title: "Beispielfrage: Welche Variante?".into(),
            agent: research_agent_name("AIH-F0007"),
        },
        merge: MergeValues {
            ticket_id: ticket_id.into(),
            title: title.into(),
            agent,
            branch: branch.into(),
            target: DEFAULT_TARGET.into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::dispatch::DispatchConfigNormalizer;

    #[test]
    fn die_namen_der_agenten_sind_wortgleich_zu_typescript() {
        assert_eq!(plan_agent_name("ARC-0007"), "planner-ARC-0007");
        assert_eq!(research_agent_name("ARC-F0001"), "research-ARC-F0001");
    }

    #[test]
    fn beispielwerte_zeigen_antwort_und_rueckmeldung() {
        let normalizer = DispatchConfigNormalizer::new();
        let cfg = normalizer
            .normalize(&normalizer.example())
            .expect("Beispiel-Einstellung");
        let sample = sample(&cfg);
        assert_eq!(sample.work.answers.len(), 1);
        assert_eq!(sample.work.feedback.len(), 1);
        assert_eq!(sample.work.agent, format!("{}AIH-0042", cfg.agent_prefix));
        assert_eq!(sample.merge.target, "main");
        assert_eq!(sample.plan.agent, "planner-AIH-0042");
    }
}
