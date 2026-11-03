//! Telegram-Bot: Einstellung in `~/.nexus/telegram.yaml` samt Prüfung, Maskierung der
//! Geheimnisse und Store; reine Logik ohne Netz für HTML-Texte, Teilen langer Nachrichten
//! und die Kodierung der Knopf-Aktionen; Stand des Bots und sein Laufzeit-Zustand, dazu
//! das Verstehen von Ideen (Board-Kontext, Prompt, Prüfer).

pub mod callback;
pub mod config;
pub mod config_store;
pub mod html;
pub mod idea;
pub mod routes;
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
pub use idea::{
    build_prompt as build_idea_prompt, IdeaBoard, IdeaContext, IdeaDraft, IdeaDraftCheck,
    IdeaDraftValidator, IdeaDuplicate, IdeaKind, IdeaNote, IdeaQuestion, IdeaRefinement,
    IdeaRelated, IdeaTargetRef, IdeaTicket, MAX_CONTEXT_CHARS, MAX_CONTEXT_TICKETS, MAX_DETAILS,
    MAX_QUESTIONS, MAX_TITLE,
};
pub use routes::{
    TelegramPairingDto, TelegramProfileDto, TelegramStatusDto, TelegramTestDto, TelegramTestError,
};
pub use split::{split_message, MAX_MESSAGE_LEN};
pub use state_store::{TelegramState, TelegramStateStore};
pub use status::{BotState, BotStatus};
