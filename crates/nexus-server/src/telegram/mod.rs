//! Telegram-Bot im Server. Bisher: das Verstehen einer Idee (Board-Kontext sammeln und mit
//! Claude zu einem Entwurf auswerten).

pub mod idea;

pub use idea::{IdeaInterpreter, IdeaRequest};
