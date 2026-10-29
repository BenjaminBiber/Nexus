//! Fachobjekte einer Frage im Datei-Format: Bezüge, Herkunft, Optionen, Recherchen,
//! Entscheidung und Verlauf. Portiert aus `packages/core/src/questions/Question.ts`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::actors::AuthorKind;
use crate::questions::state::QuestionState;

/// Arten von Bezügen einer Frage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkType {
    Ticket,
    Spec,
    SpecTask,
    File,
}

impl LinkType {
    /// Alle Arten in der Reihenfolge von TypeScript (`LINK_TYPES`)
    pub const ALL: [LinkType; 4] = [
        LinkType::Ticket,
        LinkType::Spec,
        LinkType::SpecTask,
        LinkType::File,
    ];

    /// Schreibweise in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkType::Ticket => "ticket",
            LinkType::Spec => "spec",
            LinkType::SpecTask => "spec-task",
            LinkType::File => "file",
        }
    }
}

impl fmt::Display for LinkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ein Bezug auf ein Ticket, eine Spec, eine Spec-Aufgabe oder eine Datei im Code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionLink {
    #[serde(rename = "type")]
    pub link_type: LinkType,
    pub target: String,
}

/// Wer die Frage angelegt hat; bei Sessions mit Kennung der Session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionOrigin {
    pub kind: AuthorKind,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// Ein Lösungsvorschlag einer Recherche; Nummer fortlaufend je Frage (`O1`, `O2`, …)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionOption {
    pub id: String,
    pub title: String,
    pub text: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub recommended: bool,
}

/// Ein datierter Eintrag eines Agenten mit Kontext und Optionen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Research {
    pub at: String,
    pub author: String,
    pub kind: AuthorKind,
    pub context: String,
    pub options: Vec<QuestionOption>,
}

/// Ausgang einer Entscheidung: beantwortet oder verworfen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOutcome {
    Answered,
    Discarded,
}

impl DecisionOutcome {
    /// Schreibweise in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            DecisionOutcome::Answered => "answered",
            DecisionOutcome::Discarded => "discarded",
        }
    }
}

impl fmt::Display for DecisionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Die vom Menschen getroffene Entscheidung (oder das Verwerfen) mit Begründung
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub outcome: DecisionOutcome,
    pub option: Option<String>,
    pub text: Option<String>,
    pub reason: String,
    pub by: String,
    pub at: String,
}

/// Arten von Verlaufseinträgen einer Datei-Frage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionEntryType {
    Comment,
    Created,
    Title,
    Tags,
    Description,
    Occasion,
    Links,
    Depends,
    Research,
    Decided,
    Discarded,
    Reopened,
}

impl QuestionEntryType {
    /// Schreibweise in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionEntryType::Comment => "comment",
            QuestionEntryType::Created => "created",
            QuestionEntryType::Title => "title",
            QuestionEntryType::Tags => "tags",
            QuestionEntryType::Description => "description",
            QuestionEntryType::Occasion => "occasion",
            QuestionEntryType::Links => "links",
            QuestionEntryType::Depends => "depends",
            QuestionEntryType::Research => "research",
            QuestionEntryType::Decided => "decided",
            QuestionEntryType::Discarded => "discarded",
            QuestionEntryType::Reopened => "reopened",
        }
    }
}

impl fmt::Display for QuestionEntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ein datierter Eintrag im Verlauf; `read` gibt es nur bei Kommentaren
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionEntry {
    pub at: String,
    #[serde(rename = "type")]
    pub entry_type: QuestionEntryType,
    pub author: String,
    pub kind: AuthorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read: Option<bool>,
    pub text: String,
}

/// Eine Frage mit allen Feldern aus dem Frontmatter, Beschreibung, Recherchen und Verlauf
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub title: String,
    pub state: QuestionState,
    pub tags: Vec<String>,
    pub origin: QuestionOrigin,
    pub occasion: Option<String>,
    pub links: Vec<QuestionLink>,
    pub depends_on: Vec<String>,
    pub decision: Option<Decision>,
    pub created: String,
    pub updated: String,
    pub description: String,
    pub research: Vec<Research>,
    pub history: Vec<QuestionEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezugsarten_haben_die_schreibweise_von_typescript() {
        assert_eq!(
            LinkType::ALL.map(|link| link.as_str()),
            ["ticket", "spec", "spec-task", "file"]
        );
        assert_eq!(
            serde_json::to_value(&QuestionLink {
                link_type: LinkType::SpecTask,
                target: "2026-10-27-nexus#3".into(),
            })
            .unwrap(),
            serde_json::json!({ "type": "spec-task", "target": "2026-10-27-nexus#3" })
        );
    }

    #[test]
    fn herkunft_lasst_die_session_weg_wenn_sie_fehlt() {
        let origin = QuestionOrigin {
            kind: AuthorKind::Human,
            author: "Florian".into(),
            session: None,
        };
        assert_eq!(
            serde_json::to_value(&origin).unwrap(),
            serde_json::json!({ "kind": "human", "author": "Florian" })
        );
    }

    #[test]
    fn verlaufseintrag_heisst_in_json_type() {
        let entry = QuestionEntry {
            at: "2026-10-13T10:00:00.000Z".into(),
            entry_type: QuestionEntryType::Decided,
            author: "Florian".into(),
            kind: AuthorKind::Human,
            read: None,
            text: "Option O1 gewählt.".into(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["type"], "decided");
        assert!(json.get("read").is_none());
    }
}
