//! Die Umsetzung von [`TelegramApi`] über HTTP (reqwest).
//!
//! Aufrufe gehen als JSON an `POST {api_base}/bot{token}/{methode}`, Dateien kommen über
//! `{api_base}/file/bot{token}/{file_path}`. Weil in jeder dieser Adressen das Bot-Token
//! steht, landet nie eine Adresse in einem Fehlertext – auch nicht die von reqwest gemeldete.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::telegram::api::{
    FileInfo, InlineKeyboard, TelegramApi, TelegramError, TelegramResult, Update, User,
};

/// Zeitlimit eines gewöhnlichen Aufrufs
const TIMEOUT: Duration = Duration::from_secs(30);

/// Zuschlag auf das Zeitlimit beim Long Polling: Telegram antwortet spätestens nach `timeout`
const POLL_EXTRA: Duration = Duration::from_secs(10);

/// Name des Dateiteils im Multipart von `setMyProfilePhoto`
const PHOTO_PART: &str = "avatar";

/// Die Antwort der Bot-API
#[derive(Debug, Deserialize)]
struct ApiEnvelope {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error_code: Option<i64>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    parameters: Option<ApiParameters>,
}

/// Zusätzliche Angaben zu einem Fehler
#[derive(Debug, Deserialize)]
struct ApiParameters {
    #[serde(default)]
    retry_after: Option<u64>,
}

/// Spricht die Bot-API von Telegram über HTTP an
pub struct ReqwestTelegramApi {
    client: reqwest::Client,
    /// Adresse der Bot-API ohne Schrägstrich am Ende
    api_base: String,
    /// Bot-Token von @BotFather
    token: String,
}

impl ReqwestTelegramApi {
    /// Client für eine Adresse der Bot-API und ein Bot-Token
    pub fn new(api_base: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_base: api_base.into().trim_end_matches('/').to_string(),
            token: token.into(),
        }
    }

    /// Adresse einer Methode; enthält das Token und darf deshalb nie nach außen
    fn method_url(&self, method: &str) -> String {
        format!("{}/bot{}/{method}", self.api_base, self.token)
    }

    /// Ruft eine Methode mit JSON-Körper auf und liefert das `result`
    async fn call(&self, method: &str, body: Value, timeout: Duration) -> TelegramResult<Value> {
        let request = self
            .client
            .post(self.method_url(method))
            .timeout(timeout)
            .json(&body);
        self.finish(method, request).await
    }

    /// Ruft eine Methode auf und verwirft das `result` (Methoden, die `True` liefern)
    async fn call_ok(&self, method: &str, body: Value) -> TelegramResult<()> {
        self.call(method, body, TIMEOUT).await.map(|_| ())
    }

    /// Schickt die vorbereitete Anfrage und wertet die Antwort aus
    async fn finish(
        &self,
        method: &str,
        request: reqwest::RequestBuilder,
    ) -> TelegramResult<Value> {
        let response = request
            .send()
            .await
            .map_err(|error| network(method, error))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|error| network(method, error))?;
        let envelope: ApiEnvelope = serde_json::from_str(&text).map_err(|_| {
            // Kein JSON: etwa die Fehlerseite eines Proxys. Der Körper könnte alles enthalten,
            // deshalb nur der Statuscode.
            TelegramError::Network(format!("{method} antwortete mit {status} und keinem JSON."))
        })?;
        if !envelope.ok {
            return Err(TelegramError::from_api(
                envelope.error_code.unwrap_or(i64::from(status)),
                envelope.description.unwrap_or_default(),
                envelope
                    .parameters
                    .and_then(|parameters| parameters.retry_after),
            ));
        }
        Ok(envelope.result.unwrap_or(Value::Null))
    }
}

/// Netzwerkfehler ohne Adresse: `reqwest::Error` nennt die URL und damit das Token
fn network(method: &str, error: reqwest::Error) -> TelegramError {
    let reason = if error.is_timeout() {
        "Zeitlimit überschritten".to_string()
    } else if error.is_connect() {
        "keine Verbindung".to_string()
    } else if error.is_decode() {
        "Antwort nicht lesbar".to_string()
    } else {
        "Anfrage fehlgeschlagen".to_string()
    };
    TelegramError::Network(format!("{method}: {reason}."))
}

/// Liest `result` in den erwarteten Typ
fn parse<T: serde::de::DeserializeOwned>(method: &str, result: Value) -> TelegramResult<T> {
    serde_json::from_value(result).map_err(|error| {
        TelegramError::Network(format!("{method} lieferte unerwartete Felder: {error}"))
    })
}

/// Trägt ein Feld nur ein, wenn es gesetzt ist
fn put(body: &mut Value, name: &str, value: Option<Value>) {
    if let (Some(object), Some(value)) = (body.as_object_mut(), value) {
        object.insert(name.to_string(), value);
    }
}

/// Die Tastatur als `reply_markup`
fn markup(keyboard: Option<InlineKeyboard>) -> Option<Value> {
    keyboard.and_then(|keyboard| serde_json::to_value(keyboard).ok())
}

#[async_trait]
impl TelegramApi for ReqwestTelegramApi {
    async fn get_me(&self) -> TelegramResult<User> {
        let result = self.call("getMe", json!({}), TIMEOUT).await?;
        parse("getMe", result)
    }

    async fn get_updates(&self, offset: i64, timeout_secs: u64) -> TelegramResult<Vec<Update>> {
        let body = json!({
            "offset": offset,
            "timeout": timeout_secs,
            "allowed_updates": ["message", "callback_query"],
        });
        // Telegram hält die Anfrage bis zu `timeout` offen; der Zuschlag deckt die Laufzeit
        let timeout = Duration::from_secs(timeout_secs) + POLL_EXTRA;
        let result = self.call("getUpdates", body, timeout).await?;
        parse("getUpdates", result)
    }

    async fn send_message(
        &self,
        chat_id: i64,
        html: &str,
        keyboard: Option<InlineKeyboard>,
        reply_to: Option<i64>,
    ) -> TelegramResult<i64> {
        let mut body = json!({
            "chat_id": chat_id,
            "text": html,
            "parse_mode": "HTML",
            "disable_web_page_preview": true,
        });
        put(&mut body, "reply_markup", markup(keyboard));
        put(&mut body, "reply_to_message_id", reply_to.map(Value::from));
        let result = self.call("sendMessage", body, TIMEOUT).await?;
        let message_id = result
            .get("message_id")
            .and_then(Value::as_i64)
            .ok_or_else(|| {
                TelegramError::Network("sendMessage lieferte keine message_id.".to_string())
            })?;
        Ok(message_id)
    }

    async fn edit_message_text(
        &self,
        chat_id: i64,
        message_id: i64,
        html: &str,
        keyboard: Option<InlineKeyboard>,
    ) -> TelegramResult<()> {
        let mut body = json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": html,
            "parse_mode": "HTML",
            "disable_web_page_preview": true,
        });
        put(&mut body, "reply_markup", markup(keyboard));
        self.call_ok("editMessageText", body).await
    }

    async fn answer_callback_query(
        &self,
        id: &str,
        text: Option<&str>,
        alert: bool,
    ) -> TelegramResult<()> {
        let mut body = json!({ "callback_query_id": id, "show_alert": alert });
        put(&mut body, "text", text.map(Value::from));
        self.call_ok("answerCallbackQuery", body).await
    }

    async fn send_chat_action(&self, chat_id: i64, action: &str) -> TelegramResult<()> {
        self.call_ok(
            "sendChatAction",
            json!({ "chat_id": chat_id, "action": action }),
        )
        .await
    }

    async fn get_file(&self, file_id: &str) -> TelegramResult<FileInfo> {
        let result = self
            .call("getFile", json!({ "file_id": file_id }), TIMEOUT)
            .await?;
        parse("getFile", result)
    }

    async fn download_file(&self, file_path: &str) -> TelegramResult<Vec<u8>> {
        let url = format!("{}/file/bot{}/{file_path}", self.api_base, self.token);
        let response = self
            .client
            .get(url)
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(|error| network("Download", error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(TelegramError::from_api(
                i64::from(status.as_u16()),
                "Download fehlgeschlagen".to_string(),
                None,
            ));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| network("Download", error))?;
        Ok(bytes.to_vec())
    }

    async fn set_my_commands(&self, commands: &[(String, String)]) -> TelegramResult<()> {
        let commands: Vec<Value> = commands
            .iter()
            .map(|(command, description)| json!({ "command": command, "description": description }))
            .collect();
        self.call_ok("setMyCommands", json!({ "commands": commands }))
            .await
    }

    async fn set_my_name(&self, name: &str) -> TelegramResult<()> {
        self.call_ok("setMyName", json!({ "name": name })).await
    }

    async fn set_my_description(&self, text: &str) -> TelegramResult<()> {
        self.call_ok("setMyDescription", json!({ "description": text }))
            .await
    }

    async fn set_my_short_description(&self, text: &str) -> TelegramResult<()> {
        self.call_ok(
            "setMyShortDescription",
            json!({ "short_description": text }),
        )
        .await
    }

    async fn set_my_profile_photo(&self, jpeg: Vec<u8>) -> TelegramResult<()> {
        // `InputProfilePhotoStatic`: das Feld `photo` verweist mit `attach://` auf den
        // Dateiteil desselben Multipart (Bot-API-Doku, Stand 2026-11).
        let photo = json!({ "type": "static", "photo": format!("attach://{PHOTO_PART}") });
        let file = reqwest::multipart::Part::bytes(jpeg)
            .file_name("avatar.jpg")
            .mime_str("image/jpeg")
            .map_err(|_| TelegramError::Network("Profilbild nicht lesbar.".to_string()))?;
        let form = reqwest::multipart::Form::new()
            .text("photo", photo.to_string())
            .part(PHOTO_PART, file);
        let request = self
            .client
            .post(self.method_url("setMyProfilePhoto"))
            .timeout(TIMEOUT)
            .multipart(form);
        self.finish("setMyProfilePhoto", request).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::{StatusCode, Uri};
    use axum::response::IntoResponse;
    use axum::routing::any;
    use axum::Router;

    use super::*;
    use crate::telegram::api::{InlineButton, InlineKeyboard};

    /// Ein Token im Muster von @BotFather; in keinem Fehlertext erlaubt
    const TOKEN: &str = "123456:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw";

    /// Eine Anfrage, wie der Stub sie gesehen hat
    #[derive(Debug, Clone)]
    struct Seen {
        path: String,
        body: String,
    }

    /// Was der Stub antwortet: Statuscode und Körper
    #[derive(Clone)]
    struct Reply {
        status: StatusCode,
        body: String,
    }

    /// Der geteilte Zustand des Stubs
    #[derive(Clone)]
    struct Stub {
        seen: Arc<Mutex<Vec<Seen>>>,
        /// Antworten der Reihe nach; die letzte gilt für alle weiteren Anfragen
        replies: Arc<Mutex<Vec<Reply>>>,
    }

    impl Stub {
        /// Die Anfragen in ihrer Reihenfolge
        fn seen(&self) -> Vec<Seen> {
            self.seen.lock().expect("Sperre").clone()
        }
    }

    /// Nimmt jede Anfrage auf und liefert die nächste eingestellte Antwort
    async fn handle(State(stub): State<Stub>, uri: Uri, body: Bytes) -> impl IntoResponse {
        stub.seen.lock().expect("Sperre").push(Seen {
            path: uri.path().to_string(),
            // Multipart ist kein reines UTF-8; für die Prüfung genügt die verlustfreie Sicht
            body: String::from_utf8_lossy(&body).to_string(),
        });
        let mut replies = stub.replies.lock().expect("Sperre");
        let reply = match replies.len() {
            0 | 1 => replies.first().cloned(),
            _ => Some(replies.remove(0)),
        };
        let reply = reply.unwrap_or(Reply {
            status: StatusCode::OK,
            body: r#"{"ok":true,"result":true}"#.to_string(),
        });
        (
            reply.status,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            reply.body,
        )
    }

    /// Startet einen Stub der Bot-API auf 127.0.0.1 mit einem freien Port
    async fn start(replies: Vec<(StatusCode, &str)>) -> (ReqwestTelegramApi, Stub) {
        let stub = Stub {
            seen: Arc::new(Mutex::new(Vec::new())),
            replies: Arc::new(Mutex::new(
                replies
                    .into_iter()
                    .map(|(status, body)| Reply {
                        status,
                        body: body.to_string(),
                    })
                    .collect(),
            )),
        };
        let app = Router::new().fallback(any(handle)).with_state(stub.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Port");
        let port = listener.local_addr().expect("Adresse").port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let api = ReqwestTelegramApi::new(format!("http://127.0.0.1:{port}"), TOKEN);
        (api, stub)
    }

    /// Stub, der jedem Aufruf mit `ok: true` und `result` antwortet
    async fn start_ok(result: &str) -> (ReqwestTelegramApi, Stub) {
        start(vec![(
            StatusCode::OK,
            &format!(r#"{{"ok":true,"result":{result}}}"#),
        )])
        .await
    }

    /// Eine Adresse, auf der nichts hört (der Listener ist sofort wieder zu)
    async fn closed_port() -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Port");
        listener.local_addr().expect("Adresse").port()
    }

    #[tokio::test]
    async fn schickt_eine_nachricht_als_html_und_liefert_die_message_id() {
        let (api, stub) = start_ok(r#"{"message_id":42}"#).await;

        let message_id = api
            .send_message(
                -100,
                "<b>fertig</b>",
                Some(InlineKeyboard::row(vec![InlineButton::callback(
                    "Merge", "merge:1",
                )])),
                Some(7),
            )
            .await
            .unwrap();

        assert_eq!(message_id, 42);
        let seen = stub.seen();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].path, format!("/bot{TOKEN}/sendMessage"));
        let body: Value = serde_json::from_str(&seen[0].body).unwrap();
        assert_eq!(body["chat_id"], -100);
        assert_eq!(body["text"], "<b>fertig</b>");
        assert_eq!(body["parse_mode"], "HTML");
        assert_eq!(body["disable_web_page_preview"], true);
        assert_eq!(body["reply_to_message_id"], 7);
        assert_eq!(
            body["reply_markup"]["inline_keyboard"][0][0]["callback_data"],
            "merge:1"
        );
    }

    #[tokio::test]
    async fn lasst_fehlende_angaben_aus_dem_korper_weg() {
        let (api, stub) = start_ok(r#"{"message_id":1}"#).await;

        api.send_message(1, "hallo", None, None).await.unwrap();

        let body: Value = serde_json::from_str(&stub.seen()[0].body).unwrap();
        assert!(body.get("reply_markup").is_none(), "{body}");
        assert!(body.get("reply_to_message_id").is_none(), "{body}");
    }

    #[tokio::test]
    async fn holt_updates_mit_offset_und_zeitlimit() {
        let (api, stub) = start_ok(
            r#"[{"update_id":11,"message":{"message_id":5,"chat":{"id":42,"type":"private"},"text":"/status"}},
                {"update_id":12,"callback_query":{"id":"cb1","from":{"id":7,"first_name":"Ben"},"data":"merge:1"}}]"#,
        )
        .await;

        let updates = api.get_updates(10, 25).await.unwrap();

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].message.as_ref().unwrap().chat.id, 42);
        assert_eq!(
            updates[1].callback_query.as_ref().unwrap().data.as_deref(),
            Some("merge:1")
        );
        let seen = stub.seen();
        assert_eq!(seen[0].path, format!("/bot{TOKEN}/getUpdates"));
        let body: Value = serde_json::from_str(&seen[0].body).unwrap();
        assert_eq!(body["offset"], 10);
        assert_eq!(body["timeout"], 25);
    }

    #[tokio::test]
    async fn liest_getme_und_getfile() {
        let (api, _) =
            start_ok(r#"{"id":7012345678,"first_name":"Nexus","username":"nexus_bot"}"#).await;
        assert_eq!(api.get_me().await.unwrap().id, 7012345678);

        let (api, stub) =
            start_ok(r#"{"file_id":"AwA","file_path":"voice/file_1.oga","file_size":9}"#).await;
        let info = api.get_file("AwA").await.unwrap();

        assert_eq!(info.file_path.as_deref(), Some("voice/file_1.oga"));
        assert_eq!(info.file_size, Some(9));
        let body: Value = serde_json::from_str(&stub.seen()[0].body).unwrap();
        assert_eq!(body["file_id"], "AwA");
    }

    #[tokio::test]
    async fn ladt_eine_datei_uber_den_dateipfad_herunter() {
        let (api, stub) = start(vec![(StatusCode::OK, "OggS-Daten")]).await;

        let bytes = api.download_file("voice/file_1.oga").await.unwrap();

        assert_eq!(bytes, b"OggS-Daten".to_vec());
        assert_eq!(
            stub.seen()[0].path,
            format!("/file/bot{TOKEN}/voice/file_1.oga")
        );
    }

    #[tokio::test]
    async fn meldet_eine_fehlerantwort_mit_code_und_text() {
        let (api, _) = start(vec![(
            StatusCode::BAD_REQUEST,
            r#"{"ok":false,"error_code":400,"description":"message text is empty"}"#,
        )])
        .await;

        let error = api.send_message(1, "", None, None).await.unwrap_err();

        assert_eq!(
            error,
            TelegramError::Api {
                code: 400,
                description: "message text is empty".to_string()
            }
        );
    }

    #[tokio::test]
    async fn erkennt_401_409_und_429_mit_retry_after() {
        let (api, _) = start(vec![(
            StatusCode::UNAUTHORIZED,
            r#"{"ok":false,"error_code":401,"description":"Unauthorized"}"#,
        )])
        .await;
        assert_eq!(api.get_me().await.unwrap_err(), TelegramError::Unauthorized);

        let (api, _) = start(vec![(
            StatusCode::CONFLICT,
            r#"{"ok":false,"error_code":409,"description":"terminated by other getUpdates request"}"#,
        )])
        .await;
        assert_eq!(
            api.get_updates(0, 1).await.unwrap_err(),
            TelegramError::Conflict
        );

        let (api, _) = start(vec![(
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"ok":false,"error_code":429,"description":"Too Many Requests: retry after 12","parameters":{"retry_after":12}}"#,
        )])
        .await;
        assert_eq!(
            api.send_message(1, "x", None, None).await.unwrap_err(),
            TelegramError::TooManyRequests { retry_after: 12 }
        );
    }

    #[tokio::test]
    async fn baut_setmyprofilephoto_als_multipart_mit_attach() {
        let (api, stub) = start(vec![(StatusCode::OK, r#"{"ok":true,"result":true}"#)]).await;

        api.set_my_profile_photo(b"\xff\xd8JPEG-Daten".to_vec())
            .await
            .unwrap();

        let seen = stub.seen();
        assert_eq!(seen[0].path, format!("/bot{TOKEN}/setMyProfilePhoto"));
        let body = &seen[0].body;
        assert!(body.contains(r#"name="photo""#), "{body}");
        assert!(body.contains(r#""type":"static""#), "{body}");
        assert!(body.contains(r#""photo":"attach://avatar""#), "{body}");
        assert!(body.contains(r#"name="avatar""#), "{body}");
        assert!(body.contains(r#"filename="avatar.jpg""#), "{body}");
        assert!(body.contains("Content-Type: image/jpeg"), "{body}");
        assert!(body.contains("JPEG-Daten"), "{body}");
    }

    #[tokio::test]
    async fn setzt_befehle_und_profiltexte() {
        let (api, stub) = start(vec![]).await;

        api.set_my_commands(&[("status".to_string(), "Zeigt den Stand".to_string())])
            .await
            .unwrap();
        api.set_my_name("Nexus").await.unwrap();
        api.set_my_description("Dein Harness").await.unwrap();
        api.set_my_short_description("Harness").await.unwrap();
        api.edit_message_text(1, 2, "<b>neu</b>", None)
            .await
            .unwrap();
        api.answer_callback_query("cb1", Some("Gemerged"), true)
            .await
            .unwrap();
        api.send_chat_action(1, "typing").await.unwrap();

        let seen = stub.seen();
        let paths: Vec<String> = seen.iter().map(|one| one.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                format!("/bot{TOKEN}/setMyCommands"),
                format!("/bot{TOKEN}/setMyName"),
                format!("/bot{TOKEN}/setMyDescription"),
                format!("/bot{TOKEN}/setMyShortDescription"),
                format!("/bot{TOKEN}/editMessageText"),
                format!("/bot{TOKEN}/answerCallbackQuery"),
                format!("/bot{TOKEN}/sendChatAction"),
            ]
        );
        let commands: Value = serde_json::from_str(&seen[0].body).unwrap();
        assert_eq!(commands["commands"][0]["command"], "status");
        assert_eq!(commands["commands"][0]["description"], "Zeigt den Stand");
        let callback: Value = serde_json::from_str(&seen[5].body).unwrap();
        assert_eq!(callback["callback_query_id"], "cb1");
        assert_eq!(callback["text"], "Gemerged");
        assert_eq!(callback["show_alert"], true);
    }

    #[tokio::test]
    async fn eine_antwort_ohne_json_bleibt_ein_netzfehler_ohne_token() {
        let (api, _) = start(vec![(
            StatusCode::BAD_GATEWAY,
            "<html>Proxy-Fehler für /bot123456:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw/getMe</html>",
        )])
        .await;

        let error = api.get_me().await.unwrap_err();

        let message = error.to_string();
        assert!(message.contains("502"), "{message}");
        assert!(!message.contains(TOKEN), "{message}");
    }

    #[tokio::test]
    async fn ein_netzfehler_nennt_weder_adresse_noch_token() {
        let port = closed_port().await;
        let api = ReqwestTelegramApi::new(format!("http://127.0.0.1:{port}"), TOKEN);

        let send = api.send_message(1, "x", None, None).await.unwrap_err();
        let download = api.download_file("voice/file_1.oga").await.unwrap_err();

        for error in [send, download] {
            let message = error.to_string();
            assert!(matches!(error, TelegramError::Network(_)), "{message}");
            assert!(!message.contains(TOKEN), "{message}");
            assert!(!message.contains("127.0.0.1"), "{message}");
        }
    }
}
