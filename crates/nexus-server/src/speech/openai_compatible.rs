//! Spracherkennung über die Schnittstelle von OpenAI: `POST {base_url}/v1/audio/transcriptions`.
//!
//! Dieselbe Schnittstelle sprechen OpenAI selbst, Speaches, der Server von whisper.cpp und
//! LocalAI. Der Unterschied steckt allein in der Adresse und darin, ob ein Schlüssel nötig ist:
//! bei einem eigenen Dienst im Netz ist er leer, und dann darf auch kein `Authorization`-Kopf
//! mitgehen – manche Server lehnen eine Anfrage mit leerem Bearer sonst ab.
//!
//! Fehlermeldungen nennen Status und einen kurzen Auszug der Antwort, nie den Schlüssel.

use std::time::Duration;

use async_trait::async_trait;
use nexus_core::telegram::{SpeechBackend, SpeechConfig};
use nexus_core::{HarnessError, Result};
use serde::Deserialize;

use crate::speech::{AudioClip, SpeechHint, SpeechToText, Transcript};

/// Zeitlimit einer Erkennung; lange Aufnahmen brauchen auf einem eigenen Server Minuten
pub const TIMEOUT: Duration = Duration::from_secs(120);

/// So viele Zeichen der fremden Antwort stehen im Fehlertext
const EXCERPT_CHARS: usize = 200;

/// Erkennung über einen Dienst mit der Schnittstelle von OpenAI.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleSpeech {
    /// Adresse des Dienstes, ohne Schrägstrich am Ende
    base_url: String,
    /// Schlüssel; leer = ohne Anmeldung, dann ohne `Authorization`-Kopf
    api_key: String,
    model: String,
    timeout: Duration,
}

impl OpenAiCompatibleSpeech {
    /// Erkennung aus der Einstellung
    pub fn from_config(config: &SpeechConfig) -> Self {
        Self {
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            timeout: TIMEOUT,
        }
    }

    /// Dieselbe Erkennung mit anderem Zeitlimit (Tests warten nicht zwei Minuten)
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Die Adresse, an die die Aufnahme geht
    pub fn url(&self) -> String {
        format!("{}/v1/audio/transcriptions", self.base_url)
    }
}

/// Die Antwort des Dienstes, soweit die Erkennung sie braucht
#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
    language: Option<String>,
}

#[async_trait]
impl SpeechToText for OpenAiCompatibleSpeech {
    fn backend(&self) -> SpeechBackend {
        SpeechBackend::OpenaiCompatible
    }

    async fn transcribe(&self, clip: AudioClip, hint: &SpeechHint) -> Result<Transcript> {
        clip.within_limits()?;
        let file = reqwest::multipart::Part::bytes(clip.bytes)
            .file_name(clip.file_name)
            .mime_str(&clip.mime)
            .map_err(|_| failed(format!("Der Inhaltstyp {} passt nicht.", clip.mime)))?;
        let mut form = reqwest::multipart::Form::new()
            .part("file", file)
            .text("model", self.model.clone())
            .text("response_format", "json");
        if let Some(language) = hint.language.clone() {
            form = form.text("language", language);
        }
        if !hint.prompt.is_empty() {
            form = form.text("prompt", hint.prompt.clone());
        }

        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|error| failed(format!("Der HTTP-Client ließ sich nicht bauen: {error}")))?;
        let mut request = client.post(self.url()).multipart(form);
        // Nur mit Schlüssel einen Kopf setzen; ein leerer Bearer stört eigene Server
        if !self.api_key.is_empty() {
            request = request.bearer_auth(&self.api_key);
        }
        let response = request
            .send()
            .await
            .map_err(|error| self.sende_fehler(error))?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(failed(format!(
                "Die Spracherkennung antwortete mit {}{}.",
                status.as_u16(),
                self.excerpt(&body)
            )));
        }
        let parsed: TranscriptionResponse = serde_json::from_str(&body).map_err(|_| {
            failed(format!(
                "Die Antwort der Spracherkennung passt nicht{}.",
                self.excerpt(&body)
            ))
        })?;
        Ok(Transcript {
            text: parsed.text.trim().to_string(),
            language: parsed
                .language
                .map(|language| language.trim().to_string())
                .filter(|language| !language.is_empty()),
        })
    }
}

impl OpenAiCompatibleSpeech {
    /// Fehler des Sendens: das Zeitlimit wird eigens genannt, weil es der häufigste Fall ist
    fn sende_fehler(&self, error: reqwest::Error) -> nexus_core::Error {
        if error.is_timeout() {
            return failed(format!(
                "Die Spracherkennung hat nicht innerhalb von {} s geantwortet.",
                self.timeout.as_secs()
            ));
        }
        failed(format!(
            "Die Spracherkennung ist nicht erreichbar: {}",
            self.scrub(&error.to_string())
        ))
    }

    /// Kurzer Auszug der fremden Antwort als `: …`; leer bleibt leer
    fn excerpt(&self, body: &str) -> String {
        let text = self.scrub(body);
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return String::new();
        }
        let cut: String = text.chars().take(EXCERPT_CHARS).collect();
        match cut.chars().count() < text.chars().count() {
            true => format!(": {cut}…"),
            false => format!(": {cut}"),
        }
    }

    /// Nimmt den Schlüssel aus einem Text. Normalerweise steht er dort nicht – aber ein Dienst,
    /// der ihn in seiner Fehlermeldung wiederholt, soll ihn nicht in unser Protokoll bringen.
    fn scrub(&self, text: &str) -> String {
        let text = text.trim();
        if self.api_key.is_empty() {
            return text.to_string();
        }
        text.replace(&self.api_key, "…")
    }
}

/// Die Erkennung ist gescheitert; die Route macht daraus 502
fn failed(message: impl Into<String>) -> nexus_core::Error {
    HarnessError::new("speech-failed", message).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Erkennung auf einer Adresse, mit oder ohne Schlüssel
    fn speech(base_url: &str, api_key: &str) -> OpenAiCompatibleSpeech {
        OpenAiCompatibleSpeech::from_config(&SpeechConfig {
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            ..SpeechConfig::default()
        })
    }

    #[test]
    fn haengt_den_pfad_der_schnittstelle_an_die_adresse() {
        assert_eq!(
            speech("http://localhost:8000", "").url(),
            "http://localhost:8000/v1/audio/transcriptions"
        );
        // Ein Schrägstrich am Ende ergibt keinen doppelten
        assert_eq!(
            speech("http://localhost:8000/", "").url(),
            "http://localhost:8000/v1/audio/transcriptions"
        );
    }

    #[test]
    fn kurzt_den_auszug_und_zieht_den_schlussel_heraus() {
        let speech = speech("http://localhost:8000", "sk-geheim");
        assert_eq!(speech.excerpt("   "), "");
        assert_eq!(
            speech.excerpt("{\n  \"error\": \"nope\"\n}"),
            ": { \"error\": \"nope\" }"
        );
        assert_eq!(
            speech.excerpt("Schlüssel sk-geheim falsch"),
            ": Schlüssel … falsch"
        );
        let lang = "x".repeat(EXCERPT_CHARS + 50);
        assert!(speech.excerpt(&lang).ends_with('…'));
        assert_eq!(speech.excerpt(&lang).chars().count(), EXCERPT_CHARS + 3);
    }

    #[test]
    fn das_zeitlimit_ist_zwei_minuten() {
        assert_eq!(speech("http://localhost:8000", "").timeout, TIMEOUT);
        assert_eq!(TIMEOUT.as_secs(), 120);
    }
}
