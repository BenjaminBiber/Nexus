//! Die Knöpfe an einer offenen Frage: Option der neuesten Recherche, eigener Text, Recherche
//! starten und Verwerfen mit Pflicht-Begründung.
//!
//! Wie bei den Freigaben laufen die Knöpfe über den Router des Bots im Serverzustand – also
//! über dieselben Dienste wie die Routen. Telegram selbst ist die Attrappe `FakeTelegram`,
//! das Board liegt im Speicher.

mod common;

use std::sync::Arc;

use common::{BoardBackendWithServices, TempRepo, TestServer};
use nexus_core::actors::AuthorKind;
use nexus_core::board::question_service::NewBoardQuestionInput;
use nexus_core::board::question_types::{BoardQuestionActor, BoardQuestionDetail};
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use nexus_core::clock::Clock;
use nexus_core::questions::{QuestionState, ResearchInput, ResearchOptionInput};
use nexus_server::telegram::api::{Message, TelegramApi};
use nexus_server::telegram::fake::FakeTelegram;
use nexus_server::telegram::question_views::{DISCARD_WITHOUT_REASON, NO_RESEARCH, RECOMMENDED};
use nexus_server::telegram::BotContext;
use serde_json::{json, Value};

/// Der Chat, aus dem die Tests schreiben
const CHAT: i64 = 4242;

/// Die Uhr aller Tests
const NOW: &str = "2026-11-03T12:00:00Z";

/// Server, Board und Attrappe für einen Test der Fragen-Knöpfe
struct Bot {
    server: TestServer,
    backend: Arc<BoardBackendWithServices>,
    fake: Arc<FakeTelegram>,
    board_id: i32,
}

impl Bot {
    /// Ein Projekt mit einem Board; das Board ist das aktive
    async fn start() -> Self {
        let (server, backend) = TestServer::start_with_board_and(common::TestServerOptions {
            clock: Some(Clock::fixed(NOW.parse().expect("Zeitpunkt"))),
            ..common::TestServerOptions::default()
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

    /// Hängt ein Projekt mit diesem Branch an das Board; so gibt es einen Ordner, in dem
    /// recherchiert werden kann
    fn with_repo(&self, branch: &str) -> TempRepo {
        let repo = TempRepo::new();
        repo.run_git(&["branch", branch]);
        self.backend.set_repo_path(self.board_id, repo.path());
        repo
    }

    /// Eine offene Frage zu einem Ticket des Boards; der Fragesteller ist ein Agent, damit
    /// die Frage in „Meine Aufgaben“ steht
    async fn ask(&self, title: &str) -> String {
        let ticket = self
            .backend
            .services
            .tickets
            .create(
                self.board_id,
                &NewTicketInput::titled("Preise anzeigen"),
                &self.backend.actor(),
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket")
            .ticket
            .id
            .clone();
        self.backend
            .services
            .questions
            .create(
                "NXS",
                &NewBoardQuestionInput {
                    description: Some("Brutto oder netto?".into()),
                    ticket_ids: Some(vec![ticket]),
                    ..NewBoardQuestionInput::titled(title)
                },
                &self.agent(),
            )
            .await
            .expect("Frage")
            .view
            .question
            .id
            .clone()
    }

    /// Ergänzt eine Recherche mit zwei Optionen; die zweite ist die empfohlene
    async fn research(&self, id: &str) {
        self.backend
            .services
            .questions
            .add_research(
                id,
                &ResearchInput {
                    context: Some("Fundstelle src/preise.rs:12".into()),
                    options: Some(vec![
                        ResearchOptionInput {
                            title: Some("Brutto".into()),
                            text: Some("immer brutto".into()),
                            pros: Some(vec!["einfach".into()]),
                            cons: Some(vec!["unklar für Händler".into()]),
                            ..ResearchOptionInput::default()
                        },
                        ResearchOptionInput {
                            title: Some("Netto".into()),
                            text: Some("immer netto".into()),
                            recommended: Some(true),
                            ..ResearchOptionInput::default()
                        },
                    ]),
                },
                &self.agent(),
            )
            .await
            .expect("Recherche");
    }

    /// Der Agent, der die Fragen stellt und recherchiert
    fn agent(&self) -> BoardQuestionActor {
        BoardQuestionActor {
            actor: self.backend.actor(),
            kind: AuthorKind::Session,
            session: None,
        }
    }

    /// Die Frage samt Recherchen und Entscheidung
    async fn question(&self, id: &str) -> BoardQuestionDetail {
        self.backend
            .services
            .questions
            .get(id)
            .await
            .expect("Frage")
    }

    /// Schickt eine Nachricht aus dem Chat an den Router des Bots
    async fn send(&self, text: &str) {
        self.server
            .state
            .telegram
            .router()
            .handle_message(&self.context(), &message(text))
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

    /// Text und Knöpfe aller geschickten Nachrichten
    fn sent(&self) -> Vec<(String, Vec<(String, String)>)> {
        self.calls("sendMessage")
    }

    /// Text und Knöpfe aller geänderten Nachrichten
    fn edits(&self) -> Vec<(String, Vec<(String, String)>)> {
        self.calls("editMessageText")
    }

    /// Die letzte geänderte Nachricht
    fn edit(&self) -> (String, Vec<(String, String)>) {
        self.edits().pop().expect("eine geänderte Nachricht")
    }

    /// Die Texte der Hinweisfenster an den Knöpfen (nur die mit Text)
    fn alerts(&self) -> Vec<String> {
        self.fake
            .calls_of("answerCallbackQuery")
            .into_iter()
            .filter_map(|call| call.params["text"].as_str().map(str::to_string))
            .collect()
    }

    /// Alles Geschickte und Geänderte in einem Text
    fn text(&self) -> String {
        self.sent()
            .into_iter()
            .chain(self.edits())
            .map(|(text, _)| text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Text und Knöpfe aller Aufrufe dieser Methode
    fn calls(&self, method: &str) -> Vec<(String, Vec<(String, String)>)> {
        self.fake
            .calls_of(method)
            .into_iter()
            .map(|call| {
                (
                    call.params["text"].as_str().unwrap_or_default().to_string(),
                    buttons(&call.params["reply_markup"]),
                )
            })
            .collect()
    }

    /// Vergisst alles Aufgezeichnete
    fn clear(&self) {
        self.fake.take_calls();
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

/// Die Knöpfe einer Tastatur als Paare (Beschriftung, Callback-Daten)
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

#[tokio::test]
async fn eine_frage_ohne_recherche_bietet_text_recherche_und_verwerfen() {
    let bot = Bot::start().await;
    let id = bot.ask("Brutto oder netto?").await;

    bot.send("/aufgaben").await;

    let (text, knoepfe) = bot
        .sent()
        .into_iter()
        .find(|(text, _)| text.contains(&id))
        .expect("die Frage steht in den Aufgaben");
    assert!(text.starts_with(&format!("<b>Frage · {id}</b>")), "{text}");
    assert!(text.contains(NO_RESEARCH), "{text}");
    assert_eq!(
        knoepfe,
        vec![
            ("Eigener Text".to_string(), format!("qf:{id}")),
            ("Recherche starten".to_string(), format!("qr:{id}")),
            ("Verwerfen".to_string(), format!("qd:{id}")),
        ]
    );
}

#[tokio::test]
async fn eine_frage_mit_recherche_zeigt_je_option_einen_knopf_und_markiert_die_empfohlene() {
    let bot = Bot::start().await;
    let id = bot.ask("Brutto oder netto?").await;
    bot.research(&id).await;

    bot.send("/aufgaben").await;

    let (text, knoepfe) = bot
        .sent()
        .into_iter()
        .find(|(text, _)| text.contains(&id))
        .expect("die Frage steht in den Aufgaben");
    assert!(text.contains("Fundstelle src/preise.rs:12"), "{text}");
    assert!(
        text.contains("<b>O1</b> Brutto\n+ einfach\n− unklar für Händler"),
        "{text}"
    );
    assert!(
        text.contains(&format!("<b>O2</b> Netto {RECOMMENDED}")),
        "{text}"
    );
    assert_eq!(
        knoepfe,
        vec![
            ("O1 · Brutto".to_string(), format!("qo:{id}:O1")),
            ("★ O2 · Netto".to_string(), format!("qo:{id}:O2")),
            ("Eigener Text".to_string(), format!("qf:{id}")),
            ("Recherche starten".to_string(), format!("qr:{id}")),
            ("Verwerfen".to_string(), format!("qd:{id}")),
        ]
    );
}

#[tokio::test]
async fn eine_option_entscheidet_erst_nach_der_bestaetigung() {
    let bot = Bot::start().await;
    let id = bot.ask("Brutto oder netto?").await;
    bot.research(&id).await;

    // Erster Klick: nur die Sicherheitsfrage, an der Frage ändert sich nichts
    bot.press(&format!("qo:{id}:O2")).await;

    let (text, knoepfe) = bot.edit();
    assert_eq!(
        text,
        format!("<b>{id} wirklich mit O2 entscheiden?</b>\nNetto")
    );
    assert_eq!(
        knoepfe,
        vec![
            ("Ja, O2".to_string(), format!("qoc:{id}:O2")),
            ("Abbrechen".to_string(), "x".to_string()),
        ]
    );
    assert_eq!(
        bot.question(&id).await.view.question.state,
        QuestionState::Open
    );

    // Zweiter Klick: jetzt ist die Frage entschieden, und die Knöpfe sind weg
    bot.clear();
    bot.press(&format!("qoc:{id}:O2")).await;

    let (text, knoepfe) = bot.edit();
    assert!(
        text.starts_with("✅ Frage entschieden über Telegram, "),
        "{text}"
    );
    assert!(text.contains(&format!("{id}: O2 Netto")), "{text}");
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    let decision = bot
        .question(&id)
        .await
        .view
        .question
        .decision
        .expect("Entscheidung");
    assert_eq!(decision.option.as_deref(), Some("O2"));
    assert_eq!(decision.reason, "Entschieden (über Telegram)");
}

#[tokio::test]
async fn abbrechen_unter_der_sicherheitsfrage_laesst_die_frage_offen() {
    let bot = Bot::start().await;
    let id = bot.ask("Brutto oder netto?").await;
    bot.research(&id).await;

    bot.press(&format!("qo:{id}:O1")).await;
    bot.clear();
    bot.press("x").await;

    assert_eq!(bot.edit().0, "Abgebrochen.");
    assert_eq!(
        bot.question(&id).await.view.question.state,
        QuestionState::Open
    );
}

#[tokio::test]
async fn ein_eigener_text_entscheidet_die_frage_ueber_den_dialog() {
    let bot = Bot::start().await;
    let id = bot.ask("Brutto oder netto?").await;

    bot.press(&format!("qf:{id}")).await;

    let (text, knoepfe) = bot.edit();
    assert!(
        text.contains(&format!("Wie ist {id} entschieden?")),
        "{text}"
    );
    assert!(text.contains("/abbrechen"), "{text}");
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    assert_eq!(
        bot.question(&id).await.view.question.state,
        QuestionState::Open
    );

    bot.clear();
    bot.send("Brutto, aber netto im Händlerbereich").await;

    let text = bot.text();
    assert!(
        text.contains("✅ Frage entschieden über Telegram, "),
        "{text}"
    );
    let decision = bot
        .question(&id)
        .await
        .view
        .question
        .decision
        .expect("Entscheidung");
    assert_eq!(
        decision.text.as_deref(),
        Some("Brutto, aber netto im Händlerbereich")
    );
    assert_eq!(decision.option, None);
}

#[tokio::test]
async fn verwerfen_braucht_eine_begruendung() {
    let bot = Bot::start().await;
    let id = bot.ask("Brutto oder netto?").await;

    bot.press(&format!("qd:{id}")).await;
    assert!(
        bot.edit()
            .0
            .contains(&format!("Warum soll {id} verworfen werden?")),
        "{}",
        bot.edit().0
    );

    // Ein Text ohne Inhalt verwirft nichts
    bot.clear();
    bot.send("   ").await;
    assert_eq!(bot.sent()[0].0, format!("⚠️ {DISCARD_WITHOUT_REASON}"));
    assert_eq!(
        bot.question(&id).await.view.question.state,
        QuestionState::Open
    );

    bot.clear();
    bot.press(&format!("qd:{id}")).await;
    bot.clear();
    bot.send("Die Frage hat sich erledigt").await;

    assert!(
        bot.text().contains("✅ Frage verworfen über Telegram, "),
        "{}",
        bot.text()
    );
    let question = bot.question(&id).await.view.question.clone();
    assert_eq!(question.state, QuestionState::Discarded);
    assert_eq!(
        question.decision.expect("Entscheidung").reason,
        "Die Frage hat sich erledigt (über Telegram)"
    );
}

#[tokio::test]
async fn recherche_starten_stoesst_den_lauf_an() {
    let bot = Bot::start().await;
    let _repo = bot.with_repo("ticket/x");
    let id = bot.ask("Brutto oder netto?").await;

    bot.press(&format!("qr:{id}")).await;

    let (text, knoepfe) = bot.edit();
    assert!(
        text.starts_with("✅ Recherche gestartet über Telegram, "),
        "{text}"
    );
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    let status = bot
        .server
        .state
        .dispatch
        .research_status(&id)
        .await
        .expect("Recherchestand");
    assert_eq!(
        status.run.map(|run| run.question_id),
        Some(Some(id)),
        "die Recherche läuft"
    );
}

#[tokio::test]
async fn ein_knopf_zu_einer_entschiedenen_frage_meldet_das_und_aendert_nichts() {
    let bot = Bot::start().await;
    let id = bot.ask("Brutto oder netto?").await;
    bot.research(&id).await;
    bot.press(&format!("qo:{id}:O1")).await;
    bot.press(&format!("qoc:{id}:O1")).await;
    bot.clear();

    let hinweis = format!("{id} ist nicht mehr offen – nichts geändert.");
    for data in [
        format!("qo:{id}:O1"),
        format!("qoc:{id}:O1"),
        format!("qf:{id}"),
        format!("qr:{id}"),
        format!("qd:{id}"),
    ] {
        bot.clear();
        bot.press(&data).await;

        assert_eq!(bot.alerts(), vec![hinweis.clone()], "{data}");
        assert_eq!(bot.edit().0, format!("⚠️ {hinweis}"), "{data}");
    }
    let decision = bot
        .question(&id)
        .await
        .view
        .question
        .decision
        .expect("Entscheidung");
    assert_eq!(decision.option.as_deref(), Some("O1"));
}

#[tokio::test]
async fn eine_option_die_es_nicht_mehr_gibt_aendert_nichts() {
    let bot = Bot::start().await;
    let id = bot.ask("Brutto oder netto?").await;
    bot.research(&id).await;

    bot.press(&format!("qo:{id}:O9")).await;

    assert_eq!(
        bot.alerts(),
        vec!["Die Option O9 steht nicht mehr zur Wahl – nichts geändert.".to_string()]
    );
    assert_eq!(
        bot.question(&id).await.view.question.state,
        QuestionState::Open
    );
}

#[tokio::test]
async fn ein_knopf_zu_einer_geloeschten_frage_meldet_das_im_chat() {
    let bot = Bot::start().await;

    bot.press("qf:NXS-F9999").await;

    assert!(
        bot.text().contains("Das ging gerade nicht"),
        "{}",
        bot.text()
    );
}
