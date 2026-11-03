//! Einstellung des Telegram-Bots (`~/.nexus/telegram.yaml`), vollständig aufgelöst.
//!
//! Die Datei liegt neben den übrigen Stores, aber nicht in `settings.json`, weil sie
//! Geheimnisse (Bot-Token, API-Schlüssel) und Zustand (gekoppelte Chats, gesetztes Profil)
//! enthält. Fehlende Felder gelten als Vorgabe, unbekannte Felder werden überlesen.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::error::{Error, HarnessError, Result};

/// Vorgabe für die Adresse der Bot-API
pub const DEFAULT_API_BASE: &str = "https://api.telegram.org";

/// Vorgabe für die Adresse der Spracherkennung
pub const DEFAULT_SPEECH_BASE_URL: &str = "https://api.openai.com";

/// Vorgabe für das Modell der Spracherkennung
pub const DEFAULT_SPEECH_MODEL: &str = "gpt-4o-mini-transcribe";

/// Vorgabe für die Sprache der Spracherkennung
pub const DEFAULT_SPEECH_LANGUAGE: &str = "de";

/// Bot-Token von @BotFather: Zahl, Doppelpunkt, mindestens 30 Zeichen des Geheimnisses
static BOT_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+:[A-Za-z0-9_-]{30,}$").expect("Muster des Bot-Tokens"));

/// Uhrzeit HH:MM (24 Stunden)
static HHMM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([01]\d|2[0-3]):[0-5]\d$").expect("Muster der Uhrzeit"));

/// Zeichen, das in maskierten Geheimnissen für den weggelassenen Teil steht
const ELLIPSIS: &str = "…";

/// Nachtruhe: in diesem Fenster werden keine Meldungen geschickt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct QuietHours {
    /// HH:MM Ortszeit, einschließlich
    pub from: String,
    /// HH:MM Ortszeit, ausschließlich; liegt `to` vor `from`, geht das Fenster über Mitternacht
    pub to: String,
}

/// Anlässe, zu denen der Bot von sich aus schreibt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct TelegramNotify {
    /// Ein Lauf ist fertig und wartet auf Review
    pub review: bool,
    /// Ein Ticket ist blockiert
    pub blocked: bool,
    /// Eine Frage wartet auf eine Entscheidung
    pub question: bool,
    /// Ein Ticket ist bereit zum Mergen
    pub merge: bool,
    /// Ein Merge ist durch
    pub merged: bool,
    /// Der Dispatcher pausiert
    pub paused: bool,
}

impl Default for TelegramNotify {
    fn default() -> Self {
        Self {
            review: true,
            blocked: true,
            question: true,
            merge: true,
            merged: false,
            paused: false,
        }
    }
}

/// Was Nexus am Bot-Profil zuletzt eingerichtet hat; schreibt nur der Server
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct AppliedProfile {
    /// Bot-ID aus `getMe`, für die das Profil gilt
    pub bot_id: i64,
    /// Fingerabdruck des hochgeladenen Profilbilds
    pub photo: String,
    /// Fingerabdruck der gesetzten Texte
    pub texts: String,
}

/// Profil des Bots (Bild, Name, Beschreibungen)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct BotProfile {
    /// true = Nexus richtet Profilbild, Name und Beschreibung des Bots ein
    pub brand: bool,
    /// Was zuletzt eingerichtet wurde; null = noch nichts
    pub applied: Option<AppliedProfile>,
}

impl Default for BotProfile {
    fn default() -> Self {
        Self {
            brand: true,
            applied: None,
        }
    }
}

/// Wohin eine per Bot erfasste Idee standardmäßig geht
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "telegram.ts")]
pub enum IdeaTarget {
    /// Notiz auf dem Board
    Note,
    /// Ticket auf dem Board
    Ticket,
}

impl IdeaTarget {
    /// Schreibweise in `telegram.yaml`
    pub fn as_str(&self) -> &'static str {
        match self {
            IdeaTarget::Note => "note",
            IdeaTarget::Ticket => "ticket",
        }
    }
}

/// Kleinste und größte erlaubte Zahl der Rückfragerunden einer Idee
pub const MAX_ROUNDS_RANGE: (i64, i64) = (1, 10);

/// Erfassen von Ideen über den Bot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct IdeaSettings {
    /// Vorgabe für das Ziel einer Idee
    pub default_target: IdeaTarget,
    /// Höchstzahl der Rückfragerunden (1–10)
    pub max_rounds: i64,
}

impl Default for IdeaSettings {
    fn default() -> Self {
        Self {
            default_target: IdeaTarget::Note,
            max_rounds: 5,
        }
    }
}

/// Weg, über den Sprachnachrichten in Text verwandelt werden
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "telegram.ts")]
pub enum SpeechBackend {
    /// Dienst mit der Schnittstelle von OpenAI (OpenAI selbst, Speaches, whisper.cpp-Server …)
    OpenaiCompatible,
    /// Lokales `whisper-cli` aus whisper.cpp
    WhisperCli,
    /// Keine Spracherkennung
    Off,
}

impl SpeechBackend {
    /// Schreibweise in `telegram.yaml`
    pub fn as_str(&self) -> &'static str {
        match self {
            SpeechBackend::OpenaiCompatible => "openai-compatible",
            SpeechBackend::WhisperCli => "whisper-cli",
            SpeechBackend::Off => "off",
        }
    }
}

/// Aufruf des lokalen `whisper-cli`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct WhisperCli {
    /// Aufruf von whisper-cli – Name im Pfad oder voller Pfad
    pub binary: String,
    /// Pfad der Modelldatei (`ggml-*.bin`); leer = nicht eingerichtet
    pub model: String,
    /// Aufruf von ffmpeg für die Umwandlung in WAV
    pub ffmpeg: String,
}

impl Default for WhisperCli {
    fn default() -> Self {
        Self {
            binary: "whisper-cli".to_string(),
            model: String::new(),
            ffmpeg: "ffmpeg".to_string(),
        }
    }
}

/// Spracherkennung für Sprachnachrichten
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct SpeechConfig {
    pub backend: SpeechBackend,
    /// Adresse des Dienstes mit der OpenAI-Schnittstelle (ohne Schrägstrich am Ende)
    pub base_url: String,
    /// Schlüssel des Dienstes; leer = ohne Anmeldung (lokale Dienste)
    pub api_key: String,
    pub model: String,
    /// Sprache der Aufnahme als Kürzel; null = der Dienst erkennt sie selbst
    pub language: Option<String>,
    pub whisper_cli: WhisperCli,
}

impl Default for SpeechConfig {
    fn default() -> Self {
        Self {
            backend: SpeechBackend::OpenaiCompatible,
            base_url: DEFAULT_SPEECH_BASE_URL.to_string(),
            api_key: String::new(),
            model: DEFAULT_SPEECH_MODEL.to_string(),
            language: Some(DEFAULT_SPEECH_LANGUAGE.to_string()),
            whisper_cli: WhisperCli::default(),
        }
    }
}

/// Einstellung des Telegram-Bots, vollständig aufgelöst
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct TelegramConfig {
    /// true = der Bot pollt und schickt Meldungen
    pub enabled: bool,
    /// Bot-Token von @BotFather; leer = noch keiner eingetragen
    pub bot_token: String,
    /// Adresse der Bot-API (ohne Schrägstrich am Ende); abweichend für einen selbst
    /// betriebenen `telegram-bot-api`-Server und für die Attrappe im Vertragstest
    pub api_base: String,
    /// Gekoppelte Chat-IDs; schreibt nur die Kopplung, nie die Oberfläche
    pub chats: Vec<i64>,
    pub notify: TelegramNotify,
    /// Nachtruhe; null = jederzeit melden
    pub quiet_hours: Option<QuietHours>,
    pub profile: BotProfile,
    pub ideas: IdeaSettings,
    pub speech: SpeechConfig,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bot_token: String::new(),
            api_base: DEFAULT_API_BASE.to_string(),
            chats: Vec::new(),
            notify: TelegramNotify::default(),
            quiet_hours: None,
            profile: BotProfile::default(),
            ideas: IdeaSettings::default(),
            speech: SpeechConfig::default(),
        }
    }
}

impl TelegramConfig {
    /// Liest die Einstellung aus einem rohen Wert (YAML oder JSON): fehlende Felder gelten als
    /// Vorgabe, unbekannte Felder werden überlesen. Prüft danach alles Weitere.
    pub fn from_value(raw: &Value) -> Result<Self> {
        // Eine leere Datei ergibt null; das zählt wie eine leere Einstellung
        let empty = Value::Object(Default::default());
        let raw = if raw.is_null() { &empty } else { raw };
        if !raw.is_object() {
            return Err(fail("Die Einstellung muss ein Objekt sein."));
        }
        let config: TelegramConfig = serde_json::from_value(raw.clone())
            .map_err(|error| fail(format!("Die Einstellung passt nicht: {error}")))?;
        config.normalized()
    }

    /// Räumt Adressen auf und prüft die Felder; Fehler nennen das Feld
    pub fn normalized(mut self) -> Result<Self> {
        self.bot_token = self.bot_token.trim().to_string();
        if !self.bot_token.is_empty() && !BOT_TOKEN.is_match(&self.bot_token) {
            // Das Token selbst darf nicht in die Meldung
            return Err(fail(
                "botToken sieht nicht wie ein Bot-Token von @BotFather aus (Zahl, Doppelpunkt, Geheimnis).",
            ));
        }
        self.api_base = http_url(&self.api_base, "apiBase", DEFAULT_API_BASE)?;
        if let Some(quiet_hours) = &self.quiet_hours {
            if !HHMM.is_match(&quiet_hours.from) || !HHMM.is_match(&quiet_hours.to) {
                return Err(fail(
                    "quietHours braucht from und to im Format HH:MM, z. B. from: \"22:00\", to: \"07:00\".",
                ));
            }
        }
        let (min, max) = MAX_ROUNDS_RANGE;
        if self.ideas.max_rounds < min || self.ideas.max_rounds > max {
            return Err(fail(format!(
                "ideas.maxRounds muss eine ganze Zahl von {min} bis {max} sein."
            )));
        }
        self.speech.base_url = http_url(
            &self.speech.base_url,
            "speech.baseUrl",
            DEFAULT_SPEECH_BASE_URL,
        )?;
        self.speech.api_key = self.speech.api_key.trim().to_string();
        self.speech.language = self
            .speech
            .language
            .as_deref()
            .map(str::trim)
            .filter(|language| !language.is_empty())
            .map(ToString::to_string);
        Ok(self)
    }

    /// Kopie für die Oberfläche: Bot-Token und API-Schlüssel nur noch als Maske
    pub fn masked(&self) -> Self {
        let mut masked = self.clone();
        masked.bot_token = mask_bot_token(&self.bot_token);
        masked.speech.api_key = mask_api_key(&self.speech.api_key);
        masked
    }

    /// Übernimmt eine Eingabe der Oberfläche auf die gespeicherte Einstellung.
    ///
    /// Geheimnisse behalten ihren Wert, wenn sie fehlen oder der Maske aus [`Self::masked`]
    /// entsprechen; `""` löscht sie. `chats` und `profile.applied` kommen nie aus der
    /// Oberfläche und bleiben, wie sie sind.
    pub fn merge_update(&self, input: &Value) -> Result<Self> {
        let mut raw = input.clone();
        let Some(object) = raw.as_object_mut() else {
            return Err(fail("Die Einstellung muss ein Objekt sein."));
        };
        let bot_token = merge_secret(object.get("botToken"), &self.bot_token, mask_bot_token);
        object.insert("botToken".to_string(), Value::String(bot_token));
        // Die Oberfläche darf die Kopplung und das eingerichtete Profil nicht überschreiben
        object.insert(
            "chats".to_string(),
            serde_json::to_value(&self.chats).map_err(Error::unexpected)?,
        );
        let speech = object
            .entry("speech")
            .or_insert_with(|| Value::Object(Default::default()));
        if let Some(speech) = speech.as_object_mut() {
            let api_key = merge_secret(speech.get("apiKey"), &self.speech.api_key, mask_api_key);
            speech.insert("apiKey".to_string(), Value::String(api_key));
        }
        let mut merged = TelegramConfig::from_value(&raw)?;
        merged.profile.applied = self.profile.applied.clone();
        Ok(merged)
    }
}

/// Neuer Wert eines Geheimnisses: fehlend oder maskiert behält den gespeicherten Wert
fn merge_secret(input: Option<&Value>, stored: &str, mask: fn(&str) -> String) -> String {
    let Some(given) = input.and_then(Value::as_str) else {
        return stored.to_string();
    };
    let given = given.trim();
    if given.is_empty() {
        return String::new();
    }
    if given == mask(stored) {
        return stored.to_string();
    }
    given.to_string()
}

/// Maske des Bot-Tokens: `123456:…abcd` – die Bot-ID bleibt lesbar, das Geheimnis nicht
pub fn mask_bot_token(token: &str) -> String {
    if token.is_empty() {
        return String::new();
    }
    match token.split_once(':') {
        Some((id, secret)) if !id.is_empty() && secret.chars().count() >= 4 => {
            format!("{id}:{ELLIPSIS}{}", last(secret, 4))
        }
        _ => ELLIPSIS.to_string(),
    }
}

/// Maske des API-Schlüssels: erste 3 und letzte 4 Zeichen (`sk-…abcd`); kurze Werte ganz verdeckt
pub fn mask_api_key(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    if key.chars().count() < 8 {
        return ELLIPSIS.to_string();
    }
    let first: String = key.chars().take(3).collect();
    format!("{first}{ELLIPSIS}{}", last(key, 4))
}

/// Die letzten `count` Zeichen
fn last(text: &str, count: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    chars[chars.len() - count..].iter().collect()
}

/// http(s)-Adresse ohne Schrägstrich am Ende; leer gilt als Vorgabe
fn http_url(value: &str, name: &str, default: &str) -> Result<String> {
    let value = value.trim().trim_end_matches('/');
    if value.is_empty() {
        return Ok(default.to_string());
    }
    if !value.starts_with("http://") && !value.starts_with("https://") {
        return Err(fail(format!(
            "{name} muss eine Adresse mit http:// oder https:// sein."
        )));
    }
    Ok(value.to_string())
}

/// Prüffehler der Einstellung
fn fail(message: impl Into<String>) -> Error {
    HarnessError::new("telegram-config-invalid", message).into()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Ein Token, das dem Muster von @BotFather entspricht
    const TOKEN: &str = "123456:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw";

    /// Erwartet einen Prüffehler, in dessen Meldung `text` steht
    fn rejects(raw: Value, text: &str) {
        let error = TelegramConfig::from_value(&raw).expect_err("kein Fehler");
        assert_eq!(error.code(), Some("telegram-config-invalid"), "{raw}");
        assert!(
            error.to_string().contains(text),
            "„{text}“ fehlt in „{error}“"
        );
    }

    #[test]
    fn leere_einstellung_ergibt_die_vorgaben() {
        let config = TelegramConfig::from_value(&json!({})).unwrap();
        assert_eq!(config, TelegramConfig::default());
        assert!(!config.enabled);
        assert_eq!(config.api_base, "https://api.telegram.org");
        assert!(config.chats.is_empty());
        assert!(config.notify.review);
        assert!(!config.notify.merged);
        assert_eq!(config.quiet_hours, None);
        assert!(config.profile.brand);
        assert_eq!(config.ideas.default_target, IdeaTarget::Note);
        assert_eq!(config.ideas.max_rounds, 5);
        assert_eq!(config.speech.backend, SpeechBackend::OpenaiCompatible);
        assert_eq!(config.speech.model, "gpt-4o-mini-transcribe");
        assert_eq!(config.speech.language.as_deref(), Some("de"));
        assert_eq!(config.speech.whisper_cli.binary, "whisper-cli");
    }

    #[test]
    fn null_gilt_wie_eine_leere_einstellung() {
        assert_eq!(
            TelegramConfig::from_value(&Value::Null).unwrap(),
            TelegramConfig::default()
        );
    }

    #[test]
    fn unbekannte_felder_werden_uberlesen() {
        let config =
            TelegramConfig::from_value(&json!({ "enabled": true, "vonMorgen": 7 })).unwrap();
        assert!(config.enabled);
    }

    #[test]
    fn entfernt_den_schragstrich_am_ende_der_adressen() {
        let config = TelegramConfig::from_value(&json!({
            "apiBase": "http://localhost:8081/",
            "speech": { "baseUrl": "http://localhost:8000/v1/" },
        }))
        .unwrap();
        assert_eq!(config.api_base, "http://localhost:8081");
        assert_eq!(config.speech.base_url, "http://localhost:8000/v1");
    }

    #[test]
    fn leere_adressen_fallen_auf_die_vorgabe_zuruck() {
        let config =
            TelegramConfig::from_value(&json!({ "apiBase": "", "speech": { "baseUrl": "" } }))
                .unwrap();
        assert_eq!(config.api_base, DEFAULT_API_BASE);
        assert_eq!(config.speech.base_url, DEFAULT_SPEECH_BASE_URL);
    }

    #[test]
    fn nimmt_ein_gultiges_token_und_leer() {
        assert_eq!(
            TelegramConfig::from_value(&json!({ "botToken": format!("  {TOKEN}  ") }))
                .unwrap()
                .bot_token,
            TOKEN
        );
        assert_eq!(
            TelegramConfig::from_value(&json!({ "botToken": "" }))
                .unwrap()
                .bot_token,
            ""
        );
    }

    #[test]
    fn lehnt_ein_token_ohne_muster_ab_und_nennt_es_nicht() {
        let error = TelegramConfig::from_value(&json!({ "botToken": "abc:kurz" })).unwrap_err();
        assert_eq!(error.code(), Some("telegram-config-invalid"));
        let message = error.to_string();
        assert!(message.contains("botToken"), "{message}");
        assert!(!message.contains("abc:kurz"), "{message}");
    }

    #[test]
    fn lehnt_eine_ungultige_uhrzeit_ab() {
        rejects(
            json!({ "quietHours": { "from": "22:00", "to": "7 Uhr" } }),
            "quietHours",
        );
        rejects(
            json!({ "quietHours": { "from": "24:00", "to": "07:00" } }),
            "HH:MM",
        );
    }

    #[test]
    fn nimmt_eine_nachtruhe_uber_mitternacht() {
        let config = TelegramConfig::from_value(
            &json!({ "quietHours": { "from": "22:00", "to": "07:00" } }),
        )
        .unwrap();
        assert_eq!(
            config.quiet_hours,
            Some(QuietHours {
                from: "22:00".into(),
                to: "07:00".into()
            })
        );
    }

    #[test]
    fn lehnt_runden_ausserhalb_von_eins_bis_zehn_ab() {
        rejects(json!({ "ideas": { "maxRounds": 0 } }), "ideas.maxRounds");
        rejects(json!({ "ideas": { "maxRounds": 11 } }), "von 1 bis 10");
    }

    #[test]
    fn lehnt_adressen_ohne_http_ab() {
        rejects(json!({ "apiBase": "ftp://x" }), "apiBase");
        rejects(
            json!({ "speech": { "baseUrl": "localhost:8000" } }),
            "speech.baseUrl",
        );
    }

    #[test]
    fn maskiert_token_und_schlussel() {
        let config = TelegramConfig {
            bot_token: TOKEN.to_string(),
            speech: SpeechConfig {
                api_key: "sk-geheim-1234abcd".to_string(),
                ..SpeechConfig::default()
            },
            ..TelegramConfig::default()
        };

        let masked = config.masked();

        assert_eq!(masked.bot_token, "123456:…Dsaw");
        assert_eq!(masked.speech.api_key, "sk-…abcd");
        // Der Rest bleibt unverändert
        assert_eq!(masked.api_base, config.api_base);
    }

    #[test]
    fn maskiert_leere_und_kurze_werte() {
        assert_eq!(mask_bot_token(""), "");
        assert_eq!(mask_bot_token("kaputt"), "…");
        assert_eq!(mask_api_key(""), "");
        assert_eq!(mask_api_key("sk-123"), "…");
    }

    /// Gespeicherte Einstellung mit beiden Geheimnissen, Chats und gesetztem Profil
    fn stored() -> TelegramConfig {
        TelegramConfig {
            bot_token: TOKEN.to_string(),
            chats: vec![123, 456],
            profile: BotProfile {
                brand: true,
                applied: Some(AppliedProfile {
                    bot_id: 7012345678,
                    photo: "3f2a".into(),
                    texts: "9c1e".into(),
                }),
            },
            speech: SpeechConfig {
                api_key: "sk-geheim-1234abcd".to_string(),
                ..SpeechConfig::default()
            },
            ..TelegramConfig::default()
        }
    }

    #[test]
    fn behalt_fehlende_geheimnisse() {
        let merged = stored().merge_update(&json!({ "enabled": true })).unwrap();
        assert!(merged.enabled);
        assert_eq!(merged.bot_token, TOKEN);
        assert_eq!(merged.speech.api_key, "sk-geheim-1234abcd");
    }

    #[test]
    fn behalt_maskierte_geheimnisse() {
        let stored = stored();
        let masked = stored.masked();
        let merged = stored
            .merge_update(&json!({
                "botToken": masked.bot_token,
                "speech": { "apiKey": masked.speech.api_key },
            }))
            .unwrap();
        assert_eq!(merged.bot_token, TOKEN);
        assert_eq!(merged.speech.api_key, "sk-geheim-1234abcd");
    }

    #[test]
    fn loscht_geheimnisse_mit_leerem_text() {
        let merged = stored()
            .merge_update(&json!({ "botToken": "", "speech": { "apiKey": "" } }))
            .unwrap();
        assert_eq!(merged.bot_token, "");
        assert_eq!(merged.speech.api_key, "");
    }

    #[test]
    fn ubernimmt_neue_geheimnisse() {
        let neu = "999:BBHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw";
        let merged = stored()
            .merge_update(&json!({ "botToken": neu, "speech": { "apiKey": "sk-neu-9999" } }))
            .unwrap();
        assert_eq!(merged.bot_token, neu);
        assert_eq!(merged.speech.api_key, "sk-neu-9999");
    }

    #[test]
    fn behalt_chats_und_eingerichtetes_profil() {
        let merged = stored()
            .merge_update(&json!({
                "chats": [999],
                "profile": { "brand": false, "applied": { "botId": 1, "photo": "x", "texts": "y" } },
            }))
            .unwrap();
        assert_eq!(merged.chats, vec![123, 456]);
        assert!(!merged.profile.brand);
        assert_eq!(merged.profile.applied.unwrap().bot_id, 7012345678);
    }

    #[test]
    fn pruft_die_eingabe_der_oberflache() {
        let error = stored()
            .merge_update(&json!({ "ideas": { "maxRounds": 99 } }))
            .unwrap_err();
        assert_eq!(error.code(), Some("telegram-config-invalid"));
    }
}
