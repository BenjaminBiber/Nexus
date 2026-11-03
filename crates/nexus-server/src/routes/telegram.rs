//! Routengruppe `telegram`: Einstellung, Stand, Kopplung, Entkoppeln, Testnachricht und das
//! Einrichten des Bot-Profils.
//!
//! Die Einstellung geht nur maskiert hinaus und kommt nur als Teil-Eingabe herein
//! (`merge_update`): Bot-Token und API-Schlüssel behalten ihren Wert, wenn die Oberfläche die
//! Maske zurückschickt. Nach dem Speichern weckt die Route den Poller, damit er die neue
//! Einstellung sofort liest.
//!
//! Keine Antwort dieser Gruppe enthält ein Geheimnis: In jeder Telegram-Adresse steht das
//! Bot-Token, deshalb nennt auch kein Fehlertext eine Adresse ([`TelegramError`]).

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::Router;
use nexus_core::telegram::{
    BotState, TelegramConfigStore, TelegramPairingDto, TelegramProfileDto, TelegramStatusDto,
    TelegramTestDto, TelegramTestError,
};
use serde_json::Value;

use crate::error::{json, ApiError, ApiResult};
use crate::extract::JsonBody;
use crate::routes::static_or_not_found;
use crate::state::AppState;
use crate::telegram::PairingCode;

/// Text der Testnachricht; Telegram-HTML wie jede andere Nachricht des Bots
pub const TEST_MESSAGE: &str = "<b>Testnachricht von Nexus</b>\nDie Verbindung steht.";

/// Meldet die Routen der Gruppe an; andere Methoden auf denselben Pfaden fallen in den
/// Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/telegram/config",
            get(config).put(save_config).fallback(static_or_not_found),
        )
        .route(
            "/api/telegram/status",
            get(status).fallback(static_or_not_found),
        )
        .route(
            "/api/telegram/pair",
            post(pair).fallback(static_or_not_found),
        )
        .route(
            "/api/telegram/chats/{id}",
            delete(unpair).fallback(static_or_not_found),
        )
        .route(
            "/api/telegram/test",
            post(test).fallback(static_or_not_found),
        )
        .route(
            "/api/telegram/profile",
            post(profile).fallback(static_or_not_found),
        )
}

/// Die gespeicherte Einstellung, Geheimnisse nur als Maske
async fn config(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let config = store(&state).read()?;
    Ok(json(&config.masked()))
}

/// Übernimmt die Eingabe der Oberfläche, speichert sie und weckt den Poller; antwortet wie
/// [`config`]
async fn save_config(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    // Ein fehlender Körper ist eine leere Eingabe: sie lässt alles, wie es ist
    let input = if body.is_null() {
        Value::Object(Default::default())
    } else {
        body
    };
    let saved = store(&state).save_update(&input)?;
    state.telegram.restart();
    Ok(json(&saved.masked()))
}

/// Stand des Pollers samt Zahl der gekoppelten Chats und der offenen Einmal-Codes
async fn status(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let config = store(&state).read()?;
    Ok(json(&TelegramStatusDto {
        bot: state.telegram.status().await,
        paired_chats: config.chats.len() as i64,
        open_codes: state.telegram.pairing().open_count() as i64,
    }))
}

/// Gibt einen Einmal-Code aus; ohne laufenden Bot gäbe es niemanden, der ihn einlöst
async fn pair(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let bot = state.telegram.status().await;
    if bot.state != BotState::Running {
        return Err(not_running());
    }
    let code = state.telegram.pairing().create();
    Ok(json(&dto_of(&code, bot.bot_name.as_deref())))
}

/// Nimmt einen Chat aus der Freigabeliste. Die Chat-ID kommt aus der Adresse; was keine Zahl
/// ist, kann auch nicht gekoppelt sein – deshalb dieselbe Antwort wie für einen unbekannten Chat
async fn unpair(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult<Response> {
    let chat_id = id.parse::<i64>().ok();
    let entkoppelt = match chat_id {
        Some(chat_id) => store(&state).remove_chat(chat_id)?,
        None => false,
    };
    if !entkoppelt {
        return Err(ApiError::new(
            "telegram-chat-not-found",
            format!("Der Chat {id} ist nicht gekoppelt."),
        ));
    }
    Ok(json(&serde_json::json!({ "ok": true })))
}

/// Schickt die Testnachricht an alle gekoppelten Chats und sagt, was angekommen ist
async fn test(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let config = store(&state).read()?;
    if !config.enabled || config.bot_token.is_empty() {
        return Err(not_running());
    }
    let api = state.telegram.api(&config);
    let mut sent = 0;
    let mut errors = Vec::new();
    for chat_id in &config.chats {
        match api.send_message(*chat_id, TEST_MESSAGE, None, None).await {
            Ok(_) => sent += 1,
            // Die Fehlertexte der Bot-API nennen nie die Adresse, also auch nie das Token
            Err(error) => errors.push(TelegramTestError {
                chat_id: *chat_id,
                error: error.to_string(),
            }),
        }
    }
    Ok(json(&TelegramTestDto { sent, errors }))
}

/// Richtet Profilbild, Name, Beschreibungen und Befehlsliste des Bots neu ein – erzwungen,
/// also auch dann, wenn alles schon gesetzt war. Die Bot-ID kommt aus dem Stand des Pollers;
/// ohne sie wüsste niemand, für welchen Bot das Profil gilt
async fn profile(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let config = store(&state).read()?;
    if !config.enabled || config.bot_token.is_empty() {
        return Err(not_running());
    }
    let Some(bot_id) = state.telegram.status().await.bot_id else {
        return Err(not_running());
    };
    let api = state.telegram.api(&config);
    let outcome = state.telegram.sync_profile(api.as_ref(), bot_id).await;
    Ok(json(&TelegramProfileDto {
        applied: outcome.applied,
        error: outcome.error,
    }))
}

/// Der Store der Einstellung auf dem Benutzerordner des Servers
fn store(state: &Arc<AppState>) -> TelegramConfigStore {
    TelegramConfigStore::new(state.home.clone())
}

/// Antwort auf einen Code oder eine Testnachricht ohne laufenden Bot
fn not_running() -> ApiError {
    ApiError::new(
        "telegram-not-running",
        "Der Bot läuft nicht – erst einschalten und ein Bot-Token eintragen.",
    )
}

/// Code, Ende der Gültigkeit und der Link in den Chat des Bots
fn dto_of(code: &PairingCode, bot_name: Option<&str>) -> TelegramPairingDto {
    let link = match bot_name {
        Some(name) => format!("https://t.me/{name}?start={}", code.code),
        // Ohne Benutzernamen (sehr alte Bots) bleibt nur der Code zum Abtippen
        None => String::new(),
    };
    TelegramPairingDto {
        code: code.code.clone(),
        expires_at: code.expires_at.clone(),
        link,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baut_den_link_mit_code_und_bot() {
        let code = PairingCode {
            code: "ABC234".into(),
            expires_at: "2026-11-03T12:10:00.000Z".into(),
        };

        let dto = dto_of(&code, Some("nexus_bot"));

        assert_eq!(dto.link, "https://t.me/nexus_bot?start=ABC234");
        assert_eq!(dto.expires_at, code.expires_at);
    }

    #[test]
    fn lasst_den_link_ohne_bot_namen_leer() {
        let code = PairingCode {
            code: "ABC234".into(),
            expires_at: "2026-11-03T12:10:00.000Z".into(),
        };

        assert_eq!(dto_of(&code, None).link, "");
    }
}
