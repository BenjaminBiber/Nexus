//! Prüft eine Recherche und baut sie samt fortlaufend nummerierten Optionen (`O1`, `O2`, …).
//! Portiert aus `packages/core/src/questions/ResearchValidator.ts`.

use serde::{Deserialize, Serialize};

use crate::actors::Actor;
use crate::error::{HarnessError, Result};
use crate::questions::question::{QuestionOption, Research};

/// Höchstzahl der Optionen je Recherche
pub const MAX_OPTIONS: usize = 5;

/// Eine Option, wie sie hereinkommt: ohne Nummer, die vergibt das System
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchOptionInput {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub pros: Option<Vec<String>>,
    #[serde(default)]
    pub cons: Option<Vec<String>>,
    #[serde(default)]
    pub recommended: Option<bool>,
}

impl ResearchOptionInput {
    /// Eine Option mit Titel und Text (für Tests und Aufrufer, die nichts weiter setzen)
    pub fn new(title: &str, text: &str) -> Self {
        Self {
            title: Some(title.to_string()),
            text: Some(text.to_string()),
            ..Self::default()
        }
    }
}

/// Eingabe einer Recherche: Kontext und/oder Optionen
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchInput {
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub options: Option<Vec<ResearchOptionInput>>,
}

impl ResearchInput {
    /// Eine Recherche nur mit Kontext
    pub fn context(text: &str) -> Self {
        Self {
            context: Some(text.to_string()),
            ..Self::default()
        }
    }

    /// Eine Recherche nur mit Optionen
    pub fn options(options: Vec<ResearchOptionInput>) -> Self {
        Self {
            options: Some(options),
            ..Self::default()
        }
    }
}

/// Prüft und baut Recherchen.
pub struct ResearchValidator;

impl ResearchValidator {
    /// Liefert die Recherche; `existing` bestimmt, bei welcher Nummer die Optionen weiterzählen
    pub fn build(
        input: &ResearchInput,
        existing: &[Research],
        actor: &Actor,
        at: &str,
    ) -> Result<Research> {
        let context = input.context.as_deref().unwrap_or("").trim().to_string();
        let raw = input.options.as_deref().unwrap_or(&[]);
        if context.is_empty() && raw.is_empty() {
            return Err(HarnessError::new(
                "research-invalid",
                "Eine Recherche braucht Kontext oder mindestens eine Option.",
            )
            .into());
        }
        if raw.len() > MAX_OPTIONS {
            return Err(HarnessError::new(
                "research-invalid",
                format!("Eine Recherche hat höchstens {MAX_OPTIONS} Optionen."),
            )
            .into());
        }
        if raw
            .iter()
            .filter(|option| option.recommended == Some(true))
            .count()
            > 1
        {
            return Err(HarnessError::new(
                "research-recommended-multiple",
                "Höchstens eine Option je Recherche darf empfohlen sein.",
            )
            .into());
        }
        let mut next = Self::highest_number(existing);
        let mut options = Vec::with_capacity(raw.len());
        for option in raw {
            let title = option.title.as_deref().unwrap_or("").trim().to_string();
            let text = option.text.as_deref().unwrap_or("").trim().to_string();
            if title.is_empty() || text.is_empty() {
                return Err(HarnessError::new(
                    "research-invalid",
                    "Jede Option braucht einen Titel und einen Text.",
                )
                .into());
            }
            next += 1;
            options.push(QuestionOption {
                id: format!("O{next}"),
                title,
                text,
                pros: Self::texts(option.pros.as_deref()),
                cons: Self::texts(option.cons.as_deref()),
                recommended: option.recommended == Some(true),
            });
        }
        Ok(Research {
            at: at.to_string(),
            author: actor.author.clone(),
            kind: actor.kind,
            context,
            options,
        })
    }

    /// Text für den Verlauf, z. B. „3 Optionen (O1 bis O3), O2 empfohlen“; ohne Optionen
    /// „Kontext ergänzt“
    pub fn describe(options: &[QuestionOption]) -> String {
        if options.is_empty() {
            return "Kontext ergänzt".to_string();
        }
        let first = options[0].id.as_str();
        let last = options[options.len() - 1].id.as_str();
        let count = if options.len() == 1 {
            "1 Option".to_string()
        } else {
            format!("{} Optionen", options.len())
        };
        let range = if options.len() == 1 {
            format!("({first})")
        } else {
            format!("({first} bis {last})")
        };
        let recommended = options
            .iter()
            .find(|option| option.recommended)
            .map(|option| format!(", {} empfohlen", option.id))
            .unwrap_or_default();
        format!("{count} {range}{recommended}")
    }

    /// Höchste bisher vergebene Optionsnummer über alle Recherchen
    fn highest_number(existing: &[Research]) -> usize {
        existing
            .iter()
            .flat_map(|research| research.options.iter())
            .filter_map(|option| option.id.get(1..).and_then(|n| n.parse::<usize>().ok()))
            .max()
            .unwrap_or(0)
    }

    /// Bereinigt eine Liste von Texten
    fn texts(items: Option<&[String]>) -> Vec<String> {
        items
            .unwrap_or(&[])
            .iter()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::AuthorKind;

    /// Der Handelnde der Tests (`actor` in `QuestionResearch.test.ts`)
    fn actor() -> Actor {
        Actor {
            author: "Claude".into(),
            kind: AuthorKind::Session,
        }
    }

    /// Eine Option mit Titel, Text und je einem Vor- und Nachteil (`option` in TypeScript)
    fn option(title: &str) -> ResearchOptionInput {
        ResearchOptionInput {
            title: Some(title.to_string()),
            text: Some(format!("{title} Text")),
            pros: Some(vec!["a".into()]),
            cons: Some(vec!["b".into()]),
            recommended: None,
        }
    }

    /// Dieselbe Option, aber empfohlen
    fn recommended(title: &str) -> ResearchOptionInput {
        ResearchOptionInput {
            recommended: Some(true),
            ..option(title)
        }
    }

    #[test]
    fn vergibt_optionsnummern_ab_o1_und_ubernimmt_vor_und_nachteile() {
        let research = ResearchValidator::build(
            &ResearchInput {
                context: Some("Kontext".into()),
                options: Some(vec![option("A"), recommended("B")]),
            },
            &[],
            &actor(),
            "2026-10-10T10:00:00.000Z",
        )
        .unwrap();

        assert_eq!(
            research
                .options
                .iter()
                .map(|option| option.id.as_str())
                .collect::<Vec<_>>(),
            ["O1", "O2"]
        );
        assert_eq!(research.options[1].title, "B");
        assert_eq!(research.options[1].pros, ["a"]);
        assert_eq!(research.options[1].cons, ["b"]);
        assert!(research.options[1].recommended);
        assert_eq!(research.at, "2026-10-10T10:00:00.000Z");
        assert_eq!(research.author, "Claude");
        assert_eq!(research.kind, AuthorKind::Session);
        assert_eq!(research.context, "Kontext");
    }

    #[test]
    fn nummeriert_uber_recherchen_hinweg_fortlaufend_weiter() {
        let first = ResearchValidator::build(
            &ResearchInput::options(vec![option("A"), option("B")]),
            &[],
            &actor(),
            "t1",
        )
        .unwrap();
        let second = ResearchValidator::build(
            &ResearchInput::options(vec![option("C")]),
            &[first],
            &actor(),
            "t2",
        )
        .unwrap();

        assert_eq!(
            second
                .options
                .iter()
                .map(|option| option.id.as_str())
                .collect::<Vec<_>>(),
            ["O3"]
        );
    }

    #[test]
    fn lasst_vor_und_nachteile_weg_wenn_sie_fehlen() {
        let research = ResearchValidator::build(
            &ResearchInput::options(vec![ResearchOptionInput::new("A", "x")]),
            &[],
            &actor(),
            "t",
        )
        .unwrap();

        assert!(research.options[0].pros.is_empty());
        assert!(research.options[0].cons.is_empty());
        assert!(!research.options[0].recommended);
    }

    #[test]
    fn lehnt_mehr_als_eine_empfohlene_option_ab() {
        let error = ResearchValidator::build(
            &ResearchInput::options(vec![recommended("A"), recommended("B")]),
            &[],
            &actor(),
            "t",
        )
        .unwrap_err();

        assert_eq!(error.code(), Some("research-recommended-multiple"));
    }

    #[test]
    fn lehnt_mehr_als_funf_optionen_ab() {
        let six: Vec<ResearchOptionInput> = (0..6).map(|i| option(&format!("O{i}"))).collect();
        let error =
            ResearchValidator::build(&ResearchInput::options(six), &[], &actor(), "t").unwrap_err();

        assert_eq!(error.code(), Some("research-invalid"));
    }

    #[test]
    fn lehnt_eine_recherche_ohne_kontext_und_ohne_optionen_ab() {
        assert_eq!(
            ResearchValidator::build(&ResearchInput::default(), &[], &actor(), "t")
                .unwrap_err()
                .code(),
            Some("research-invalid")
        );
        assert_eq!(
            ResearchValidator::build(
                &ResearchInput {
                    context: Some("   ".into()),
                    options: Some(Vec::new()),
                },
                &[],
                &actor(),
                "t"
            )
            .unwrap_err()
            .code(),
            Some("research-invalid")
        );
    }

    #[test]
    fn lehnt_optionen_ohne_titel_oder_text_ab() {
        assert_eq!(
            ResearchValidator::build(
                &ResearchInput::options(vec![ResearchOptionInput::new("", "x")]),
                &[],
                &actor(),
                "t"
            )
            .unwrap_err()
            .code(),
            Some("research-invalid")
        );
        assert_eq!(
            ResearchValidator::build(
                &ResearchInput::options(vec![ResearchOptionInput::new("A", " ")]),
                &[],
                &actor(),
                "t"
            )
            .unwrap_err()
            .code(),
            Some("research-invalid")
        );
    }

    #[test]
    fn lasst_recherche_nur_mit_kontext_zu() {
        let research =
            ResearchValidator::build(&ResearchInput::context("Nur Kontext"), &[], &actor(), "t")
                .unwrap();

        assert!(research.options.is_empty());
    }

    /// Aus `ResearchDescribe.test.ts`
    #[test]
    fn beschreibt_optionen_mit_bereich_und_empfehlung() {
        let built = |ids: &[(&str, bool)]| -> Vec<QuestionOption> {
            ids.iter()
                .map(|(id, recommended)| QuestionOption {
                    id: (*id).to_string(),
                    title: if *id == "O1" { "Datei" } else { id }.to_string(),
                    text: "t".into(),
                    pros: Vec::new(),
                    cons: Vec::new(),
                    recommended: *recommended,
                })
                .collect()
        };

        assert_eq!(
            ResearchValidator::describe(&built(&[("O1", false), ("O2", true), ("O3", false)])),
            "3 Optionen (O1 bis O3), O2 empfohlen"
        );
        assert_eq!(
            ResearchValidator::describe(&built(&[("O4", false)])),
            "1 Option (O4)"
        );
        assert_eq!(ResearchValidator::describe(&[]), "Kontext ergänzt");
    }
}
