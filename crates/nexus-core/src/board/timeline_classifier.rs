//! Ordnet Verlaufseinträge und Kommentare einer Art der Zeitleiste zu und schätzt, ob ein
//! Mensch oder ein Agent handelte. Portiert aus
//! `packages/core/src/board/TimelineClassifier.ts`.

use std::sync::LazyLock;

use regex::Regex;

use crate::actors::AuthorKind;
use crate::board::ticket_timeline::{
    TimelineActor, TimelineActorType, TimelineAgentNames, TimelineEvent, TimelineKind, TimelineRef,
};
use crate::board::types::{Comment, HistoryEntry};
use crate::dispatch::markers;

/// Fragen-ID im Vermerk eines Verlaufseintrags
static QUESTION_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Z][A-Z0-9]*-F[0-9]+\b").expect("festes Muster"));

/// Profil im Start-Kommentar des Dispatchers
static PROFILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Profil ([^\s(,]+)").expect("festes Muster"));

/// Commit im Merge-Kommentar des Dispatchers
static COMMIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[dispatch\] Gemergt in .+? \(([^)]+)\)\.").expect("festes Muster")
});

/// Art je Feld des Verlaufs; nicht genannte Felder sind `other`
fn field_kind(field: &str) -> TimelineKind {
    match field {
        "status" | "blockedReason" => TimelineKind::Status,
        "assignee" | "planningClaimBy" => TimelineKind::Claim,
        "question" => TimelineKind::Question,
        "branch" | "worktree" | "targetBranch" => TimelineKind::Git,
        "title" | "description" | "plan" | "handbook" | "diagram" | "acceptance" | "step"
        | "steps" => TimelineKind::Content,
        _ => TimelineKind::Other,
    }
}

/// Dispatcher-Kommentare: Marker → Art und Typ; spezielle Marker vor ihren Vorsilben
/// („Merge-Konflikt“ vor „Merge“)
const DISPATCH_KINDS: [(&str, TimelineKind, &str); 11] = [
    (markers::START, TimelineKind::Run, "dispatch:start"),
    (markers::ABORT, TimelineKind::Run, "dispatch:abort"),
    (markers::PAUSE, TimelineKind::Run, "dispatch:pause"),
    (markers::QUESTION, TimelineKind::Run, "dispatch:question"),
    (markers::SETUP, TimelineKind::Run, "dispatch:setup"),
    (
        markers::REVIEW_RETURN,
        TimelineKind::Run,
        "dispatch:review-return",
    ),
    (markers::REVIEW, TimelineKind::Run, "dispatch:review"),
    (markers::MERGED, TimelineKind::Git, "merge"),
    (
        markers::MERGE_CONFLICT,
        TimelineKind::Git,
        "dispatch:merge-conflict",
    ),
    (
        markers::MERGE_WAIT,
        TimelineKind::Git,
        "dispatch:merge-wait",
    ),
    (markers::MERGE, TimelineKind::Git, "dispatch:merge"),
];

/// Ordnet Einträge und Kommentare der Zeitleiste zu.
#[derive(Debug, Clone)]
pub struct TimelineClassifier {
    names: Option<TimelineAgentNames>,
}

impl TimelineClassifier {
    /// `names` `None` = keine Dispatch-Konfiguration; dann zählen nur „claude“ und die Art
    /// des Autors
    pub fn new(names: Option<TimelineAgentNames>) -> Self {
        Self { names }
    }

    /// Agent, wenn die Art des Autors es sagt (Fragen) oder der Name nach Agent aussieht
    pub fn actor(&self, name: &str, kind: Option<AuthorKind>) -> TimelineActor {
        let actor_type = match kind {
            Some(AuthorKind::Human) => TimelineActorType::Person,
            Some(_) => TimelineActorType::Agent,
            None if self.is_agent_name(name) => TimelineActorType::Agent,
            None => TimelineActorType::Person,
        };
        TimelineActor {
            name: name.to_string(),
            actor_type,
        }
    }

    /// Ereignis zu einem Verlaufseintrag des Tickets
    pub fn history(&self, entry: &HistoryEntry) -> TimelineEvent {
        let question = (entry.field == "question")
            .then(|| QUESTION_ID.find(entry.note.as_deref().unwrap_or("")))
            .flatten()
            .map(|found| found.as_str().to_string());
        TimelineEvent {
            at: entry.at.clone(),
            kind: field_kind(&entry.field),
            event_type: format!("field:{}", entry.field),
            actor: self.actor(&entry.by_name, None),
            field: Some(entry.field.clone()),
            from: Some(entry.from.clone()),
            to: Some(entry.to.clone()),
            note: Some(entry.note.clone()),
            text: None,
            reference: question.map(|question_id| TimelineRef {
                question_id: Some(question_id),
                ..TimelineRef::default()
            }),
        }
    }

    /// Ereignis zu einem Kommentar; Dispatcher-Kommentare werden an ihrem Marker erkannt
    pub fn comment(&self, comment: &Comment) -> TimelineEvent {
        let found = DISPATCH_KINDS
            .into_iter()
            .find(|(marker, _, _)| comment.body.starts_with(marker));
        let (kind, event_type) = match found {
            Some((_, kind, event_type)) => (kind, event_type),
            None => (TimelineKind::Comment, "comment"),
        };
        let reference = match event_type {
            "dispatch:start" => PROFILE.captures(&comment.body).map(|found| TimelineRef {
                profile: Some(found[1].to_string()),
                ..TimelineRef::default()
            }),
            "merge" => COMMIT.captures(&comment.body).map(|found| TimelineRef {
                commit: Some(found[1].to_string()),
                ..TimelineRef::default()
            }),
            _ => None,
        };
        TimelineEvent {
            at: comment.created_at.clone(),
            kind,
            event_type: event_type.to_string(),
            actor: self.actor(&comment.author_name, None),
            field: None,
            from: None,
            to: None,
            note: None,
            text: Some(comment.body.clone()),
            reference,
        }
    }

    /// Sieht der Name nach einem Agenten aus?
    fn is_agent_name(&self, name: &str) -> bool {
        let lower = name.trim().to_lowercase();
        if lower == "claude" || lower.starts_with("claude-") {
            return true;
        }
        self.names.as_ref().is_some_and(|names| {
            name == names.dispatcher_name
                || (!names.agent_prefix.is_empty() && name.starts_with(&names.agent_prefix))
        })
    }
}

#[cfg(test)]
mod tests;
