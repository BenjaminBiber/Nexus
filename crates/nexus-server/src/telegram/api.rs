//! Schnittstelle zur Bot-API von Telegram: Trait, Typen und Fehlerarten.
//!
//! Nur die Felder, die Nexus wirklich braucht; alles Übrige der Antwort wird überlesen.
//! Der Zugriff steckt hinter [`TelegramApi`], damit Tests nie wirklich bei Telegram landen
//! (Muster wie `NtfyFetch` in `notifications::ntfy`).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Ergebnis eines Aufrufs der Bot-API
pub type TelegramResult<T> = std::result::Result<T, TelegramError>;

/// Fehler eines Aufrufs der Bot-API.
///
/// Die Texte sind deutsch und enthalten nie die aufgerufene Adresse, weil in jeder
/// Telegram-URL das Bot-Token steht.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TelegramError {
    /// 401: das Bot-Token stimmt nicht (mehr)
    #[error("Telegram lehnt das Bot-Token ab (401).")]
    Unauthorized,
    /// 409: ein zweiter Poller holt dieselben Updates ab
    #[error("Telegram meldet einen zweiten Abholer derselben Updates (409).")]
    Conflict,
    /// 429: zu viele Anfragen; erst nach `retry_after` Sekunden wieder versuchen
    #[error("Telegram bremst: erst in {retry_after} s wieder versuchen (429).")]
    TooManyRequests {
        /// Wartezeit in Sekunden aus `parameters.retry_after`
        retry_after: u64,
    },
    /// Jede andere Antwort mit `ok: false`
    #[error("Telegram antwortete mit Fehler {code}: {description}")]
    Api {
        /// `error_code` der Antwort
        code: i64,
        /// `description` der Antwort
        description: String,
    },
    /// Die Anfrage kam nicht durch oder die Antwort war kein verwertbares JSON
    #[error("Telegram nicht erreichbar: {0}")]
    Network(String),
}

impl TelegramError {
    /// Fehler aus `error_code` und `description` einer Antwort mit `ok: false`
    pub fn from_api(code: i64, description: String, retry_after: Option<u64>) -> Self {
        match code {
            401 => TelegramError::Unauthorized,
            409 => TelegramError::Conflict,
            429 => TelegramError::TooManyRequests {
                // Ohne Angabe eine Sekunde warten, damit der Aufrufer nie mit 0 weiterrennt
                retry_after: retry_after.unwrap_or(1),
            },
            _ => TelegramError::Api { code, description },
        }
    }
}

/// Ein Benutzer oder der Bot selbst
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub first_name: String,
}

/// Der Chat einer Nachricht
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chat {
    pub id: i64,
    /// `private`, `group`, `supergroup` oder `channel`
    #[serde(rename = "type", default)]
    pub chat_type: String,
}

/// Eine Sprachnachricht oder eine Audiodatei
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFile {
    pub file_id: String,
    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub file_size: Option<i64>,
}

/// Eine Nachricht in einem Chat
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub message_id: i64,
    pub chat: Chat,
    #[serde(default)]
    pub from: Option<User>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub voice: Option<AudioFile>,
    #[serde(default)]
    pub audio: Option<AudioFile>,
    /// Die Nachricht, auf die geantwortet wurde
    #[serde(default)]
    pub reply_to_message: Option<Box<Message>>,
}

/// Der Druck auf einen Knopf unter einer Nachricht
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallbackQuery {
    pub id: String,
    pub from: User,
    /// Die Nachricht mit dem Knopf; fehlt bei sehr alten Nachrichten
    #[serde(default)]
    pub message: Option<Message>,
    /// Die `callback_data` des Knopfes
    #[serde(default)]
    pub data: Option<String>,
}

/// Ein Update aus `getUpdates`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Update {
    pub update_id: i64,
    #[serde(default)]
    pub message: Option<Message>,
    #[serde(default)]
    pub callback_query: Option<CallbackQuery>,
}

/// Ein Knopf einer Tastatur unter einer Nachricht: entweder mit `callback_data` oder mit `url`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineButton {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback_data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl InlineButton {
    /// Knopf, der ein Update mit `data` auslöst
    pub fn callback(text: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            callback_data: Some(data.into()),
            url: None,
        }
    }

    /// Knopf, der eine Adresse öffnet
    pub fn url(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            callback_data: None,
            url: Some(url.into()),
        }
    }
}

/// Die Tastatur unter einer Nachricht, Zeile für Zeile
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InlineKeyboard {
    pub inline_keyboard: Vec<Vec<InlineButton>>,
}

impl InlineKeyboard {
    /// Tastatur aus Zeilen von Knöpfen
    pub fn new(rows: Vec<Vec<InlineButton>>) -> Self {
        Self {
            inline_keyboard: rows,
        }
    }

    /// Tastatur mit einer Zeile
    pub fn row(buttons: Vec<InlineButton>) -> Self {
        Self::new(vec![buttons])
    }
}

/// Was `getFile` über eine Datei verrät
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInfo {
    /// Pfad für den Download; fehlt, wenn Telegram die Datei nicht bereitstellt
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub file_size: Option<i64>,
}

/// Die Bot-API von Telegram, soweit Nexus sie nutzt.
///
/// Texte gehen als Telegram-HTML (`parse_mode: HTML`) hinaus, Vorschauen von Links sind aus.
#[async_trait]
pub trait TelegramApi: Send + Sync {
    /// Der Bot selbst; prüft zugleich das Token
    async fn get_me(&self) -> TelegramResult<User>;

    /// Holt Updates ab `offset` und wartet dabei bis zu `timeout_secs` Sekunden (Long Polling)
    async fn get_updates(&self, offset: i64, timeout_secs: u64) -> TelegramResult<Vec<Update>>;

    /// Schickt eine Nachricht und liefert ihre `message_id`
    async fn send_message(
        &self,
        chat_id: i64,
        html: &str,
        keyboard: Option<InlineKeyboard>,
        reply_to: Option<i64>,
    ) -> TelegramResult<i64>;

    /// Ersetzt Text und Tastatur einer schon geschickten Nachricht
    async fn edit_message_text(
        &self,
        chat_id: i64,
        message_id: i64,
        html: &str,
        keyboard: Option<InlineKeyboard>,
    ) -> TelegramResult<()>;

    /// Beantwortet den Druck auf einen Knopf; `alert` zeigt den Text als Hinweisfenster
    async fn answer_callback_query(
        &self,
        id: &str,
        text: Option<&str>,
        alert: bool,
    ) -> TelegramResult<()>;

    /// Zeigt im Chat eine Tätigkeit an, z. B. `typing`
    async fn send_chat_action(&self, chat_id: i64, action: &str) -> TelegramResult<()>;

    /// Pfad und Größe einer Datei zu einer `file_id`
    async fn get_file(&self, file_id: &str) -> TelegramResult<FileInfo>;

    /// Lädt die Datei zu einem `file_path` aus [`TelegramApi::get_file`] herunter
    async fn download_file(&self, file_path: &str) -> TelegramResult<Vec<u8>>;

    /// Setzt die Befehlsliste des Bots aus Paaren von Befehl und Beschreibung
    async fn set_my_commands(&self, commands: &[(String, String)]) -> TelegramResult<()>;

    /// Setzt den angezeigten Namen des Bots
    async fn set_my_name(&self, name: &str) -> TelegramResult<()>;

    /// Setzt die Beschreibung (leeres Chatfenster)
    async fn set_my_description(&self, text: &str) -> TelegramResult<()>;

    /// Setzt die kurze Beschreibung (Profil)
    async fn set_my_short_description(&self, text: &str) -> TelegramResult<()>;

    /// Setzt das Profilbild des Bots aus einem JPEG
    async fn set_my_profile_photo(&self, jpeg: Vec<u8>) -> TelegramResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordnet_die_statuscodes_den_fehlerarten_zu() {
        assert_eq!(
            TelegramError::from_api(401, "Unauthorized".into(), None),
            TelegramError::Unauthorized
        );
        assert_eq!(
            TelegramError::from_api(409, "Conflict".into(), None),
            TelegramError::Conflict
        );
        assert_eq!(
            TelegramError::from_api(429, "Too Many Requests".into(), Some(7)),
            TelegramError::TooManyRequests { retry_after: 7 }
        );
        assert_eq!(
            TelegramError::from_api(400, "Bad Request".into(), None),
            TelegramError::Api {
                code: 400,
                description: "Bad Request".into()
            }
        );
    }

    #[test]
    fn wartet_ohne_angabe_mindestens_eine_sekunde() {
        assert_eq!(
            TelegramError::from_api(429, "Too Many Requests".into(), None),
            TelegramError::TooManyRequests { retry_after: 1 }
        );
    }

    #[test]
    fn liest_eine_nachricht_mit_sprachnachricht_und_antwort() {
        let update: Update = serde_json::from_str(
            r#"{
                "update_id": 11,
                "message": {
                    "message_id": 5,
                    "chat": { "id": -100, "type": "group", "title": "egal" },
                    "from": { "id": 7, "first_name": "Ben", "username": "ben" },
                    "voice": { "file_id": "AwA", "duration": 3, "mime_type": "audio/ogg", "file_size": 1234 },
                    "reply_to_message": { "message_id": 4, "chat": { "id": -100, "type": "group" }, "text": "vorher" }
                }
            }"#,
        )
        .unwrap();

        let message = update.message.unwrap();
        assert_eq!(message.chat.chat_type, "group");
        assert_eq!(message.from.unwrap().username.as_deref(), Some("ben"));
        let voice = message.voice.unwrap();
        assert_eq!(voice.file_id, "AwA");
        assert_eq!(voice.duration, 3);
        assert_eq!(
            message.reply_to_message.unwrap().text.as_deref(),
            Some("vorher")
        );
    }

    #[test]
    fn schreibt_die_tastatur_wie_telegram_sie_erwartet() {
        let keyboard = InlineKeyboard::row(vec![
            InlineButton::callback("Merge", "merge:NXS-1"),
            InlineButton::url("Öffnen", "https://example.test/x"),
        ]);

        let json = serde_json::to_value(&keyboard).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "inline_keyboard": [[
                    { "text": "Merge", "callback_data": "merge:NXS-1" },
                    { "text": "Öffnen", "url": "https://example.test/x" },
                ]]
            })
        );
    }
}
