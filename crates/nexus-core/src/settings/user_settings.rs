//! Benutzerweite Einstellungen des Harness (gelten für alle Projekte).

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Art der Meldung, wenn eine Session bereit ist oder auf Eingabe wartet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationMode {
    Off,
    Browser,
    Windows,
}

impl NotificationMode {
    /// Alle Arten in der Reihenfolge von TypeScript (`NOTIFICATION_MODES`)
    pub const ALL: [NotificationMode; 3] = [
        NotificationMode::Off,
        NotificationMode::Browser,
        NotificationMode::Windows,
    ];

    /// Schreibweise in `settings.json`
    pub fn as_str(&self) -> &'static str {
        match self {
            NotificationMode::Off => "off",
            NotificationMode::Browser => "browser",
            NotificationMode::Windows => "windows",
        }
    }
}

/// Alle Arten der Meldung in der Reihenfolge von TypeScript
pub const NOTIFICATION_MODES: [NotificationMode; 3] = NotificationMode::ALL;

/// Farbschema der Oberfläche; „system“ folgt dem Betriebssystem, „signal“ und „pastel“ sind
/// farbigere helle Schemata, „win95“ ist Windows 95 samt Desktop, Fenster und Taskleiste
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
    Signal,
    Pastel,
    Win95,
}

impl ThemeMode {
    /// Alle Farbschemata in der Reihenfolge von TypeScript (`THEME_MODES`)
    pub const ALL: [ThemeMode; 6] = [
        ThemeMode::System,
        ThemeMode::Light,
        ThemeMode::Dark,
        ThemeMode::Signal,
        ThemeMode::Pastel,
        ThemeMode::Win95,
    ];

    /// Schreibweise in `settings.json`
    pub fn as_str(&self) -> &'static str {
        match self {
            ThemeMode::System => "system",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
            ThemeMode::Signal => "signal",
            ThemeMode::Pastel => "pastel",
            ThemeMode::Win95 => "win95",
        }
    }
}

/// Alle Farbschemata in der Reihenfolge von TypeScript
pub const THEME_MODES: [ThemeMode; 6] = ThemeMode::ALL;

/// Standardadresse des ntfy-Servers
pub const DEFAULT_NTFY_SERVER: &str = "https://ntfy.sh";

/// Erlaubte Zeichen eines ntfy-Themas (wie beim ntfy-Server)
pub static NTFY_TOPIC_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-_A-Za-z0-9]{1,64}$").unwrap());

/// Adresse des ntfy-Servers: http oder https, danach mindestens ein Zeichen ohne Leerraum
pub(crate) static NTFY_SERVER_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^https?://\S+$").unwrap());

/// Benutzerweite Einstellungen des Harness (gelten für alle Projekte)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    /// Art der Meldung, wenn eine Session bereit ist oder auf Eingabe wartet
    pub notifications: NotificationMode,
    /// Großes Bild in der Windows-Meldung „braucht deine Antwort“
    pub hero_images: bool,
    /// Farbschema der Oberfläche
    pub theme: ThemeMode,
    /// Claude-Nutzungslimits über das Anmelde-Token auslesen (Ring in der Seitenleiste);
    /// bewusst aus, bis es jemand einschaltet
    pub claude_usage: bool,
    /// Kompakte Ticket-Kacheln auf dem Board: Tags eingeklappt, Agent-Pille nur als Claude-Logo
    pub compact_cards: bool,
    /// Spalte „Nicht mehr nötig“ auf dem Board ausblenden
    pub hide_not_needed: bool,
    /// Spalte „In Planung“ auf dem Board ausblenden
    pub hide_in_planning: bool,
    /// Meldungen zusätzlich als Push über ntfy schicken (unabhängig von `notifications`)
    pub ntfy: bool,
    /// Adresse des ntfy-Servers, z. B. https://ntfy.sh
    pub ntfy_server: String,
    /// Thema, das die ntfy-App abonniert; leer: kein Push
    pub ntfy_topic: String,
    /// Zugriffstoken für geschützte Themen; leer: ohne Anmeldung
    pub ntfy_token: String,
}

impl Default for UserSettings {
    /// Entspricht `DEFAULT_USER_SETTINGS` aus TypeScript
    fn default() -> Self {
        Self {
            notifications: NotificationMode::Off,
            hero_images: false,
            theme: ThemeMode::System,
            claude_usage: false,
            compact_cards: false,
            hide_not_needed: false,
            hide_in_planning: false,
            ntfy: false,
            ntfy_server: DEFAULT_NTFY_SERVER.to_string(),
            ntfy_topic: String::new(),
            ntfy_token: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standardwerte_entsprechen_typescript() {
        let json = serde_json::to_value(UserSettings::default()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "notifications": "off",
                "heroImages": false,
                "theme": "system",
                "claudeUsage": false,
                "compactCards": false,
                "hideNotNeeded": false,
                "hideInPlanning": false,
                "ntfy": false,
                "ntfyServer": "https://ntfy.sh",
                "ntfyTopic": "",
                "ntfyToken": "",
            })
        );
    }

    #[test]
    fn arten_und_farbschemata_stehen_in_der_reihenfolge_von_typescript() {
        assert_eq!(
            NOTIFICATION_MODES.map(|mode| mode.as_str()),
            ["off", "browser", "windows"]
        );
        assert_eq!(
            THEME_MODES.map(|theme| theme.as_str()),
            ["system", "light", "dark", "signal", "pastel", "win95"]
        );
    }

    #[test]
    fn prufmuster_fur_thema_und_server() {
        assert!(NTFY_TOPIC_PATTERN.is_match("harness-x"));
        assert!(!NTFY_TOPIC_PATTERN.is_match("mit leerzeichen"));
        assert!(!NTFY_TOPIC_PATTERN.is_match(""));
        assert!(NTFY_SERVER_PATTERN.is_match("https://push.example"));
        assert!(!NTFY_SERVER_PATTERN.is_match("ftp://push.example"));
    }
}
