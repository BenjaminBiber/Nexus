//! Fragen an den Menschen: Zustände, Fachobjekte, Recherche und Entscheidung.

pub mod decision_policy;
pub mod question;
pub mod research_input_reader;
pub mod research_validator;
pub mod state;

pub use decision_policy::{DecisionInput, DecisionPolicy, DecisionSubject};
pub use question::{
    Decision, DecisionOutcome, LinkType, Question, QuestionEntry, QuestionEntryType, QuestionLink,
    QuestionOption, QuestionOrigin, Research,
};
pub use research_input_reader::ResearchInputReader;
pub use research_validator::{ResearchInput, ResearchOptionInput, ResearchValidator, MAX_OPTIONS};
pub use state::QuestionState;
