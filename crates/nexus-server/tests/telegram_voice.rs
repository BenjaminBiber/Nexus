//! Sprachnachrichten im Bot: herunterladen, erkennen und als Eingabe weitergeben.
//!
//! Der Weg läuft über den Router des Bots im Serverzustand, also über die eingerichtete
//! Erkennung aus `telegram.yaml`. Telegram ist die Attrappe `FakeTelegram`, der
//! Erkennungsdienst eine Attrappe aus axum im eigenen Prozess – nichts verlässt den Rechner.

mod common;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use common::{BoardBackendWithServices, TestServer, TestServerOptions};
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
use nexus_core::telegram::{SpeechBackend, SpeechConfig, TelegramConfig, TelegramConfigStore};
use nexus_server::telegram::api::{FileInfo, Message, TelegramApi};
use nexus_server::telegram::fake::FakeTelegram;
use nexus_server::telegram::BotContext;
use serde_json::{json, Value};

/// Der Chat, aus dem die Tests schreiben
const CHAT: i64 = 4242;

/// Der Pfad, unter dem die Attrappe die Aufnahme liefert
const FILE_PATH: &str = "voice/file_1.oga";

/// Attrappe eines Erkennungsdienstes mit der Schnittstelle von OpenAI
struct SpeechStub {
    addr: SocketAddr,
    /// Zahl der angenommenen Anfragen
    count: Arc<Mutex<usize>>,
}

impl SpeechStub {
    /// Startet die Attrappe; sie antwortet immer mit diesem Text
    async fn start(text: &'static str) -> Self {
        let count = Arc::new(Mutex::new(0usize));
        let seen = count.clone();
        let app = Router::new().route(
            "/v1/audio/transcriptions",
            post(move |_body: axum::body::Bytes| {
                let seen = seen.clone();
                async move {
                    *seen.lock().expect("Sperre") += 1;
                    (
                        StatusCode::OK,
                        json!({ "text": text, "language": "german" }).to_string(),
                    )
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("Port der Attrappe");
        let addr = listener.local_addr().expect("Adresse der Attrappe");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self { addr, count }
    }

    /// Die Adresse, die in die Einstellung gehört
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Zahl der Anfragen an die Attrappe
    fn count(&self) -> usize {
        *self.count.lock().expect("Sperre")
    }
}

/// Server, Board und Attrappen für einen Test der Sprachnachrichten
struct Bot {
    server: TestServer,
    backend: Arc<BoardBackendWithServices>,
    fake: Arc<FakeTelegram>,
    board_id: i32,
}

impl Bot {
    /// Ein Projekt mit einem aktiven Board; die Erkennung zeigt auf die Attrappe (`None`
    /// schaltet sie aus)
    async fn start(stub: Option<&SpeechStub>) -> Self {
        let (server, backend) =
            TestServer::start_with_board_and(TestServerOptions::default()).await;
        let actor = backend.actor();
        backend
            .services
            .projects
            .create("NXS", "Nexus", &actor)
            .await
            .expect("Projekt");
        let board = backend
            .services
            .boards
            .create("NXS", "Sprint", &actor)
            .await
            .expect("Board");
        let board_id = board.board.id;
        server
            .post(&format!("/api/db/boards/{board_id}/activate"), "{}")
            .await;
        let config = TelegramConfig {
            speech: SpeechConfig {
                backend: match stub {
                    Some(_) => SpeechBackend::OpenaiCompatible,
                    None => SpeechBackend::Off,
                },
                base_url: stub.map(SpeechStub::url).unwrap_or_default(),
                api_key: "sk-geheim-1234".to_string(),
                model: "whisper-1".to_string(),
                language: Some("de".to_string()),
                ..SpeechConfig::default()
            },
            ..TelegramConfig::default()
        };
        TelegramConfigStore::new(server.home.clone())
            .write(&config)
            .expect("telegram.yaml schreiben");
        let fake = Arc::new(FakeTelegram::new());
        fake.put_file_info(
            "AwA",
            FileInfo {
                file_path: Some(FILE_PATH.to_string()),
                file_size: Some(13),
            },
        );
        fake.put_file(FILE_PATH, b"OggS-Attrappe".to_vec());
        Self {
            server,
            backend,
            fake,
            board_id,
        }
    }

    /// Schickt eine Sprachnachricht (`duration` in Sekunden, `file_size` wie angekündigt)
    async fn speak(&self, duration: i64, file_size: Option<i64>) {
        let message: Message = serde_json::from_value(json!({
            "message_id": 5,
            "chat": { "id": CHAT, "type": "private" },
            "voice": {
                "file_id": "AwA",
                "duration": duration,
                "mime_type": "audio/ogg",
                "file_size": file_size,
            },
        }))
        .expect("Nachricht");
        self.handle(&message).await;
    }

    /// Schickt eine Textnachricht
    async fn send(&self, text: &str) {
        let message: Message = serde_json::from_value(json!({
            "message_id": 6,
            "chat": { "id": CHAT, "type": "private" },
            "text": text,
        }))
        .expect("Nachricht");
        self.handle(&message).await;
    }

    /// Gibt die Nachricht an den Router des Bots
    async fn handle(&self, message: &Message) {
        self.server
            .state
            .telegram
            .router()
            .handle_message(&self.context(), message)
            .await
            .expect("Nachricht");
    }

    /// Drückt einen Knopf mit diesen Callback-Daten
    async fn press(&self, data: &str) {
        let query = serde_json::from_value(json!({
            "id": "cb-1",
            "from": { "id": 7, "first_name": "Ben" },
            "message": { "message_id": 5, "chat": { "id": CHAT, "type": "private" } },
            "data": data,
        }))
        .expect("Knopfdruck");
        self.server
            .state
            .telegram
            .router()
            .handle_callback(&self.context(), &query)
            .await
            .expect("Knopf");
    }

    /// Der Kontext des freigegebenen Chats auf der Attrappe
    fn context(&self) -> BotContext {
        BotContext {
            api: self.fake.clone() as Arc<dyn TelegramApi>,
            chat_id: CHAT,
            message_id: Some(5),
            bot_name: Some("nexus_test_bot".to_string()),
        }
    }

    /// Die Texte aller geschickten Nachrichten
    fn sent(&self) -> Vec<String> {
        self.calls("sendMessage")
            .into_iter()
            .map(|params| params["text"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// Alles Geschickte und Geänderte in einem Text
    fn text(&self) -> String {
        let mut all = self.sent();
        all.extend(
            self.calls("editMessageText")
                .into_iter()
                .map(|params| params["text"].as_str().unwrap_or_default().to_string()),
        );
        all.join("\n")
    }

    /// Die Parameter aller Aufrufe dieser Methode
    fn calls(&self, method: &str) -> Vec<Value> {
        self.fake
            .calls_of(method)
            .into_iter()
            .map(|call| call.params)
            .collect()
    }

    /// Vergisst alles Aufgezeichnete
    fn clear(&self) {
        self.fake.take_calls();
    }

    /// Ein blockiertes Ticket, das auf eine Entscheidung wartet
    async fn blocked(&self, title: &str) -> String {
        let actor = self.backend.actor();
        let ticket = self
            .backend
            .services
            .tickets
            .create(
                self.board_id,
                &NewTicketInput {
                    acceptance: Some("ok".into()),
                    needs_planning: Some(false),
                    ..NewTicketInput::titled(title)
                },
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket");
        let id = ticket.ticket.id.clone();
        self.backend
            .services
            .tickets
            .update(
                &id,
                &TicketPatch {
                    status: Some(TicketStatus::Blocked),
                    blocked_reason: Some(Some("Zugang fehlt".into())),
                    ..TicketPatch::default()
                },
                &actor,
                &WriteOptions {
                    force: true,
                    ..WriteOptions::default()
                },
            )
            .await
            .expect("Ticket blockieren");
        id
    }

    /// Die Kommentare eines Tickets
    async fn comments(&self, id: &str) -> Vec<String> {
        self.backend
            .services
            .comments
            .list(id)
            .await
            .expect("Kommentare")
            .into_iter()
            .map(|comment| comment.body)
            .collect()
    }
}

#[tokio::test]
async fn eine_sprachnachricht_wird_geladen_transkribiert_und_beantwortet() {
    let stub = SpeechStub::start("Setz das Ticket auf blockiert").await;
    let bot = Bot::start(Some(&stub)).await;

    bot.speak(3, Some(13)).await;

    // „Tippt…“, dann die Datei holen und laden
    assert_eq!(bot.calls("sendChatAction")[0]["action"], "typing");
    assert_eq!(bot.calls("getFile").len(), 1);
    assert_eq!(bot.calls("downloadFile")[0]["file_path"], FILE_PATH);
    assert_eq!(stub.count(), 1, "genau eine Anfrage an die Erkennung");
    let texte = bot.sent();
    assert_eq!(texte[0], "🎙️ Verstanden: „Setz das Ticket auf blockiert“");
    assert_eq!(bot.calls("sendMessage")[0]["reply_to_message_id"], 5);
}

#[tokio::test]
async fn in_einem_offenen_dialog_wird_das_transkript_zur_antwort() {
    let stub = SpeechStub::start("Der Zugang kommt morgen").await;
    let bot = Bot::start(Some(&stub)).await;
    let id = bot.blocked("Login hängt").await;

    bot.press(&format!("cmt:{id}")).await;
    bot.clear();
    bot.speak(4, Some(13)).await;

    let text = bot.text();
    assert!(
        text.contains("🎙️ Verstanden: „Der Zugang kommt morgen“"),
        "{text}"
    );
    assert!(
        text.contains("✅ Kommentar geschrieben über Telegram, "),
        "{text}"
    );
    assert_eq!(
        bot.comments(&id).await,
        vec!["Der Zugang kommt morgen (über Telegram)".to_string()]
    );
}

#[tokio::test]
async fn ohne_offenen_dialog_geht_das_transkript_denselben_weg_wie_ein_getippter_text() {
    let stub = SpeechStub::start("eine neue Idee").await;
    let bot = Bot::start(Some(&stub)).await;

    bot.send("eine neue Idee").await;
    let getippt = bot.sent();
    bot.clear();
    bot.speak(3, Some(13)).await;
    let gesprochen = bot.sent();

    // Nach dem „Verstanden“ antwortet derselbe Weg wie auf den getippten Text; den freien
    // Text nimmt noch kein Handler an, also kommt der Hinweis
    assert_eq!(getippt.len(), 1, "{getippt:?}");
    assert_eq!(gesprochen.len(), 2, "{gesprochen:?}");
    assert!(
        gesprochen[0].starts_with("🎙️ Verstanden:"),
        "{gesprochen:?}"
    );
    assert_eq!(gesprochen[1], getippt[0]);
}

#[tokio::test]
async fn eine_zu_lange_aufnahme_wird_nicht_geladen_und_verstandlich_beantwortet() {
    let stub = SpeechStub::start("nie erkannt").await;
    let bot = Bot::start(Some(&stub)).await;

    bot.speak(600, Some(13)).await;

    assert!(bot.calls("downloadFile").is_empty(), "nichts geladen");
    assert_eq!(stub.count(), 0, "nichts erkannt");
    let texte = bot.sent();
    assert_eq!(texte.len(), 1);
    assert!(texte[0].contains("300 s"), "{}", texte[0]);
    assert!(texte[0].contains("Tippen geht immer."), "{}", texte[0]);
}

#[tokio::test]
async fn ausgeschaltete_erkennung_antwortet_mit_dem_hinweis_auf_das_tippen() {
    let bot = Bot::start(None).await;

    bot.speak(3, Some(13)).await;

    let texte = bot.sent();
    assert_eq!(texte.len(), 1);
    assert!(
        texte[0].contains("Spracherkennung ist ausgeschaltet."),
        "{}",
        texte[0]
    );
    assert!(texte[0].contains("Tippen geht immer."), "{}", texte[0]);
}
