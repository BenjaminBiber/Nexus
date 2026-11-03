//! Der Telegram-Bot im Server: Zugriff auf die Bot-API, seine Laufzeit und das Verstehen
//! einer Idee (Board-Kontext sammeln und mit Claude zu einem Entwurf auswerten).
//!
//! [`api`] beschreibt die Schnittstelle, [`reqwest_api`] spricht Telegram wirklich an,
//! [`fake`] steht in Tests an deren Stelle. [`bot`] ist der Poller samt Kopplung,
//! [`router`] verteilt die Updates an die Handler, [`pairing`] gibt die Einmal-Codes aus.
//! [`commands`] sind die Befehle samt Pause/Weiter und die Knöpfe an den Aufgaben,
//! [`dialog`] hält je Chat den offenen Dialog einer Texteingabe, [`views`],
//! [`action_views`] und [`question_views`] formatieren die Antworten als Telegram-HTML.

pub mod action_views;
pub mod api;
pub mod bot;
pub mod commands;
pub mod dialog;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
pub mod idea;
pub mod pairing;
pub mod question_views;
pub mod reqwest_api;
pub mod router;
pub mod views;

pub use bot::{TelegramApiFactory, TelegramBot};
pub use dialog::{Dialog, DialogInput, DialogKind, Dialogs, DIALOG_TIMEOUT_MINUTES};
pub use idea::{IdeaInterpreter, IdeaRequest};
pub use pairing::{PairingCode, PairingCodes};
pub use router::{
    BotContext, CallbackHandler, CommandHandler, MessageHandler, UpdateRouter, RATE_LIMIT,
};
