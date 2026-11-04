//! Routengruppe `speech`: `POST /api/speech/test` probiert die eingerichtete Spracherkennung
//! aus.
//!
//! `GET /api/speech/status` meldet, wann die Erkennung zuletzt geklappt hat und woran sie
//! zuletzt gescheitert ist.
//!
//! Die Einstellungsseite braucht einen Knopf, der sagt: „Der Dienst antwortet, und er versteht
//! Deutsch.“ Ohne Körper nimmt die Route dafür das mitgelieferte Beispiel
//! (`assets/speech-sample.ogg`, ein deutscher Satz), mit Körper die mitgeschickte Aufnahme.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use nexus_core::telegram::{SpeechStatusDto, SpeechTestDto, TelegramConfigStore};

use crate::error::{json, ApiError, ApiResult};
use crate::routes::static_or_not_found;
use crate::speech::{self, AudioClip, SpeechHint, SpeechVia};
use crate::state::AppState;

/// Das mitgelieferte Beispiel: OGG/Opus, gut drei Sekunden, ein deutscher Satz
pub const SAMPLE_AUDIO: &[u8] = include_bytes!("../../assets/speech-sample.ogg");

/// Inhaltstyp des Beispiels
pub const SAMPLE_MIME: &str = "audio/ogg";

/// Länge des Beispiels in Sekunden (aus der Datei; nur für die Grenzprüfung)
pub const SAMPLE_SECONDS: f64 = 3.4;

/// Meldet die Routen der Gruppe an. Der Körper darf so groß sein wie die größte Aufnahme –
/// der Standard von axum liegt weit darunter.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/speech/test",
            post(test)
                .layer(DefaultBodyLimit::max(speech::MAX_BYTES))
                .fallback(static_or_not_found),
        )
        .route(
            "/api/speech/status",
            get(status).fallback(static_or_not_found),
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
    let hint = SpeechHint::for_state(&state, config.speech.language.clone()).await;
    let started = std::time::Instant::now();
    let result = speech.transcribe(clip, &hint).await;
    state
        .speech_activity
        .record(speech.backend(), SpeechVia::Test, started.elapsed(), &result);
    let transcript = result?;
    Ok(json(&SpeechTestDto {
        text: transcript.text,
        language: transcript.language,
        backend: speech.backend().as_str().to_string(),
        millis: started.elapsed().as_millis() as u64,
    }))
}

/// Wann die Erkennung zuletzt geklappt hat und woran sie zuletzt gescheitert ist
async fn status(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let dto: SpeechStatusDto = state.speech_activity.snapshot();
    Ok(json(&dto))
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
