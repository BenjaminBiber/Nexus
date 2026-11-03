//! Der Telegram-Bot im Server: Zugriff auf die Bot-API, seine Laufzeit und das Verstehen
//! einer Idee (Board-Kontext sammeln und mit Claude zu einem Entwurf auswerten).
//!
//! [`api`] beschreibt die Schnittstelle, [`reqwest_api`] spricht Telegram wirklich an,
//! [`fake`] steht in Tests an deren Stelle. [`bot`] ist der Poller samt Kopplung,
//! [`router`] verteilt die Updates an die Handler, [`pairing`] gibt die Einmal-Codes aus.

pub mod api;
pub mod bot;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
pub mod idea;
pub mod pairing;
pub mod reqwest_api;
pub mod router;

pub use bot::{TelegramApiFactory, TelegramBot};
pub use idea::{IdeaInterpreter, IdeaRequest};
pub use pairing::{PairingCode, PairingCodes};
pub use router::{
    BotContext, CallbackHandler, CommandHandler, MessageHandler, UpdateRouter, RATE_LIMIT,
};
