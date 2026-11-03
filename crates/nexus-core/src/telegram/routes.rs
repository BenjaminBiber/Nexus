//! Antworten der Telegram-Routen, soweit sie die Oberfläche braucht.
//!
//! Die Typen stehen hier und nicht im Server, weil die Oberfläche sie über ts-rs bekommt
//! (`packages/web/src/api/generated/telegram.ts`). Geheimnisse kommen in keiner dieser
//! Antworten vor: die Einstellung geht nur maskiert hinaus ([`super::TelegramConfig::masked`]),
//! und der Kopplungslink enthält den Einmal-Code, nie das Bot-Token.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::telegram::status::BotStatus;

/// Antwort von `GET /api/telegram/status`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct TelegramStatusDto {
    /// Stand des Pollers
    pub bot: BotStatus,
    /// Zahl der gekoppelten Chats
    pub paired_chats: i64,
    /// Zahl der ausgegebenen, noch gültigen Einmal-Codes
    pub open_codes: i64,
}

/// Antwort von `POST /api/telegram/pair`: ein Einmal-Code samt Link in den Chat
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct TelegramPairingDto {
    /// Der Code, wie die Oberfläche ihn zeigt
    pub code: String,
    /// Zeitpunkt, ab dem der Code nicht mehr zieht (ISO)
    pub expires_at: String,
    /// Link, der den Chat mit dem Bot öffnet und den Code gleich mitschickt
    pub link: String,
}

/// Ein Chat, an den die Testnachricht nicht herauskam
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct TelegramTestError {
    pub chat_id: i64,
    /// Der Fehler in Worten; nie mit Adresse oder Token
    pub error: String,
}

/// Antwort von `POST /api/telegram/test`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct TelegramTestDto {
    /// Zahl der Chats, die die Nachricht bekommen haben
    pub sent: i64,
    /// Chats, an die sie nicht herauskam
    pub errors: Vec<TelegramTestError>,
}

/// Antwort von `POST /api/telegram/profile`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct TelegramProfileDto {
    /// true = Profilbild, Name, Beschreibungen und Befehlsliste wurden gerade gesetzt
    pub applied: bool,
    /// Woran es scheiterte; null = kein Fehler. Nie mit Adresse oder Token
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telegram::status::BotState;

    #[test]
    fn schreibt_den_stand_in_camelcase() {
        let dto = TelegramStatusDto {
            bot: BotStatus {
                state: BotState::Running,
                bot_name: Some("nexus_bot".into()),
                ..BotStatus::default()
            },
            paired_chats: 2,
            open_codes: 1,
        };

        let json = serde_json::to_value(&dto).unwrap();

        assert_eq!(json["bot"]["state"], "running");
        assert_eq!(json["bot"]["botName"], "nexus_bot");
        assert_eq!(json["pairedChats"], 2);
        assert_eq!(json["openCodes"], 1);
    }

    #[test]
    fn schreibt_das_ergebnis_der_testnachricht_in_camelcase() {
        let dto = TelegramTestDto {
            sent: 1,
            errors: vec![TelegramTestError {
                chat_id: 42,
                error: "Telegram nicht erreichbar".into(),
            }],
        };

        let json = serde_json::to_value(&dto).unwrap();

        assert_eq!(json["sent"], 1);
        assert_eq!(json["errors"][0]["chatId"], 42);
    }

    #[test]
    fn schreibt_das_ergebnis_des_profils_in_camelcase() {
        let json = serde_json::to_value(TelegramProfileDto {
            applied: false,
            error: Some("Profilbild nicht gesetzt: Telegram bremst".into()),
        })
        .unwrap();

        assert_eq!(json["applied"], false);
        assert!(json["error"].as_str().unwrap().starts_with("Profilbild"));
    }
}
