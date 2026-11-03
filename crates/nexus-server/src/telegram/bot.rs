//! Die Laufzeit des Telegram-Bots im Server: Long Polling als Hintergrundaufgabe,
//! Kopplung über `/start <code>` und die Freigabeliste der Chats.
//!
//! Gestartet wird der Bot in `serve_on` wie der Takt des Dispatchers; beendet wird er über
//! das Abbruch-Signal des Servers. Ändert sich die Einstellung (Token, Schalter), weckt
//! [`TelegramBot::restart`] den Poller, damit er sie sofort liest.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use nexus_core::clock::Clock;
use nexus_core::telegram::{
    escape, BotState, BotStatus, TelegramConfig, TelegramConfigStore, TelegramStateStore,
};
use nexus_core::HarnessHome;
use tokio::sync::{watch, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::log::ServerLog;
use crate::telegram::api::{Message, TelegramApi, TelegramError, Update, User};
use crate::telegram::pairing::PairingCodes;
use crate::telegram::reqwest_api::ReqwestTelegramApi;
use crate::telegram::router::{BotContext, UpdateRouter};

/// So lange hält Telegram ein `getUpdates` offen, wenn nichts anliegt
pub const LONG_POLL_SECS: u64 = 50;

/// So lange wartet der Poller nach einem 409, bevor er es wieder versucht
pub const CONFLICT_WAIT: Duration = Duration::from_secs(5 * 60);

/// Erster Rückversatz nach einem Netzfehler
pub const BACKOFF_START: Duration = Duration::from_secs(1);

/// Größter Rückversatz nach wiederholten Netzfehlern
pub const BACKOFF_MAX: Duration = Duration::from_secs(60);

/// So lange wartet [`TelegramBot::stop`] höchstens auf einen laufenden Abruf
pub const STOP_WAIT: Duration = Duration::from_secs(5);

/// Baut den Zugriff auf die Bot-API zu einer Einstellung; im Betrieb reqwest, in Tests die
/// Attrappe
pub type TelegramApiFactory = Arc<dyn Fn(&TelegramConfig) -> Arc<dyn TelegramApi> + Send + Sync>;

/// Der Zugriff über reqwest mit `api_base` und Token aus der Einstellung
pub fn reqwest_factory() -> TelegramApiFactory {
    Arc::new(|config: &TelegramConfig| {
        Arc::new(ReqwestTelegramApi::new(&config.api_base, &config.bot_token))
    })
}

/// Der Telegram-Bot des Servers: Poller, Kopplung und Stand.
pub struct TelegramBot {
    poller: Arc<Poller>,
    /// Die laufende Aufgabe des Pollers
    task: Mutex<Option<JoinHandle<()>>>,
    /// Zählt bei jedem [`Self::restart`] hoch; der Poller wartet auf jede Änderung
    wake: watch::Sender<u64>,
}

impl std::fmt::Debug for TelegramBot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramBot").finish_non_exhaustive()
    }
}

impl TelegramBot {
    /// Bot auf dem Benutzerordner mit dem echten Zugriff auf Telegram
    pub fn new(home: HarnessHome, clock: Clock) -> Self {
        let router = UpdateRouter::with_clock(clock.clone());
        Self::with_api(home, clock, reqwest_factory(), router)
    }

    /// Wie [`Self::new`], mit einem Router, in dem schon Handler angemeldet sind, und mit dem
    /// Zugriff aus den Einstellungen des Servers: ohne Angabe über reqwest, in Tests die
    /// Attrappe
    pub fn with_optional_api(
        home: HarnessHome,
        clock: Clock,
        api: Option<TelegramApiFactory>,
        router: UpdateRouter,
    ) -> Self {
        Self::with_api(home, clock, api.unwrap_or_else(reqwest_factory), router)
    }

    /// Bot mit eigenem Zugriff auf die Bot-API und eigenem Router (Tests, Vertrag)
    pub fn with_api(
        home: HarnessHome,
        clock: Clock,
        api: TelegramApiFactory,
        router: UpdateRouter,
    ) -> Self {
        let (wake, woken) = watch::channel(0);
        Self {
            poller: Arc::new(Poller {
                config: TelegramConfigStore::new(home.clone()),
                state: TelegramStateStore::new(home.clone()),
                api,
                router: Arc::new(router),
                pairing: PairingCodes::with_clock(clock.clone()),
                status: Arc::new(RwLock::new(BotStatus::default())),
                clock,
                log: ServerLog::new(&home),
                greeted: std::sync::Mutex::new(HashSet::new()),
                woken,
            }),
            task: Mutex::new(None),
            wake,
        }
    }

    /// Startet den Poller als Hintergrundaufgabe; ein zweiter Aufruf tut nichts.
    /// Die Aufgabe endet, wenn `shutdown` ausgelöst wird
    pub async fn start(&self, shutdown: CancellationToken) {
        let mut slot = self.task.lock().await;
        if slot.is_some() {
            return;
        }
        let poller = self.poller.clone();
        *slot = Some(tokio::spawn(async move { poller.run(shutdown).await }));
    }

    /// Weckt den Poller, damit er die Einstellung neu liest (nach dem Speichern, auch nach
    /// einem 401 oder 409)
    pub fn restart(&self) {
        self.wake.send_modify(|runde| *runde += 1);
    }

    /// Hält den Poller an und wartet höchstens [`STOP_WAIT`] auf den laufenden Abruf.
    /// Das Abbruch-Signal des Servers beendet ihn ohnehin; das hier ist für Tests und für
    /// den geordneten Stopp
    pub async fn stop(&self) {
        let Some(handle) = self.task.lock().await.take() else {
            return;
        };
        if tokio::time::timeout(STOP_WAIT, handle).await.is_err() {
            self.poller.log.write(
                "Telegram: Poller nach dem Abbruch nicht beendet – Server schließt trotzdem",
            );
        }
    }

    /// Der Stand des Bots für die Oberfläche
    pub async fn status(&self) -> BotStatus {
        self.poller.status.read().await.clone()
    }

    /// Die offenen Einmal-Codes der Kopplung
    pub fn pairing(&self) -> &PairingCodes {
        &self.poller.pairing
    }

    /// Der Router; spätere Tickets melden ihre Handler vor dem Start an
    pub fn router(&self) -> &UpdateRouter {
        &self.poller.router
    }

    /// Zugriff auf die Bot-API zu dieser Einstellung – derselbe Weg, den der Poller nimmt.
    /// Die Routen brauchen ihn für die Testnachricht
    pub fn api(&self, config: &TelegramConfig) -> Arc<dyn TelegramApi> {
        (self.poller.api)(config)
    }
}

/// Was nach einem Abruf von `getUpdates` geschehen soll
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    /// Gleich wieder abrufen
    Again,
    /// So lange warten, dann wieder abrufen (Rate-Grenze, Netzfehler)
    Wait(Duration),
    /// Die Einstellung neu lesen: so lange warten oder bis sie sich ändert
    Reconnect(Duration),
    /// Warten, bis sich die Einstellung ändert (401)
    AwaitConfig,
}

/// Wie ein Anlauf des Pollers geendet hat
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cycle {
    /// Der Server endet
    Stop,
    /// Warten, bis sich die Einstellung ändert
    AwaitConfig,
    /// So lange warten (oder bis sich die Einstellung ändert), dann neu anlaufen
    Retry(Duration),
}

/// Der Poller: alles, was die Hintergrundaufgabe braucht.
struct Poller {
    config: TelegramConfigStore,
    state: TelegramStateStore,
    api: TelegramApiFactory,
    router: Arc<UpdateRouter>,
    pairing: PairingCodes,
    status: Arc<RwLock<BotStatus>>,
    clock: Clock,
    log: ServerLog,
    /// Chats, die ohne Kopplung geschrieben haben; jeder steht nur einmal im Protokoll
    greeted: std::sync::Mutex<HashSet<i64>>,
    /// Zählt hoch, sobald sich die Einstellung geändert hat
    woken: watch::Receiver<u64>,
}

impl Poller {
    /// Die Schleife: Einstellung lesen, Bot begrüßen, Updates abholen – bis zum Abbruch
    async fn run(self: Arc<Self>, shutdown: CancellationToken) {
        while !shutdown.is_cancelled() {
            let weiter = match self.cycle(&shutdown).await {
                Cycle::Stop => break,
                Cycle::AwaitConfig => self.wait_for_config(&shutdown, None).await,
                Cycle::Retry(wait) => self.wait_for_config(&shutdown, Some(wait)).await,
            };
            if !weiter {
                break;
            }
        }
        self.set_state(BotState::Off).await;
    }

    /// Ein Anlauf: Einstellung lesen, `getMe`, dann abrufen, bis etwas dazwischenkommt
    async fn cycle(&self, shutdown: &CancellationToken) -> Cycle {
        let config = match self.config.read() {
            Ok(config) => config,
            Err(error) => {
                self.fail(
                    BotState::Error,
                    &format!("Einstellung nicht lesbar: {error}"),
                )
                .await;
                return Cycle::AwaitConfig;
            }
        };
        if !config.enabled || config.bot_token.is_empty() {
            self.set_state(BotState::Off).await;
            return Cycle::AwaitConfig;
        }
        self.set_state(BotState::Starting).await;
        let api = (self.api)(&config);
        let me = match api.get_me().await {
            Ok(me) => me,
            Err(TelegramError::Unauthorized) => {
                self.token_invalid().await;
                return Cycle::AwaitConfig;
            }
            Err(error) => {
                self.fail(BotState::Error, &error.to_string()).await;
                return Cycle::Retry(BACKOFF_START);
            }
        };
        self.running(&me).await;
        // Die Befehlsliste des Bots stammt aus dem Router; ein Fehler dabei hält nichts auf
        if let Err(error) = api.set_my_commands(&self.router.command_list()).await {
            self.log
                .write(&format!("Telegram: Befehlsliste nicht gesetzt – {error}"));
        }
        self.poll(shutdown, &api, &me).await
    }

    /// Holt Updates ab, bis die Einstellung neu gelesen werden muss oder der Server endet
    async fn poll(
        &self,
        shutdown: &CancellationToken,
        api: &Arc<dyn TelegramApi>,
        me: &User,
    ) -> Cycle {
        let mut backoff = BACKOFF_START;
        loop {
            if shutdown.is_cancelled() {
                return Cycle::Stop;
            }
            let offset = self.state.read().map(|state| state.offset).unwrap_or(0);
            let abgerufen = tokio::select! {
                _ = shutdown.cancelled() => return Cycle::Stop,
                abgerufen = api.get_updates(offset, LONG_POLL_SECS) => abgerufen,
            };
            let next = match abgerufen {
                Ok(updates) => {
                    backoff = BACKOFF_START;
                    self.consume(api, me, updates).await
                }
                Err(error) => self.on_error(error, &mut backoff).await,
            };
            match next {
                Next::Again => continue,
                Next::Wait(wait) => {
                    if !self.sleep(shutdown, wait).await {
                        return Cycle::Stop;
                    }
                }
                Next::Reconnect(wait) => return Cycle::Retry(wait),
                Next::AwaitConfig => return Cycle::AwaitConfig,
            }
        }
    }

    /// Verarbeitet die abgeholten Updates und merkt den Versatz nach jedem einzelnen
    async fn consume(&self, api: &Arc<dyn TelegramApi>, me: &User, updates: Vec<Update>) -> Next {
        // Jedes abgeholte Update wird verarbeitet; wie viele es höchstens sind, bestimmt
        // Telegram (ohne eigene Angabe 100 je Abruf)
        for update in updates {
            let update_id = update.update_id;
            self.handle(api, me, update).await;
            // Erst merken, wenn das Update verarbeitet ist: ein Absturz davor wiederholt es,
            // ein Absturz danach überspringt es nicht
            if let Err(error) = self.state.write_offset(update_id + 1) {
                self.log
                    .write(&format!("Telegram: Versatz nicht gespeichert – {error}"));
            }
            self.seen().await;
        }
        Next::Again
    }

    /// Was ein Fehler beim Abruf bedeutet
    async fn on_error(&self, error: TelegramError, backoff: &mut Duration) -> Next {
        match error {
            TelegramError::Unauthorized => {
                self.token_invalid().await;
                Next::AwaitConfig
            }
            TelegramError::Conflict => {
                self.fail(BotState::Conflict, CONFLICT_MESSAGE).await;
                Next::Reconnect(CONFLICT_WAIT)
            }
            TelegramError::TooManyRequests { retry_after } => {
                // Kein Fehlerzustand: Telegram bremst nur, der Bot läuft weiter
                Next::Wait(Duration::from_secs(retry_after))
            }
            error => {
                self.fail(BotState::Error, &error.to_string()).await;
                let wait = *backoff;
                *backoff = (*backoff * 2).min(BACKOFF_MAX);
                Next::Wait(wait)
            }
        }
    }

    /// Ein einzelnes Update: Freigabe prüfen, dann an die Kopplung oder den Router
    async fn handle(&self, api: &Arc<dyn TelegramApi>, me: &User, update: Update) {
        let chats = self
            .config
            .read()
            .map(|config| config.chats)
            .unwrap_or_default();
        if let Some(message) = update.message {
            self.handle_message(api, me, &chats, message).await;
            return;
        }
        if let Some(query) = update.callback_query {
            let Some(chat_id) = query.message.as_ref().map(|message| message.chat.id) else {
                return;
            };
            if !chats.contains(&chat_id) {
                // Ein Knopf aus einem fremden Chat bleibt ohne Antwort
                return;
            }
            let ctx = self.context(
                api,
                me,
                chat_id,
                query.message.as_ref().map(|m| m.message_id),
            );
            if let Err(error) = self.router.handle_callback(&ctx, &query).await {
                self.log.write(&format!("Telegram: Knopf – {error}"));
            }
        }
    }

    /// Eine Nachricht: gekoppelt → Router, sonst nur `/start <code>`
    async fn handle_message(
        &self,
        api: &Arc<dyn TelegramApi>,
        me: &User,
        chats: &[i64],
        message: Message,
    ) {
        let chat_id = message.chat.id;
        let ctx = self.context(api, me, chat_id, Some(message.message_id));
        if chats.contains(&chat_id) {
            if let Err(error) = self.router.handle_message(&ctx, &message).await {
                self.log.write(&format!("Telegram: Nachricht – {error}"));
            }
            return;
        }
        if let Some(code) = pairing_code(&message) {
            // Auch ein Versuch zu koppeln zählt auf die Rate-Grenze; sonst ließe sich der
            // Bot aus einem fremden Chat endlos antworten lassen
            match self.router.allow(&ctx).await {
                Ok(true) => self.pair(&ctx, &message, &code).await,
                Ok(false) => {}
                Err(error) => self.log.write(&format!("Telegram: Kopplung – {error}")),
            }
            return;
        }
        // Nicht gekoppelt und kein Code: ohne Antwort, aber einmal je Chat im Protokoll
        if self
            .greeted
            .lock()
            .expect("Sperre der ungekoppelten Chats")
            .insert(chat_id)
        {
            self.log.write(&format!(
                "Telegram: Nachricht aus dem nicht gekoppelten Chat {chat_id} verworfen"
            ));
        }
    }

    /// Löst `/start <code>` ein; Gruppen werden nie gekoppelt
    async fn pair(&self, ctx: &BotContext, message: &Message, code: &str) {
        if message.chat.chat_type != "private" {
            self.log.write(&format!(
                "Telegram: Kopplung aus dem Chat {} abgelehnt – nur Einzelchats",
                message.chat.id
            ));
            let _ = ctx.reply(GROUP_REFUSED).await;
            return;
        }
        if !self.pairing.redeem(code) {
            let _ = ctx.reply(CODE_INVALID).await;
            return;
        }
        if let Err(error) = self.config.add_chat(message.chat.id) {
            self.log
                .write(&format!("Telegram: Kopplung nicht gespeichert – {error}"));
            let _ = ctx.reply(PAIRING_FAILED).await;
            return;
        }
        self.log
            .write(&format!("Telegram: Chat {} gekoppelt", message.chat.id));
        self.greeted
            .lock()
            .expect("Sperre der ungekoppelten Chats")
            .remove(&message.chat.id);
        let begruessung = format!("{}\n\n{}", paired_text(ctx), self.router.help_text());
        let _ = ctx.reply(&begruessung).await;
    }

    /// Kontext für die Handler
    fn context(
        &self,
        api: &Arc<dyn TelegramApi>,
        me: &User,
        chat_id: i64,
        message_id: Option<i64>,
    ) -> BotContext {
        BotContext {
            api: api.clone(),
            chat_id,
            message_id,
            bot_name: me.username.clone(),
        }
    }

    /// Wartet `wait` oder bis zum Abbruch; `false` heißt „der Server endet“
    async fn sleep(&self, shutdown: &CancellationToken, wait: Duration) -> bool {
        tokio::select! {
            _ = shutdown.cancelled() => false,
            _ = tokio::time::sleep(wait) => true,
        }
    }

    /// Wartet auf eine geänderte Einstellung, höchstens `at_most` lang (ohne Angabe
    /// unbegrenzt); `false` heißt „der Server endet“
    async fn wait_for_config(
        &self,
        shutdown: &CancellationToken,
        at_most: Option<Duration>,
    ) -> bool {
        let mut woken = self.woken.clone();
        let geaendert = async {
            let _ = woken.changed().await;
        };
        match at_most {
            Some(at_most) => tokio::select! {
                _ = shutdown.cancelled() => false,
                _ = geaendert => true,
                _ = tokio::time::sleep(at_most) => true,
            },
            None => tokio::select! {
                _ = shutdown.cancelled() => false,
                _ = geaendert => true,
            },
        }
    }

    /// Setzt den Zustand, lässt alles Übrige stehen
    async fn set_state(&self, state: BotState) {
        let mut status = self.status.write().await;
        status.state = state;
        if state == BotState::Running {
            status.last_error = None;
        }
    }

    /// Der Bot ist da: Name, ID und Zustand `running`
    async fn running(&self, me: &User) {
        let mut status = self.status.write().await;
        status.state = BotState::Running;
        status.bot_name = me.username.clone();
        status.bot_id = Some(me.id);
        status.last_error = None;
    }

    /// Merkt den Zeitpunkt des letzten verarbeiteten Updates
    async fn seen(&self) {
        self.status.write().await.last_update_at = Some(self.clock.iso());
    }

    /// Setzt Zustand und letzten Fehler und schreibt ihn einmal ins Protokoll
    async fn fail(&self, state: BotState, message: &str) {
        {
            let mut status = self.status.write().await;
            // Dieselbe Meldung im selben Zustand nicht in jedem Durchlauf protokollieren
            if status.state == state && status.last_error.as_deref() == Some(message) {
                return;
            }
            status.state = state;
            status.last_error = Some(message.to_string());
        }
        self.log.write(&format!("Telegram: {message}"));
    }

    /// Telegram lehnt das Token ab: Zustand merken und bis zur nächsten Einstellung ruhen
    async fn token_invalid(&self) {
        self.fail(BotState::TokenInvalid, TOKEN_MESSAGE).await;
    }
}

/// Meldung im Zustand [`BotState::Conflict`]
const CONFLICT_MESSAGE: &str = "Bot läuft auf einem anderen Rechner";

/// Meldung im Zustand [`BotState::TokenInvalid`]
const TOKEN_MESSAGE: &str = "Telegram lehnt das Bot-Token ab – bitte in den Einstellungen prüfen";

/// Antwort auf einen falschen oder abgelaufenen Code
const CODE_INVALID: &str = "Code ungültig oder abgelaufen";

/// Antwort auf `/start <code>` in einer Gruppe
const GROUP_REFUSED: &str = "Koppeln geht nur im Einzelchat mit mir.";

/// Antwort, wenn die Kopplung nicht gespeichert werden konnte
const PAIRING_FAILED: &str = "Die Kopplung ließ sich nicht speichern – bitte später wieder.";

/// Der Code aus `/start <code>`; `None`, wenn die Nachricht kein `/start` mit Argument ist
fn pairing_code(message: &Message) -> Option<String> {
    let text = message.text.as_deref()?;
    let command = crate::telegram::router::parse_command(text)?;
    if command.name != "start" || command.args.is_empty() {
        return None;
    }
    Some(command.args)
}

/// Die Begrüßung nach einer gelungenen Kopplung
fn paired_text(ctx: &BotContext) -> String {
    match &ctx.bot_name {
        Some(name) => format!(
            "<b>Gekoppelt mit Nexus</b>\nDieser Chat ist jetzt für @{} freigegeben.",
            escape(name)
        ),
        None => "<b>Gekoppelt mit Nexus</b>\nDieser Chat ist jetzt freigegeben.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use chrono::{DateTime, TimeDelta, Utc};
    use nexus_core::telegram::TelegramConfig;
    use serde_json::json;

    use super::*;
    use crate::telegram::fake::FakeTelegram;

    /// Ein Token, das dem Muster von @BotFather entspricht
    const TOKEN: &str = "123456:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw";

    /// Der Chat, aus dem die Tests schreiben
    const CHAT: i64 = 4242;

    /// Ein Bot samt Attrappe auf einem eigenen Benutzerordner
    struct Harness {
        _dir: tempfile::TempDir,
        home: HarnessHome,
        fake: Arc<FakeTelegram>,
        bot: Arc<TelegramBot>,
        shutdown: CancellationToken,
        /// Hebel, der die Uhr des Bots um Minuten vorstellt (für abgelaufene Codes)
        minuten: Arc<AtomicI64>,
    }

    impl Harness {
        /// Benutzerordner mit `telegram.yaml` und ein Bot auf der Attrappe
        fn new(config: TelegramConfig) -> Self {
            let dir = tempfile::Builder::new()
                .prefix("harness-telegram-bot-")
                .tempdir()
                .expect("Ordner");
            let home = HarnessHome::at(dir.path());
            TelegramConfigStore::new(home.clone())
                .write(&config)
                .expect("Einstellung");
            let fake = Arc::new(FakeTelegram::new());
            // Ohne das fragte der Poller die Attrappe ohne Pause
            fake.wait_when_empty();
            let start: DateTime<Utc> = "2026-11-03T12:00:00Z".parse().expect("Zeitpunkt");
            let minuten = Arc::new(AtomicI64::new(0));
            let gelesen = minuten.clone();
            let clock =
                Clock::from_fn(move || start + TimeDelta::minutes(gelesen.load(Ordering::Relaxed)));
            let api: TelegramApiFactory = {
                let fake = fake.clone();
                Arc::new(move |_config| fake.clone() as Arc<dyn TelegramApi>)
            };
            let router = UpdateRouter::with_clock(clock.clone());
            Self {
                _dir: dir,
                home: home.clone(),
                fake,
                bot: Arc::new(TelegramBot::with_api(home, clock, api, router)),
                shutdown: CancellationToken::new(),
                minuten,
            }
        }

        /// Ein eingeschalteter Bot mit Token und diesen freigegebenen Chats
        fn enabled(chats: Vec<i64>) -> Self {
            Self::new(TelegramConfig {
                enabled: true,
                bot_token: TOKEN.to_string(),
                chats,
                ..TelegramConfig::default()
            })
        }

        /// Startet den Poller
        async fn start(&self) {
            self.bot.start(self.shutdown.clone()).await;
        }

        /// Lässt den Poller laufen, ohne die Uhr vorzustellen
        async fn durchlaufen(&self) {
            for _ in 0..100 {
                tokio::task::yield_now().await;
            }
        }

        /// Beendet den Poller wie das Herunterfahren des Servers
        async fn beenden(&self) {
            self.shutdown.cancel();
            self.bot.stop().await;
        }

        /// Die gespeicherte Einstellung
        fn config(&self) -> TelegramConfig {
            TelegramConfigStore::new(self.home.clone())
                .read()
                .expect("Einstellung")
        }

        /// Der gespeicherte Versatz
        fn offset(&self) -> i64 {
            TelegramStateStore::new(self.home.clone())
                .read()
                .expect("Zustand")
                .offset
        }

        /// Die Texte aller geschickten Nachrichten
        fn sent(&self) -> Vec<String> {
            self.fake
                .calls_of("sendMessage")
                .into_iter()
                .map(|call| call.params["text"].as_str().unwrap_or_default().to_string())
                .collect()
        }

        /// Zahl der Abrufe von `getUpdates`
        fn polls(&self) -> usize {
            self.fake.calls_of("getUpdates").len()
        }
    }

    /// Ein Update mit einer Textnachricht aus einem Chat
    fn text_update(update_id: i64, chat_id: i64, chat_type: &str, text: &str) -> Update {
        serde_json::from_value(json!({
            "update_id": update_id,
            "message": {
                "message_id": update_id,
                "chat": { "id": chat_id, "type": chat_type },
                "from": { "id": 7, "first_name": "Ben" },
                "text": text,
            },
        }))
        .expect("Update")
    }

    /// Ein Update aus dem Einzelchat [`CHAT`]
    fn aus_chat(update_id: i64, text: &str) -> Update {
        text_update(update_id, CHAT, "private", text)
    }

    #[tokio::test(start_paused = true)]
    async fn koppelt_einen_chat_mit_einem_gultigen_code() {
        let h = Harness::enabled(vec![]);
        let code = h.bot.pairing().create().code;
        h.fake
            .push_updates(vec![aus_chat(1, &format!("/start {code}"))]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        assert_eq!(h.config().chats, vec![CHAT]);
        let texts = h.sent();
        assert_eq!(texts.len(), 1);
        assert!(texts[0].contains("Gekoppelt mit Nexus"), "{}", texts[0]);
        assert!(
            texts[0].contains("/hilfe"),
            "die Hilfe kommt mit: {}",
            texts[0]
        );
        assert_eq!(h.bot.pairing().open_count(), 0, "der Code ist verbraucht");
    }

    #[tokio::test(start_paused = true)]
    async fn weist_einen_falschen_code_ab() {
        let h = Harness::enabled(vec![]);
        h.bot.pairing().create();
        h.fake.push_updates(vec![aus_chat(1, "/start ZZZZZZ")]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        assert!(h.config().chats.is_empty());
        assert_eq!(h.sent(), vec![CODE_INVALID.to_string()]);
    }

    #[tokio::test(start_paused = true)]
    async fn weist_einen_abgelaufenen_code_ab() {
        let h = Harness::enabled(vec![]);
        let code = h.bot.pairing().create().code;
        // Der Code gilt zehn Minuten; die Uhr des Bots steht jetzt elf Minuten weiter
        h.minuten.store(11, Ordering::Relaxed);
        h.fake
            .push_updates(vec![aus_chat(1, &format!("/start {code}"))]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        assert!(h.config().chats.is_empty());
        assert_eq!(h.sent(), vec![CODE_INVALID.to_string()]);
    }

    #[tokio::test(start_paused = true)]
    async fn koppelt_eine_gruppe_nie() {
        let h = Harness::enabled(vec![]);
        let code = h.bot.pairing().create().code;
        h.fake.push_updates(vec![text_update(
            1,
            -100777,
            "group",
            &format!("/start {code}"),
        )]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        assert!(h.config().chats.is_empty());
        assert_eq!(h.sent(), vec![GROUP_REFUSED.to_string()]);
        assert_eq!(
            h.bot.pairing().open_count(),
            1,
            "der Code bleibt für den Einzelchat offen"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn lasst_eine_nachricht_aus_einem_fremden_chat_ohne_antwort() {
        let h = Harness::enabled(vec![CHAT]);
        h.fake
            .push_updates(vec![text_update(1, 9999, "private", "/hilfe")]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        assert!(h.sent().is_empty(), "{:?}", h.sent());
        assert_eq!(h.offset(), 2, "das Update gilt trotzdem als verarbeitet");
    }

    #[tokio::test(start_paused = true)]
    async fn antwortet_einem_gekoppelten_chat_auf_hilfe() {
        let h = Harness::enabled(vec![CHAT]);
        h.fake.push_updates(vec![aus_chat(1, "/hilfe")]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        let texts = h.sent();
        assert_eq!(texts.len(), 1);
        assert!(texts[0].contains("Das kann ich"), "{}", texts[0]);
        assert!(texts[0].contains("/hilfe"), "{}", texts[0]);
    }

    #[tokio::test(start_paused = true)]
    async fn verweist_bei_einem_unbekannten_befehl_auf_hilfe() {
        let h = Harness::enabled(vec![CHAT]);
        h.fake.push_updates(vec![aus_chat(1, "/gibtesnicht")]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        let texts = h.sent();
        assert_eq!(texts.len(), 1);
        assert!(texts[0].contains("/hilfe"), "{}", texts[0]);
    }

    #[tokio::test(start_paused = true)]
    async fn setzt_die_befehlsliste_des_bots() {
        let h = Harness::enabled(vec![CHAT]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        let calls = h.fake.calls_of("setMyCommands");
        assert_eq!(calls.len(), 1);
        let commands = calls[0].params["commands"].as_array().unwrap().clone();
        assert_eq!(commands[0][0], "start");
        assert_eq!(commands[1][0], "hilfe");
    }

    #[tokio::test(start_paused = true)]
    async fn merkt_den_versatz_und_setzt_nach_einem_neustart_dort_fort() {
        let h = Harness::enabled(vec![CHAT]);
        h.fake
            .push_updates(vec![aus_chat(7, "/hilfe"), aus_chat(8, "/hilfe")]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        assert_eq!(h.offset(), 9, "nach update_id 8 folgt 9");
        // Ein zweiter Bot auf demselben Benutzerordner ist der Server nach dem Neustart
        let fake = Arc::new(FakeTelegram::new());
        fake.wait_when_empty();
        let api: TelegramApiFactory = {
            let fake = fake.clone();
            Arc::new(move |_config| fake.clone() as Arc<dyn TelegramApi>)
        };
        let neu = Arc::new(TelegramBot::with_api(
            h.home.clone(),
            Clock::system(),
            api,
            UpdateRouter::new(),
        ));
        let shutdown = CancellationToken::new();
        neu.start(shutdown.clone()).await;
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }
        shutdown.cancel();
        neu.stop().await;

        let polls = fake.calls_of("getUpdates");
        assert!(!polls.is_empty());
        assert_eq!(
            polls[0].params["offset"], 9,
            "der Abruf setzt beim Versatz an"
        );
        assert_eq!(polls[0].params["timeout"], LONG_POLL_SECS);
    }

    #[tokio::test(start_paused = true)]
    async fn bleibt_ohne_token_aus() {
        let h = Harness::new(TelegramConfig {
            enabled: true,
            ..TelegramConfig::default()
        });

        h.start().await;
        h.durchlaufen().await;

        assert_eq!(h.bot.status().await.state, BotState::Off);
        assert_eq!(h.polls(), 0, "ohne Token wird nichts abgerufen");
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn bleibt_ausgeschaltet_aus() {
        let h = Harness::new(TelegramConfig {
            enabled: false,
            bot_token: TOKEN.to_string(),
            ..TelegramConfig::default()
        });

        h.start().await;
        h.durchlaufen().await;

        assert_eq!(h.bot.status().await.state, BotState::Off);
        assert_eq!(h.polls(), 0);
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn meldet_den_bot_im_zustand_running() {
        let h = Harness::enabled(vec![]);

        h.start().await;
        h.durchlaufen().await;

        let status = h.bot.status().await;
        assert_eq!(status.state, BotState::Running);
        assert_eq!(status.bot_name.as_deref(), Some("nexus_test_bot"));
        assert_eq!(status.bot_id, Some(7012345678));
        assert_eq!(status.last_error, None);
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn merkt_den_zeitpunkt_des_letzten_updates() {
        let h = Harness::enabled(vec![CHAT]);
        h.fake.push_updates(vec![aus_chat(1, "/hilfe")]);

        h.start().await;
        h.durchlaufen().await;

        assert_eq!(
            h.bot.status().await.last_update_at.as_deref(),
            Some("2026-11-03T12:00:00.000Z")
        );
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn setzt_bei_401_tokeninvalid_und_pollt_nicht_weiter() {
        let h = Harness::enabled(vec![]);
        h.fake.fail("getUpdates", TelegramError::Unauthorized);

        h.start().await;
        h.durchlaufen().await;
        let nach_401 = h.polls();

        assert_eq!(h.bot.status().await.state, BotState::TokenInvalid);
        assert_eq!(nach_401, 1);
        // Auch nach einer Stunde fragt er von sich aus nicht wieder
        tokio::time::advance(Duration::from_secs(3600)).await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), nach_401, "ohne neue Einstellung bleibt er ruhig");

        // Erst die geänderte Einstellung weckt ihn
        h.fake.clear_failure("getUpdates");
        h.bot.restart();
        h.durchlaufen().await;

        assert!(h.polls() > nach_401, "nach dem Speichern pollt er wieder");
        assert_eq!(h.bot.status().await.state, BotState::Running);
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn setzt_bei_401_aus_getme_tokeninvalid() {
        let h = Harness::enabled(vec![]);
        h.fake.fail("getMe", TelegramError::Unauthorized);

        h.start().await;
        h.durchlaufen().await;

        assert_eq!(h.bot.status().await.state, BotState::TokenInvalid);
        assert_eq!(h.polls(), 0);
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn setzt_bei_409_conflict_und_wartet_funf_minuten() {
        let h = Harness::enabled(vec![]);
        h.fake.fail("getUpdates", TelegramError::Conflict);

        h.start().await;
        h.durchlaufen().await;

        let status = h.bot.status().await;
        assert_eq!(status.state, BotState::Conflict);
        assert_eq!(status.last_error.as_deref(), Some(CONFLICT_MESSAGE));
        assert_eq!(h.polls(), 1);

        // Nach vier Minuten hat er es noch nicht wieder versucht
        tokio::time::advance(Duration::from_secs(4 * 60)).await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), 1);

        // Nach fünf Minuten läuft er neu an
        tokio::time::advance(Duration::from_secs(2 * 60)).await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), 2);
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn wartet_bei_429_die_angegebene_zeit_ab() {
        let h = Harness::enabled(vec![]);
        h.fake.fail(
            "getUpdates",
            TelegramError::TooManyRequests { retry_after: 30 },
        );

        h.start().await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), 1);
        // Telegram bremst nur; das ist kein Fehlerzustand
        assert_eq!(h.bot.status().await.state, BotState::Running);

        tokio::time::advance(Duration::from_secs(29)).await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), 1);

        tokio::time::advance(Duration::from_secs(2)).await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), 2);
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn wachst_den_ruckversatz_nach_netzfehlern() {
        let h = Harness::enabled(vec![]);
        h.fake.fail(
            "getUpdates",
            TelegramError::Network("Verbindung abgelehnt".to_string()),
        );

        h.start().await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), 1);
        assert_eq!(h.bot.status().await.state, BotState::Error);

        // Erst nach einer Sekunde der zweite Versuch
        tokio::time::advance(Duration::from_millis(1100)).await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), 2);

        // Dann nach zwei Sekunden der dritte
        tokio::time::advance(Duration::from_millis(1100)).await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), 2, "eine Sekunde reicht jetzt nicht mehr");
        tokio::time::advance(Duration::from_millis(1000)).await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), 3);
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn nennt_in_einem_netzfehler_kein_token() {
        let h = Harness::enabled(vec![]);
        h.fake.fail(
            "getUpdates",
            TelegramError::Network("Verbindung abgelehnt".to_string()),
        );

        h.start().await;
        h.durchlaufen().await;

        let error = h.bot.status().await.last_error.unwrap_or_default();
        assert!(!error.contains(TOKEN), "{error}");
        assert!(!error.contains("123456:"), "{error}");
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn beendet_den_poller_beim_herunterfahren() {
        let h = Harness::enabled(vec![]);
        h.start().await;
        h.durchlaufen().await;
        assert_eq!(h.bot.status().await.state, BotState::Running);

        h.beenden().await;

        assert_eq!(h.bot.status().await.state, BotState::Off);
        let vorher = h.polls();
        tokio::time::advance(Duration::from_secs(600)).await;
        h.durchlaufen().await;
        assert_eq!(h.polls(), vorher, "nach dem Abbruch ruft er nichts mehr ab");
    }

    #[tokio::test(start_paused = true)]
    async fn startet_den_poller_nur_einmal() {
        let h = Harness::enabled(vec![]);

        h.start().await;
        h.start().await;
        h.durchlaufen().await;

        assert_eq!(h.fake.calls_of("getMe").len(), 1);
        h.beenden().await;
    }

    #[tokio::test(start_paused = true)]
    async fn greift_die_rate_grenze_je_chat() {
        let h = Harness::enabled(vec![CHAT]);
        let updates = (1..=crate::telegram::router::RATE_LIMIT as i64 + 3)
            .map(|id| aus_chat(id, "/hilfe"))
            .collect::<Vec<_>>();
        h.fake.push_updates(updates);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        let texts = h.sent();
        assert_eq!(
            texts.len(),
            crate::telegram::router::RATE_LIMIT + 1,
            "30 Antworten und ein Hinweis auf die Grenze"
        );
        assert_eq!(
            h.offset(),
            crate::telegram::router::RATE_LIMIT as i64 + 4,
            "auch die abgewiesenen Updates gelten als verarbeitet"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn bremst_auch_die_versuche_zu_koppeln() {
        let h = Harness::enabled(vec![]);
        let updates = (1..=crate::telegram::router::RATE_LIMIT as i64 + 3)
            .map(|id| aus_chat(id, "/start ZZZZZZ"))
            .collect::<Vec<_>>();
        h.fake.push_updates(updates);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        assert_eq!(
            h.sent().len(),
            crate::telegram::router::RATE_LIMIT + 1,
            "30 Abweisungen und ein Hinweis auf die Grenze"
        );
        assert!(h.config().chats.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn beantwortet_einen_veralteten_knopf_aus_einem_gekoppelten_chat() {
        let h = Harness::enabled(vec![CHAT]);
        h.fake.push_updates(vec![serde_json::from_value(json!({
            "update_id": 1,
            "callback_query": {
                "id": "cb-1",
                "from": { "id": 7, "first_name": "Ben" },
                "message": { "message_id": 5, "chat": { "id": CHAT, "type": "private" } },
                "data": "quatsch:1:2:3",
            },
        }))
        .expect("Update")]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        let calls = h.fake.calls_of("answerCallbackQuery");
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].params["text"],
            crate::telegram::router::STALE_BUTTON
        );
    }

    #[tokio::test(start_paused = true)]
    async fn lasst_einen_knopf_aus_einem_fremden_chat_ohne_antwort() {
        let h = Harness::enabled(vec![CHAT]);
        h.fake.push_updates(vec![serde_json::from_value(json!({
            "update_id": 1,
            "callback_query": {
                "id": "cb-1",
                "from": { "id": 7, "first_name": "Ben" },
                "message": { "message_id": 5, "chat": { "id": 9999, "type": "private" } },
                "data": "x",
            },
        }))
        .expect("Update")]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        assert!(h.fake.calls_of("answerCallbackQuery").is_empty());
        assert_eq!(h.offset(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn antwortet_nach_der_kopplung_im_selben_abruf_auf_befehle() {
        let h = Harness::enabled(vec![]);
        let code = h.bot.pairing().create().code;
        h.fake.push_updates(vec![
            aus_chat(1, &format!("/start {code}")),
            aus_chat(2, "/hilfe"),
        ]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        let texts = h.sent();
        assert_eq!(texts.len(), 2, "{texts:?}");
        assert!(texts[1].contains("Das kann ich"), "{}", texts[1]);
    }

    #[tokio::test(start_paused = true)]
    async fn schreibt_den_ungekoppelten_chat_nur_einmal_ins_protokoll() {
        let h = Harness::enabled(vec![]);
        h.fake
            .push_updates(vec![aus_chat(1, "hallo"), aus_chat(2, "noch mal")]);

        h.start().await;
        h.durchlaufen().await;
        h.beenden().await;

        let log = std::fs::read_to_string(ServerLog::new(&h.home).file()).unwrap_or_default();
        let zeilen = log
            .lines()
            .filter(|line| line.contains("nicht gekoppelten Chat"))
            .count();
        assert_eq!(zeilen, 1, "{log}");
        assert!(h.sent().is_empty(), "{:?}", h.sent());
    }

    #[test]
    fn baut_den_zugriff_aus_adresse_und_token_der_einstellung() {
        // Die Fabrik des Betriebs darf die Einstellung annehmen, ohne zu stolpern
        let factory = reqwest_factory();
        let api = factory(&TelegramConfig {
            bot_token: TOKEN.to_string(),
            ..TelegramConfig::default()
        });
        // Mehr lässt sich ohne Netz nicht prüfen; dass nichts das Token verrät, prüft
        // `reqwest_api`
        assert_eq!(Arc::strong_count(&api), 1);
    }
}
