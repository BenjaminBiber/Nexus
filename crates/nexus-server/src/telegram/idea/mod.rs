//! Ideen aus dem Bot: Board-Kontext sammeln, mit Claude zu einem Entwurf auswerten und den
//! Entwurf im Chat bestätigen, korrigieren und anlegen.
//!
//! [`interpreter`] ruft Claude, [`store`] hält je Chat höchstens einen Entwurf (auch über
//! einen Neustart), [`flow`] ist der Dialog im Chat und [`views`] formatiert seine Texte.

pub(crate) mod flow;
pub mod interpreter;
pub mod store;
pub mod views;

pub use interpreter::{IdeaInterpreter, IdeaRequest};
pub use store::{IdeaChat, IdeaDraftStore, IdeaEntry, IDEA_EXPIRY_HOURS};
