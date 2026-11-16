//! Abgleich des Claude-Themes: legt den Mod des aktiven Themes in den Slot
//! `<Claude-Konfiguration>/skills/nexus-theme/` oder räumt ihn weg.
//!
//! Soll: Einstellung `claudeTheme` an, aktives Theme installiert und mit `claudeTheme: true` im
//! Manifest → `apply`, sonst `clear`. Ein fremder Ordner bleibt unberührt. Ein Mutex hält die
//! Läufe nacheinander, das Dateisystem läuft in `spawn_blocking`. Fehler lassen die auslösende
//! Anfrage nie scheitern; sie landen im Server-Log und im Status (`GET /api/claude-theme`).

use std::path::Path;
use std::sync::Mutex;

use nexus_core::registry::NexusHome;
use nexus_core::settings::UserSettingsStore;
use nexus_core::themes::{
    list_themes, resolve_settings, ClaudeSlotError, ClaudeThemeSlot, ClaudeThemeState,
    ClaudeThemeStatus, ThemeSettingsStore,
};

use crate::log::ServerLog;

/// Hält den Slot des Claude-Themes im Einklang mit dem aktiven Theme
pub struct ClaudeThemeSync {
    home: NexusHome,
    slot: ClaudeThemeSlot,
    log: ServerLog,
    /// Hält die Abgleiche nacheinander
    run: tokio::sync::Mutex<()>,
    /// Ergebnis des letzten Abgleichs; `None` vor dem ersten
    last: Mutex<Option<ClaudeThemeStatus>>,
}

impl ClaudeThemeSync {
    /// Abgleich für den Konfigurationsordner von Claude Code `config_dir`
    pub fn new(home: NexusHome, config_dir: &Path, log: ServerLog) -> Self {
        Self {
            home,
            slot: ClaudeThemeSlot::for_config_dir(config_dir),
            log,
            run: tokio::sync::Mutex::new(()),
            last: Mutex::new(None),
        }
    }

    /// Pfad des Slots
    pub fn slot_dir(&self) -> &Path {
        self.slot.dir()
    }

    /// Gleicht den Slot ab und liefert den neuen Status; scheitert nie
    pub async fn reconcile(&self) -> ClaudeThemeStatus {
        let _turn = self.run.lock().await;
        let home = self.home.clone();
        let slot = self.slot.clone();
        let (status, line) =
            match tokio::task::spawn_blocking(move || reconcile_blocking(&home, &slot)).await {
                Ok(result) => result,
                Err(error) => {
                    let message = format!("Abgleich abgebrochen: {error}");
                    let status = ClaudeThemeStatus {
                        enabled: true,
                        state: ClaudeThemeState::Error,
                        theme_id: None,
                        theme_name: None,
                        path: self.slot.dir().display().to_string(),
                        message: Some(message.clone()),
                    };
                    (
                        status,
                        Some(format!("Claude-Theme nicht abgeglichen: {message}")),
                    )
                }
            };
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Ein Fehler, der bleibt (fremder Ordner …), steht nur einmal im Log
        let repeated = matches!(
            status.state,
            ClaudeThemeState::Error | ClaudeThemeState::Foreign
        ) && last.as_ref().is_some_and(|previous| {
            previous.state == status.state && previous.message == status.message
        });
        if let Some(line) = line.filter(|_| !repeated) {
            self.log.write(&line);
        }
        *last = Some(status.clone());
        status
    }

    /// Gleicht ab, wenn `id` das aktive Theme ist (Einstellungen, Installation, Update)
    pub async fn reconcile_if_active(&self, id: &str) {
        let home = self.home.clone();
        let active = tokio::task::spawn_blocking(move || {
            UserSettingsStore::new(home)
                .read()
                .theme
                .as_str()
                .to_string()
        })
        .await
        .unwrap_or_default();
        if active == id {
            self.reconcile().await;
        }
    }

    /// Status des letzten Abgleichs; vor dem ersten wird abgeglichen
    pub async fn status(&self) -> ClaudeThemeStatus {
        let last = self
            .last
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        match last {
            Some(status) => status,
            None => self.reconcile().await,
        }
    }
}

/// Ein Abgleich: neuer Status und, wenn sich etwas getan hat, eine Zeile fürs Server-Log
fn reconcile_blocking(
    home: &NexusHome,
    slot: &ClaudeThemeSlot,
) -> (ClaudeThemeStatus, Option<String>) {
    let settings = UserSettingsStore::new(home.clone()).read();
    let path = slot.dir().display().to_string();
    let mut status = ClaudeThemeStatus {
        enabled: settings.claude_theme,
        state: ClaudeThemeState::None,
        theme_id: None,
        theme_name: None,
        path: path.clone(),
        message: None,
    };
    let active = match list_themes(home) {
        Ok(themes) => themes
            .into_iter()
            .find(|theme| theme.manifest.id == settings.theme.as_str()),
        Err(error) => return failed(status, format!("Installierte Themes nicht lesbar: {error}")),
    };
    if let Some(theme) = &active {
        status.theme_id = Some(theme.manifest.id.clone());
        status.theme_name = Some(theme.manifest.name.clone());
    }

    let wanted = active.filter(|theme| settings.claude_theme && theme.manifest.claude_theme);
    let Some(theme) = wanted else {
        return match slot.clear() {
            Ok(true) => (status, Some(format!("Claude-Theme aus {path} entfernt"))),
            // Kein Slot oder ein fremder Ordner: nichts zu tun, der fremde bleibt, wie er ist
            Ok(false) | Err(ClaudeSlotError::Foreign { .. }) => (status, None),
            Err(error) => failed(status, error.to_string()),
        };
    };

    let stored = match ThemeSettingsStore::new(home.clone()).get(&theme.manifest.id) {
        Ok(values) => values,
        Err(error) => {
            return failed(
                status,
                format!("Einstellungen des Themes nicht lesbar: {error}"),
            )
        }
    };
    let values = resolve_settings(theme.manifest.settings_schema.as_ref(), &stored);
    match slot.apply(&theme.dir, &theme.manifest, &values) {
        Ok(outcome) => {
            status.state = ClaudeThemeState::Active;
            let changed = outcome.written + outcome.removed > 0;
            let line = changed.then(|| {
                format!(
                    "Claude-Theme {} {} in {path} abgelegt ({} Dateien geschrieben, {} entfernt)",
                    theme.manifest.id, theme.manifest.version, outcome.written, outcome.removed
                )
            });
            (status, line)
        }
        Err(error @ ClaudeSlotError::Foreign { .. }) => {
            let message = error.to_string();
            status.state = ClaudeThemeState::Foreign;
            status.message = Some(message.clone());
            (
                status,
                Some(format!("Claude-Theme nicht abgelegt: {message}")),
            )
        }
        Err(error) => failed(status, error.to_string()),
    }
}

/// Status `error` mit Meldung und Log-Zeile
fn failed(mut status: ClaudeThemeStatus, message: String) -> (ClaudeThemeStatus, Option<String>) {
    status.state = ClaudeThemeState::Error;
    status.message = Some(message.clone());
    (
        status,
        Some(format!("Claude-Theme nicht abgeglichen: {message}")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::settings::UserSettings;
    use std::fs;
    use tempfile::TempDir;

    /// Benutzerordner mit einem installierten Theme `id` (mit oder ohne Claude-Theme)
    fn install(home: &NexusHome, id: &str, claude: bool) {
        let dir = home.themes_dir().join(id);
        fs::create_dir_all(dir.join("claude/hooks")).unwrap();
        let mut manifest = serde_json::json!({
            "id": id,
            "name": format!("Theme {id}"),
            "version": "1.0.0",
            "sdkVersion": "^1.3",
            "author": "Test",
            "colorScheme": "dark",
            "settingsSchema": {
                "palette": {
                    "type": "select",
                    "label": "Palette",
                    "default": "a",
                    "options": [{ "value": "a", "label": "A" }, { "value": "b", "label": "B" }]
                }
            }
        });
        if claude {
            manifest["claudeTheme"] = serde_json::json!(true);
            fs::write(
                dir.join("claude/hooks/hooks.json"),
                r#"{ "modules": ["./register.tsx"] }"#,
            )
            .unwrap();
            fs::write(
                dir.join("claude/hooks/register.tsx"),
                "export const register = () => {}\n",
            )
            .unwrap();
        }
        fs::write(
            dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(dir.join("theme.css"), ":root {}\n").unwrap();
    }

    fn settings(home: &NexusHome, theme: &str, claude_theme: bool) {
        let store = UserSettingsStore::new(home.clone());
        let mut next: UserSettings = store.read();
        next.theme = theme.into();
        next.claude_theme = claude_theme;
        store.write(&next).unwrap();
    }

    struct Fixture {
        _dir: TempDir,
        home: NexusHome,
        config: std::path::PathBuf,
        sync: ClaudeThemeSync,
    }

    fn fixture() -> Fixture {
        let dir = TempDir::new().unwrap();
        let home = NexusHome::at(dir.path().join("nexus"));
        let config = dir.path().join("claude-config");
        fs::create_dir_all(&config).unwrap();
        let sync = ClaudeThemeSync::new(home.clone(), &config, ServerLog::new(&home));
        Fixture {
            _dir: dir,
            home,
            config,
            sync,
        }
    }

    fn slot(fixture: &Fixture) -> std::path::PathBuf {
        fixture.config.join("skills").join("nexus-theme")
    }

    #[tokio::test]
    async fn aktives_theme_mit_claude_theme_landet_im_slot() {
        let f = fixture();
        install(&f.home, "neon", true);
        settings(&f.home, "neon", true);

        let status = f.sync.reconcile().await;

        assert_eq!(status.state, ClaudeThemeState::Active);
        assert_eq!(status.theme_id.as_deref(), Some("neon"));
        assert_eq!(status.theme_name.as_deref(), Some("Theme neon"));
        assert!(slot(&f).join("hooks/register.tsx").is_file());
        assert!(slot(&f).join(".claude-plugin/plugin.json").is_file());
        let module = fs::read_to_string(slot(&f).join("hooks/nexus-settings.ts")).unwrap();
        assert!(module.contains(r#"{"palette":"a"}"#), "{module}");
        let log = fs::read_to_string(f.sync.log.file()).unwrap();
        assert!(log.contains("Claude-Theme neon 1.0.0"), "{log}");
    }

    #[tokio::test]
    async fn gespeicherte_theme_einstellungen_gelten_im_slot() {
        let f = fixture();
        install(&f.home, "neon", true);
        settings(&f.home, "neon", true);
        let values = serde_json::json!({ "palette": "b", "fremd": true });
        ThemeSettingsStore::new(f.home.clone())
            .put("neon", values.as_object().unwrap().clone())
            .unwrap();

        f.sync.reconcile_if_active("neon").await;

        let module = fs::read_to_string(slot(&f).join("hooks/nexus-settings.ts")).unwrap();
        assert!(module.contains(r#"{"palette":"b"}"#), "{module}");
    }

    #[tokio::test]
    async fn schalter_aus_raeumt_den_slot_weg() {
        let f = fixture();
        install(&f.home, "neon", true);
        settings(&f.home, "neon", true);
        f.sync.reconcile().await;

        settings(&f.home, "neon", false);
        let status = f.sync.reconcile().await;

        assert_eq!(status.state, ClaudeThemeState::None);
        assert!(!status.enabled);
        assert!(!slot(&f).exists());
    }

    #[tokio::test]
    async fn theme_ohne_claude_theme_oder_eingebautes_raeumt_den_slot_weg() {
        let f = fixture();
        install(&f.home, "neon", true);
        install(&f.home, "plain", false);
        settings(&f.home, "neon", true);
        f.sync.reconcile().await;

        settings(&f.home, "plain", true);
        let status = f.sync.reconcile().await;
        assert_eq!(status.state, ClaudeThemeState::None);
        assert_eq!(status.theme_id.as_deref(), Some("plain"));
        assert!(!slot(&f).exists());

        settings(&f.home, "neon", true);
        f.sync.reconcile().await;
        settings(&f.home, "dark", true);
        let status = f.sync.reconcile().await;
        assert_eq!(status.state, ClaudeThemeState::None);
        assert_eq!(status.theme_id, None);
        assert!(!slot(&f).exists());
    }

    #[tokio::test]
    async fn entferntes_aktives_theme_raeumt_den_slot_weg() {
        let f = fixture();
        install(&f.home, "neon", true);
        settings(&f.home, "neon", true);
        f.sync.reconcile().await;

        fs::remove_dir_all(f.home.themes_dir().join("neon")).unwrap();
        let status = f.sync.reconcile().await;

        assert_eq!(status.state, ClaudeThemeState::None);
        assert!(!slot(&f).exists());
    }

    #[tokio::test]
    async fn fremder_ordner_bleibt_und_meldet_foreign() {
        let f = fixture();
        install(&f.home, "neon", true);
        settings(&f.home, "neon", true);
        fs::create_dir_all(slot(&f)).unwrap();
        fs::write(slot(&f).join("eigenes.txt"), "meins").unwrap();

        let status = f.sync.reconcile().await;
        assert_eq!(status.state, ClaudeThemeState::Foreign);
        assert!(status.message.unwrap().contains("gehört nicht Nexus"));
        assert_eq!(
            fs::read_to_string(slot(&f).join("eigenes.txt")).unwrap(),
            "meins"
        );
        assert!(!slot(&f).join("hooks").exists());

        // Ohne Claude-Theme wird der fremde Ordner nicht angefasst und ist kein Fehler
        settings(&f.home, "neon", false);
        let status = f.sync.reconcile().await;
        assert_eq!(status.state, ClaudeThemeState::None);
        assert!(slot(&f).join("eigenes.txt").is_file());

        // Die Meldung steht nur einmal im Log
        settings(&f.home, "neon", true);
        f.sync.reconcile().await;
        f.sync.reconcile().await;
        let log = fs::read_to_string(f.sync.log.file()).unwrap();
        assert_eq!(log.matches("gehört nicht Nexus").count(), 2, "{log}");
    }

    #[tokio::test]
    async fn status_gleicht_beim_ersten_aufruf_ab() {
        let f = fixture();
        install(&f.home, "neon", true);
        settings(&f.home, "neon", true);

        let status = f.sync.status().await;

        assert_eq!(status.state, ClaudeThemeState::Active);
        assert!(slot(&f).join("hooks/hooks.json").is_file());
        assert_eq!(status.path, slot(&f).display().to_string());
    }

    #[tokio::test]
    async fn reconcile_if_active_ignoriert_andere_themes() {
        let f = fixture();
        install(&f.home, "neon", true);
        install(&f.home, "other", true);
        settings(&f.home, "neon", true);

        f.sync.reconcile_if_active("other").await;

        assert!(!slot(&f).exists());
    }
}
