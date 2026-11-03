//! Der Telegram-Bot: Zugriff auf die Bot-API.
//!
//! [`api`] beschreibt die Schnittstelle, [`reqwest_api`] spricht Telegram wirklich an,
//! [`fake`] steht in Tests an deren Stelle.

pub mod api;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
pub mod reqwest_api;
