//! Stand des Bots, wie ihn die Oberfläche anzeigt.
//!
//! Der Zustand entsteht im Server (Poller) und wird nirgends gespeichert: Nach einem
//! Neustart beginnt er wieder bei [`BotState::Off`].

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Woran der Poller gerade ist
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub enum BotState {
    /// Nicht eingeschaltet oder kein Bot-Token eingetragen
    #[default]
    Off,
    /// Der Poller fragt gerade, wer der Bot ist
    Starting,
    /// Der Poller holt Updates ab
    Running,
    /// Ein zweiter Poller holt dieselben Updates ab (409)
    Conflict,
    /// Telegram lehnt das Bot-Token ab (401)
    TokenInvalid,
    /// Jeder andere Fehler; der Poller versucht es weiter
    Error,
}

/// Stand des Bots samt letztem Fehler
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "telegram.ts")]
pub struct BotStatus {
    pub state: BotState,
    /// Benutzername des Bots aus `getMe` (ohne @); null = noch nicht gefragt
    pub bot_name: Option<String>,
    /// Bot-ID aus `getMe`; null = noch nicht gefragt
    pub bot_id: Option<i64>,
    /// Zeitpunkt des letzten verarbeiteten Updates (ISO); null = noch keines
    pub last_update_at: Option<String>,
    /// Letzter Fehler in Worten; null = keiner seit dem letzten Erfolg
    pub last_error: Option<String>,
}

impl BotStatus {
    /// Stand mit diesem Zustand, sonst wie bisher
    pub fn with_state(&self, state: BotState) -> Self {
        Self {
            state,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schreibt_den_zustand_in_camelcase() {
        let status = BotStatus {
            state: BotState::TokenInvalid,
            bot_name: Some("nexus_bot".into()),
            bot_id: Some(7),
            ..BotStatus::default()
        };

        let json = serde_json::to_value(&status).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "state": "tokenInvalid",
                "botName": "nexus_bot",
                "botId": 7,
                "lastUpdateAt": null,
                "lastError": null,
            })
        );
    }

    #[test]
    fn beginnt_ausgeschaltet() {
        assert_eq!(BotStatus::default().state, BotState::Off);
    }

    #[test]
    fn behalt_beim_wechsel_des_zustands_den_rest() {
        let status = BotStatus {
            state: BotState::Running,
            bot_id: Some(7),
            ..BotStatus::default()
        };

        let next = status.with_state(BotState::Conflict);

        assert_eq!(next.state, BotState::Conflict);
        assert_eq!(next.bot_id, Some(7));
    }
}
