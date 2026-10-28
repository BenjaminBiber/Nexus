//! Die persönliche Arbeitsliste „Meine Aufgaben“.

pub mod types;

pub use types::{
    InboxCounts, InboxItem, InboxKind, InboxList, INBOX_KEY_PATTERN, INBOX_KINDS, MAX_INBOX_KEYS,
    MAX_INBOX_KEY_LENGTH,
};
