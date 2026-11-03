//! Der Telegram-Bot im Server: Zugriff auf die Bot-API und das Verstehen einer Idee
//! (Board-Kontext sammeln und mit Claude zu einem Entwurf auswerten).
//!
//! [`api`] beschreibt die Schnittstelle, [`reqwest_api`] spricht Telegram wirklich an,
//! [`fake`] steht in Tests an deren Stelle.

pub mod api;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
pub mod idea;
pub mod reqwest_api;

pub use idea::{IdeaInterpreter, IdeaRequest};
