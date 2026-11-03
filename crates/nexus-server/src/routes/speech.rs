//! Routengruppe `speech`: `POST /api/speech/test` probiert die eingerichtete Spracherkennung
//! aus.
//!
//! Die Einstellungsseite braucht einen Knopf, der sagt: „Der Dienst antwortet, und er versteht
//! Deutsch.“ Ohne Körper nimmt die Route dafür das mitgelieferte Beispiel
//! (`assets/speech-sample.ogg`, ein deutscher Satz), mit Körper die mitgeschickte Aufnahme.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap};
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::telegram::TelegramConfigStore;
use serde::Serialize;

use crate::error::{json, ApiError, ApiResult};
use crate::routes::static_or_not_found;
use crate::speech::{self, AudioClip, SpeechHint};
use crate::state::AppState;

/// Das mitgelieferte Beispiel: OGG/Opus, gut drei Sekunden, ein deutscher Satz
pub const SAMPLE_AUDIO: &[u8] = include_bytes!("../../assets/speech-sample.ogg");

/// Inhaltstyp des Beispiels
pub const SAMPLE_MIME: &str = "audio/ogg";

/// Länge des Beispiels in Sekunden (aus der Datei; nur für die Grenzprüfung)
pub const SAMPLE_SECONDS: f64 = 3.4;

/// Das Ergebnis der Testerkennung
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechTestDto {
    /// Der erkannte Text
    pub text: String,
    /// Die Sprache, die der Dienst gemeldet hat; `null`, wenn er keine nennt
    pub language: Option<String>,
    /// Der Weg, der erkannt hat (`openai-compatible`, `whisper-cli`, `off`)
    pub backend: String,
    /// Wie lange die Erkennung gebraucht hat
    pub millis: u64,
}

/// Meldet die Route der Gruppe an. Der Körper darf so groß sein wie die größte Aufnahme –
/// der Standard von axum liegt weit darunter.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/speech/test",
        post(test)
            .layer(DefaultBodyLimit::max(speech::MAX_BYTES))
            .fallback(static_or_not_found),
    )
}

/// Erkennt die mitgeschickte oder die mitgelieferte Aufnahme mit der eingerichteten Erkennung
async fn test(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Response> {
    let config = TelegramConfigStore::new(state.home.clone()).read()?;
    let clip = clip_of(&headers, body)?;
    clip.within_limits()?;

    let speech = speech::from_config(&config.speech);
    let hint = hint(&state, config.speech.language.clone()).await;
    let started = std::time::Instant::now();
    let transcript = speech.transcribe(clip, &hint).await?;
    Ok(json(&SpeechTestDto {
        text: transcript.text,
        language: transcript.language,
        backend: speech.backend().as_str().to_string(),
        millis: started.elapsed().as_millis() as u64,
    }))
}

/// Die Aufnahme der Anfrage: ein leerer Körper ergibt das mitgelieferte Beispiel
fn clip_of(headers: &HeaderMap, body: Bytes) -> ApiResult<AudioClip> {
    if body.is_empty() {
        return Ok(
            AudioClip::new(SAMPLE_AUDIO.to_vec(), SAMPLE_MIME).with_seconds(Some(SAMPLE_SECONDS))
        );
    }
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .trim();
    // Ein JSON-Körper ist keine Aufnahme; die Oberfläche schickt entweder nichts oder Audio
    if mime.is_empty() || mime.starts_with("application/json") {
        return Err(ApiError::new(
            "bad-request",
            "Ohne Aufnahme den Körper leer lassen, sonst die Audiodaten mit ihrem Inhaltstyp schicken (z. B. audio/ogg).",
        ));
    }
    Ok(AudioClip::new(body.to_vec(), mime))
}

/// Die Erkennungshilfe aus dem Board; ohne erreichbares Board bleibt nur die Sprache
async fn hint(state: &Arc<AppState>, language: Option<String>) -> SpeechHint {
    let active_board = BoardConfigStore::new(state.home.clone())
        .read()
        .ok()
        .flatten()
        .and_then(|config| config.active_board);
    match state.board.get().await {
        Ok(backend) => SpeechHint::collect(backend.services(), active_board, language).await,
        Err(_) => SpeechHint::language_only(language),
    }
}
