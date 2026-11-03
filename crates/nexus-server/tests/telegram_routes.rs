//! Die Routengruppe `/api/telegram`: Einstellung (maskiert), Stand, Kopplung, Entkoppeln und
//! Testnachricht.
//!
//! Die Bot-API ist immer die Attrappe – kein Test fragt wirklich bei Telegram an.

mod common;

use std::sync::Arc;
use std::time::Duration;

use nexus_core::telegram::{BotState, TelegramConfig, TelegramConfigStore};
use nexus_server::telegram::api::{TelegramApi, TelegramError};
use nexus_server::telegram::fake::FakeTelegram;
use nexus_server::telegram::TelegramApiFactory;

use common::{TestServer, TestServerOptions};

/// Ein Token, das dem Muster von @BotFather entspricht
const TOKEN: &str = "123456:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw";

/// Die Maske dieses Tokens, wie die Oberfläche sie sieht
const MASKED: &str = "123456:…Dsaw";

/// Ein Server samt Attrappe der Bot-API
struct Harness {
    server: TestServer,
    fake: Arc<FakeTelegram>,
}

impl Harness {
    /// Server mit der Attrappe als Bot-API; der Poller läuft noch nicht
    fn new() -> Self {
        let fake = Arc::new(FakeTelegram::new());
        // Ohne das fragte der Poller die Attrappe ohne Pause
        fake.wait_when_empty();
        let api: TelegramApiFactory = {
            let fake = fake.clone();
            Arc::new(move |_config| fake.clone() as Arc<dyn TelegramApi>)
        };
        let server = TestServer::with(TestServerOptions {
            telegram_api: Some(api),
            ..TestServerOptions::default()
        });
        Self { server, fake }
    }

    /// Der Store der Einstellung dieses Servers
    fn store(&self) -> TelegramConfigStore {
        TelegramConfigStore::new(self.server.home.clone())
    }

    /// Legt eine Einstellung hin, ohne über die Route zu gehen
    fn given(&self, config: TelegramConfig) {
        self.store().write(&config).expect("Einstellung");
    }

    /// Startet den Poller wie `serve_on`
    async fn start(&self) {
        self.server
            .state
            .telegram
            .start(self.server.state.shutdown.clone())
            .await;
    }

    /// Startet den Poller und wartet, bis der Bot läuft
    async fn running(&self) {
        self.start().await;
        for _ in 0..200 {
            if self.server.state.telegram.status().await.state == BotState::Running {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!(
            "Der Bot läuft nicht: {:?}",
            self.server.state.telegram.status().await
        );
    }

    /// Wartet, bis die Attrappe `method` mindestens `count` Mal gesehen hat
    async fn wait_for(&self, method: &str, count: usize) {
        for _ in 0..200 {
            if self.fake.calls_of(method).len() >= count {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("{method} kam nur {} Mal", self.fake.calls_of(method).len());
    }

    /// Beendet den Poller
    async fn stop(&self) {
        self.server.state.shutdown.cancel();
        self.server.state.telegram.stop().await;
    }
}

#[tokio::test]
async fn liefert_die_einstellung_mit_maskierten_geheimnissen() {
    let h = Harness::new();

    let leer = h.server.get("/api/telegram/config").await;
    assert_eq!(leer.status, 200);
    let body = leer.body();
    assert_eq!(body["enabled"], false);
    assert_eq!(body["botToken"], "");
    assert_eq!(body["apiBase"], "https://api.telegram.org");
    assert_eq!(body["speech"]["apiKey"], "");

    h.given(TelegramConfig {
        bot_token: TOKEN.into(),
        chats: vec![42],
        ..TelegramConfig::default()
    });

    let gefuellt = h.server.get("/api/telegram/config").await.body();
    assert_eq!(gefuellt["botToken"], MASKED);
    assert_eq!(gefuellt["chats"], serde_json::json!([42]));
    assert!(
        !gefuellt.to_string().contains(TOKEN),
        "das Token darf nicht in der Antwort stehen: {gefuellt}"
    );
}

#[tokio::test]
async fn speichert_die_eingabe_behalt_das_token_und_weckt_den_poller() {
    let h = Harness::new();
    // Der Poller ruht, solange kein Token eingetragen und der Bot ausgeschaltet ist
    h.start().await;
    assert_eq!(h.fake.calls_of("getMe").len(), 0);

    let gespeichert = h
        .server
        .put(
            "/api/telegram/config",
            &format!(r#"{{"enabled":true,"botToken":"{TOKEN}"}}"#),
        )
        .await;

    assert_eq!(gespeichert.status, 200);
    let body = gespeichert.body();
    assert_eq!(body["enabled"], true);
    assert_eq!(body["botToken"], MASKED);
    assert_eq!(h.store().read().unwrap().bot_token, TOKEN);
    // Nach dem Speichern liest der Poller die Einstellung neu und begrüßt den Bot
    h.wait_for("getMe", 1).await;

    // Die Maske zurückzuschicken behält das Token, die übrige Eingabe greift
    let erneut = h
        .server
        .put(
            "/api/telegram/config",
            &format!(r#"{{"botToken":"{MASKED}","ideas":{{"maxRounds":3}}}}"#),
        )
        .await
        .body();
    assert_eq!(erneut["botToken"], MASKED);
    assert_eq!(erneut["ideas"]["maxRounds"], 3);
    assert_eq!(h.store().read().unwrap().bot_token, TOKEN);

    h.stop().await;
}

#[tokio::test]
async fn lehnt_eine_ungultige_einstellung_ab_ohne_das_token_zu_nennen() {
    let h = Harness::new();

    let abgelehnt = h
        .server
        .put("/api/telegram/config", r#"{"botToken":"kaputt:kurz"}"#)
        .await;

    assert_eq!(abgelehnt.status, 400);
    assert_eq!(abgelehnt.code().as_deref(), Some("telegram-config-invalid"));
    assert!(
        !abgelehnt.text.contains("kaputt:kurz"),
        "die Eingabe darf nicht in der Meldung stehen: {}",
        abgelehnt.text
    );
    // Ungültiges wird nicht geschrieben
    assert_eq!(h.store().read().unwrap().bot_token, "");
}

#[tokio::test]
async fn nennt_den_stand_samt_zahl_der_gekoppelten_chats() {
    let h = Harness::new();
    h.given(TelegramConfig {
        bot_token: TOKEN.into(),
        chats: vec![42, 7],
        ..TelegramConfig::default()
    });

    let aus = h.server.get("/api/telegram/status").await;
    assert_eq!(aus.status, 200);
    let body = aus.body();
    assert_eq!(body["bot"]["state"], "off");
    assert_eq!(body["bot"]["botName"], serde_json::Value::Null);
    assert_eq!(body["pairedChats"], 2);
    assert_eq!(body["openCodes"], 0);

    h.given(TelegramConfig {
        enabled: true,
        bot_token: TOKEN.into(),
        chats: vec![42, 7],
        ..TelegramConfig::default()
    });
    h.running().await;

    let laeuft = h.server.get("/api/telegram/status").await.body();
    assert_eq!(laeuft["bot"]["state"], "running");
    assert_eq!(laeuft["bot"]["botName"], "nexus_test_bot");
    assert_eq!(laeuft["bot"]["lastError"], serde_json::Value::Null);

    h.stop().await;
}

#[tokio::test]
async fn gibt_einen_einmal_code_mit_link_aus_nur_wenn_der_bot_lauft() {
    let h = Harness::new();

    let ohne_bot = h.server.post("/api/telegram/pair", "{}").await;
    assert_eq!(ohne_bot.status, 409);
    assert_eq!(ohne_bot.code().as_deref(), Some("telegram-not-running"));

    h.given(TelegramConfig {
        enabled: true,
        bot_token: TOKEN.into(),
        ..TelegramConfig::default()
    });
    h.running().await;

    let code = h.server.post("/api/telegram/pair", "{}").await;
    assert_eq!(code.status, 200);
    let body = code.body();
    let text = body["code"].as_str().expect("Code").to_string();
    assert_eq!(text.chars().count(), 6);
    assert_eq!(
        body["link"],
        format!("https://t.me/nexus_test_bot?start={text}")
    );
    assert!(body["expiresAt"].as_str().expect("Ende").ends_with('Z'));
    assert_eq!(
        h.server.get("/api/telegram/status").await.body()["openCodes"],
        1
    );

    h.stop().await;
}

#[tokio::test]
async fn entkoppelt_einen_chat_und_meldet_einen_unbekannten() {
    let h = Harness::new();
    h.given(TelegramConfig {
        bot_token: TOKEN.into(),
        chats: vec![42, 7],
        ..TelegramConfig::default()
    });

    let entkoppelt = h.server.delete("/api/telegram/chats/42").await;
    assert_eq!(entkoppelt.status, 200);
    assert_eq!(entkoppelt.body()["ok"], true);
    assert_eq!(h.store().read().unwrap().chats, vec![7]);

    let unbekannt = h.server.delete("/api/telegram/chats/42").await;
    assert_eq!(unbekannt.status, 404);
    assert_eq!(unbekannt.code().as_deref(), Some("telegram-chat-not-found"));

    // Was keine Zahl ist, kann auch nicht gekoppelt sein
    let keine_zahl = h.server.delete("/api/telegram/chats/abc").await;
    assert_eq!(keine_zahl.status, 404);
    assert_eq!(
        keine_zahl.code().as_deref(),
        Some("telegram-chat-not-found")
    );
}

#[tokio::test]
async fn schickt_die_testnachricht_an_alle_chats_und_nennt_die_fehler() {
    let h = Harness::new();

    let aus = h.server.post("/api/telegram/test", "{}").await;
    assert_eq!(aus.status, 409);
    assert_eq!(aus.code().as_deref(), Some("telegram-not-running"));

    h.given(TelegramConfig {
        enabled: true,
        bot_token: TOKEN.into(),
        chats: vec![42, 7],
        ..TelegramConfig::default()
    });

    let geschickt = h.server.post("/api/telegram/test", "{}").await;
    assert_eq!(geschickt.status, 200);
    assert_eq!(geschickt.body()["sent"], 2);
    assert_eq!(geschickt.body()["errors"], serde_json::json!([]));
    let chats: Vec<i64> = h
        .fake
        .calls_of("sendMessage")
        .iter()
        .map(|call| call.params["chat_id"].as_i64().expect("Chat"))
        .collect();
    assert_eq!(chats, vec![42, 7]);

    h.fake.fail(
        "sendMessage",
        TelegramError::Api {
            code: 403,
            description: "Forbidden: bot was blocked by the user".into(),
        },
    );

    let gescheitert = h.server.post("/api/telegram/test", "{}").await.body();
    assert_eq!(gescheitert["sent"], 0);
    assert_eq!(gescheitert["errors"][0]["chatId"], 42);
    let fehler = gescheitert["errors"][0]["error"].as_str().expect("Fehler");
    assert!(fehler.contains("403"), "{fehler}");
    assert!(
        !gescheitert.to_string().contains(TOKEN),
        "kein Token in der Antwort: {gescheitert}"
    );
}
