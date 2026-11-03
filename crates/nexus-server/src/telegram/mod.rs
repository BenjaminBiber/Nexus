//! Der Telegram-Bot: Zugriff auf die Bot-API und seine Laufzeit im Server.
//!
//! [`api`] beschreibt die Schnittstelle, [`reqwest_api`] spricht Telegram wirklich an,
//! [`fake`] steht in Tests an deren Stelle. [`bot`] ist der Poller samt Kopplung,
//! [`router`] verteilt die Updates an die Handler, [`pairing`] gibt die Einmal-Codes aus.

pub mod api;
pub mod bot;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
pub mod pairing;
pub mod reqwest_api;
pub mod router;

pub use bot::{TelegramApiFactory, TelegramBot};
pub use pairing::{PairingCode, PairingCodes};
pub use router::{
    BotContext, CallbackHandler, CommandHandler, MessageHandler, UpdateRouter, RATE_LIMIT,
};
