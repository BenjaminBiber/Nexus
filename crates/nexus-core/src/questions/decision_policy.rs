//! Regeln für Entscheiden, Verwerfen und Wiedereröffnen: nur Menschen, nur offene Fragen;
//! Verwerfen nur mit Begründung.
//! Portiert aus `packages/core/src/questions/DecisionPolicy.ts`.

use serde::{Deserialize, Serialize};

use crate::actors::{Actor, AuthorKind};
use crate::error::{HarnessError, Result};
use crate::questions::question::{Decision, DecisionOutcome, Research};
use crate::questions::state::QuestionState;

/// Eingabe einer Entscheidung: genau eines von Option und Freitext, dazu wahlweise eine
/// Begründung
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionInput {
    #[serde(default)]
    pub option: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

impl DecisionInput {
    /// Eine Entscheidung für eine Option der neuesten Recherche
    pub fn option(id: &str, reason: &str) -> Self {
        Self {
            option: Some(id.to_string()),
            reason: Some(reason.to_string()),
            ..Self::default()
        }
    }

    /// Eine Entscheidung mit eigenem Text
    pub fn text(text: &str, reason: &str) -> Self {
        Self {
            text: Some(text.to_string()),
            reason: Some(reason.to_string()),
            ..Self::default()
        }
    }
}

/// Was die Regeln von einer Frage brauchen: eine Datei-Frage oder eine Frage der
/// Board-Datenbank (deren Recherchen im Datei-Format übergeben werden)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionSubject {
    pub id: String,
    pub state: QuestionState,
    pub research: Vec<Research>,
}

/// Regeln der Entscheidung.
pub struct DecisionPolicy;

impl DecisionPolicy {
    /// Nur ein Mensch darf entscheiden, verwerfen oder wieder öffnen
    pub fn assert_human(actor: &Actor) -> Result<()> {
        if actor.kind != AuthorKind::Human {
            return Err(HarnessError::new(
                "human-only",
                "Diese Entscheidung trifft ein Mensch. Bitte in der Oberfläche oder in einem eigenen Terminal ausführen.",
            )
            .into());
        }
        Ok(())
    }

    /// Prüft eine Entscheidung und liefert sie; die Frage muss offen sein
    pub fn answer(
        question: &DecisionSubject,
        input: &DecisionInput,
        actor: &Actor,
        at: &str,
    ) -> Result<Decision> {
        Self::assert_human(actor)?;
        Self::assert_open(&question.id, question.state)?;
        let option = Self::trimmed(input.option.as_deref());
        let text = Self::trimmed(input.text.as_deref());
        if option.is_none() == text.is_none() {
            return Err(HarnessError::new(
                "decision-ambiguous",
                "Bitte entweder eine Option wählen oder einen eigenen Text eingeben, nicht beides und nicht keines.",
            )
            .into());
        }
        // Beim Entscheiden ist die Begründung freiwillig (nur Verwerfen verlangt eine)
        let reason = input.reason.as_deref().unwrap_or("").trim().to_string();
        if let Some(option) = option.as_deref() {
            Self::assert_option(&question.research, option)?;
        }
        Ok(Decision {
            outcome: DecisionOutcome::Answered,
            option,
            text,
            reason,
            by: actor.author.clone(),
            at: at.to_string(),
        })
    }

    /// Prüft das Verwerfen und liefert die Entscheidung; die Begründung ist Pflicht
    pub fn discard(
        id: &str,
        state: QuestionState,
        reason: Option<&str>,
        actor: &Actor,
        at: &str,
    ) -> Result<Decision> {
        Self::assert_human(actor)?;
        Self::assert_open(id, state)?;
        Ok(Decision {
            outcome: DecisionOutcome::Discarded,
            option: None,
            text: None,
            reason: Self::require_reason(reason)?,
            by: actor.author.clone(),
            at: at.to_string(),
        })
    }

    /// Prüft, dass die Frage wieder geöffnet werden darf (entschieden oder verworfen)
    pub fn assert_reopenable(id: &str, state: QuestionState, actor: &Actor) -> Result<()> {
        Self::assert_human(actor)?;
        if state == QuestionState::Open {
            return Err(HarnessError::new(
                "question-state-invalid",
                format!("Die Frage {id} ist bereits offen."),
            )
            .into());
        }
        Ok(())
    }

    /// Beschreibt eine Entscheidung für den Verlauf, z. B. „Option O2 „ExcelJS“ gewählt.
    /// Begründung: …“
    pub fn describe(research: &[Research], decision: &Decision) -> String {
        if decision.outcome == DecisionOutcome::Discarded {
            return format!("Verworfen. Begründung: {}", decision.reason);
        }
        let because = if decision.reason.is_empty() {
            String::new()
        } else {
            format!(" Begründung: {}", decision.reason)
        };
        match decision.option.as_deref() {
            Some(option) => {
                let title = Self::option_title(research, option)
                    .map(|title| format!(" „{title}“"))
                    .unwrap_or_default();
                format!("Option {option}{title} gewählt.{because}")
            }
            None => format!(
                "Antwort: {}.{because}",
                decision.text.as_deref().unwrap_or("")
            ),
        }
    }

    /// Titel einer Option aus den Recherchen (neueste zuerst)
    fn option_title(research: &[Research], id: &str) -> Option<String> {
        research
            .iter()
            .rev()
            .flat_map(|entry| entry.options.iter())
            .find(|option| option.id == id)
            .map(|option| option.title.clone())
    }

    /// Die Frage muss offen sein
    fn assert_open(id: &str, state: QuestionState) -> Result<()> {
        if state != QuestionState::Open {
            return Err(HarnessError::new(
                "question-state-invalid",
                format!("Die Frage {id} ist nicht offen. Zuerst wieder öffnen."),
            )
            .into());
        }
        Ok(())
    }

    /// Gewählt werden darf nur eine Option der neuesten Recherche
    fn assert_option(research: &[Research], id: &str) -> Result<()> {
        let known = research
            .last()
            .is_some_and(|newest| newest.options.iter().any(|option| option.id == id));
        if !known {
            return Err(HarnessError::new(
                "decision-option-unknown",
                format!("Die Option {id} gibt es in der neuesten Recherche nicht."),
            )
            .into());
        }
        Ok(())
    }

    /// Begründung, die Pflicht ist (Verwerfen)
    fn require_reason(value: Option<&str>) -> Result<String> {
        let reason = value.unwrap_or("").trim();
        if reason.is_empty() {
            return Err(HarnessError::new(
                "decision-reason-missing",
                "Bitte eine Begründung angeben.",
            )
            .into());
        }
        Ok(reason.to_string())
    }

    /// Text ohne Leerraum am Rand; leer wird zu `None`
    fn trimmed(value: Option<&str>) -> Option<String> {
        value
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::question::QuestionOption;

    /// Die schlanke Frage aus `ResearchDescribe.test.ts`: nur Nummer, Zustand und Recherchen
    fn subject() -> DecisionSubject {
        DecisionSubject {
            id: "ARC-F0001".into(),
            state: QuestionState::Open,
            research: vec![Research {
                at: "x".into(),
                author: "claude".into(),
                kind: AuthorKind::Session,
                context: String::new(),
                options: vec![QuestionOption {
                    id: "O1".into(),
                    title: "Datei".into(),
                    text: "t".into(),
                    pros: Vec::new(),
                    cons: Vec::new(),
                    recommended: false,
                }],
            }],
        }
    }

    fn human() -> Actor {
        Actor {
            author: "Florian".into(),
            kind: AuthorKind::Human,
        }
    }

    #[test]
    fn braucht_nur_nummer_zustand_und_recherchen() {
        let subject = subject();
        let decision = DecisionPolicy::answer(
            &subject,
            &DecisionInput::option("O1", "passt"),
            &human(),
            "jetzt",
        )
        .unwrap();

        assert_eq!(
            DecisionPolicy::describe(&subject.research, &decision),
            "Option O1 „Datei“ gewählt. Begründung: passt"
        );
    }

    #[test]
    fn nur_menschen_entscheiden() {
        let agent = Actor {
            author: "claude".into(),
            kind: AuthorKind::Session,
        };
        let error = DecisionPolicy::answer(
            &subject(),
            &DecisionInput::option("O1", "x"),
            &agent,
            "jetzt",
        )
        .unwrap_err();

        assert_eq!(error.code(), Some("human-only"));
    }
}
