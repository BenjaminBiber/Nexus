//! Spracherkennung: eine Aufnahme wird zu Text.
//!
//! Hinter [`SpeechToText`] steckt der Weg, den die Einstellung
//! ([`nexus_core::telegram::SpeechConfig`]) wählt: ein Dienst mit der Schnittstelle von OpenAI
//! (OpenAI selbst, Speaches, der Server von whisper.cpp, LocalAI – alle sprechen
//! `POST /v1/audio/transcriptions`), später das lokale `whisper-cli`, oder gar keiner.
//!
//! Tests und der Vertragslauf rufen nie einen echten Dienst: die Adresse zeigt dort auf eine
//! Attrappe im eigenen Prozess.

pub mod activity;
pub mod hint;
pub mod openai_compatible;

use async_trait::async_trait;
use nexus_core::telegram::{SpeechBackend, SpeechConfig};
use nexus_core::{HarnessError, Result};

pub use activity::{SpeechActivity, SpeechVia};
pub use hint::{glossary_terms, SpeechHint, MAX_HINT_CHARS, MAX_TICKETS};
pub use openai_compatible::OpenAiCompatibleSpeech;

/// Größte Aufnahme, die hochgeladen wird (20 MB)
pub const MAX_BYTES: usize = 20 * 1024 * 1024;

/// Längste Aufnahme, die hochgeladen wird (300 s)
pub const MAX_SECONDS: f64 = 300.0;

/// Die Aufnahme, die erkannt werden soll
#[derive(Debug, Clone, PartialEq)]
pub struct AudioClip {
    /// Die Daten der Datei, so wie sie der Dienst bekommt
    pub bytes: Vec<u8>,
    /// Dateiname für den Upload; der Dienst erkennt daran das Format
    pub file_name: String,
    /// Inhaltstyp der Datei (`audio/ogg`, `audio/mpeg` …)
    pub mime: String,
    /// Länge in Sekunden, wenn sie bekannt ist (Telegram liefert sie mit)
    pub seconds: Option<f64>,
}

impl AudioClip {
    /// Aufnahme mit Dateinamen aus dem Inhaltstyp; die Länge ist unbekannt
    pub fn new(bytes: Vec<u8>, mime: impl Into<String>) -> Self {
        let mime = mime.into();
        let file_name = format!("aufnahme.{}", extension_of(&mime));
        Self {
            bytes,
            file_name,
            mime,
            seconds: None,
        }
    }

    /// Dieselbe Aufnahme mit bekannter Länge
    pub fn with_seconds(mut self, seconds: Option<f64>) -> Self {
        self.seconds = seconds;
        self
    }

    /// Prüft die Grenzen, die vor jedem Upload gelten (20 MB und 300 s)
    pub fn within_limits(&self) -> Result<()> {
        check_limits(Some(self.bytes.len()), self.seconds)
    }
}

/// Prüft Größe und Länge einer Aufnahme gegen [`MAX_BYTES`] und [`MAX_SECONDS`]; was
/// unbekannt ist, gilt als in der Grenze.
///
/// Getrennt von [`AudioClip`], weil Telegram Größe und Dauer schon ankündigt – eine zu große
/// Sprachnachricht wird so gar nicht erst geladen.
pub fn check_limits(bytes: Option<usize>, seconds: Option<f64>) -> Result<()> {
    if bytes.is_some_and(|bytes| bytes > MAX_BYTES) {
        return Err(too_large(format!(
            "Die Aufnahme ist {} MB groß; erkannt werden höchstens {} MB.",
            bytes.unwrap_or_default() / (1024 * 1024),
            MAX_BYTES / (1024 * 1024)
        )));
    }
    if seconds.is_some_and(|seconds| seconds > MAX_SECONDS) {
        return Err(too_large(format!(
            "Die Aufnahme ist länger als {} s.",
            MAX_SECONDS as i64
        )));
    }
    Ok(())
}

/// Dateiendung zum Inhaltstyp; alles Unbekannte geht als `ogg` durch, weil Telegram
/// Sprachnachrichten als OGG/Opus liefert
fn extension_of(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or("").trim() {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "m4a",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/webm" => "webm",
        "audio/flac" | "audio/x-flac" => "flac",
        _ => "ogg",
    }
}

/// Das Ergebnis der Erkennung
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    /// Der erkannte Text, ohne Leerraum am Rand
    pub text: String,
    /// Die Sprache, die der Dienst gemeldet hat; `None`, wenn er keine nennt
    pub language: Option<String>,
}

/// Verwandelt eine Aufnahme in Text.
#[async_trait]
pub trait SpeechToText: Send + Sync {
    /// Welcher Weg hier arbeitet; die Testroute meldet ihn mit
    fn backend(&self) -> SpeechBackend;

    /// Erkennt die Aufnahme; `Err` nennt deutsch, woran es lag – nie mit dem Schlüssel
    async fn transcribe(&self, clip: AudioClip, hint: &SpeechHint) -> Result<Transcript>;
}

/// Baut den Weg, den die Einstellung wählt.
///
/// `whisper-cli` gibt es noch nicht; bis dahin lehnt es wie ein abgeschalteter Weg ab.
pub fn from_config(config: &SpeechConfig) -> Box<dyn SpeechToText> {
    match config.backend {
        SpeechBackend::OpenaiCompatible => Box::new(OpenAiCompatibleSpeech::from_config(config)),
        SpeechBackend::WhisperCli => Box::new(UnavailableSpeech {
            backend: SpeechBackend::WhisperCli,
            message: "Die Spracherkennung über whisper-cli ist noch nicht verfügbar.",
        }),
        SpeechBackend::Off => Box::new(UnavailableSpeech {
            backend: SpeechBackend::Off,
            message: "Spracherkennung ist ausgeschaltet.",
        }),
    }
}

/// Ein Weg, der nichts erkennt, sondern immer mit demselben Grund ablehnt
struct UnavailableSpeech {
    backend: SpeechBackend,
    message: &'static str,
}

#[async_trait]
impl SpeechToText for UnavailableSpeech {
    fn backend(&self) -> SpeechBackend {
        self.backend
    }

    async fn transcribe(&self, _clip: AudioClip, _hint: &SpeechHint) -> Result<Transcript> {
        Err(HarnessError::new("speech-off", self.message).into())
    }
}

/// Die Aufnahme sprengt eine der Grenzen
fn too_large(message: impl Into<String>) -> nexus_core::Error {
    HarnessError::new("speech-too-large", message).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eine Einstellung mit dem gewählten Weg
    fn config(backend: SpeechBackend) -> SpeechConfig {
        SpeechConfig {
            backend,
            ..SpeechConfig::default()
        }
    }

    #[test]
    fn leitet_den_dateinamen_aus_dem_inhaltstyp_ab() {
        assert_eq!(
            AudioClip::new(vec![1], "audio/ogg; codecs=opus").file_name,
            "aufnahme.ogg"
        );
        assert_eq!(
            AudioClip::new(vec![1], "audio/mpeg").file_name,
            "aufnahme.mp3"
        );
        assert_eq!(
            AudioClip::new(vec![1], "audio/wav").file_name,
            "aufnahme.wav"
        );
        // Unbekanntes gilt als OGG, weil Telegram Sprachnachrichten so schickt
        assert_eq!(
            AudioClip::new(vec![1], "application/octet-stream").file_name,
            "aufnahme.ogg"
        );
    }

    #[test]
    fn nimmt_eine_aufnahme_innerhalb_der_grenzen() {
        let clip = AudioClip::new(vec![0; 1024], "audio/ogg").with_seconds(Some(12.5));
        assert!(clip.within_limits().is_ok());
    }

    #[test]
    fn lehnt_eine_zu_grosse_aufnahme_ab() {
        let clip = AudioClip::new(vec![0; MAX_BYTES + 1], "audio/ogg");
        let error = clip.within_limits().expect_err("kein Fehler");
        assert_eq!(error.code(), Some("speech-too-large"));
        assert!(error.to_string().contains("20 MB"), "{error}");
    }

    #[test]
    fn lehnt_eine_zu_lange_aufnahme_ab() {
        let clip = AudioClip::new(vec![0; 16], "audio/ogg").with_seconds(Some(300.5));
        let error = clip.within_limits().expect_err("kein Fehler");
        assert_eq!(error.code(), Some("speech-too-large"));
        assert!(error.to_string().contains("300 s"), "{error}");
    }

    #[tokio::test]
    async fn backend_off_lehnt_mit_speech_off_ab() {
        let speech = from_config(&config(SpeechBackend::Off));
        assert_eq!(speech.backend(), SpeechBackend::Off);
        let error = speech
            .transcribe(AudioClip::new(vec![1], "audio/ogg"), &SpeechHint::default())
            .await
            .expect_err("kein Fehler");
        assert_eq!(error.code(), Some("speech-off"));
        assert_eq!(error.to_string(), "Spracherkennung ist ausgeschaltet.");
    }

    #[tokio::test]
    async fn whisper_cli_ist_noch_nicht_verfugbar() {
        let speech = from_config(&config(SpeechBackend::WhisperCli));
        assert_eq!(speech.backend(), SpeechBackend::WhisperCli);
        let error = speech
            .transcribe(AudioClip::new(vec![1], "audio/ogg"), &SpeechHint::default())
            .await
            .expect_err("kein Fehler");
        assert_eq!(error.code(), Some("speech-off"));
        assert!(
            error.to_string().contains("noch nicht verfügbar"),
            "{error}"
        );
    }

    #[test]
    fn openai_kompatibel_ist_der_vorgabeweg() {
        let speech = from_config(&SpeechConfig::default());
        assert_eq!(speech.backend(), SpeechBackend::OpenaiCompatible);
    }
}
