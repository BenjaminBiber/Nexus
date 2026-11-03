//! Sprach- und Audionachrichten: herunterladen, erkennen und als Eingabe weitergeben.
//!
//! Der Router legt jede Nachricht ohne Text hier vor. Ist eine Sprachnachricht (`voice`) oder
//! eine Audiodatei (`audio`) dabei, prüft [`VoiceMessages`] erst die angekündigte Größe und
//! Dauer, zeigt „Tippt…“, lädt die Datei über `getFile`/`download_file` und gibt sie der
//! eingerichteten Erkennung ([`crate::speech`]) samt Erkennungshilfe. Der erkannte Text geht
//! als Antwort „🎙️ Verstanden: …“ in den Chat zurück und läuft danach denselben Weg wie ein
//! getippter Text: erst in einen offenen Dialog, sonst an die Handler für freie Texte.
//!
//! Was schiefgeht – Erkennung aus, Dienst nicht erreichbar, Aufnahme zu groß oder zu lang –
//! wird eine kurze Antwort mit dem Hinweis, dass Tippen immer geht. Nie steht darin eine
//! Adresse oder ein Schlüssel; die Fehlertexte der Erkennung sind darauf schon angelegt.

use std::sync::Arc;

use async_trait::async_trait;
use nexus_core::telegram::{escape, TelegramConfigStore};
use nexus_core::Result;

use crate::speech::{self, AudioClip, SpeechHint, SpeechToText};
use crate::telegram::api::{AudioFile, Message, TelegramApi};
use crate::telegram::commands::Deps;
use crate::telegram::router::{BotContext, VoiceHandler, VoiceOutcome};

/// Tätigkeit, die der Chat während des Ladens und Erkennens zeigt
pub const TYPING: &str = "typing";

/// Inhaltstyp, wenn die Nachricht keinen nennt: Telegram schickt Sprachnachrichten als
/// OGG/Opus
pub const DEFAULT_MIME: &str = "audio/ogg";

/// Antwort auf eine Aufnahme, in der die Erkennung nichts gefunden hat
pub const NOTHING_HEARD: &str =
    "🎙️ Da war nichts zu verstehen. Schick es noch einmal oder tippe es – Tippen geht immer.";

/// Hinweis am Ende jeder Antwort auf eine Sprachnachricht, die nicht geklappt hat
pub const TYPING_ALWAYS_WORKS: &str = "Tippen geht immer.";

/// Woher die eingerichtete Erkennung und ihre Erkennungshilfe kommen.
///
/// Als eigener Schritt, damit Tests eine Attrappe einsetzen können, ohne einen Dienst oder
/// den Serverzustand zu brauchen.
#[async_trait]
pub trait SpeechSource: Send + Sync {
    /// Die Erkennung samt Hilfe; `Err`, wenn schon die Einstellung nicht zu lesen ist
    async fn prepare(&self) -> Result<(Box<dyn SpeechToText>, SpeechHint)>;
}

/// Die Erkennung aus `telegram.yaml` samt Hilfe aus dem Board des Servers
pub(crate) struct ConfiguredSpeech(pub(crate) Deps);

#[async_trait]
impl SpeechSource for ConfiguredSpeech {
    async fn prepare(&self) -> Result<(Box<dyn SpeechToText>, SpeechHint)> {
        let state = self.0.state()?;
        let config = TelegramConfigStore::new(state.home.clone()).read()?;
        let hint = SpeechHint::for_state(&state, config.speech.language.clone()).await;
        Ok((speech::from_config(&config.speech), hint))
    }
}

/// Nimmt Sprach- und Audionachrichten an und macht daraus Text.
pub struct VoiceMessages {
    speech: Arc<dyn SpeechSource>,
}

impl std::fmt::Debug for VoiceMessages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoiceMessages").finish_non_exhaustive()
    }
}

impl VoiceMessages {
    /// Erkennung über diese Quelle
    pub fn new(speech: Arc<dyn SpeechSource>) -> Self {
        Self { speech }
    }

    /// Lädt die Aufnahme und erkennt sie
    async fn transcript(&self, api: &dyn TelegramApi, audio: &AudioFile) -> Result<String> {
        // Die angekündigten Maße zuerst: eine zu große Aufnahme wird gar nicht geladen
        speech::check_limits(
            audio.file_size.and_then(|size| usize::try_from(size).ok()),
            seconds_of(audio),
        )?;
        let (speech, hint) = self.speech.prepare().await?;
        let clip = download(api, audio).await?;
        clip.within_limits()?;
        Ok(speech.transcribe(clip, &hint).await?.text)
    }
}

#[async_trait]
impl VoiceHandler for VoiceMessages {
    async fn run(&self, ctx: &BotContext, message: &Message) -> Result<VoiceOutcome> {
        let Some(audio) = audio_of(message) else {
            return Ok(VoiceOutcome::NotVoice);
        };
        // Erkennen dauert; „Tippt…“ sagt, dass etwas läuft. Scheitert schon das, läuft der
        // Rest trotzdem weiter
        let _ = ctx.api.send_chat_action(ctx.chat_id, TYPING).await;
        match self.transcript(ctx.api.as_ref(), audio).await {
            Ok(text) if text.trim().is_empty() => {
                ctx.reply_to(NOTHING_HEARD, message.message_id).await?;
                Ok(VoiceOutcome::Answered)
            }
            Ok(text) => {
                let text = text.trim().to_string();
                ctx.reply_to(&understood(&text), message.message_id).await?;
                Ok(VoiceOutcome::Text(text))
            }
            Err(error) => {
                ctx.reply_to(&failed(&error), message.message_id).await?;
                Ok(VoiceOutcome::Answered)
            }
        }
    }
}

/// Die Aufnahme einer Nachricht: die Sprachnachricht hat Vorrang, sonst die Audiodatei
pub fn audio_of(message: &Message) -> Option<&AudioFile> {
    message.voice.as_ref().or(message.audio.as_ref())
}

/// Lädt die Datei zu einer Aufnahme herunter und macht daraus die Aufnahme für die Erkennung.
///
/// Der Inhaltstyp kommt aus der Nachricht, sonst gilt [`DEFAULT_MIME`]; der Dateiname fällt
/// daraus ab, weil der Dienst daran das Format erkennt.
pub async fn download(api: &dyn TelegramApi, audio: &AudioFile) -> Result<AudioClip> {
    let info = api
        .get_file(&audio.file_id)
        .await
        .map_err(nexus_core::Error::unexpected)?;
    let Some(path) = info.file_path else {
        return Err(nexus_core::HarnessError::new(
            "speech-failed",
            "Telegram gibt die Aufnahme nicht zum Herunterladen frei.",
        )
        .into());
    };
    // Die Größe aus `getFile` ist genauer als die der Nachricht und steht vor dem Download
    speech::check_limits(
        info.file_size.and_then(|size| usize::try_from(size).ok()),
        seconds_of(audio),
    )?;
    let bytes = api
        .download_file(&path)
        .await
        .map_err(nexus_core::Error::unexpected)?;
    let mime = audio
        .mime_type
        .as_deref()
        .map(str::trim)
        .filter(|mime| !mime.is_empty())
        .unwrap_or(DEFAULT_MIME);
    Ok(AudioClip::new(bytes, mime).with_seconds(seconds_of(audio)))
}

/// Die Länge der Aufnahme in Sekunden; `0` heißt bei Telegram „nicht bekannt“
fn seconds_of(audio: &AudioFile) -> Option<f64> {
    (audio.duration > 0).then_some(audio.duration as f64)
}

/// Die Antwort mit dem erkannten Text
pub fn understood(text: &str) -> String {
    format!("🎙️ Verstanden: „{}“", escape(text))
}

/// Die Antwort auf eine Aufnahme, die nicht zu Text wurde
pub fn failed(error: &nexus_core::Error) -> String {
    let reason = match error.harness() {
        Some(harness) => harness.message.clone(),
        None => error.to_string(),
    };
    format!(
        "🎙️ Das ging gerade nicht: {}\n{TYPING_ALWAYS_WORKS}",
        escape(&reason)
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use nexus_core::telegram::SpeechBackend;
    use nexus_core::HarnessError;
    use serde_json::json;

    use super::*;
    use crate::speech::Transcript;
    use crate::telegram::api::FileInfo;
    use crate::telegram::fake::FakeTelegram;
    use crate::telegram::router::{MessageHandler, UpdateRouter};

    /// Der Pfad, unter dem die Attrappe die Aufnahme liefert
    const FILE_PATH: &str = "voice/file_1.oga";

    /// Eine Erkennung, die immer dasselbe meldet, und die Aufnahmen mitschreibt
    struct FakeSpeech {
        reply: std::result::Result<&'static str, &'static str>,
        clips: Mutex<Vec<AudioClip>>,
        hints: Mutex<Vec<SpeechHint>>,
    }

    impl FakeSpeech {
        /// Erkennung, die diesen Text meldet
        fn says(text: &'static str) -> Arc<Self> {
            Arc::new(Self {
                reply: Ok(text),
                clips: Mutex::new(Vec::new()),
                hints: Mutex::new(Vec::new()),
            })
        }

        /// Erkennung, die mit diesem Grund scheitert
        fn fails(reason: &'static str) -> Arc<Self> {
            Arc::new(Self {
                reply: Err(reason),
                clips: Mutex::new(Vec::new()),
                hints: Mutex::new(Vec::new()),
            })
        }

        /// Die Aufnahmen, die sie bekommen hat
        fn clips(&self) -> Vec<AudioClip> {
            self.clips.lock().expect("Sperre").clone()
        }
    }

    /// Die Erkennung selbst; sie schreibt an der Attrappe mit, was ankam
    struct Fixed {
        source: Arc<FakeSpeech>,
    }

    #[async_trait]
    impl SpeechToText for Fixed {
        fn backend(&self) -> SpeechBackend {
            SpeechBackend::OpenaiCompatible
        }

        async fn transcribe(&self, clip: AudioClip, hint: &SpeechHint) -> Result<Transcript> {
            self.source.clips.lock().expect("Sperre").push(clip);
            self.source.hints.lock().expect("Sperre").push(hint.clone());
            match self.source.reply {
                Ok(text) => Ok(Transcript {
                    text: text.to_string(),
                    language: Some("de".to_string()),
                }),
                Err(reason) => Err(HarnessError::new("speech-failed", reason).into()),
            }
        }
    }

    /// Nimmt jede Nachricht an und schreibt ihren Text mit (steht für den Dialog und für die
    /// Handler freier Texte)
    struct Texts(Mutex<Vec<String>>);

    #[async_trait]
    impl MessageHandler for Texts {
        async fn run(&self, _ctx: &BotContext, message: &Message) -> Result<bool> {
            self.0
                .lock()
                .expect("Sperre")
                .push(message.text.clone().unwrap_or_default());
            Ok(true)
        }
    }

    /// Eine Nachricht mit einer Aufnahme unter diesem Feld (`voice` oder `audio`)
    fn voice_message(field: &str, duration: i64, file_size: Option<i64>) -> Message {
        serde_json::from_value(json!({
            "message_id": 5,
            "chat": { "id": 42, "type": "private" },
            field: {
                "file_id": "AwA",
                "duration": duration,
                "mime_type": "audio/ogg",
                "file_size": file_size,
            },
        }))
        .expect("Nachricht")
    }

    /// Attrappe, Router und der Mitschreiber der weitergegebenen Texte
    fn bot(speech: Arc<FakeSpeech>) -> (Arc<FakeTelegram>, UpdateRouter, Arc<Texts>) {
        let fake = Arc::new(FakeTelegram::new());
        fake.put_file_info(
            "AwA",
            FileInfo {
                file_path: Some(FILE_PATH.to_string()),
                file_size: Some(12),
            },
        );
        fake.put_file(FILE_PATH, b"OggS-Attrappe".to_vec());
        let texts = Arc::new(Texts(Mutex::new(Vec::new())));
        let mut router = UpdateRouter::new();
        router.set_voice(Arc::new(VoiceMessages::new(Arc::new(Quelle(speech)))));
        router.register_message(texts.clone());
        (fake, router, texts)
    }

    /// Die Quelle der Tests: sie gibt die Erkennung heraus, die an derselben Attrappe
    /// mitschreibt
    struct Quelle(Arc<FakeSpeech>);

    #[async_trait]
    impl SpeechSource for Quelle {
        async fn prepare(&self) -> Result<(Box<dyn SpeechToText>, SpeechHint)> {
            Ok((
                Box::new(Fixed {
                    source: self.0.clone(),
                }),
                SpeechHint::from_terms(Some("de".to_string()), ["NXS"]),
            ))
        }
    }

    /// Kontext auf der Attrappe
    fn context(fake: Arc<FakeTelegram>) -> BotContext {
        BotContext {
            api: fake,
            chat_id: 42,
            message_id: Some(5),
            bot_name: Some("nexus_test_bot".to_string()),
        }
    }

    /// Die Texte aller geschickten Nachrichten
    fn sent(fake: &FakeTelegram) -> Vec<String> {
        fake.calls_of("sendMessage")
            .into_iter()
            .map(|call| call.params["text"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    #[tokio::test]
    async fn ladt_eine_sprachnachricht_und_antwortet_mit_dem_erkannten_text() {
        let speech = FakeSpeech::says("Setz NXS-0042 auf blockiert");
        let (fake, router, _) = bot(speech.clone());

        router
            .handle_message(&context(fake.clone()), &voice_message("voice", 3, Some(12)))
            .await
            .unwrap();

        // Erst „Tippt…“, dann Datei holen und laden
        assert_eq!(fake.calls_of("sendChatAction")[0].params["action"], TYPING);
        assert_eq!(fake.calls_of("getFile").len(), 1);
        assert_eq!(
            fake.calls_of("downloadFile")[0].params["file_path"],
            FILE_PATH
        );
        // Die Aufnahme kommt mit Inhaltstyp und Länge bei der Erkennung an
        let clips = speech.clips();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].mime, "audio/ogg");
        assert_eq!(clips[0].seconds, Some(3.0));
        assert_eq!(clips[0].bytes, b"OggS-Attrappe".to_vec());
        // Die Antwort hängt an der Sprachnachricht
        let calls = fake.calls_of("sendMessage");
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].params["text"],
            "🎙️ Verstanden: „Setz NXS-0042 auf blockiert“"
        );
        assert_eq!(calls[0].params["reply_to_message_id"], 5);
    }

    #[tokio::test]
    async fn gibt_das_transkript_weiter_wie_einen_getippten_text() {
        let (fake, router, texts) = bot(FakeSpeech::says("  eine Idee  "));

        router
            .handle_message(&context(fake), &voice_message("voice", 3, Some(12)))
            .await
            .unwrap();

        assert_eq!(
            texts.0.lock().unwrap().clone(),
            vec!["eine Idee".to_string()],
            "beschnitten und ohne Umweg beim Handler"
        );
    }

    #[tokio::test]
    async fn erkennt_auch_eine_audiodatei() {
        let (fake, router, texts) = bot(FakeSpeech::says("aus einer Audiodatei"));

        router
            .handle_message(&context(fake.clone()), &voice_message("audio", 7, Some(99)))
            .await
            .unwrap();

        assert_eq!(fake.calls_of("downloadFile").len(), 1);
        assert_eq!(
            texts.0.lock().unwrap().clone(),
            vec!["aus einer Audiodatei".to_string()]
        );
    }

    #[tokio::test]
    async fn ladt_eine_zu_grosse_aufnahme_nicht_erst_herunter() {
        let (fake, router, texts) = bot(FakeSpeech::says("nie erkannt"));
        let zu_gross = (speech::MAX_BYTES + 1) as i64;

        router
            .handle_message(
                &context(fake.clone()),
                &voice_message("voice", 3, Some(zu_gross)),
            )
            .await
            .unwrap();

        assert!(fake.calls_of("getFile").is_empty(), "nichts geholt");
        assert!(fake.calls_of("downloadFile").is_empty(), "nichts geladen");
        assert!(texts.0.lock().unwrap().is_empty(), "nichts weitergegeben");
        let texte = sent(&fake);
        assert_eq!(texte.len(), 1);
        assert!(texte[0].contains("20 MB"), "{}", texte[0]);
        assert!(texte[0].contains(TYPING_ALWAYS_WORKS), "{}", texte[0]);
    }

    #[tokio::test]
    async fn weist_eine_zu_lange_aufnahme_mit_einem_hinweis_ab() {
        let (fake, router, texts) = bot(FakeSpeech::says("nie erkannt"));

        router
            .handle_message(
                &context(fake.clone()),
                &voice_message("voice", speech::MAX_SECONDS as i64 + 1, Some(12)),
            )
            .await
            .unwrap();

        assert!(fake.calls_of("downloadFile").is_empty());
        assert!(texts.0.lock().unwrap().is_empty());
        let texte = sent(&fake);
        assert!(texte[0].contains("300 s"), "{}", texte[0]);
    }

    #[tokio::test]
    async fn meldet_einen_fehler_der_erkennung_verstandlich() {
        let (fake, router, texts) = bot(FakeSpeech::fails("Spracherkennung ist ausgeschaltet."));

        router
            .handle_message(&context(fake.clone()), &voice_message("voice", 3, Some(12)))
            .await
            .unwrap();

        assert!(texts.0.lock().unwrap().is_empty(), "nichts weitergegeben");
        let texte = sent(&fake);
        assert_eq!(texte.len(), 1);
        assert!(
            texte[0].contains("Spracherkennung ist ausgeschaltet."),
            "{}",
            texte[0]
        );
        assert!(texte[0].contains(TYPING_ALWAYS_WORKS), "{}", texte[0]);
    }

    #[tokio::test]
    async fn antwortet_auf_eine_stumme_aufnahme_mit_einem_hinweis() {
        let (fake, router, texts) = bot(FakeSpeech::says("   "));

        router
            .handle_message(&context(fake.clone()), &voice_message("voice", 3, Some(12)))
            .await
            .unwrap();

        assert_eq!(sent(&fake), vec![NOTHING_HEARD.to_string()]);
        assert!(texts.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn lasst_eine_nachricht_ohne_aufnahme_unberuhrt() {
        let speech = FakeSpeech::says("nie erkannt");
        let (fake, router, texts) = bot(speech.clone());
        let ohne: Message = serde_json::from_value(json!({
            "message_id": 5,
            "chat": { "id": 42, "type": "private" },
            "text": "getippt",
        }))
        .expect("Nachricht");

        router.handle_message(&context(fake), &ohne).await.unwrap();

        assert!(speech.clips().is_empty(), "nichts erkannt");
        assert_eq!(texts.0.lock().unwrap().clone(), vec!["getippt".to_string()]);
    }

    #[tokio::test]
    async fn maskiert_html_im_erkannten_text() {
        let (fake, router, _) = bot(FakeSpeech::says("<b>fett</b> & frei"));

        router
            .handle_message(&context(fake.clone()), &voice_message("voice", 3, Some(12)))
            .await
            .unwrap();

        assert_eq!(
            sent(&fake)[0],
            "🎙️ Verstanden: „&lt;b&gt;fett&lt;/b&gt; &amp; frei“"
        );
    }

    #[tokio::test]
    async fn meldet_eine_datei_ohne_pfad_als_fehler() {
        let (fake, router, _) = bot(FakeSpeech::says("nie erkannt"));
        fake.put_file_info(
            "AwA",
            FileInfo {
                file_path: None,
                file_size: Some(12),
            },
        );

        router
            .handle_message(&context(fake.clone()), &voice_message("voice", 3, Some(12)))
            .await
            .unwrap();

        assert!(fake.calls_of("downloadFile").is_empty());
        assert!(
            sent(&fake)[0].contains("nicht zum Herunterladen frei"),
            "{}",
            sent(&fake)[0]
        );
    }
}
