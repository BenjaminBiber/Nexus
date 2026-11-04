//! Der Ideen-Dialog im Bot: aus `/idee`, freiem Text oder einer Sprachnachricht wird ein
//! Entwurf, den der Chat bestätigt, korrigiert und als Notiz, Ticket oder Kommentar anlegt.
//!
//! Alles läuft über den Router des Bots im Serverzustand – also über dieselben Dienste wie
//! die Routen. Telegram ist die Attrappe `FakeTelegram`, die Spracherkennung eine Attrappe
//! aus axum im eigenen Prozess und Claude der Ersatz `nexus-fake-claude`. Seine Antwort steht
//! in einer Datei, die der Test vor jeder Runde neu schreibt (`FAKE_CLAUDE_REPLY_FILE`);
//! nichts verlässt den Rechner.

mod common;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use chrono::{DateTime, TimeDelta, Utc};
use common::{memory_config, BoardBackendWithServices, TestServer, TestServerOptions};
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use nexus_core::clock::Clock;
use nexus_core::dispatch::DispatchConfigStore;
use nexus_core::telegram::{
    IdeaSettings, IdeaTarget, SpeechBackend, SpeechConfig, TelegramConfig, TelegramConfigStore,
};
use nexus_core::HarnessHome;
use nexus_server::board_runtime::{factory, BoardBackend, BoardBackendFuture};
use nexus_server::telegram::api::{FileInfo, Message, TelegramApi};
use nexus_server::telegram::fake::FakeTelegram;
use nexus_server::telegram::{BotContext, IdeaInterpreter, IDEA_EXPIRY_HOURS};
use serde_json::{json, Value};

/// Der Chat, aus dem die Tests schreiben
const CHAT: i64 = 4242;

/// Die Uhr aller Tests
const NOW: &str = "2026-11-03T12:00:00Z";

/// Der Pfad, unter dem die Telegram-Attrappe die Aufnahme liefert
const FILE_PATH: &str = "voice/file_1.oga";

/// Attrappe eines Erkennungsdienstes mit der Schnittstelle von OpenAI
struct SpeechStub {
    addr: SocketAddr,
}

impl SpeechStub {
    /// Startet die Attrappe; sie antwortet immer mit diesem Text
    async fn start(text: &'static str) -> Self {
        let app = Router::new().route(
            "/v1/audio/transcriptions",
            post(move |_body: axum::body::Bytes| async move {
                (
                    StatusCode::OK,
                    json!({ "text": text, "language": "german" }).to_string(),
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("Port der Attrappe");
        let addr = listener.local_addr().expect("Adresse der Attrappe");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self { addr }
    }

    /// Die Adresse, die in die Einstellung gehört
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

/// Server, Board und Attrappen für einen Test des Ideen-Dialogs
struct Bot {
    server: TestServer,
    backend: Arc<BoardBackendWithServices>,
    fake: Arc<FakeTelegram>,
    /// Das aktive Board
    board_id: i32,
    /// Ein zweites eigenes Board, auf das sich der Entwurf schieben lässt
    other_board_id: i32,
    /// Datei mit der nächsten Antwort des Ersatz-Claude
    reply: PathBuf,
    /// Stunden, die seit [`NOW`] vergangen sind (für den Ablauf eines Entwurfs)
    hours: Arc<AtomicI64>,
    home: HarnessHome,
    _home_dir: tempfile::TempDir,
}

impl Bot {
    /// Ein Projekt mit zwei Boards, zwei Tickets und diesen Einstellungen für die Ideen
    async fn start(ideas: IdeaSettings, stub: Option<&SpeechStub>) -> Self {
        let home_dir = tempfile::Builder::new()
            .prefix("nexus-ideen-")
            .tempdir()
            .expect("Benutzerordner");
        let home = HarnessHome::at(home_dir.path());
        // Der Ersatz liest seine Antwort aus dieser Datei; so wechselt sie zwischen zwei Runden
        let reply = home_dir.path().join("claude-antwort.txt");
        DispatchConfigStore::new(home.clone())
            .write(&json!({
                "claude": { "command": env!("CARGO_BIN_EXE_nexus-fake-claude") },
            }))
            .expect("dispatch.yaml schreiben");
        let env: BTreeMap<String, String> = [
            ("FAKE_CLAUDE_MODE".to_string(), "antwort".to_string()),
            (
                "FAKE_CLAUDE_REPLY_FILE".to_string(),
                reply.display().to_string(),
            ),
        ]
        .into();

        let hours = Arc::new(AtomicI64::new(0));
        let read = hours.clone();
        let start: DateTime<Utc> = NOW.parse().expect("Zeitpunkt");
        let (server, backend) = TestServer::start_with_board_and(TestServerOptions {
            home: Some(home.clone()),
            clock: Some(Clock::from_fn(move || {
                start + TimeDelta::hours(read.load(Ordering::Relaxed))
            })),
            idea_interpreter: Some(Arc::new(IdeaInterpreter::with_env(home.clone(), env))),
            ..TestServerOptions::default()
        })
        .await;

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
        let other = backend
            .services
            .boards
            .create("NXS", "Ideen", &actor)
            .await
            .expect("zweites Board");
        let board_id = board.board.id;
        server
            .post(&format!("/api/db/boards/{board_id}/activate"), "{}")
            .await;
        for title in ["Board-Ansicht bauen", "Alte Spalte weg"] {
            backend
                .services
                .tickets
                .create(
                    board_id,
                    &NewTicketInput {
                        status: Some(TicketStatus::Backlog),
                        acceptance: Some("Man sieht es in der Ansicht.".into()),
                        ..NewTicketInput::titled(title)
                    },
                    &actor,
                    &WriteOptions::default(),
                )
                .await
                .expect("Ticket");
        }

        TelegramConfigStore::new(home.clone())
            .write(&TelegramConfig {
                ideas,
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
            })
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
            other_board_id: other.board.id,
            reply,
            hours,
            home,
            _home_dir: home_dir,
        }
    }

    /// Der Bot mit den Vorgaben der Einstellung und ohne Spracherkennung
    async fn simple() -> Self {
        Self::start(IdeaSettings::default(), None).await
    }

    /// Legt fest, was der Ersatz-Claude in der nächsten Runde antwortet
    fn answers(&self, draft: Value) {
        std::fs::write(&self.reply, format!("Hier ist der Entwurf: {draft}"))
            .expect("Antwort schreiben");
    }

    /// Stellt die Uhr des Servers um so viele Stunden vor
    fn pass(&self, hours: i64) {
        self.hours.store(hours, Ordering::Relaxed);
    }

    /// Startet den Server auf demselben Benutzerordner neu; das Board bleibt dasselbe
    fn restart(&self) -> TestServer {
        let shared = self.backend.clone();
        let start: DateTime<Utc> = NOW.parse().expect("Zeitpunkt");
        TestServer::with(TestServerOptions {
            home: Some(self.home.clone()),
            clock: Some(Clock::fixed(start)),
            board: Some(factory(move || -> BoardBackendFuture {
                let backend = shared.clone();
                Box::pin(async move { Ok(backend as Arc<dyn BoardBackend>) })
            })),
            ..TestServerOptions::default()
        })
    }

    /// Schickt eine Textnachricht an den Router
    async fn send(&self, text: &str) {
        self.send_to(&self.server, text).await;
    }

    /// Wie [`Bot::send`], an den Router dieses Servers (für den Neustart)
    async fn send_to(&self, server: &TestServer, text: &str) {
        let message: Message = serde_json::from_value(json!({
            "message_id": 6,
            "chat": { "id": CHAT, "type": "private" },
            "text": text,
        }))
        .expect("Nachricht");
        server
            .state
            .telegram
            .router()
            .handle_message(&self.context(), &message)
            .await
            .expect("Nachricht");
    }

    /// Schickt eine Sprachnachricht; die Attrappe der Erkennung macht Text daraus
    async fn speak(&self) {
        let message: Message = serde_json::from_value(json!({
            "message_id": 5,
            "chat": { "id": CHAT, "type": "private" },
            "voice": { "file_id": "AwA", "duration": 4, "mime_type": "audio/ogg", "file_size": 13 },
        }))
        .expect("Nachricht");
        self.server
            .state
            .telegram
            .router()
            .handle_message(&self.context(), &message)
            .await
            .expect("Sprachnachricht");
    }

    /// Drückt den Knopf mit dieser Beschriftung aus der letzten Bestätigung; was bis dahin
    /// aufgezeichnet wurde, ist danach vergessen
    async fn tap(&self, label: &str) {
        let data = self.button(label);
        self.clear();
        self.press(&data).await;
    }

    /// Drückt einen Knopf mit diesen Callback-Daten
    async fn press(&self, data: &str) {
        self.press_on(&self.server, data).await;
    }

    /// Wie [`Bot::press`], am Router dieses Servers (für den Neustart)
    async fn press_on(&self, server: &TestServer, data: &str) {
        let query = serde_json::from_value(json!({
            "id": "cb-1",
            "from": { "id": 7, "first_name": "Ben" },
            "message": { "message_id": 5, "chat": { "id": CHAT, "type": "private" } },
            "data": data,
        }))
        .expect("Knopfdruck");
        server
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

    /// Text und Knöpfe der letzten geänderten Nachricht
    fn edit(&self) -> (String, Vec<(String, String)>) {
        self.calls("editMessageText")
            .into_iter()
            .map(|params| {
                (
                    params["text"].as_str().unwrap_or_default().to_string(),
                    buttons(&params["reply_markup"]),
                )
            })
            .next_back()
            .unwrap_or_else(|| panic!("keine geänderte Nachricht; geschickt: {:?}", self.sent()))
    }

    /// Die Knöpfe der letzten geänderten Nachricht als Beschriftungen
    fn labels(&self) -> Vec<String> {
        self.edit().1.into_iter().map(|(text, _)| text).collect()
    }

    /// Die Daten des Knopfes mit dieser Beschriftung, aus der letzten geänderten Nachricht
    fn button(&self, label: &str) -> String {
        self.edit()
            .1
            .into_iter()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| panic!("Knopf „{label}“ fehlt in {:?}", self.labels()))
            .1
    }

    /// Die Texte der Hinweisfenster an den Knöpfen (nur die mit Text)
    fn alerts(&self) -> Vec<String> {
        self.calls("answerCallbackQuery")
            .into_iter()
            .filter_map(|params| params["text"].as_str().map(str::to_string))
            .collect()
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

    /// Die Notizen des Boards als (Titel, Text)
    async fn notes(&self, board_id: i32) -> Vec<(String, String)> {
        self.backend
            .services
            .notes
            .list(board_id, None, None)
            .await
            .expect("Notizen")
            .into_iter()
            .map(|note| (note.title, note.body))
            .collect()
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

/// Beschriftung und Daten der Knöpfe einer Nachricht
fn buttons(markup: &Value) -> Vec<(String, String)> {
    markup["inline_keyboard"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .flat_map(|row| row.as_array().cloned().unwrap_or_default())
                .map(|button| {
                    (
                        button["text"].as_str().unwrap_or_default().to_string(),
                        button["callback_data"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Ein Entwurf, wie Claude ihn liefert; `extra` überschreibt einzelne Felder
fn entwurf(board_id: i32, extra: Value) -> Value {
    let mut draft = json!({
        "summary": "Der Cache der Board-Ansicht läuft nie ab.",
        "target": { "kind": "note", "boardId": board_id, "ticketId": null },
        "title": "Cache der Board-Ansicht läuft nie ab",
        "details": "Fällt in der Spaltenansicht auf; Vorschlag: Lebenszeit setzen.",
        "related": [{ "ticketId": "NXS-0001", "why": "baut dieselbe Ansicht" }],
        "duplicates": [],
        "questions": ["Auch für SQL Server?"],
        "confidence": 0.7,
    });
    for (key, value) in extra.as_object().cloned().unwrap_or_default() {
        draft[key] = value;
    }
    draft
}

#[tokio::test]
async fn eine_sprachnachricht_wird_ueber_zwei_korrekturrunden_zur_notiz() {
    let stub = SpeechStub::start("Der Cache der Board-Ansicht läuft nie ab").await;
    let bot = Bot::start(IdeaSettings::default(), Some(&stub)).await;
    bot.answers(entwurf(bot.board_id, json!({})));

    bot.speak().await;

    // Die Bestätigung nennt Ziel samt Board, Bezüge und die Rückfrage – und fragt gleich
    let (text, knoepfe) = bot.edit();
    assert!(text.starts_with("<b>Bevor ich es anlege:</b>"), "{text}");
    assert!(text.contains("Auch für SQL Server?"), "{text}");
    assert!(
        text.contains(&format!("<b>Notiz auf Board {} „Sprint“</b>", bot.board_id)),
        "{text}"
    );
    assert!(text.contains("NXS-0001 – baut dieselbe Ansicht"), "{text}");
    assert!(text.contains("<i>Runde 1</i>"), "{text}");
    assert_eq!(
        knoepfe.iter().map(|(label, _)| label).collect::<Vec<_>>(),
        [
            "Anlegen",
            "Als Ticket",
            "Anderes Board",
            "Korrigieren",
            "Verwerfen"
        ]
    );
    assert!(
        bot.sent().iter().any(|text| text.contains("🤔")),
        "{:?}",
        bot.sent()
    );

    // Erste Korrekturrunde: die Antwort auf die Rückfrage, ohne dass ein Knopf nötig wäre
    bot.answers(entwurf(
        bot.board_id,
        json!({ "title": "Cache der Board-Ansicht läuft nie ab (nur MSSQL)", "questions": [] }),
    ));
    bot.clear();
    bot.send("Nein, nur für MSSQL").await;
    let (text, _) = bot.edit();
    assert!(text.contains("(nur MSSQL)"), "{text}");
    assert!(text.contains("<i>Runde 2</i>"), "{text}");

    // Zweite Korrekturrunde: diesmal über den Knopf
    bot.tap("Korrigieren").await;
    assert!(
        bot.sent().iter().any(|text| text.contains("✍️")),
        "{:?}",
        bot.sent()
    );
    bot.answers(entwurf(
        bot.board_id,
        json!({
            "title": "Cache der Board-Ansicht läuft nie ab (nur MSSQL)",
            "details": "Die Spaltenansicht zeigt alte Zahlen; Lebenszeit von 5 Minuten setzen.",
            "questions": [],
        }),
    ));
    bot.clear();
    bot.send("Schreib dazu: Lebenszeit 5 Minuten").await;
    let (text, _) = bot.edit();
    assert!(text.contains("Lebenszeit von 5 Minuten"), "{text}");
    assert!(text.contains("<i>Runde 3</i>"), "{text}");

    // Anlegen: die Notiz trägt die Details und darunter das Transkript
    let anlegen = bot.button("Anlegen");
    bot.tap("Anlegen").await;
    let (text, knoepfe) = bot.edit();
    assert!(text.starts_with("✅ Notiz angelegt:"), "{text}");
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    let notes = bot.notes(bot.board_id).await;
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(
        notes[0].0,
        "Cache der Board-Ansicht läuft nie ab (nur MSSQL)"
    );
    assert!(
        notes[0]
            .1
            .contains("Transkript: Der Cache der Board-Ansicht läuft nie ab"),
        "{:?}",
        notes[0]
    );
    assert!(notes[0].1.contains("Lebenszeit von 5 Minuten"), "{notes:?}");

    // Der Entwurf ist weg: derselbe Knopf tut nichts mehr
    bot.clear();
    bot.press(&anlegen).await;
    assert_eq!(bot.alerts(), ["Diesen Entwurf gibt es nicht mehr"]);
    assert_eq!(bot.notes(bot.board_id).await.len(), 1);
}

#[tokio::test]
async fn umschalten_auf_ticket_legt_es_im_backlog_mit_kommentar_an() {
    let bot = Bot::simple().await;
    bot.answers(entwurf(bot.board_id, json!({ "questions": [] })));

    bot.send("/idee Der Cache der Board-Ansicht läuft nie ab")
        .await;
    bot.press(&bot.button("Als Ticket")).await;

    let (text, _) = bot.edit();
    assert!(
        text.contains(&format!(
            "<b>Ticket im Backlog von Board {} „Sprint“</b>",
            bot.board_id
        )),
        "{text}"
    );

    bot.tap("Anlegen").await;
    let (text, _) = bot.edit();
    assert!(text.starts_with("✅ Ticket angelegt:"), "{text}");

    let tickets = bot
        .backend
        .services
        .tickets
        .list(&nexus_core::board::ticket_service::TicketListFilter {
            board_ids: vec![bot.board_id],
            ..Default::default()
        })
        .await
        .expect("Tickets");
    let neu = tickets
        .iter()
        .find(|view| view.ticket.title.starts_with("Cache der Board-Ansicht"))
        .expect("das neue Ticket");
    assert_eq!(neu.ticket.status, TicketStatus::Backlog);
    let kommentare = bot.comments(&neu.ticket.id).await;
    assert_eq!(kommentare.len(), 1, "{kommentare:?}");
    assert!(
        kommentare[0].starts_with("Aus Telegram-Sprachnotiz (über Telegram)"),
        "{kommentare:?}"
    );
    assert!(
        kommentare[0].contains("Transkript: Der Cache der Board-Ansicht läuft nie ab"),
        "{kommentare:?}"
    );
    assert!(bot.notes(bot.board_id).await.is_empty(), "keine Notiz");
}

#[tokio::test]
async fn ein_kommentar_haengt_am_genannten_ticket() {
    let bot = Bot::simple().await;
    bot.answers(entwurf(
        bot.board_id,
        json!({
            "target": { "kind": "comment", "boardId": bot.board_id, "ticketId": "NXS-0001" },
            "details": "Die Spaltenansicht zeigt alte Zahlen.",
            "questions": [],
        }),
    ));

    bot.send("Zu NXS-0001: die Ansicht zeigt alte Zahlen").await;

    // Ein Kommentar hängt an seinem Ticket; umschalten geht nicht
    let (text, _) = bot.edit();
    assert!(text.contains("<b>Kommentar an NXS-0001</b>"), "{text}");
    assert_eq!(bot.labels(), ["Anlegen", "Korrigieren", "Verwerfen"]);

    bot.tap("Anlegen").await;
    let (text, _) = bot.edit();
    assert!(
        text.starts_with("✅ Kommentar geschrieben an NXS-0001"),
        "{text}"
    );
    assert_eq!(
        bot.comments("NXS-0001").await,
        ["Die Spaltenansicht zeigt alte Zahlen. (über Telegram)".to_string()]
    );
}

#[tokio::test]
async fn ein_anderes_board_nimmt_den_entwurf_mit() {
    let bot = Bot::simple().await;
    bot.answers(entwurf(bot.board_id, json!({ "questions": [] })));
    bot.send("/idee Der Cache läuft nie ab").await;

    bot.tap("Anderes Board").await;
    let (text, _) = bot.edit();
    assert_eq!(text, "<b>Auf welches Board?</b>");
    assert_eq!(bot.labels(), ["Ideen", "Abbrechen"]);

    bot.tap("Ideen").await;
    let (text, _) = bot.edit();
    assert!(
        text.contains(&format!(
            "<b>Notiz auf Board {} „Ideen“</b>",
            bot.other_board_id
        )),
        "{text}"
    );

    bot.tap("Anlegen").await;
    assert!(bot.notes(bot.board_id).await.is_empty(), "nicht auf Sprint");
    assert_eq!(bot.notes(bot.other_board_id).await.len(), 1);
}

#[tokio::test]
async fn verwerfen_legt_nichts_an() {
    let bot = Bot::simple().await;
    bot.answers(entwurf(bot.board_id, json!({ "questions": [] })));
    bot.send("/idee Der Cache läuft nie ab").await;

    bot.tap("Verwerfen").await;

    let (text, knoepfe) = bot.edit();
    assert_eq!(text, "🗑 Verworfen – nichts angelegt.");
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    assert!(bot.notes(bot.board_id).await.is_empty());
}

#[tokio::test]
async fn nach_den_korrekturrunden_bleiben_nur_anlegen_und_verwerfen() {
    let bot = Bot::start(
        IdeaSettings {
            default_target: IdeaTarget::Note,
            max_rounds: 2,
        },
        None,
    )
    .await;
    bot.answers(entwurf(bot.board_id, json!({})));

    bot.send("/idee Der Cache läuft nie ab").await;
    bot.send("Nein, nur für MSSQL").await;

    // Runde 2 ist die letzte: „Korrigieren“ fällt weg, das Ziel bleibt umschaltbar
    let (text, _) = bot.edit();
    assert!(text.contains("<i>Runde 2</i>"), "{text}");
    assert_eq!(
        bot.labels(),
        ["Anlegen", "Als Ticket", "Anderes Board", "Verwerfen"]
    );

    // Der Entwurf wartet nicht mehr auf eine Antwort: die nächste Nachricht ist eine neue
    // Idee und fragt erst, was mit der alten wird
    bot.clear();
    bot.send("Und noch etwas").await;
    assert!(
        bot.sent()
            .iter()
            .any(|text| text.contains("Es liegt noch ein Entwurf hier")),
        "{:?}",
        bot.sent()
    );
}

#[tokio::test]
async fn ein_abgelaufener_entwurf_laesst_sich_nicht_mehr_anlegen() {
    let bot = Bot::simple().await;
    bot.answers(entwurf(bot.board_id, json!({ "questions": [] })));
    bot.send("/idee Der Cache läuft nie ab").await;
    let anlegen = bot.button("Anlegen");

    bot.pass(IDEA_EXPIRY_HOURS);
    bot.clear();
    bot.press(&anlegen).await;

    assert_eq!(bot.alerts(), ["Diesen Entwurf gibt es nicht mehr"]);
    assert!(bot.notes(bot.board_id).await.is_empty());
}

#[tokio::test]
async fn eine_zweite_idee_fragt_nach_dem_alten_entwurf() {
    let bot = Bot::simple().await;
    bot.answers(entwurf(bot.board_id, json!({ "questions": [] })));
    bot.send("/idee Der Cache läuft nie ab").await;

    bot.clear();
    bot.send("/idee Die Spalte fehlt").await;

    // Die zweite Idee wartet; erst die Entscheidung über den alten Entwurf
    let gefragt = bot.sent();
    assert_eq!(gefragt.len(), 1, "{gefragt:?}");
    assert!(
        gefragt[0].contains("Es liegt noch ein Entwurf hier"),
        "{gefragt:?}"
    );
    let verwerfen = buttons(&bot.calls("sendMessage")[0]["reply_markup"])
        .into_iter()
        .find(|(label, _)| label == "Alten verwerfen")
        .expect("Knopf „Alten verwerfen“")
        .1;

    bot.answers(entwurf(
        bot.board_id,
        json!({ "title": "Spalte fehlt in der Board-Ansicht", "questions": [] }),
    ));
    bot.clear();
    bot.press(&verwerfen).await;

    let (text, _) = bot.edit();
    assert!(text.contains("Spalte fehlt in der Board-Ansicht"), "{text}");
    assert!(text.contains("<i>Runde 1</i>"), "neu gezählt: {text}");

    bot.tap("Anlegen").await;
    let notes = bot.notes(bot.board_id).await;
    assert_eq!(notes.len(), 1, "nur die zweite Idee: {notes:?}");
    assert_eq!(notes[0].0, "Spalte fehlt in der Board-Ansicht");
}

#[tokio::test]
async fn ein_entwurf_uebersteht_einen_neustart() {
    let bot = Bot::simple().await;
    bot.answers(entwurf(bot.board_id, json!({ "questions": [] })));
    bot.send("/idee Der Cache läuft nie ab").await;
    let anlegen = bot.button("Anlegen");
    assert!(
        bot.home.telegram_drafts_file().exists(),
        "der Entwurf steht in drafts.json"
    );

    // Derselbe Benutzerordner, ein neuer Server: der Knopf aus der alten Nachricht gilt weiter
    let neu = bot.restart();
    BoardConfigStore::new(bot.home.clone())
        .write(&nexus_core::board::config::BoardConfig {
            active_board: Some(bot.board_id),
            ..memory_config()
        })
        .expect("board.yaml schreiben");
    bot.clear();
    bot.press_on(&neu, &anlegen).await;

    let (text, _) = bot.edit();
    assert!(text.starts_with("✅ Notiz angelegt:"), "{text}");
    assert_eq!(bot.notes(bot.board_id).await.len(), 1);
}
