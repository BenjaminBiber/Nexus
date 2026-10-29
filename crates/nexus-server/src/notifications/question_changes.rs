//! Erkennt zwischen zwei Ständen neue Fragen und neue Recherchen von Agenten; portiert aus
//! `packages/server/src/notifications/QuestionChangeDetector.ts`.

use std::collections::HashMap;
use std::marker::PhantomData;

use nexus_core::actors::AuthorKind;
use nexus_core::board::question_types::{NotifiableBoardQuestion, NotifiableResearch};

/// Was eine Frage für Meldungen mitbringen muss: Kennung, Herkunft und ihre Recherchen.
/// In TypeScript ist das die strukturelle Schnittstelle `NotifiableQuestion`.
pub trait QuestionSnapshot {
    fn id(&self) -> &str;
    fn origin_kind(&self) -> AuthorKind;
    fn research(&self) -> &[NotifiableResearch];
}

impl QuestionSnapshot for NotifiableBoardQuestion {
    fn id(&self) -> &str {
        &self.id
    }

    fn origin_kind(&self) -> AuthorKind {
        self.origin.kind
    }

    fn research(&self) -> &[NotifiableResearch] {
        &self.research
    }
}

/// Anlass einer Fragen-Meldung
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionChangeKind {
    /// Eine neue Frage eines Agenten
    FrageNeu,
    /// Eine neue Recherche eines Agenten
    Recherche,
}

/// Eine erkannte Änderung samt der Frage dazu
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundQuestionChange<Q> {
    pub kind: QuestionChangeKind,
    pub question: Q,
}

/// Erkennt neue Fragen und neue Recherchen von Agenten; merkt sich je Frage die Zahl der
/// Recherchen, damit jede Änderung genau einmal gemeldet wird.
#[derive(Debug)]
pub struct QuestionChangeDetector<Q: QuestionSnapshot> {
    known: HashMap<String, usize>,
    primed: bool,
    question: PhantomData<fn() -> Q>,
}

impl<Q: QuestionSnapshot> Default for QuestionChangeDetector<Q> {
    fn default() -> Self {
        Self {
            known: HashMap::new(),
            primed: false,
            question: PhantomData,
        }
    }
}

impl<Q: QuestionSnapshot> QuestionChangeDetector<Q> {
    /// Ohne eingelesenen Bestand
    pub fn new() -> Self {
        Self::default()
    }

    /// Wurde der Bestand schon einmal eingelesen?
    pub fn primed(&self) -> bool {
        self.primed
    }

    /// Merkt sich den Bestand, ohne etwas zu melden
    pub fn prime(&mut self, questions: &[Q]) {
        for question in questions {
            self.known
                .insert(question.id().to_string(), question.research().len());
        }
        self.primed = true;
    }

    /// Vergleicht mit dem gemerkten Stand; vor dem ersten Einlesen wird nur eingelesen
    pub fn compare(&mut self, questions: Vec<Q>) -> Vec<FoundQuestionChange<Q>> {
        if !self.primed {
            self.prime(&questions);
            return Vec::new();
        }
        let mut found = Vec::new();
        for question in questions {
            let before = self
                .known
                .insert(question.id().to_string(), question.research().len());
            if let Some(kind) = Self::kind_of(&question, before) {
                found.push(FoundQuestionChange { kind, question });
            }
        }
        found
    }

    /// Neue Frage eines Agenten oder neue Recherche eines Agenten; sonst nichts
    fn kind_of(question: &Q, before: Option<usize>) -> Option<QuestionChangeKind> {
        let Some(before) = before else {
            return match question.origin_kind() {
                AuthorKind::Human => None,
                _ => Some(QuestionChangeKind::FrageNeu),
            };
        };
        let research = question.research();
        if research.len() <= before {
            return None;
        }
        match research.last()?.kind {
            AuthorKind::Human => None,
            _ => Some(QuestionChangeKind::Recherche),
        }
    }
}
