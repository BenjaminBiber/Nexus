//! Wer handelt: Mensch, Claude-Session oder Subagent.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{Error, HarnessError};
use crate::text::join_names;

/// Art des Handelnden: Mensch, Claude-Session oder Subagent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorKind {
    Human,
    Session,
    Subagent,
}

impl AuthorKind {
    /// Alle Arten in der Reihenfolge von TypeScript
    pub const ALL: [AuthorKind; 3] = [AuthorKind::Human, AuthorKind::Session, AuthorKind::Subagent];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthorKind::Human => "human",
            AuthorKind::Session => "session",
            AuthorKind::Subagent => "subagent",
        }
    }
}

impl fmt::Display for AuthorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AuthorKind {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        AuthorKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == input)
            .ok_or_else(|| {
                HarnessError::new(
                    "kind-invalid",
                    format!(
                        "Art „{input}“ ist ungültig. Erlaubt: {}.",
                        join_names(AuthorKind::ALL.map(|kind| kind.as_str()))
                    ),
                )
                .into()
            })
    }
}

/// Wer eine Änderung oder einen Kommentar macht
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub author: String,
    pub kind: AuthorKind,
}

/// Wer bei Fragen handelt; bei Sessions mit der Kennung der Session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionActor {
    pub author: String,
    pub kind: AuthorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arten_haben_die_schreibweise_von_typescript() {
        assert_eq!(AuthorKind::Human.as_str(), "human");
        assert_eq!(AuthorKind::Subagent.to_string(), "subagent");
        assert_eq!(
            "session".parse::<AuthorKind>().unwrap(),
            AuthorKind::Session
        );
    }

    #[test]
    fn unbekannte_art_wird_mit_kind_invalid_abgelehnt() {
        let error = "mensch".parse::<AuthorKind>().unwrap_err();
        assert_eq!(error.code(), Some("kind-invalid"));
        assert_eq!(
            error.to_string(),
            "Art „mensch“ ist ungültig. Erlaubt: human, session, subagent."
        );
    }

    #[test]
    fn frage_akteur_lasst_die_session_weg_wenn_sie_fehlt() {
        let ohne = QuestionActor {
            author: "claude".into(),
            kind: AuthorKind::Session,
            session: None,
        };
        assert_eq!(
            serde_json::to_value(&ohne).unwrap(),
            serde_json::json!({ "author": "claude", "kind": "session" })
        );

        let mit = QuestionActor {
            session: Some("s-1".into()),
            ..ohne
        };
        assert_eq!(
            serde_json::to_value(&mit).unwrap(),
            serde_json::json!({ "author": "claude", "kind": "session", "session": "s-1" })
        );
    }

    #[test]
    fn akteur_fahrt_rund() {
        let actor = Actor {
            author: "Benjamin".into(),
            kind: AuthorKind::Human,
        };
        let json = serde_json::to_string(&actor).unwrap();
        assert_eq!(json, r#"{"author":"Benjamin","kind":"human"}"#);
        assert_eq!(serde_json::from_str::<Actor>(&json).unwrap(), actor);
    }
}
