//! Ein Claude-Account des Dispatchers; portiert aus
//! `packages/core/src/accounts/ClaudeAccount.ts`.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// ID des Accounts, den Claude Code ohne eigenen Ordner nutzt (Umgebung des Servers);
/// es gibt ihn immer
pub const STANDARD_ACCOUNT_ID: &str = "standard";

/// Höchstlänge des Anzeigenamens
pub const MAX_ACCOUNT_NAME_LENGTH: usize = 60;

/// Erlaubte Zeichen einer Account-ID (Slug)
static ACCOUNT_ID_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9-]{0,39}$").unwrap());

/// Ist der Text eine gültige Account-ID?
pub fn is_account_id(id: &str) -> bool {
    ACCOUNT_ID_PATTERN.is_match(id)
}

/// Ein Claude-Account des Dispatchers: eigener Konfigurationsordner für Claude Code
/// (`CLAUDE_CONFIG_DIR`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "accounts.ts")]
pub struct ClaudeAccount {
    /// Slug, unveränderlich; Profile und Regeln in `dispatch.yaml` verweisen darauf
    pub id: String,
    pub name: String,
    /// Konfigurationsordner, wie eingegeben (`~` erlaubt); null = Standard: die Umgebung des
    /// Servers bleibt unverändert
    pub config_dir: Option<String>,
    /// false = der Dispatcher wählt den Account nie
    pub enabled: bool,
    /// true = nur als Fallback: der Dispatcher nimmt ihn erst, wenn kein anderer erlaubter
    /// Account frei ist
    pub fallback: bool,
    /// Eigene Obergrenze in Prozent für das 5-Stunden-Fenster; darüber wählt der Dispatcher den
    /// Account nicht. null = Vorgabe des Dispatchers (`accountLimitPercent`)
    pub limit_five_hour: Option<i64>,
    /// Eigene Obergrenze in Prozent für die Woche; null = Vorgabe des Dispatchers
    pub limit_week: Option<i64>,
}

/// Der Standard-Account, wie er ohne Eintrag in der Datei gilt
pub fn standard_account() -> ClaudeAccount {
    ClaudeAccount {
        id: STANDARD_ACCOUNT_ID.to_string(),
        name: "Standard".to_string(),
        config_dir: None,
        enabled: true,
        fallback: false,
        limit_five_hour: None,
        limit_week: None,
    }
}

/// Gültige Obergrenze (1–100, ganzzahlig) oder `None`
pub fn account_limit(value: &serde_json::Value) -> Option<i64> {
    let number = value.as_f64()?;
    if number.fract() != 0.0 || !(1.0..=100.0).contains(&number) {
        return None;
    }
    Some(number as i64)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn erkennt_gueltige_account_ids() {
        assert!(is_account_id("standard"));
        assert!(is_account_id("pro-1"));
        assert!(!is_account_id("Böse ID"));
        assert!(!is_account_id("-start"));
        assert!(!is_account_id(""));
        assert!(!is_account_id(&"a".repeat(41)));
    }

    #[test]
    fn nimmt_nur_ganze_zahlen_von_1_bis_100_als_obergrenze() {
        assert_eq!(account_limit(&json!(1)), Some(1));
        assert_eq!(account_limit(&json!(100)), Some(100));
        assert_eq!(account_limit(&json!(0)), None);
        assert_eq!(account_limit(&json!(101)), None);
        assert_eq!(account_limit(&json!(50.5)), None);
        assert_eq!(account_limit(&json!("50")), None);
        assert_eq!(account_limit(&json!(null)), None);
    }

    #[test]
    fn der_standard_account_hat_keinen_eigenen_ordner() {
        let standard = standard_account();
        assert_eq!(standard.id, STANDARD_ACCOUNT_ID);
        assert_eq!(standard.name, "Standard");
        assert_eq!(standard.config_dir, None);
        assert!(standard.enabled);
    }

    #[test]
    fn schreibt_die_felder_in_camelcase() {
        let json = serde_json::to_value(standard_account()).unwrap();
        assert_eq!(
            json,
            json!({
                "id": "standard", "name": "Standard", "configDir": null,
                "enabled": true, "fallback": false, "limitFiveHour": null, "limitWeek": null,
            })
        );
    }
}
