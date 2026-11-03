//! Verteilt die Updates eines freigegebenen Chats: Befehle, Knöpfe und freie Texte.
//!
//! Spätere Tickets melden hier nur noch Handler an – `/status`, `/board`, „Meine Aufgaben“,
//! die Knöpfe der Meldungen und das Erfassen von Ideen. Eingebaut sind nur `/start` und
//! `/hilfe`, der Hinweis auf `/hilfe` bei einem unbekannten Befehl, die Antwort auf einen
//! veralteten Knopf und die Rate-Grenze je Chat.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use nexus_core::clock::Clock;
use nexus_core::telegram::{escape, split_message, CallbackAction, MAX_MESSAGE_LEN};
use nexus_core::Result;

use crate::telegram::api::{CallbackQuery, InlineKeyboard, Message, TelegramApi};

/// So viele Aktionen darf ein Chat in einem Fenster auslösen
pub const RATE_LIMIT: usize = 30;

/// Länge des Fensters der Rate-Grenze in Sekunden
pub const RATE_WINDOW_SECS: i64 = 60;

/// Antwort auf einen Knopf, dessen Daten niemand (mehr) versteht
pub const STALE_BUTTON: &str = "Dieser Knopf ist veraltet";

/// Was ein Handler zum Antworten braucht: der Zugriff auf die Bot-API und der Chat.
///
/// Der Zugriff steckt im Kontext und nicht im Router, weil der Poller ihn beim Wechsel des
/// Bot-Tokens neu baut.
pub struct BotContext {
    /// Zugriff auf die Bot-API, schon mit dem Token des Bots
    pub api: Arc<dyn TelegramApi>,
    /// Der freigegebene Chat, aus dem das Update kam
    pub chat_id: i64,
    /// Die auslösende Nachricht; bei einem Knopfdruck die Nachricht mit dem Knopf
    pub message_id: Option<i64>,
    /// Benutzername des Bots (ohne @); für `/befehl@botname`
    pub bot_name: Option<String>,
}

impl BotContext {
    /// Schickt eine Antwort in den Chat; zu lange Texte gehen als mehrere Nachrichten
    pub async fn reply(&self, html: &str) -> Result<()> {
        self.send(html, None).await
    }

    /// Wie [`Self::reply`], mit Knöpfen unter der letzten Nachricht
    pub async fn reply_with(&self, html: &str, keyboard: InlineKeyboard) -> Result<()> {
        self.send(html, Some(keyboard)).await
    }

    /// Schickt `html` geteilt; die Knöpfe hängen unter dem letzten Stück
    async fn send(&self, html: &str, keyboard: Option<InlineKeyboard>) -> Result<()> {
        let chunks = split_message(html, MAX_MESSAGE_LEN);
        let last = chunks.len() - 1;
        for (index, chunk) in chunks.iter().enumerate() {
            let keyboard = if index == last {
                keyboard.clone()
            } else {
                None
            };
            self.api
                .send_message(self.chat_id, chunk, keyboard, None)
                .await
                .map_err(nexus_core::Error::unexpected)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for BotContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotContext")
            .field("chat_id", &self.chat_id)
            .field("message_id", &self.message_id)
            .finish_non_exhaustive()
    }
}

/// Ein Befehl des Bots, z. B. `/status`
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Führt den Befehl aus; `args` ist alles nach dem Befehlswort (schon beschnitten)
    async fn run(&self, ctx: &BotContext, args: &str) -> Result<()>;
}

/// Ein Knopf unter einer Nachricht des Bots
#[async_trait]
pub trait CallbackHandler: Send + Sync {
    /// Führt die Aktion aus; `false` heißt „nicht meine Aktion“, dann fragt der Router den
    /// nächsten Handler
    async fn run(&self, ctx: &BotContext, action: &CallbackAction, query_id: &str) -> Result<bool>;
}

/// Eine Nachricht ohne Befehl: freier Text oder eine Sprachnachricht
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Verarbeitet die Nachricht; `false` heißt „nicht für mich“, dann antwortet der Router
    /// mit dem Hinweis auf `/hilfe`
    async fn run(&self, ctx: &BotContext, message: &Message) -> Result<bool>;
}

/// Ein angemeldeter Befehl samt Beschreibung für `/hilfe` und `setMyCommands`
struct Command {
    name: String,
    description: String,
    handler: Arc<dyn CommandHandler>,
}

/// Ein Befehl, wie er in einer Nachricht steht
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    /// Das Befehlswort ohne Schrägstrich, klein geschrieben
    pub name: String,
    /// Der Bot hinter dem `@`, falls angegeben (in Gruppen üblich)
    pub mention: Option<String>,
    /// Alles nach dem Befehlswort, beschnitten
    pub args: String,
}

/// Zerlegt `/befehl@botname argumente`; `None`, wenn der Text nicht mit einem Befehl beginnt
pub fn parse_command(text: &str) -> Option<ParsedCommand> {
    let text = text.trim_start();
    let rest = text.strip_prefix('/')?;
    let (head, args) = match rest.find(char::is_whitespace) {
        Some(index) => (&rest[..index], rest[index..].trim()),
        None => (rest, ""),
    };
    let (name, mention) = match head.split_once('@') {
        Some((name, bot)) => (name, Some(bot.to_string())),
        None => (head, None),
    };
    if name.is_empty() {
        return None;
    }
    Some(ParsedCommand {
        name: name.to_lowercase(),
        mention,
        args: args.to_string(),
    })
}

/// Verteilt die Updates eines freigegebenen Chats an die angemeldeten Handler.
pub struct UpdateRouter {
    commands: Vec<Command>,
    callbacks: Vec<Arc<dyn CallbackHandler>>,
    messages: Vec<Arc<dyn MessageHandler>>,
    limits: RateLimits,
}

impl std::fmt::Debug for UpdateRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateRouter")
            .field(
                "commands",
                &self.commands.iter().map(|c| &c.name).collect::<Vec<_>>(),
            )
            .field("callbacks", &self.callbacks.len())
            .field("messages", &self.messages.len())
            .finish_non_exhaustive()
    }
}

impl UpdateRouter {
    /// Router mit den eingebauten Befehlen und der Systemuhr
    pub fn new() -> Self {
        Self::with_clock(Clock::system())
    }

    /// Router mit eigener Uhr (Tests lassen das Fenster der Rate-Grenze ablaufen)
    pub fn with_clock(clock: Clock) -> Self {
        Self {
            commands: Vec::new(),
            callbacks: Vec::new(),
            messages: Vec::new(),
            limits: RateLimits::new(clock),
        }
    }

    /// Meldet einen Befehl an; `name` ohne Schrägstrich. Ein zweiter Handler für denselben
    /// Namen ersetzt den ersten
    pub fn register_command(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: Arc<dyn CommandHandler>,
    ) {
        let name = name.into().trim_start_matches('/').to_lowercase();
        let command = Command {
            name,
            description: description.into(),
            handler,
        };
        match self
            .commands
            .iter()
            .position(|known| known.name == command.name)
        {
            Some(index) => self.commands[index] = command,
            None => self.commands.push(command),
        }
    }

    /// Meldet einen Handler für Knöpfe an; die Handler werden in der Reihenfolge ihrer
    /// Anmeldung gefragt, bis einer die Aktion annimmt
    pub fn register_callback(&mut self, handler: Arc<dyn CallbackHandler>) {
        self.callbacks.push(handler);
    }

    /// Meldet einen Handler für Nachrichten ohne Befehl an (freier Text, Sprachnachricht)
    pub fn register_message(&mut self, handler: Arc<dyn MessageHandler>) {
        self.messages.push(handler);
    }

    /// Die angemeldeten Befehle samt Beschreibung, wie `setMyCommands` sie braucht;
    /// die eingebauten stehen vorn
    pub fn command_list(&self) -> Vec<(String, String)> {
        let mut list: Vec<(String, String)> = BUILTIN
            .iter()
            .map(|(name, description)| {
                // Überschreibt ein späteres Ticket einen eingebauten Befehl, gilt auch
                // seine Beschreibung
                let own = self.commands.iter().find(|known| known.name == *name);
                let description = own.map_or_else(
                    || description.to_string(),
                    |known| known.description.clone(),
                );
                (name.to_string(), description)
            })
            .collect();
        list.extend(
            self.commands
                .iter()
                .filter(|command| !is_builtin(&command.name))
                .map(|command| (command.name.clone(), command.description.clone())),
        );
        list
    }

    /// Der Text von `/hilfe`
    pub fn help_text(&self) -> String {
        let mut text = String::from("<b>Das kann ich:</b>");
        for (name, description) in self.command_list() {
            text.push_str(&format!("\n/{name} – {}", escape(&description)));
        }
        text
    }

    /// Verarbeitet eine Nachricht aus einem freigegebenen Chat
    pub async fn handle_message(&self, ctx: &BotContext, message: &Message) -> Result<()> {
        let text = message.text.clone().unwrap_or_default();
        let command = parse_command(&text);
        // In Gruppen hängt Telegram `@botname` an; ein Befehl an einen anderen Bot ist nicht
        // unser Befehl und bekommt auch keinen Hinweis auf /hilfe
        if let Some(command) = &command {
            if !self.addresses_us(ctx, command) {
                return Ok(());
            }
        }
        if !self.allow(ctx).await? {
            return Ok(());
        }
        match command {
            Some(command) => self.run_command(ctx, &command).await,
            None => self.run_message(ctx, message).await,
        }
    }

    /// Verarbeitet einen Knopfdruck aus einem freigegebenen Chat
    pub async fn handle_callback(&self, ctx: &BotContext, query: &CallbackQuery) -> Result<()> {
        if !self.allow(ctx).await? {
            // Ohne Antwort dreht sich der Knopf beim Nutzer weiter; das Fenster sagt, warum
            let _ = ctx
                .api
                .answer_callback_query(&query.id, Some(RATE_HINT), true)
                .await;
            return Ok(());
        }
        let action = query.data.as_deref().and_then(CallbackAction::decode);
        let Some(action) = action else {
            return self.answer_stale(ctx, &query.id).await;
        };
        for handler in &self.callbacks {
            if handler.run(ctx, &action, &query.id).await? {
                return Ok(());
            }
        }
        // Dekodierbar, aber niemand ist zuständig: aus Sicht des Nutzers dasselbe wie ein
        // veralteter Knopf
        self.answer_stale(ctx, &query.id).await
    }

    /// Führt einen Befehl aus: eingebaut, angemeldet oder unbekannt
    async fn run_command(&self, ctx: &BotContext, command: &ParsedCommand) -> Result<()> {
        if let Some(known) = self
            .commands
            .iter()
            .find(|known| known.name == command.name)
        {
            return known.handler.run(ctx, &command.args).await;
        }
        match command.name.as_str() {
            "start" => {
                ctx.reply(&format!("{GREETING}\n\n{}", self.help_text()))
                    .await
            }
            "hilfe" => ctx.reply(&self.help_text()).await,
            _ => ctx.reply(UNKNOWN_COMMAND).await,
        }
    }

    /// Gibt eine Nachricht ohne Befehl an die Handler; nimmt keiner sie, kommt der Hinweis
    async fn run_message(&self, ctx: &BotContext, message: &Message) -> Result<()> {
        for handler in &self.messages {
            if handler.run(ctx, message).await? {
                return Ok(());
            }
        }
        ctx.reply(PLAIN_TEXT_HINT).await
    }

    /// „Dieser Knopf ist veraltet“ als Hinweisfenster
    async fn answer_stale(&self, ctx: &BotContext, query_id: &str) -> Result<()> {
        ctx.api
            .answer_callback_query(query_id, Some(STALE_BUTTON), true)
            .await
            .map_err(nexus_core::Error::unexpected)
    }

    /// Richtet sich `command` an uns? Ohne `@` immer, mit `@` nur beim eigenen Namen
    fn addresses_us(&self, ctx: &BotContext, command: &ParsedCommand) -> bool {
        match (&command.mention, &ctx.bot_name) {
            (None, _) => true,
            (Some(mention), Some(bot_name)) => mention.eq_ignore_ascii_case(bot_name),
            // Ohne eigenen Namen lässt sich das nicht entscheiden; dann gilt der Befehl
            (Some(_), None) => true,
        }
    }

    /// Zählt die Aktion auf die Rate-Grenze; `false` heißt „zu viele“. Genau die erste
    /// abgewiesene Aktion eines Fensters bekommt einen Hinweis, danach ist Ruhe.
    ///
    /// Öffentlich, weil auch die Kopplung eines noch nicht freigegebenen Chats sie zählen
    /// muss: Sonst ließe sich der Bot mit `/start <code>` endlos antworten lassen
    pub async fn allow(&self, ctx: &BotContext) -> Result<bool> {
        match self.limits.take(ctx.chat_id) {
            Verdict::Allowed => Ok(true),
            Verdict::FirstDenied => {
                ctx.reply(RATE_HINT).await?;
                Ok(false)
            }
            Verdict::Denied => Ok(false),
        }
    }
}

impl Default for UpdateRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Die eingebauten Befehle samt Beschreibung für `/hilfe` und `setMyCommands`
const BUILTIN: &[(&str, &str)] = &[
    ("start", "Begrüßung und Kopplung"),
    ("hilfe", "Diese Liste"),
];

/// Antwort auf `/start` in einem schon freigegebenen Chat
const GREETING: &str = "<b>Nexus</b>\nDieser Chat ist mit Nexus gekoppelt.";

/// Antwort auf einen Befehl, den niemand kennt
const UNKNOWN_COMMAND: &str = "Den Befehl kenne ich nicht. /hilfe zeigt, was ich kann.";

/// Antwort auf einen freien Text, solange ihn kein Handler annimmt
const PLAIN_TEXT_HINT: &str = "Schick mir einen Befehl, z. B. /hilfe";

/// Antwort auf die erste abgewiesene Aktion eines Fensters
const RATE_HINT: &str =
    "Das waren zu viele Anfragen auf einmal – versuch es in einer Minute wieder.";

/// Ist das ein eingebauter Befehl? Dann steht er in [`UpdateRouter::command_list`] ohnehin
fn is_builtin(name: &str) -> bool {
    BUILTIN.iter().any(|(builtin, _)| *builtin == name)
}

/// Ausgang einer Prüfung der Rate-Grenze
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Die Aktion zählt und darf laufen
    Allowed,
    /// Die erste abgewiesene Aktion dieses Fensters
    FirstDenied,
    /// Eine weitere abgewiesene Aktion dieses Fensters
    Denied,
}

/// Zählt die Aktionen je Chat in einem Fenster von [`RATE_WINDOW_SECS`] Sekunden.
struct RateLimits {
    clock: Clock,
    chats: Mutex<HashMap<i64, Window>>,
}

/// Das laufende Fenster eines Chats
struct Window {
    /// Beginn des Fensters
    started_at: DateTime<Utc>,
    /// Zahl der Aktionen in diesem Fenster
    count: usize,
    /// Wurde in diesem Fenster schon auf die Grenze hingewiesen?
    warned: bool,
}

impl RateLimits {
    /// Grenzen mit dieser Uhr
    fn new(clock: Clock) -> Self {
        Self {
            clock,
            chats: Mutex::new(HashMap::new()),
        }
    }

    /// Nimmt eine Aktion für `chat_id` ab
    fn take(&self, chat_id: i64) -> Verdict {
        let now = self.clock.now();
        let mut chats = self.chats.lock().expect("Sperre der Rate-Grenzen");
        let window = chats.entry(chat_id).or_insert_with(|| Window {
            started_at: now,
            count: 0,
            warned: false,
        });
        if now - window.started_at >= TimeDelta::seconds(RATE_WINDOW_SECS) {
            *window = Window {
                started_at: now,
                count: 0,
                warned: false,
            };
        }
        if window.count < RATE_LIMIT {
            window.count += 1;
            return Verdict::Allowed;
        }
        match std::mem::replace(&mut window.warned, true) {
            false => Verdict::FirstDenied,
            true => Verdict::Denied,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::telegram::fake::FakeTelegram;

    /// Kontext auf einer Attrappe, deren Aufrufe der Test prüft
    fn context(fake: Arc<FakeTelegram>) -> BotContext {
        BotContext {
            api: fake,
            chat_id: 42,
            message_id: Some(1),
            bot_name: Some("nexus_test_bot".to_string()),
        }
    }

    /// Eine Textnachricht aus dem Chat 42
    fn message(text: &str) -> Message {
        serde_json::from_value(json!({
            "message_id": 1,
            "chat": { "id": 42, "type": "private" },
            "text": text,
        }))
        .expect("Nachricht")
    }

    /// Ein Knopfdruck mit diesen Callback-Daten
    fn query(data: &str) -> CallbackQuery {
        serde_json::from_value(json!({
            "id": "cb-1",
            "from": { "id": 7, "first_name": "Ben" },
            "message": { "message_id": 1, "chat": { "id": 42, "type": "private" } },
            "data": data,
        }))
        .expect("Knopfdruck")
    }

    /// Die Texte aller geschickten Nachrichten
    fn sent(fake: &FakeTelegram) -> Vec<String> {
        fake.calls_of("sendMessage")
            .into_iter()
            .map(|call| call.params["text"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// Ein Befehl, der mitschreibt, womit er gerufen wurde
    struct Recording(Mutex<Vec<String>>);

    #[async_trait]
    impl CommandHandler for Recording {
        async fn run(&self, ctx: &BotContext, args: &str) -> Result<()> {
            self.0.lock().unwrap().push(args.to_string());
            ctx.reply("erledigt").await
        }
    }

    #[test]
    fn zerlegt_einen_befehl_mit_botnamen_und_argumenten() {
        let command = parse_command("/board@nexus_test_bot NXS 7").unwrap();

        assert_eq!(command.name, "board");
        assert_eq!(command.mention.as_deref(), Some("nexus_test_bot"));
        assert_eq!(command.args, "NXS 7");
    }

    #[test]
    fn zerlegt_einen_befehl_ohne_argumente() {
        let command = parse_command("  /Hilfe  ").unwrap();

        assert_eq!(command.name, "hilfe", "der Name wird klein geschrieben");
        assert_eq!(command.mention, None);
        assert_eq!(command.args, "");
    }

    #[test]
    fn erkennt_text_ohne_befehl_nicht_als_befehl() {
        assert!(parse_command("kein Befehl").is_none());
        assert!(parse_command("").is_none());
        assert!(parse_command("/").is_none());
        assert!(parse_command("/ start").is_none());
    }

    #[tokio::test]
    async fn antwortet_auf_hilfe_mit_den_angemeldeten_befehlen() {
        let fake = Arc::new(FakeTelegram::new());
        let mut router = UpdateRouter::new();
        router.register_command(
            "status",
            "Was der Dispatcher tut",
            Arc::new(Recording(Mutex::new(Vec::new()))),
        );

        router
            .handle_message(&context(fake.clone()), &message("/hilfe"))
            .await
            .unwrap();

        let texts = sent(&fake);
        assert_eq!(texts.len(), 1);
        assert!(texts[0].contains("/hilfe – Diese Liste"), "{}", texts[0]);
        assert!(
            texts[0].contains("/status – Was der Dispatcher tut"),
            "{}",
            texts[0]
        );
    }

    #[tokio::test]
    async fn verweist_bei_einem_unbekannten_befehl_auf_hilfe() {
        let fake = Arc::new(FakeTelegram::new());
        let router = UpdateRouter::new();

        router
            .handle_message(&context(fake.clone()), &message("/gibtesnicht"))
            .await
            .unwrap();

        assert_eq!(sent(&fake), vec![UNKNOWN_COMMAND.to_string()]);
    }

    #[tokio::test]
    async fn ruft_einen_angemeldeten_befehl_mit_seinen_argumenten() {
        let fake = Arc::new(FakeTelegram::new());
        let handler = Arc::new(Recording(Mutex::new(Vec::new())));
        let mut router = UpdateRouter::new();
        router.register_command("/Board", "Board wählen", handler.clone());

        router
            .handle_message(&context(fake.clone()), &message("/board NXS"))
            .await
            .unwrap();

        assert_eq!(handler.0.lock().unwrap().clone(), vec!["NXS".to_string()]);
        assert_eq!(sent(&fake), vec!["erledigt".to_string()]);
    }

    #[tokio::test]
    async fn lasst_einen_befehl_an_einen_anderen_bot_liegen() {
        let fake = Arc::new(FakeTelegram::new());
        let router = UpdateRouter::new();

        router
            .handle_message(&context(fake.clone()), &message("/hilfe@anderer_bot"))
            .await
            .unwrap();

        assert!(sent(&fake).is_empty(), "{:?}", sent(&fake));
    }

    #[tokio::test]
    async fn antwortet_auf_freien_text_mit_dem_hinweis() {
        let fake = Arc::new(FakeTelegram::new());
        let router = UpdateRouter::new();

        router
            .handle_message(&context(fake.clone()), &message("mach mal was"))
            .await
            .unwrap();

        assert_eq!(sent(&fake), vec![PLAIN_TEXT_HINT.to_string()]);
    }

    #[tokio::test]
    async fn gibt_freien_text_an_einen_angemeldeten_handler() {
        /// Nimmt jede Nachricht an
        struct Annehmend;
        #[async_trait]
        impl MessageHandler for Annehmend {
            async fn run(&self, ctx: &BotContext, _message: &Message) -> Result<bool> {
                ctx.reply("verstanden").await?;
                Ok(true)
            }
        }

        let fake = Arc::new(FakeTelegram::new());
        let mut router = UpdateRouter::new();
        router.register_message(Arc::new(Annehmend));

        router
            .handle_message(&context(fake.clone()), &message("eine Idee"))
            .await
            .unwrap();

        assert_eq!(sent(&fake), vec!["verstanden".to_string()]);
    }

    #[tokio::test]
    async fn meldet_einen_veralteten_knopf() {
        let fake = Arc::new(FakeTelegram::new());
        let router = UpdateRouter::new();

        router
            .handle_callback(&context(fake.clone()), &query("quatsch:1:2:3"))
            .await
            .unwrap();

        let calls = fake.calls_of("answerCallbackQuery");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].params["text"], STALE_BUTTON);
        assert_eq!(calls[0].params["show_alert"], true);
    }

    #[tokio::test]
    async fn gibt_einen_knopf_an_den_zustandigen_handler() {
        /// Nimmt nur `Cancel` an
        struct NurAbbruch(Mutex<Vec<CallbackAction>>);
        #[async_trait]
        impl CallbackHandler for NurAbbruch {
            async fn run(
                &self,
                ctx: &BotContext,
                action: &CallbackAction,
                query_id: &str,
            ) -> Result<bool> {
                if !matches!(action, CallbackAction::Cancel) {
                    return Ok(false);
                }
                self.0.lock().unwrap().push(action.clone());
                ctx.api
                    .answer_callback_query(query_id, None, false)
                    .await
                    .map_err(nexus_core::Error::unexpected)?;
                Ok(true)
            }
        }

        let fake = Arc::new(FakeTelegram::new());
        let handler = Arc::new(NurAbbruch(Mutex::new(Vec::new())));
        let mut router = UpdateRouter::new();
        router.register_callback(handler.clone());

        router
            .handle_callback(&context(fake.clone()), &query("x"))
            .await
            .unwrap();
        router
            .handle_callback(&context(fake.clone()), &query("brd:7"))
            .await
            .unwrap();

        assert_eq!(
            handler.0.lock().unwrap().clone(),
            vec![CallbackAction::Cancel]
        );
        let calls = fake.calls_of("answerCallbackQuery");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].params["text"], Value::Null);
        // Für `brd:7` ist niemand zuständig; der Knopf gilt als veraltet
        assert_eq!(calls[1].params["text"], STALE_BUTTON);
    }

    #[tokio::test]
    async fn lasst_je_chat_dreissig_aktionen_in_der_minute_zu() {
        let fake = Arc::new(FakeTelegram::new());
        let router =
            UpdateRouter::with_clock(Clock::fixed("2026-11-03T12:00:00Z".parse().unwrap()));
        let ctx = context(fake.clone());

        for _ in 0..RATE_LIMIT + 3 {
            router
                .handle_message(&ctx, &message("/hilfe"))
                .await
                .unwrap();
        }

        let texts = sent(&fake);
        assert_eq!(
            texts.len(),
            RATE_LIMIT + 1,
            "30 Antworten und genau ein Hinweis auf die Grenze"
        );
        assert_eq!(texts[RATE_LIMIT], RATE_HINT);
    }

    #[tokio::test]
    async fn zahlt_die_grenze_je_chat_getrennt() {
        let fake = Arc::new(FakeTelegram::new());
        let router =
            UpdateRouter::with_clock(Clock::fixed("2026-11-03T12:00:00Z".parse().unwrap()));
        let erster = context(fake.clone());
        let zweiter = BotContext {
            chat_id: 99,
            ..context(fake.clone())
        };

        for _ in 0..RATE_LIMIT {
            router
                .handle_message(&erster, &message("/hilfe"))
                .await
                .unwrap();
        }
        fake.take_calls();
        router
            .handle_message(&zweiter, &message("/hilfe"))
            .await
            .unwrap();

        assert_eq!(sent(&fake).len(), 1, "der andere Chat ist nicht gebremst");
    }

    #[tokio::test]
    async fn lasst_nach_einer_minute_wieder_aktionen_zu() {
        let minuten = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let gelesen = minuten.clone();
        let start: DateTime<Utc> = "2026-11-03T12:00:00Z".parse().unwrap();
        let fake = Arc::new(FakeTelegram::new());
        let router = UpdateRouter::with_clock(Clock::from_fn(move || {
            start + TimeDelta::minutes(gelesen.load(std::sync::atomic::Ordering::Relaxed))
        }));
        let ctx = context(fake.clone());
        for _ in 0..RATE_LIMIT + 1 {
            router
                .handle_message(&ctx, &message("/hilfe"))
                .await
                .unwrap();
        }
        fake.take_calls();

        minuten.store(1, std::sync::atomic::Ordering::Relaxed);
        router
            .handle_message(&ctx, &message("/hilfe"))
            .await
            .unwrap();

        assert_eq!(sent(&fake).len(), 1);
        assert_ne!(sent(&fake)[0], RATE_HINT);
    }

    #[tokio::test]
    async fn weist_einen_gebremsten_knopf_mit_einem_hinweisfenster_ab() {
        let fake = Arc::new(FakeTelegram::new());
        let router =
            UpdateRouter::with_clock(Clock::fixed("2026-11-03T12:00:00Z".parse().unwrap()));
        let ctx = context(fake.clone());
        for _ in 0..RATE_LIMIT + 1 {
            router
                .handle_message(&ctx, &message("/hilfe"))
                .await
                .unwrap();
        }
        fake.take_calls();

        router.handle_callback(&ctx, &query("x")).await.unwrap();

        let calls = fake.calls_of("answerCallbackQuery");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].params["text"], RATE_HINT);
    }

    #[tokio::test]
    async fn teilt_eine_zu_lange_antwort_in_mehrere_nachrichten() {
        let fake = Arc::new(FakeTelegram::new());
        let ctx = context(fake.clone());
        let lang = (0..200)
            .map(|index| format!("Zeile {index} mit genug Text, damit es über die Grenze geht"))
            .collect::<Vec<_>>()
            .join("\n");

        ctx.reply(&lang).await.unwrap();

        let texts = sent(&fake);
        assert!(texts.len() > 1, "{} Nachrichten", texts.len());
        assert!(texts
            .iter()
            .all(|text| nexus_core::text::utf16_len(text) <= MAX_MESSAGE_LEN));
    }

    #[tokio::test]
    async fn hangt_die_knopfe_unter_das_letzte_stuck() {
        use crate::telegram::api::InlineButton;

        let fake = Arc::new(FakeTelegram::new());
        let ctx = context(fake.clone());
        let lang = (0..200)
            .map(|index| format!("Zeile {index} mit genug Text, damit es über die Grenze geht"))
            .collect::<Vec<_>>()
            .join("\n");

        ctx.reply_with(
            &lang,
            InlineKeyboard::row(vec![InlineButton::callback("Abbruch", "x")]),
        )
        .await
        .unwrap();

        let calls = fake.calls_of("sendMessage");
        assert_eq!(calls[0].params["reply_markup"], Value::Null);
        assert_ne!(
            calls[calls.len() - 1].params["reply_markup"],
            Value::Null,
            "die Knöpfe hängen am Ende"
        );
    }

    #[test]
    fn nennt_die_befehle_fur_setmycommands_mit_den_eingebauten_vorn() {
        let mut router = UpdateRouter::new();
        router.register_command(
            "status",
            "Was der Dispatcher tut",
            Arc::new(Recording(Mutex::new(Vec::new()))),
        );

        let list = router.command_list();

        assert_eq!(list[0].0, "start");
        assert_eq!(list[1].0, "hilfe");
        assert_eq!(
            list[2],
            ("status".to_string(), "Was der Dispatcher tut".to_string())
        );
    }

    #[test]
    fn ersetzt_einen_zweimal_angemeldeten_befehl() {
        let mut router = UpdateRouter::new();
        let handler = Arc::new(Recording(Mutex::new(Vec::new())));
        router.register_command("status", "alt", handler.clone());
        router.register_command("status", "neu", handler);

        let list = router.command_list();

        assert_eq!(list.len(), 3);
        assert_eq!(list[2].1, "neu");
    }
}
