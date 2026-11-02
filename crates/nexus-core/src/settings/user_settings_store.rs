//! Speichert die benutzerweiten Einstellungen in `~/.nexus/settings.json` (atomar).

use std::fs;

use serde_json::Value;

use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;
use crate::settings::user_settings::{
    NotificationMode, SetupPrompts, ThemeMode, UserSettings, DEFAULT_NTFY_SERVER,
    NOTIFICATION_MODES, NTFY_SERVER_PATTERN, NTFY_TOPIC_PATTERN, THEME_MODES,
};

/// Liest und schreibt `settings.json`.
#[derive(Debug, Clone)]
pub struct UserSettingsStore {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl UserSettingsStore {
    /// Einstellungen im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Liest die Einstellungen; fehlende oder beschädigte Datei ergibt die Standardwerte
    pub fn read(&self) -> UserSettings {
        let file = self.home.settings_file();
        if !file.exists() {
            return UserSettings::default();
        }
        match fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        {
            Some(raw) => normalize(&raw),
            None => UserSettings::default(),
        }
    }

    /// Schreibt die Einstellungen; der Ordner wird bei Bedarf angelegt
    pub fn write(&self, settings: &UserSettings) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        // Zwei Leerzeichen Einrückung und kein Zeilenumbruch am Ende, wie `JSON.stringify(x, null, 2)`
        let json = serde_json::to_string_pretty(settings).map_err(Error::unexpected)?;
        self.writer.write(&self.home.settings_file(), &json)
    }
}

/// Übernimmt nur gültige Werte; der frühere Schalter `notifyOnWaiting` wird zu „browser“
fn normalize(raw: &Value) -> UserSettings {
    let legacy = if is_true(raw, "notifyOnWaiting") {
        NotificationMode::Browser
    } else {
        UserSettings::default().notifications
    };
    let notifications = NOTIFICATION_MODES
        .into_iter()
        .find(|mode| raw.get("notifications").and_then(Value::as_str) == Some(mode.as_str()))
        .unwrap_or(legacy);
    let theme = THEME_MODES
        .into_iter()
        .find(|theme| raw.get("theme").and_then(Value::as_str) == Some(theme.as_str()))
        .unwrap_or(ThemeMode::System);
    UserSettings {
        notifications,
        hero_images: is_true(raw, "heroImages"),
        theme,
        claude_usage: is_true(raw, "claudeUsage"),
        compact_cards: is_true(raw, "compactCards"),
        hide_not_needed: is_true(raw, "hideNotNeeded"),
        hide_in_planning: is_true(raw, "hideInPlanning"),
        ntfy: is_true(raw, "ntfy"),
        // ntfy-Felder: Server als http(s)-Adresse, Thema nur mit erlaubten Zeichen, sonst Standard
        ntfy_server: text(raw, "ntfyServer")
            .filter(|server| NTFY_SERVER_PATTERN.is_match(server))
            .unwrap_or_else(|| DEFAULT_NTFY_SERVER.to_string()),
        ntfy_topic: text(raw, "ntfyTopic")
            .filter(|topic| NTFY_TOPIC_PATTERN.is_match(topic))
            .unwrap_or_default(),
        ntfy_token: text(raw, "ntfyToken").unwrap_or_default(),
        setup_prompts: setup_prompts(raw.get("setupPrompts")),
    }
}

/// Abgewiesene Setup-Dialoge; Unbrauchbares zählt als nicht abgewiesen
fn setup_prompts(raw: Option<&Value>) -> SetupPrompts {
    let Some(raw) = raw.filter(|raw| raw.is_object()) else {
        return SetupPrompts::default();
    };
    let mut ids: Vec<i32> = raw
        .get("migrationDismissed")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_i64)
                .filter(|id| *id > 0)
                .filter_map(|id| i32::try_from(id).ok())
                .collect()
        })
        .unwrap_or_default();
    ids.sort_unstable();
    ids.dedup();
    SetupPrompts {
        plugin_dismissed: is_true(raw, "pluginDismissed"),
        migration_dismissed: ids,
    }
}

/// Steht unter dem Schlüssel genau `true`? Alles andere gilt wie in TypeScript als `false`.
fn is_true(raw: &Value, key: &str) -> bool {
    raw.get(key) == Some(&Value::Bool(true))
}

/// Text unter dem Schlüssel (None, wenn dort kein Text steht)
fn text(raw: &Value, key: &str) -> Option<String> {
    raw.get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Benutzerordner und Einstellungen darin
    fn store() -> (tempfile::TempDir, UserSettingsStore) {
        let dir = tempfile::Builder::new()
            .prefix("harness-settings-")
            .tempdir()
            .unwrap();
        let store = UserSettingsStore::new(HarnessHome::at(dir.path()));
        (dir, store)
    }

    /// Schreibt rohen JSON-Text in die Einstellungsdatei
    fn write_raw(dir: &tempfile::TempDir, content: &str) {
        fs::write(dir.path().join("settings.json"), content).unwrap();
    }

    #[test]
    fn liefert_ohne_datei_die_standardwerte() {
        let (_dir, store) = store();
        assert_eq!(store.read(), UserSettings::default());
    }

    #[test]
    fn speichert_und_liest_die_einstellungen() {
        let (_dir, store) = store();
        let settings = UserSettings {
            notifications: NotificationMode::Windows,
            hero_images: true,
            theme: ThemeMode::Dark,
            claude_usage: true,
            compact_cards: true,
            hide_not_needed: true,
            hide_in_planning: true,
            ntfy: true,
            ntfy_server: "https://push.example".into(),
            ntfy_topic: "harness-x".into(),
            ntfy_token: "tk_1".into(),
            setup_prompts: SetupPrompts {
                plugin_dismissed: true,
                migration_dismissed: vec![2, 7],
            },
        };

        store.write(&settings).unwrap();

        assert_eq!(store.read(), settings);
    }

    #[test]
    fn schreibt_json_mit_zwei_leerzeichen_wie_typescript() {
        let (dir, store) = store();
        store.write(&UserSettings::default()).unwrap();

        let text = fs::read_to_string(dir.path().join("settings.json")).unwrap();

        assert!(
            text.starts_with("{\n  \"notifications\": \"off\","),
            "{text}"
        );
        assert!(text.ends_with('}'), "{text}");
    }

    #[test]
    fn ubernimmt_den_fruheren_schalter_als_browser_meldung() {
        let (dir, store) = store();
        write_raw(&dir, r#"{"notifyOnWaiting":true}"#);

        assert_eq!(store.read().notifications, NotificationMode::Browser);
    }

    #[test]
    fn ersetzt_ungultige_werte_durch_die_standardwerte() {
        let (dir, store) = store();
        write_raw(&dir, r#"{"notifications":"laut","snoozeMinutes":7}"#);

        assert_eq!(store.read(), UserSettings::default());
    }

    #[test]
    fn fallt_bei_beschadigter_datei_auf_die_standardwerte_zuruck() {
        let (dir, store) = store();
        write_raw(&dir, "{kaputt");

        assert_eq!(store.read(), UserSettings::default());
    }

    #[test]
    fn ersetzt_ein_unbekanntes_theme_durch_system() {
        let (dir, store) = store();
        write_raw(
            &dir,
            r#"{"notifications":"off","heroImages":false,"theme":"lila"}"#,
        );

        assert_eq!(store.read().theme, ThemeMode::System);
    }

    #[test]
    fn ersetzt_einen_ungultigen_ntfy_server_und_ein_ungultiges_thema() {
        let (dir, store) = store();
        write_raw(
            &dir,
            r#"{"ntfy":true,"ntfyServer":"ftp://x","ntfyTopic":"mit leerzeichen","ntfyToken":7}"#,
        );

        let settings = store.read();

        assert!(settings.ntfy);
        assert_eq!(settings.ntfy_server, DEFAULT_NTFY_SERVER);
        assert_eq!(settings.ntfy_topic, "");
        assert_eq!(settings.ntfy_token, "");
    }

    #[test]
    fn liest_die_abgewiesenen_setup_dialoge() {
        let (dir, store) = store();
        write_raw(
            &dir,
            r#"{"setupPrompts":{"pluginDismissed":true,"migrationDismissed":[3,1,3,"x",-2]}}"#,
        );

        let settings = store.read();

        assert!(settings.setup_prompts.plugin_dismissed);
        assert_eq!(settings.setup_prompts.migration_dismissed, vec![1, 3]);
    }

    #[test]
    fn ohne_gueltige_setup_dialoge_ist_nichts_abgewiesen() {
        let (dir, store) = store();
        write_raw(&dir, r#"{"setupPrompts":"kaputt"}"#);

        assert_eq!(store.read().setup_prompts, SetupPrompts::default());
    }
}
