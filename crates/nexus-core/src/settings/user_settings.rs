//! Benutzerweite Einstellungen des Harness (gelten für alle Projekte).

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::registry::NexusHome;
use crate::themes::is_valid_theme_id;

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

/// Gruppierung der Spalte „Erledigt“ auf dem Board
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoneGrouping {
    /// Nach Zeitabschnitten des Erledigt-Datums
    Time,
    /// Nach Spec
    Spec,
}

impl DoneGrouping {
    /// Alle Gruppierungen in der Reihenfolge der Oberfläche
    pub const ALL: [DoneGrouping; 2] = [DoneGrouping::Time, DoneGrouping::Spec];

    /// Schreibweise in `settings.json`
    pub fn as_str(&self) -> &'static str {
        match self {
            DoneGrouping::Time => "time",
            DoneGrouping::Spec => "spec",
        }
    }
}

/// Alle Gruppierungen der Spalte „Erledigt“
pub const DONE_GROUPINGS: [DoneGrouping; 2] = DoneGrouping::ALL;

/// Höchstes Alter erledigter Tickets in Tagen, ab dem sie ins Archiv wandern
pub const DONE_ARCHIVE_MAX_DAYS: u16 = 365;

/// Eingebaute Themes in der Reihenfolge von TypeScript (`BUILTIN_THEME_IDS`); „system“ folgt
/// dem Betriebssystem. Alle anderen Themes sind Pakete (auch die mitgelieferten unter `themes/`).
pub const BUILTIN_THEME_IDS: [&str; 3] = ["system", "light", "dark"];

/// Gewähltes Theme als ID: eingebaut (`BUILTIN_THEME_IDS`) oder ein installiertes Theme
/// unter `~/.nexus/themes/<id>/`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThemeMode(String);

impl ThemeMode {
    /// Theme mit der angegebenen ID (ohne Prüfung)
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Folgt dem Betriebssystem; Rückfall für unbekannte IDs
    pub fn system() -> Self {
        Self::new("system")
    }

    /// Schreibweise in `settings.json`
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ThemeMode {
    fn from(id: &str) -> Self {
        Self::new(id)
    }
}

/// Ob `id` ein eingebautes oder installiertes Theme ist; installiert heißt: gültige ID und
/// `<home>/themes/<id>/manifest.json` existiert (ohne volle Manifestprüfung, billig bei jedem Lesen)
pub fn known_theme(home: &NexusHome, id: &str) -> bool {
    BUILTIN_THEME_IDS.contains(&id)
        || (is_valid_theme_id(id) && home.themes_dir().join(id).join("manifest.json").is_file())
}

/// Standardadresse des ntfy-Servers
pub const DEFAULT_NTFY_SERVER: &str = "https://ntfy.sh";

/// Erlaubte Zeichen eines ntfy-Themas (wie beim ntfy-Server)
pub static NTFY_TOPIC_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-_A-Za-z0-9]{1,64}$").unwrap());

/// Adresse des ntfy-Servers: http oder https, danach mindestens ein Zeichen ohne Leerraum
pub(crate) static NTFY_SERVER_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^https?://\S+$").unwrap());

/// Setup-Dialoge der Oberfläche, die nicht mehr kommen sollen („Nicht mehr fragen“)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPrompts {
    /// Dialog zur Installation des Claude-Code-Plugins abgewiesen
    pub plugin_dismissed: bool,
    /// Boards, deren Migrationsdialog abgewiesen wurde (aufsteigend, ohne Doppelte)
    pub migration_dismissed: Vec<i32>,
}

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
    /// Gruppierung der Spalte „Erledigt“: nach Zeitabschnitten oder nach Spec
    pub done_grouping: DoneGrouping,
    /// Erledigte Tickets, die älter als so viele Tage sind, wandern ins Archiv der Spalte
    /// „Erledigt“ (0 bis 365; 0 = nie)
    pub done_archive_days: u16,
    /// Meldungen zusätzlich als Push über ntfy schicken (unabhängig von `notifications`)
    pub ntfy: bool,
    /// Adresse des ntfy-Servers, z. B. https://ntfy.sh
    pub ntfy_server: String,
    /// Thema, das die ntfy-App abonniert; leer: kein Push
    pub ntfy_topic: String,
    /// Zugriffstoken für geschützte Themen; leer: ohne Anmeldung
    pub ntfy_token: String,
    /// Abgewiesene Setup-Dialoge
    pub setup_prompts: SetupPrompts,
}

impl Default for UserSettings {
    /// Entspricht `DEFAULT_USER_SETTINGS` aus TypeScript
    fn default() -> Self {
        Self {
            notifications: NotificationMode::Off,
            hero_images: false,
            theme: ThemeMode::system(),
            claude_usage: false,
            compact_cards: false,
            hide_not_needed: false,
            hide_in_planning: false,
            done_grouping: DoneGrouping::Time,
            done_archive_days: 0,
            ntfy: false,
            ntfy_server: DEFAULT_NTFY_SERVER.to_string(),
            ntfy_topic: String::new(),
            ntfy_token: String::new(),
            setup_prompts: SetupPrompts::default(),
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
                "doneGrouping": "time",
                "doneArchiveDays": 0,
                "ntfy": false,
                "ntfyServer": "https://ntfy.sh",
                "ntfyTopic": "",
                "ntfyToken": "",
                "setupPrompts": { "pluginDismissed": false, "migrationDismissed": [] },
            })
        );
    }

    #[test]
    fn arten_und_farbschemata_stehen_in_der_reihenfolge_von_typescript() {
        assert_eq!(
            NOTIFICATION_MODES.map(|mode| mode.as_str()),
            ["off", "browser", "windows"]
        );
        assert_eq!(BUILTIN_THEME_IDS, ["system", "light", "dark"]);
        assert_eq!(
            DONE_GROUPINGS.map(|grouping| grouping.as_str()),
            ["time", "spec"]
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
