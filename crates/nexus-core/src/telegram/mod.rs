//! Telegram-Bot: Einstellung in `~/.nexus/telegram.yaml` samt Prüfung, Maskierung der
//! Geheimnisse und Store.

pub mod config;
pub mod config_store;

pub use config::{
    mask_api_key, mask_bot_token, AppliedProfile, BotProfile, IdeaSettings, IdeaTarget, QuietHours,
    SpeechBackend, SpeechConfig, TelegramConfig, TelegramNotify, WhisperCli, DEFAULT_API_BASE,
    DEFAULT_SPEECH_BASE_URL, DEFAULT_SPEECH_LANGUAGE, DEFAULT_SPEECH_MODEL, MAX_ROUNDS_RANGE,
};
pub use config_store::TelegramConfigStore;
