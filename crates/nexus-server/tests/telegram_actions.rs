//! Die Freigaben per Knopf: Abnahme, Rückgabe, erneuter Merge, Grund, Kommentar, Planung
//! und Notizen samt den Dialogen für die Texteingaben.
//!
//! Wie bei den lesenden Befehlen laufen die Knöpfe über den Router des Bots im
//! Serverzustand – also über dieselben Dienste wie die Routen. Telegram selbst ist die
//! Attrappe `FakeTelegram`, das Board liegt im Speicher.

mod common;

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};
use common::{BoardBackendWithServices, TempRepo, TestServer, TestServerOptions};
use nexus_core::board::basics::{NoteState, TicketStatus};
use nexus_core::board::note_service::{AddNoteOptions, NewNoteText};
use nexus_core::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
use nexus_core::clock::Clock;
use nexus_server::telegram::api::{Message, TelegramApi};
use nexus_server::telegram::dialog::DIALOG_TIMEOUT_MINUTES;
use nexus_server::telegram::fake::FakeTelegram;
use nexus_server::telegram::BotContext;
use serde_json::{json, Value};

/// Der Chat, aus dem die Tests schreiben
const CHAT: i64 = 4242;

/// Die Uhr aller Tests
const NOW: &str = "2026-11-03T12:00:00Z";

/// Grund, mit dem ein Ticket auf die Abnahme wartet
const REVIEW: &str = "Human Interaction: Review vor dem Merge";

/// Server, Board und Attrappe für einen Test der Knöpfe
struct Bot {
    server: TestServer,
    backend: Arc<BoardBackendWithServices>,
    fake: Arc<FakeTelegram>,
    board_id: i32,
    /// Minuten, die seit [`NOW`] vergangen sind (für den Ablauf eines Dialogs)
    minutes: Arc<AtomicI64>,
}

impl Bot {
    /// Ein Projekt mit einem Board; das Board ist das aktive
    async fn start() -> Self {
        let minutes = Arc::new(AtomicI64::new(0));
        let read = minutes.clone();
        let start: DateTime<Utc> = NOW.parse().expect("Zeitpunkt");
        let (server, backend) = TestServer::start_with_board_and(TestServerOptions {
            clock: Some(Clock::from_fn(move || {
                start + TimeDelta::minutes(read.load(Ordering::Relaxed))
            })),
            // Freier Text wird hier zu einer Idee; die Attrappe antwortet darauf nichts
            // Brauchbares, damit nie das echte `claude` startet (die Ideen prüft
            // `telegram_ideas`)
            claude: Some(common::fake_runners(common::FakeClaude::replying(""))),
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
            minutes,
        }
    }

    /// Hängt ein Projekt mit diesem Branch an das Board; so gibt es einen Diff und einen
    /// Ordner, in dem geplant werden kann
    fn with_repo(&self, branch: &str) -> TempRepo {
        let repo = TempRepo::new();
        repo.run_git(&["branch", branch]);
        self.backend.set_repo_path(self.board_id, repo.path());
        repo
    }

    /// Stellt die Uhr des Servers um so viele Minuten vor
    fn pass(&self, minutes: i64) {
        self.minutes.store(minutes, Ordering::Relaxed);
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

    /// Die Texte aller geschickten Nachrichten
    fn sent(&self) -> Vec<String> {
        self.calls("sendMessage")
            .into_iter()
            .map(|params| params["text"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// Text und Knöpfe aller geänderten Nachrichten
    fn edits(&self) -> Vec<(String, Vec<(String, String)>)> {
        self.calls("editMessageText")
            .into_iter()
            .map(|params| {
                (
                    params["text"].as_str().unwrap_or_default().to_string(),
                    buttons(&params["reply_markup"]),
                )
            })
            .collect()
    }

    /// Die letzte geänderte Nachricht
    fn edit(&self) -> (String, Vec<(String, String)>) {
        self.edits().pop().expect("eine geänderte Nachricht")
    }

    /// Die Texte der Hinweisfenster an den Knöpfen (nur die mit Text)
    fn alerts(&self) -> Vec<String> {
        self.calls("answerCallbackQuery")
            .into_iter()
            .filter_map(|params| params["text"].as_str().map(str::to_string))
            .collect()
    }

    /// Alles Geschickte und Geänderte in einem Text
    fn text(&self) -> String {
        let mut all = self.sent();
        all.extend(self.edits().into_iter().map(|(text, _)| text));
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

    /// Legt ein Ticket an
    async fn ticket(&self, title: &str) -> String {
        self.backend
            .services
            .tickets
            .create(
                self.board_id,
                &NewTicketInput {
                    acceptance: Some("ok".into()),
                    needs_planning: Some(false),
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

    /// Setzt Status, Grund und Branch eines Tickets, ohne Rücksicht auf die Regeln
    async fn force(&self, id: &str, patch: TicketPatch) {
        self.backend
            .services
            .tickets
            .update(
                id,
                &patch,
                &self.backend.actor(),
                &WriteOptions {
                    force: true,
                    ..WriteOptions::default()
                },
            )
            .await
            .expect("Ticket setzen");
    }

    /// Ein Ticket, das mit diesem Grund blockiert ist; `branch` wie nach einem Lauf
    async fn blocked(&self, title: &str, reason: &str, branch: Option<&str>) -> String {
        let id = self.ticket(title).await;
        self.force(
            &id,
            TicketPatch {
                status: Some(TicketStatus::Blocked),
                blocked_reason: Some(Some(reason.into())),
                branch: Some(branch.map(str::to_string)),
                ..TicketPatch::default()
            },
        )
        .await;
        id
    }

    /// Der Status eines Tickets
    async fn status(&self, id: &str) -> TicketStatus {
        self.backend
            .services
            .tickets
            .get(id)
            .await
            .expect("Ticket")
            .ticket
            .status
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

    /// Eine offene Notiz
    async fn note(&self, title: &str) -> i32 {
        self.backend
            .services
            .notes
            .add(
                self.board_id,
                &NewNoteText::titled(title),
                &self.backend.actor(),
                &AddNoteOptions::default(),
            )
            .await
            .expect("Notiz")
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
async fn abnahme_braucht_zwei_klicks_und_erledigt_erst_nach_ja() {
    let bot = Bot::start().await;
    let id = bot
        .blocked("Oberfläche ansehen", REVIEW, Some("ticket/x"))
        .await;

    // Erster Klick: nur die Sicherheitsfrage, am Ticket ändert sich nichts
    bot.press(&format!("acc:{id}")).await;

    let (text, knoepfe) = bot.edit();
    assert_eq!(text, format!("<b>{id} wirklich übernehmen und mergen?</b>"));
    assert_eq!(
        knoepfe,
        vec![
            ("Ja, übernehmen".to_string(), format!("accc:{id}")),
            ("Abbrechen".to_string(), "x".to_string()),
        ]
    );
    assert_eq!(bot.status(&id).await, TicketStatus::Blocked);

    // Zweiter Klick: jetzt läuft die Abnahme, und die Knöpfe sind weg
    bot.clear();
    bot.press(&format!("accc:{id}")).await;

    let (text, knoepfe) = bot.edit();
    assert!(text.starts_with("✅ Übernommen über Telegram, "), "{text}");
    assert!(
        text.contains(&format!("{id} abgenommen und erledigt")),
        "{text}"
    );
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    assert_eq!(bot.status(&id).await, TicketStatus::Done);
}

#[tokio::test]
async fn abbrechen_unter_der_sicherheitsfrage_laesst_alles_wie_es_war() {
    let bot = Bot::start().await;
    let id = bot
        .blocked("Oberfläche ansehen", REVIEW, Some("ticket/x"))
        .await;

    bot.press(&format!("acc:{id}")).await;
    bot.clear();
    bot.press("x").await;

    let (text, knoepfe) = bot.edit();
    assert_eq!(text, "Abgebrochen.");
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    assert_eq!(bot.status(&id).await, TicketStatus::Blocked);
}

#[tokio::test]
async fn zurueck_an_den_agenten_fragt_nach_der_rueckmeldung_und_gibt_das_ticket_zurueck() {
    let bot = Bot::start().await;
    let _repo = bot.with_repo("ticket/x");
    let id = bot.blocked("Rabatt", REVIEW, Some("ticket/x")).await;

    bot.press(&format!("ret:{id}")).await;

    let (text, knoepfe) = bot.edit();
    assert!(
        text.contains(&format!("Was soll der Agent an {id} anders machen?")),
        "{text}"
    );
    assert!(text.contains("/abbrechen"), "{text}");
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    assert_eq!(
        bot.status(&id).await,
        TicketStatus::Blocked,
        "erst die Rückmeldung gibt das Ticket zurück"
    );

    bot.clear();
    bot.send("Bitte den Rand nachbessern").await;

    let text = bot.text();
    assert!(
        text.contains("✅ Zurück an den Agenten über Telegram, "),
        "{text}"
    );
    assert_eq!(bot.status(&id).await, TicketStatus::Planned);
    let kommentare = bot.comments(&id).await;
    assert!(
        kommentare
            .iter()
            .any(|body| body.contains("Bitte den Rand nachbessern (über Telegram)")),
        "{kommentare:?}"
    );
}

#[tokio::test]
async fn zurueck_nach_geplant_schiebt_ein_blockiertes_ticket_mit_kommentar() {
    let bot = Bot::start().await;
    let id = bot.blocked("Login hängt", "Zugang fehlt", None).await;

    bot.press(&format!("ret:{id}")).await;
    bot.clear();
    bot.send("Zugang ist jetzt da").await;

    assert!(
        bot.text()
            .contains("✅ Zurück nach Geplant über Telegram, "),
        "{}",
        bot.text()
    );
    let ticket = bot
        .backend
        .services
        .tickets
        .get(&id)
        .await
        .expect("Ticket")
        .ticket
        .clone();
    assert_eq!(ticket.status, TicketStatus::Planned);
    assert_eq!(ticket.blocked_reason, None);
    assert_eq!(
        bot.comments(&id).await,
        vec!["Zugang ist jetzt da (über Telegram)".to_string()]
    );
}

#[tokio::test]
async fn erneut_mergen_braucht_ja_und_stoesst_den_merge_an() {
    let bot = Bot::start().await;
    let id = bot
        .blocked("Rabatt", "Merge: Konflikt in src/a.rs", Some("ticket/x"))
        .await;

    bot.press(&format!("mrg:{id}")).await;
    let (text, knoepfe) = bot.edit();
    assert_eq!(text, format!("<b>{id} wirklich erneut mergen?</b>"));
    assert_eq!(knoepfe[0].1, format!("mrgc:{id}"));
    assert_eq!(bot.status(&id).await, TicketStatus::Blocked);

    bot.clear();
    bot.press(&format!("mrgc:{id}")).await;

    let (text, knoepfe) = bot.edit();
    assert!(
        text.starts_with("✅ Merge angestoßen über Telegram, "),
        "{text}"
    );
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    // Der Auftrag steht; ohne Repo-Pfad meldet der Durchlauf, dass nichts zu mergen ist
    assert!(text.contains(&id), "{text}");
    let kommentare = bot.comments(&id).await;
    assert!(
        kommentare
            .iter()
            .any(|body| body.contains("erneuter Merge angestoßen")),
        "{kommentare:?}"
    );
}

#[tokio::test]
async fn ein_kommentar_per_dialog_landet_mit_dem_zusatz_am_ticket() {
    let bot = Bot::start().await;
    let id = bot.blocked("Login hängt", "Zugang fehlt", None).await;

    bot.press(&format!("cmt:{id}")).await;
    let (text, knoepfe) = bot.edit();
    assert!(
        text.contains(&format!("Was soll als Kommentar an {id}?")),
        "{text}"
    );
    assert!(knoepfe.is_empty(), "{knoepfe:?}");

    bot.clear();
    bot.send("Der Zugang kommt morgen").await;

    assert!(
        bot.text()
            .contains("✅ Kommentar geschrieben über Telegram, "),
        "{}",
        bot.text()
    );
    assert_eq!(
        bot.comments(&id).await,
        vec!["Der Zugang kommt morgen (über Telegram)".to_string()]
    );
}

#[tokio::test]
async fn abbrechen_und_der_ablauf_beenden_einen_dialog() {
    let bot = Bot::start().await;
    let id = bot.blocked("Login hängt", "Zugang fehlt", None).await;

    // /abbrechen: der Text danach ist kein Kommentar mehr
    bot.press(&format!("cmt:{id}")).await;
    bot.clear();
    bot.send("/abbrechen").await;
    assert_eq!(bot.sent(), vec!["Abgebrochen."]);
    bot.clear();
    bot.send("nur so ein Satz").await;
    assert_eq!(
        bot.sent(),
        vec!["🤔 Ich schaue mir das Board an …"],
        "ohne Dialog ist freier Text eine neue Idee"
    );
    assert!(bot.comments(&id).await.is_empty());

    // Ohne offenen Dialog sagt /abbrechen das auch
    bot.clear();
    bot.send("/abbrechen").await;
    assert_eq!(bot.sent(), vec!["Da ist gerade kein Dialog offen."]);

    // Nach 15 min ist der Dialog abgelaufen und der Text geht nicht ans Ticket
    bot.press(&format!("cmt:{id}")).await;
    bot.pass(DIALOG_TIMEOUT_MINUTES);
    bot.clear();
    bot.send("zu spät").await;
    assert_eq!(bot.sent().len(), 1, "{:?}", bot.sent());
    assert!(bot.sent()[0].contains("abgelaufen"), "{:?}", bot.sent());
    assert!(bot.comments(&id).await.is_empty());
}

#[tokio::test]
async fn aus_einer_notiz_wird_ein_ticket_oder_sie_wird_verworfen() {
    let bot = Bot::start().await;
    let notiz = bot.note("Cache läuft nie ab").await;

    bot.press(&format!("ntt:{notiz}")).await;

    let (text, knoepfe) = bot.edit();
    assert!(
        text.starts_with("✅ Ticket aus der Notiz über Telegram, "),
        "{text}"
    );
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    let genommen = bot.backend.services.notes.get(notiz).await.expect("Notiz");
    assert_eq!(genommen.state, NoteState::Taken);
    assert!(
        text.contains(genommen.ticket_id.as_deref().expect("Ticket")),
        "{text}"
    );

    // Ein zweiter Klick ändert nichts und meldet das
    bot.clear();
    bot.press(&format!("ndr:{notiz}")).await;
    assert_eq!(
        bot.alerts(),
        vec!["Die Notiz ist nicht mehr offen – nichts geändert."]
    );
    assert!(bot.edit().0.contains("Die Notiz ist nicht mehr offen"));

    let zweite = bot.note("Protokoll läuft voll").await;
    bot.clear();
    bot.press(&format!("ndr:{zweite}")).await;

    assert!(
        bot.edit()
            .0
            .starts_with("✅ Notiz verworfen über Telegram, "),
        "{}",
        bot.edit().0
    );
    let verworfen = bot.backend.services.notes.get(zweite).await.expect("Notiz");
    assert_eq!(verworfen.state, NoteState::Dropped);
    assert_eq!(
        verworfen.reason.as_deref(),
        Some("Verworfen (über Telegram)")
    );
}

#[tokio::test]
async fn planen_startet_die_planung_des_tickets() {
    let bot = Bot::start().await;
    let _repo = bot.with_repo("ticket/x");
    let id = bot.ticket("Rabatt planen").await;
    bot.force(
        &id,
        TicketPatch {
            status: Some(TicketStatus::InPlanning),
            ..TicketPatch::default()
        },
    )
    .await;

    bot.press(&format!("rpl:{id}")).await;

    let (text, knoepfe) = bot.edit();
    assert!(
        text.starts_with("✅ Planung gestartet über Telegram, "),
        "{text}"
    );
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    let status = bot
        .server
        .state
        .dispatch
        .planning_status(&id)
        .await
        .expect("Planungsstand");
    assert_eq!(
        status.run.map(|run| run.ticket_id),
        Some(id),
        "die Planung läuft"
    );
}

#[tokio::test]
async fn ein_knopf_zu_einem_ticket_im_falschen_zustand_aendert_nichts() {
    let bot = Bot::start().await;
    let id = bot.ticket("Noch im Backlog").await;

    bot.press(&format!("acc:{id}")).await;

    let hinweis = format!("{id} wartet nicht mehr auf eine Abnahme – nichts geändert.");
    assert_eq!(bot.alerts(), vec![hinweis.clone()]);
    let (text, knoepfe) = bot.edit();
    assert_eq!(text, format!("⚠️ {hinweis}"));
    assert!(knoepfe.is_empty(), "{knoepfe:?}");
    assert_eq!(bot.status(&id).await, TicketStatus::Backlog);

    // Auch der zweite Klick der Sicherheitsfrage prüft den Zustand noch einmal
    bot.clear();
    bot.press(&format!("accc:{id}")).await;
    assert_eq!(bot.alerts(), vec![hinweis]);
    assert_eq!(bot.status(&id).await, TicketStatus::Backlog);

    // Erneut mergen und Planen nennen je ihren eigenen Grund
    bot.clear();
    bot.press(&format!("mrg:{id}")).await;
    assert_eq!(
        bot.alerts(),
        vec![format!(
            "{id} wartet nicht mehr auf einen Merge – nichts geändert."
        )]
    );
    bot.clear();
    bot.press(&format!("rpl:{id}")).await;
    assert_eq!(
        bot.alerts(),
        vec![format!(
            "{id} wartet nicht mehr auf eine Planung – nichts geändert."
        )]
    );
    bot.clear();
    bot.press(&format!("ret:{id}")).await;
    assert_eq!(
        bot.alerts(),
        vec![format!(
            "{id} wartet nicht mehr auf eine Rückgabe – nichts geändert."
        )]
    );
}

#[tokio::test]
async fn grund_und_geaenderte_dateien_zeigen_nur_an_und_lassen_die_knoepfe_stehen() {
    let bot = Bot::start().await;
    let repo = bot.with_repo("ticket/x");
    let id = bot.blocked("Rabatt", REVIEW, Some("ticket/x")).await;
    // Ein Commit auf dem Ticket-Branch, damit es etwas zu zeigen gibt
    let default = repo.git_out(&["rev-parse", "--abbrev-ref", "HEAD"]);
    repo.run_git(&["checkout", "-q", "ticket/x"]);
    std::fs::write(repo.path().join("feature.txt"), "neu").expect("Datei");
    repo.run_git(&["add", "."]);
    repo.run_git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "-m",
        "neu",
    ]);
    repo.run_git(&["checkout", "-q", &default]);

    bot.press(&format!("dif:{id}")).await;

    let text = bot.text();
    assert!(
        text.contains(&format!("<b>Geänderte Dateien · {id}</b>")),
        "{text}"
    );
    assert!(text.contains(&format!("ticket/x → {default}")), "{text}");
    assert!(text.contains("<code>A</code> feature.txt +1 −0"), "{text}");
    assert!(bot.edits().is_empty(), "die Nachricht bleibt, wie sie ist");

    bot.clear();
    bot.press(&format!("blk:{id}")).await;

    let text = bot.text();
    assert!(text.contains(&format!("<b>{id} · Grund</b>")), "{text}");
    assert!(text.contains(REVIEW), "{text}");
    assert!(bot.edits().is_empty(), "{:?}", bot.edits());
}

#[tokio::test]
async fn ein_knopf_zu_einem_geloeschten_ticket_meldet_das_im_chat() {
    let bot = Bot::start().await;

    bot.press("acc:NXS-9999").await;

    assert!(
        bot.text().contains("Das ging gerade nicht"),
        "{}",
        bot.text()
    );
}
