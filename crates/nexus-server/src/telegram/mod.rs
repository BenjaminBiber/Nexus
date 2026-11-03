//! Der Telegram-Bot im Server: Zugriff auf die Bot-API, seine Laufzeit und das Verstehen
//! einer Idee (Board-Kontext sammeln und mit Claude zu einem Entwurf auswerten).
//!
//! [`api`] beschreibt die Schnittstelle, [`reqwest_api`] spricht Telegram wirklich an,
//! [`fake`] steht in Tests an deren Stelle. [`bot`] ist der Poller samt Kopplung,
//! [`router`] verteilt die Updates an die Handler, [`pairing`] gibt die Einmal-Codes aus.
//! [`commands`] sind die Befehle samt Pause/Weiter und die Knöpfe an den Aufgaben,
//! [`dialog`] hält je Chat den offenen Dialog einer Texteingabe, [`views`],
//! [`action_views`] und [`question_views`] formatieren die Antworten als Telegram-HTML.
//! [`notifier`] schickt die Meldungen des Servers in die gekoppelten Chats,
//! [`profile`] richtet Profilbild, Name, Beschreibungen und Befehlsliste des Bots ein,
//! [`texts`] hält die dafür gesetzten Texte.

pub mod action_views;
pub mod api;
pub mod bot;
pub mod commands;
pub mod dialog;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
pub mod idea;
pub mod notifier;
pub mod pairing;
pub mod profile;
pub mod question_views;
pub mod reqwest_api;
pub mod router;
pub mod texts;
pub mod views;

pub use bot::{TelegramApiFactory, TelegramBot};
pub use dialog::{Dialog, DialogInput, DialogKind, Dialogs, DIALOG_TIMEOUT_MINUTES};
pub use idea::{IdeaInterpreter, IdeaRequest};
pub use notifier::{Occasion, TelegramNotice, TelegramNotifier};
pub use pairing::{PairingCode, PairingCodes};
pub use profile::{BotProfileSync, ProfileOutcome};
pub use router::{
    BotContext, CallbackHandler, CommandHandler, MessageHandler, UpdateRouter, RATE_LIMIT,
};
