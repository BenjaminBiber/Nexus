//! Telegram-Bot: Einstellung in `~/.nexus/telegram.yaml` samt Prüfung, Maskierung der
//! Geheimnisse und Store; reine Logik ohne Netz für HTML-Texte, Teilen langer Nachrichten
//! und die Kodierung der Knopf-Aktionen; Stand des Bots und sein Laufzeit-Zustand.

pub mod callback;
pub mod config;
pub mod config_store;
pub mod html;
pub mod split;
pub mod state_store;
pub mod status;

pub use callback::{CallbackAction, MAX_CALLBACK_LEN};
pub use config::{
    mask_api_key, mask_bot_token, AppliedProfile, BotProfile, IdeaSettings, IdeaTarget, QuietHours,
    SpeechBackend, SpeechConfig, TelegramConfig, TelegramNotify, WhisperCli, DEFAULT_API_BASE,
    DEFAULT_SPEECH_BASE_URL, DEFAULT_SPEECH_LANGUAGE, DEFAULT_SPEECH_MODEL, MAX_ROUNDS_RANGE,
};
pub use config_store::TelegramConfigStore;
pub use html::{bold, code, escape, italic, link};
pub use split::{split_message, MAX_MESSAGE_LEN};
pub use state_store::{TelegramState, TelegramStateStore};
pub use status::{BotState, BotStatus};
