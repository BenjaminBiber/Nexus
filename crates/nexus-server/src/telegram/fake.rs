//! Attrappe der Bot-API für Tests: zeichnet Aufrufe auf, spielt Updates ein und kann je
//! Methode einen Fehler melden.
//!
//! Tests anderer Module (Poller, Router, Meldungen) setzen sie anstelle von
//! [`crate::telegram::reqwest_api::ReqwestTelegramApi`] ein.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::telegram::api::{
    FileInfo, InlineKeyboard, TelegramApi, TelegramError, TelegramResult, Update, User,
};

/// Ein aufgezeichneter Aufruf: Name der Methode und ihre Parameter
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCall {
    /// Name wie in der Bot-API, z. B. `sendMessage`
    pub method: String,
    /// Die übergebenen Parameter als JSON
    pub params: Value,
}

/// Der veränderliche Teil der Attrappe
#[derive(Default)]
struct FakeState {
    calls: Vec<RecordedCall>,
    /// Warteschlange: jeder Eintrag ist die Antwort eines `getUpdates`
    updates: VecDeque<Vec<Update>>,
    /// Fehler je Methode, gemeldet bis er gelöscht wird
    errors: HashMap<String, TelegramError>,
    /// Dateien je `file_path` für den Download
    files: HashMap<String, Vec<u8>>,
    /// Was `getFile` je `file_id` verrät
    infos: HashMap<String, FileInfo>,
    /// Die `message_id` der nächsten geschickten Nachricht
    next_message_id: i64,
    /// Wartet `getUpdates` bei leerer Warteschlange wie Telegram sein Zeitlimit ab?
    wait_when_empty: bool,
}

/// Attrappe der Bot-API
pub struct FakeTelegram {
    state: Mutex<FakeState>,
    /// Was `getMe` liefert
    me: User,
}

impl Default for FakeTelegram {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeTelegram {
    /// Attrappe mit einem Bot namens „Nexus“
    pub fn new() -> Self {
        Self {
            state: Mutex::new(FakeState {
                next_message_id: 1,
                ..FakeState::default()
            }),
            me: User {
                id: 7012345678,
                username: Some("nexus_test_bot".to_string()),
                first_name: "Nexus".to_string(),
            },
        }
    }

    /// Attrappe, deren `getMe` diesen Bot liefert
    pub fn with_me(me: User) -> Self {
        Self { me, ..Self::new() }
    }

    /// Legt die Antwort eines weiteren `getUpdates` in die Warteschlange
    pub fn push_updates(&self, updates: Vec<Update>) {
        self.locked().updates.push_back(updates);
    }

    /// Lässt `getUpdates` bei leerer Warteschlange sein Zeitlimit abwarten, wie Telegram
    /// beim Long Polling. Ohne das kehrt die Attrappe sofort zurück – ein Poller in einer
    /// Schleife würde sie dann ohne Pause fragen
    pub fn wait_when_empty(&self) {
        self.locked().wait_when_empty = true;
    }

    /// Lässt `method` ab jetzt mit `error` fehlschlagen
    pub fn fail(&self, method: &str, error: TelegramError) {
        self.locked().errors.insert(method.to_string(), error);
    }

    /// Nimmt den eingestellten Fehler von `method` zurück
    pub fn clear_failure(&self, method: &str) {
        self.locked().errors.remove(method);
    }

    /// Legt eine herunterladbare Datei ab
    pub fn put_file(&self, file_path: &str, bytes: Vec<u8>) {
        self.locked()
            .files
            .insert(file_path.to_string(), bytes.clone());
        self.locked().infos.insert(
            file_path.to_string(),
            FileInfo {
                file_path: Some(file_path.to_string()),
                file_size: Some(bytes.len() as i64),
            },
        );
    }

    /// Legt ab, was `getFile` zu einer `file_id` liefert
    pub fn put_file_info(&self, file_id: &str, info: FileInfo) {
        self.locked().infos.insert(file_id.to_string(), info);
    }

    /// Alle aufgezeichneten Aufrufe in ihrer Reihenfolge
    pub fn calls(&self) -> Vec<RecordedCall> {
        self.locked().calls.clone()
    }

    /// Nur die Aufrufe einer Methode
    pub fn calls_of(&self, method: &str) -> Vec<RecordedCall> {
        self.calls()
            .into_iter()
            .filter(|call| call.method == method)
            .collect()
    }

    /// Leert die Aufzeichnung und liefert sie
    pub fn take_calls(&self) -> Vec<RecordedCall> {
        std::mem::take(&mut self.locked().calls)
    }

    /// Zeichnet einen Aufruf auf; `Err`, wenn für die Methode ein Fehler eingestellt ist
    fn record(&self, method: &str, params: Value) -> TelegramResult<()> {
        let mut state = self.locked();
        state.calls.push(RecordedCall {
            method: method.to_string(),
            params,
        });
        match state.errors.get(method) {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    /// Der Zustand; eine vergiftete Sperre ist ein Fehler im Test selbst
    fn locked(&self) -> std::sync::MutexGuard<'_, FakeState> {
        self.state.lock().expect("Sperre der Attrappe")
    }
}

#[async_trait]
impl TelegramApi for FakeTelegram {
    async fn get_me(&self) -> TelegramResult<User> {
        self.record("getMe", json!({}))?;
        Ok(self.me.clone())
    }

    async fn get_updates(&self, offset: i64, timeout_secs: u64) -> TelegramResult<Vec<Update>> {
        self.record(
            "getUpdates",
            json!({ "offset": offset, "timeout": timeout_secs }),
        )?;
        let (updates, warten) = {
            let mut state = self.locked();
            (state.updates.pop_front(), state.wait_when_empty)
        };
        if let Some(updates) = updates {
            return Ok(updates);
        }
        if warten {
            tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;
        }
        Ok(Vec::new())
    }

    async fn send_message(
        &self,
        chat_id: i64,
        html: &str,
        keyboard: Option<InlineKeyboard>,
        reply_to: Option<i64>,
    ) -> TelegramResult<i64> {
        self.record(
            "sendMessage",
            json!({
                "chat_id": chat_id,
                "text": html,
                "reply_markup": keyboard,
                "reply_to_message_id": reply_to,
            }),
        )?;
        let mut state = self.locked();
        let message_id = state.next_message_id;
        state.next_message_id += 1;
        Ok(message_id)
    }

    async fn edit_message_text(
        &self,
        chat_id: i64,
        message_id: i64,
        html: &str,
        keyboard: Option<InlineKeyboard>,
    ) -> TelegramResult<()> {
        self.record(
            "editMessageText",
            json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "text": html,
                "reply_markup": keyboard,
            }),
        )
    }

    async fn answer_callback_query(
        &self,
        id: &str,
        text: Option<&str>,
        alert: bool,
    ) -> TelegramResult<()> {
        self.record(
            "answerCallbackQuery",
            json!({ "callback_query_id": id, "text": text, "show_alert": alert }),
        )
    }

    async fn send_chat_action(&self, chat_id: i64, action: &str) -> TelegramResult<()> {
        self.record(
            "sendChatAction",
            json!({ "chat_id": chat_id, "action": action }),
        )
    }

    async fn get_file(&self, file_id: &str) -> TelegramResult<FileInfo> {
        self.record("getFile", json!({ "file_id": file_id }))?;
        self.locked()
            .infos
            .get(file_id)
            .cloned()
            .ok_or_else(|| TelegramError::Api {
                code: 400,
                description: "file not found".to_string(),
            })
    }

    async fn download_file(&self, file_path: &str) -> TelegramResult<Vec<u8>> {
        self.record("downloadFile", json!({ "file_path": file_path }))?;
        self.locked()
            .files
            .get(file_path)
            .cloned()
            .ok_or_else(|| TelegramError::Api {
                code: 404,
                description: "file not found".to_string(),
            })
    }

    async fn set_my_commands(&self, commands: &[(String, String)]) -> TelegramResult<()> {
        self.record("setMyCommands", json!({ "commands": commands }))
    }

    async fn set_my_name(&self, name: &str) -> TelegramResult<()> {
        self.record("setMyName", json!({ "name": name }))
    }

    async fn set_my_description(&self, text: &str) -> TelegramResult<()> {
        self.record("setMyDescription", json!({ "description": text }))
    }

    async fn set_my_short_description(&self, text: &str) -> TelegramResult<()> {
        self.record(
            "setMyShortDescription",
            json!({ "short_description": text }),
        )
    }

    async fn set_my_profile_photo(&self, jpeg: Vec<u8>) -> TelegramResult<()> {
        self.record("setMyProfilePhoto", json!({ "bytes": jpeg.len() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Update mit einer Textnachricht
    fn text_update(update_id: i64, text: &str) -> Update {
        serde_json::from_value(json!({
            "update_id": update_id,
            "message": {
                "message_id": update_id,
                "chat": { "id": 42, "type": "private" },
                "text": text,
            },
        }))
        .expect("Update")
    }

    #[tokio::test]
    async fn zeichnet_aufrufe_mit_parametern_auf() {
        let fake = FakeTelegram::new();

        let message_id = fake
            .send_message(42, "<b>hallo</b>", None, Some(7))
            .await
            .unwrap();
        fake.send_chat_action(42, "typing").await.unwrap();

        assert_eq!(message_id, 1);
        let calls = fake.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].method, "sendMessage");
        assert_eq!(calls[0].params["text"], "<b>hallo</b>");
        assert_eq!(calls[0].params["reply_to_message_id"], 7);
        assert_eq!(calls[1].method, "sendChatAction");
        assert_eq!(calls[1].params["action"], "typing");
    }

    #[tokio::test]
    async fn zahlt_die_message_id_hoch() {
        let fake = FakeTelegram::new();

        assert_eq!(fake.send_message(1, "a", None, None).await.unwrap(), 1);
        assert_eq!(fake.send_message(1, "b", None, None).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn spielt_updates_aus_der_warteschlange_ein() {
        let fake = FakeTelegram::new();
        fake.push_updates(vec![text_update(1, "eins")]);
        fake.push_updates(vec![]);

        let first = fake.get_updates(0, 25).await.unwrap();
        let second = fake.get_updates(2, 25).await.unwrap();
        let third = fake.get_updates(2, 25).await.unwrap();

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].update_id, 1);
        assert!(second.is_empty());
        assert!(third.is_empty(), "leere Warteschlange liefert nichts");
        assert_eq!(fake.calls_of("getUpdates").len(), 3);
    }

    #[tokio::test]
    async fn meldet_den_eingestellten_fehler_je_methode() {
        let fake = FakeTelegram::new();
        fake.fail("getUpdates", TelegramError::Conflict);

        let error = fake.get_updates(0, 25).await.unwrap_err();
        assert_eq!(error, TelegramError::Conflict);
        // Der Aufruf ist trotzdem aufgezeichnet, und andere Methoden laufen weiter
        assert_eq!(fake.calls_of("getUpdates").len(), 1);
        assert!(fake.get_me().await.is_ok());

        fake.clear_failure("getUpdates");
        assert!(fake.get_updates(0, 25).await.is_ok());
    }

    #[tokio::test]
    async fn liefert_abgelegte_dateien() {
        let fake = FakeTelegram::new();
        fake.put_file_info(
            "AwA",
            FileInfo {
                file_path: Some("voice/file_1.oga".to_string()),
                file_size: Some(3),
            },
        );
        fake.put_file("voice/file_1.oga", vec![1, 2, 3]);

        let info = fake.get_file("AwA").await.unwrap();
        let bytes = fake.download_file(info.file_path.as_deref().unwrap()).await;

        assert_eq!(bytes.unwrap(), vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn take_calls_leert_die_aufzeichnung() {
        let fake = FakeTelegram::new();
        fake.set_my_name("Nexus").await.unwrap();

        assert_eq!(fake.take_calls().len(), 1);
        assert!(fake.calls().is_empty());
    }
}
