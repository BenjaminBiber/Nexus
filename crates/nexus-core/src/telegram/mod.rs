//! Telegram-Bot: Einstellung in `~/.nexus/telegram.yaml` samt Prüfung, Maskierung der
//! Geheimnisse und Store, dazu das Verstehen von Ideen (Board-Kontext, Prompt, Prüfer).

pub mod config;
pub mod config_store;
pub mod idea;

pub use idea::{
    build_prompt as build_idea_prompt, IdeaBoard, IdeaContext, IdeaDraft, IdeaDraftCheck,
    IdeaDraftValidator, IdeaDuplicate, IdeaKind, IdeaNote, IdeaQuestion, IdeaRefinement,
    IdeaRelated, IdeaTargetRef, IdeaTicket, MAX_CONTEXT_CHARS, MAX_CONTEXT_TICKETS, MAX_DETAILS,
    MAX_QUESTIONS, MAX_TITLE,
};

pub use config::{
    mask_api_key, mask_bot_token, AppliedProfile, BotProfile, IdeaSettings, IdeaTarget, QuietHours,
    SpeechBackend, SpeechConfig, TelegramConfig, TelegramNotify, WhisperCli, DEFAULT_API_BASE,
    DEFAULT_SPEECH_BASE_URL, DEFAULT_SPEECH_LANGUAGE, DEFAULT_SPEECH_MODEL, MAX_ROUNDS_RANGE,
};
pub use config_store::TelegramConfigStore;
