//! Die lesenden Befehle des Telegram-Bots: `/status`, `/board`, `/ticket`, `/aufgaben`,
//! `/pause` und `/weiter`.
//!
//! Die Befehle laufen über den Router des Bots im Serverzustand – also über dieselben
//! Dienste wie die Routen. Telegram selbst ist die Attrappe `FakeTelegram`, das Board liegt
//! im Speicher.

mod common;

use std::sync::Arc;

use common::{BoardBackendWithServices, TestServer, TestServerOptions};
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::note_service::{AddNoteOptions, NewNoteText};
use nexus_core::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
use nexus_core::clock::Clock;
use nexus_core::telegram::MAX_MESSAGE_LEN;
use nexus_server::telegram::api::{Message, TelegramApi};
use nexus_server::telegram::fake::FakeTelegram;
use nexus_server::telegram::BotContext;
use serde_json::json;

/// Der Chat, aus dem die Tests schreiben
const CHAT: i64 = 4242;

/// Die Uhr aller Tests
const NOW: &str = "2026-11-03T12:00:00Z";

/// Server, Board und Attrappe für einen Befehlstest
struct Bot {
    server: TestServer,
    backend: Arc<BoardBackendWithServices>,
    fake: Arc<FakeTelegram>,
    board_id: i32,
}

impl Bot {
    /// Ein Projekt mit einem Board; das Board ist das aktive
    async fn start() -> Self {
        let (server, backend) = TestServer::start_with_board_and(TestServerOptions {
            clock: Some(Clock::fixed(NOW.parse().expect("Zeitpunkt"))),
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
        let board_id = board.board.id;
        server
            .post(&format!("/api/db/boards/{board_id}/activate"), "{}")
            .await;
        Self {
            server,
            backend,
            fake: Arc::new(FakeTelegram::new()),
            board_id,
        }
    }

    /// Ein weiteres Board desselben Projekts
    async fn second_board(&self, name: &str) -> i32 {
        self.backend
            .services
            .boards
            .create("NXS", name, &self.backend.actor())
            .await
            .expect("Board")
            .board
            .id
    }

    /// Schickt einen Befehl aus dem Chat an den Router des Bots
    async fn send(&self, text: &str) {
        self.server
            .state
            .telegram
            .router()
            .handle_message(&self.context(), &message(text))
            .await
            .expect("Befehl");
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
            message_id: Some(1),
            bot_name: Some("nexus_test_bot".to_string()),
        }
    }

    /// Die Texte aller geschickten Nachrichten
    fn sent(&self) -> Vec<String> {
        self.fake
            .calls_of("sendMessage")
            .into_iter()
            .map(|call| call.params["text"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// Alles Geschickte in einem Text (für Antworten, die geteilt ankommen)
    fn text(&self) -> String {
        self.sent().join("\n")
    }

    /// Die Knöpfe der letzten Nachricht als Paare (Beschriftung, Callback-Daten)
    fn buttons(&self) -> Vec<(String, String)> {
        let calls = self.fake.calls_of("sendMessage");
        let last = calls.last().expect("eine Nachricht");
        last.params["reply_markup"]["inline_keyboard"]
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

    /// Vergisst alles Aufgezeichnete
    fn clear(&self) {
        self.fake.take_calls();
    }

    /// Legt ein Ticket mit Beschreibung und Abnahme an
    async fn ticket(&self, title: &str, description: &str, acceptance: &str) -> String {
        self.backend
            .services
            .tickets
            .create(
                self.board_id,
                &NewTicketInput {
                    description: Some(description.to_string()),
                    acceptance: Some(acceptance.to_string()),
                    labels: Some(vec!["telegram".into(), "backend".into()]),
                    ..NewTicketInput::titled(title)
                },
                &self.backend.actor(),
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket")
            .ticket
            .id
    }
}

/// Eine Textnachricht aus dem freigegebenen Chat
fn message(text: &str) -> Message {
    serde_json::from_value(json!({
        "message_id": 1,
        "chat": { "id": CHAT, "type": "private" },
        "from": { "id": 7, "first_name": "Ben" },
        "text": text,
    }))
    .expect("Nachricht")
}

#[tokio::test]
async fn status_zeigt_dispatcher_board_accounts_und_aufgaben() {
    let bot = Bot::start().await;
    bot.backend
        .services
        .notes
        .add(
            bot.board_id,
            &NewNoteText::titled("Cache läuft nie ab"),
            &bot.backend.actor(),
            &AddNoteOptions::default(),
        )
        .await
        .expect("Notiz");

    bot.send("/status").await;

    let text = bot.text();
    assert!(text.contains("<b>Dispatcher</b>"), "{text}");
    assert!(text.contains("Zustand: "), "{text}");
    assert!(
        text.contains(&format!("Board: Sprint ({})", bot.board_id)),
        "{text}"
    );
    assert!(text.contains("<b>Läufe (0)</b>"), "{text}");
    // Die Abfrage der Abo-Limits ist in den Einstellungen aus; dann fragt nichts Anthropic
    assert!(text.contains("Auslastung ist abgeschaltet"), "{text}");
    assert!(text.contains("<b>Meine Aufgaben (1)</b>"), "{text}");
    assert!(text.contains("Notiz 1"), "{text}");
}

#[tokio::test]
async fn status_nennt_eine_laufende_pause() {
    let bot = Bot::start().await;

    bot.send("/pause 30").await;
    bot.clear();
    bot.send("/status").await;

    // Der Dispatcher ist in diesem Test nie eingeschaltet; die Pause steht trotzdem im Status
    assert!(bot.text().contains("Pause noch 30 min"), "{}", bot.text());
}

#[tokio::test]
async fn board_zeigt_spalten_mit_anzahl_und_hoechstens_funf_tickets() {
    let bot = Bot::start().await;
    for number in 1..=7 {
        bot.ticket(&format!("Ticket {number}"), "", "").await;
    }

    bot.send("/board").await;

    let text = bot.text();
    assert!(
        text.contains(&format!("<b>Board Sprint</b> (Board {})", bot.board_id)),
        "{text}"
    );
    assert!(text.contains("<b>Backlog (7)</b>"), "{text}");
    assert_eq!(
        text.matches("[telegram, backend]").count(),
        5,
        "je Spalte höchstens fünf Tickets: {text}"
    );
    assert!(text.contains("… und 2 weitere"), "{text}");
    assert!(text.contains("<b>Erledigt (0)</b>"), "{text}");
}

#[tokio::test]
async fn board_schaltet_per_knopf_das_aktive_board_um() {
    let bot = Bot::start().await;
    let ideen = bot.second_board("Ideen").await;

    bot.send("/board").await;
    let knoepfe = bot.buttons();
    assert_eq!(
        knoepfe,
        vec![
            ("✓ Sprint".to_string(), format!("brd:{}", bot.board_id)),
            ("Ideen".to_string(), format!("brd:{ideen}")),
        ]
    );

    bot.clear();
    bot.press(&format!("brd:{ideen}")).await;

    let text = bot.text();
    assert!(text.contains("<b>Board Ideen</b>"), "{text}");
    assert_eq!(bot.buttons()[1].0, "✓ Ideen", "{:?}", bot.buttons());
    // Dasselbe aktive Board, das Oberfläche und CLI lesen
    let active = bot.server.get("/api/db/active").await.body();
    assert_eq!(active["boardId"].as_i64(), Some(ideen as i64));
    let antwort = bot.fake.calls_of("answerCallbackQuery");
    assert_eq!(antwort.len(), 1);
    assert_eq!(antwort[0].params["text"], "Aktives Board umgeschaltet");
}

#[tokio::test]
async fn board_nimmt_namen_und_id_und_meldet_eine_fremde_angabe() {
    let bot = Bot::start().await;
    let ideen = bot.second_board("Ideen").await;

    bot.send("/board ideen").await;
    assert!(bot.text().contains("<b>Board Ideen</b>"), "{}", bot.text());

    bot.clear();
    bot.send(&format!("/board {ideen}")).await;
    assert!(bot.text().contains("<b>Board Ideen</b>"), "{}", bot.text());

    bot.clear();
    bot.send("/board gibtesnicht").await;
    assert!(
        bot.text().contains("So ein Board habe ich nicht"),
        "{}",
        bot.text()
    );
    // Das aktive Board bleibt, wie es war
    let active = bot.server.get("/api/db/active").await.body();
    assert_eq!(active["boardId"].as_i64(), Some(bot.board_id as i64));
}

#[tokio::test]
async fn ticket_zeigt_status_beschreibung_abnahme_und_die_letzten_kommentare() {
    let bot = Bot::start().await;
    let id = bot
        .ticket("Login bauen", &"a".repeat(900), "Tests laufen durch")
        .await;
    for number in 1..=4 {
        bot.backend
            .services
            .comments
            .add(
                &id,
                &format!("Kommentar {number}"),
                &bot.backend.actor(),
                false,
            )
            .await
            .expect("Kommentar");
    }

    bot.send(&format!("/ticket {}", id.to_lowercase())).await;

    let text = bot.text();
    assert!(
        text.contains(&format!("<b>{id} · Login bauen</b>")),
        "{text}"
    );
    assert!(text.contains("Status: Backlog"), "{text}");
    assert!(text.contains("Labels: telegram, backend"), "{text}");
    assert!(text.contains("<b>Beschreibung</b>"), "{text}");
    assert!(text.contains('…'), "die Beschreibung ist gekürzt: {text}");
    assert!(
        text.contains("<b>Abnahme</b>\nTests laufen durch"),
        "{text}"
    );
    assert!(
        !text.contains("Kommentar 1"),
        "nur die letzten drei: {text}"
    );
    for number in 2..=4 {
        assert!(text.contains(&format!("Kommentar {number}")), "{text}");
    }
}

#[tokio::test]
async fn ticket_meldet_eine_unbekannte_id_verstaendlich() {
    let bot = Bot::start().await;

    bot.send("/ticket NXS-9999").await;
    assert_eq!(bot.sent(), vec!["Das Ticket NXS-9999 gibt es nicht."]);

    bot.clear();
    bot.send("/ticket").await;
    assert!(bot.text().contains("Welches Ticket?"), "{}", bot.text());

    bot.clear();
    bot.send("/ticket quatsch").await;
    assert!(
        bot.text().contains("gibt es nicht"),
        "auch eine kaputte ID ist nur „gibt es nicht“: {}",
        bot.text()
    );
}

#[tokio::test]
async fn ticket_nennt_den_grund_eines_blockierten_tickets() {
    let bot = Bot::start().await;
    let id = bot.ticket("Login bauen", "", "").await;
    bot.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                status: Some(TicketStatus::Blocked),
                blocked_reason: Some(Some("Zugang fehlt".into())),
                ..TicketPatch::default()
            },
            &bot.backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("blockieren");

    bot.send(&format!("/ticket {id}")).await;

    let text = bot.text();
    assert!(text.contains("Status: Blockiert"), "{text}");
    assert!(text.contains("Blockiert: Zugang fehlt"), "{text}");
}

#[tokio::test]
async fn aufgaben_listet_hoechstens_zehn_eintraege_mit_knopf_mehr() {
    let bot = Bot::start().await;
    for number in 1..=12 {
        bot.backend
            .services
            .notes
            .add(
                bot.board_id,
                &NewNoteText::titled(format!("Einfall {number}")),
                &bot.backend.actor(),
                &AddNoteOptions::default(),
            )
            .await
            .expect("Notiz");
    }

    bot.send("/aufgaben").await;

    let nachrichten = bot.sent();
    assert_eq!(nachrichten.len(), 10, "{nachrichten:?}");
    assert!(
        nachrichten[0].contains("<b>Notiz</b>"),
        "{}",
        nachrichten[0]
    );
    assert!(
        nachrichten[0].contains("<i>Sprint · "),
        "{}",
        nachrichten[0]
    );
    // Unter dem letzten Eintrag hängen seine eigenen Knöpfe und darunter „Mehr“
    let knoepfe = bot.buttons();
    assert_eq!(knoepfe.len(), 3, "{knoepfe:?}");
    assert_eq!(knoepfe[0].0, "Ticket daraus");
    assert!(knoepfe[0].1.starts_with("ntt:"), "{knoepfe:?}");
    assert_eq!(knoepfe[1].0, "Verwerfen");
    assert!(knoepfe[1].1.starts_with("ndr:"), "{knoepfe:?}");
    assert_eq!(
        knoepfe[2],
        ("Mehr: Notiz (2)".to_string(), "mo:note:2".to_string())
    );

    // Der Knopf zeigt die restlichen zwei und bietet kein weiteres „Mehr“ an
    bot.clear();
    bot.press("mo:note:2").await;
    let weitere = bot.sent();
    assert_eq!(
        weitere.len(),
        3,
        "eine Kopfzeile und zwei Einträge: {weitere:?}"
    );
    assert!(weitere[0].contains("Notiz – Seite 2"), "{}", weitere[0]);
    assert!(
        !bot.buttons()
            .iter()
            .any(|(_, data)| data.starts_with("mo:")),
        "kein weiteres „Mehr“: {:?}",
        bot.buttons()
    );
}

#[tokio::test]
async fn aufgaben_meldet_eine_leere_liste() {
    let bot = Bot::start().await;

    bot.send("/aufgaben").await;

    assert_eq!(
        bot.sent(),
        vec!["Nichts zu tun – „Meine Aufgaben“ ist leer."]
    );
}

#[tokio::test]
async fn pause_und_weiter_steuern_den_dispatcher() {
    let bot = Bot::start().await;

    bot.send("/pause 30").await;
    assert_eq!(bot.sent(), vec!["Dispatcher pausiert – 30 min."]);
    let status = bot
        .server
        .state
        .dispatch
        .status(None)
        .await
        .expect("Status");
    assert_eq!(
        status.paused_until.as_deref(),
        Some("2026-11-03T12:30:00.000Z")
    );

    bot.clear();
    bot.send("/weiter").await;
    assert_eq!(bot.sent(), vec!["Dispatcher läuft wieder."]);
    let status = bot
        .server
        .state
        .dispatch
        .status(None)
        .await
        .expect("Status");
    assert_eq!(status.paused_until, None);
}

#[tokio::test]
async fn pause_ohne_angabe_dauert_eine_stunde_und_weist_unsinn_ab() {
    let bot = Bot::start().await;

    bot.send("/pause").await;
    assert_eq!(bot.sent(), vec!["Dispatcher pausiert – 1 h."]);

    bot.clear();
    bot.send("/pause morgen").await;
    assert!(
        bot.text().contains("ganze Zahl von 1 bis 1440"),
        "{}",
        bot.text()
    );
    // Die abgewiesene Angabe hat die laufende Pause nicht verändert
    let status = bot
        .server
        .state
        .dispatch
        .status(None)
        .await
        .expect("Status");
    assert_eq!(
        status.paused_until.as_deref(),
        Some("2026-11-03T13:00:00.000Z")
    );
}

#[tokio::test]
async fn teilt_eine_lange_board_antwort_in_mehrere_nachrichten() {
    let bot = Bot::start().await;
    // Zwei Spalten mit je fünf gezeigten Karten und langen Titeln reißen die Grenze von 4096
    for number in 1..=10 {
        let id = bot
            .ticket(&format!("{} {number:02}", "T".repeat(380)), "", "")
            .await;
        if number > 5 {
            bot.backend
                .services
                .tickets
                .update(
                    &id,
                    &TicketPatch {
                        status: Some(TicketStatus::Blocked),
                        blocked_reason: Some(Some("wartet".into())),
                        ..TicketPatch::default()
                    },
                    &bot.backend.actor(),
                    &WriteOptions::default(),
                )
                .await
                .expect("blockieren");
        }
    }

    bot.send("/board").await;

    let nachrichten = bot.sent();
    assert!(nachrichten.len() > 1, "{} Nachrichten", nachrichten.len());
    for nachricht in &nachrichten {
        assert!(nexus_core::text::utf16_len(nachricht) <= MAX_MESSAGE_LEN);
    }
    // Die Knöpfe hängen unter dem letzten Stück
    assert!(!bot.buttons().is_empty());
}

#[tokio::test]
async fn meldet_die_befehle_fuer_das_telegram_menue() {
    let bot = Bot::start().await;

    let liste = bot.server.state.telegram.router().command_list();

    let namen: Vec<&str> = liste.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        namen,
        vec![
            "start",
            "hilfe",
            "status",
            "board",
            "ticket",
            "aufgaben",
            "pause",
            "weiter",
            "abbrechen"
        ]
    );
    assert!(liste.iter().all(|(_, text)| !text.is_empty()));
}
