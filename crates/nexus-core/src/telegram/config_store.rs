//! Liest und schreibt `~/.nexus/telegram.yaml`. Die Datei enthält Geheimnisse, deshalb
//! schreibt der Store sie atomar und unter Unix nur für den Benutzer lesbar (`0600`).

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;
use crate::telegram::config::TelegramConfig;

/// Liest und schreibt die Einstellung des Telegram-Bots.
#[derive(Debug, Clone)]
pub struct TelegramConfigStore {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl TelegramConfigStore {
    /// Store im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Pfad der Datei (für Meldungen)
    pub fn file(&self) -> PathBuf {
        self.home.telegram_config_file()
    }

    /// Liest die Einstellung; ohne Datei gelten die Vorgaben
    pub fn read(&self) -> Result<TelegramConfig> {
        let file = self.file();
        if !file.exists() {
            return Ok(TelegramConfig::default());
        }
        let text = fs::read_to_string(&file).map_err(Error::unexpected)?;
        let raw: Value = serde_yaml_ng::from_str(&text).map_err(|_| {
            HarnessError::new(
                "telegram-config-invalid",
                format!("{} ist kein gültiges YAML.", file.display()),
            )
        })?;
        TelegramConfig::from_value(&raw)
    }

    /// Prüft und schreibt atomar; liefert die geschriebene Einstellung
    pub fn write(&self, config: &TelegramConfig) -> Result<TelegramConfig> {
        let config = config.clone().normalized()?;
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let yaml = serde_yaml_ng::to_string(&config).map_err(Error::unexpected)?;
        self.writer.write_private(&self.file(), &yaml)?;
        Ok(config)
    }

    /// Übernimmt eine Eingabe der Oberfläche auf die gespeicherte Einstellung und schreibt sie;
    /// liefert die neue Einstellung (unmaskiert)
    pub fn save_update(&self, input: &Value) -> Result<TelegramConfig> {
        let merged = self.read()?.merge_update(input)?;
        self.write(&merged)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::telegram::config::{IdeaTarget, QuietHours, SpeechBackend};

    /// Ein Token, das dem Muster von @BotFather entspricht
    const TOKEN: &str = "123456:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw";

    /// Benutzerordner und Store darin
    fn store() -> (tempfile::TempDir, TelegramConfigStore) {
        let dir = tempfile::Builder::new()
            .prefix("harness-telegram-")
            .tempdir()
            .unwrap();
        let store = TelegramConfigStore::new(HarnessHome::at(dir.path()));
        (dir, store)
    }

    #[test]
    fn liefert_ohne_datei_die_vorgaben() {
        let (_dir, store) = store();
        assert_eq!(store.read().unwrap(), TelegramConfig::default());
    }

    #[test]
    fn schreibt_und_liest_wieder() {
        let (dir, store) = store();
        let config = TelegramConfig {
            enabled: true,
            bot_token: TOKEN.to_string(),
            chats: vec![123456789],
            quiet_hours: Some(QuietHours {
                from: "22:00".into(),
                to: "07:00".into(),
            }),
            ..TelegramConfig::default()
        };

        let saved = store.write(&config).unwrap();

        assert_eq!(saved, config);
        let text = fs::read_to_string(dir.path().join("telegram.yaml")).unwrap();
        assert!(text.contains("enabled: true"), "{text}");
        assert!(text.contains("botToken:"), "{text}");
        assert_eq!(store.read().unwrap(), config);
    }

    #[test]
    fn schreibt_die_felder_in_camelcase() {
        let (dir, store) = store();
        store.write(&TelegramConfig::default()).unwrap();

        let text = fs::read_to_string(dir.path().join("telegram.yaml")).unwrap();

        assert!(text.contains("apiBase: https://api.telegram.org"), "{text}");
        assert!(text.contains("defaultTarget: note"), "{text}");
        assert!(text.contains("backend: openai-compatible"), "{text}");
        assert!(text.contains("whisperCli:"), "{text}");
    }

    #[test]
    fn liest_eine_von_hand_geschriebene_datei() {
        let (dir, store) = store();
        fs::write(
            dir.path().join("telegram.yaml"),
            concat!(
                "enabled: true\n",
                "chats: [42]\n",
                "ideas: { defaultTarget: ticket, maxRounds: 3 }\n",
                "speech:\n",
                "  backend: whisper-cli\n",
                "  language: en\n",
            ),
        )
        .unwrap();

        let config = store.read().unwrap();

        assert!(config.enabled);
        assert_eq!(config.chats, vec![42]);
        assert_eq!(config.ideas.default_target, IdeaTarget::Ticket);
        assert_eq!(config.ideas.max_rounds, 3);
        assert_eq!(config.speech.backend, SpeechBackend::WhisperCli);
        assert_eq!(config.speech.language.as_deref(), Some("en"));
        // Nicht genannte Felder der Spracherkennung bleiben bei der Vorgabe
        assert_eq!(config.speech.model, "gpt-4o-mini-transcribe");
    }

    #[test]
    fn meldet_kaputtes_yaml() {
        let (dir, store) = store();
        fs::write(dir.path().join("telegram.yaml"), "chats: [").unwrap();

        let error = store.read().unwrap_err();

        assert_eq!(error.code(), Some("telegram-config-invalid"));
    }

    #[test]
    fn schreibt_ungultiges_nicht() {
        let (dir, store) = store();
        let error = store
            .write(&TelegramConfig {
                bot_token: "kaputt".into(),
                ..TelegramConfig::default()
            })
            .unwrap_err();

        assert_eq!(error.code(), Some("telegram-config-invalid"));
        assert!(!dir.path().join("telegram.yaml").exists());
    }

    #[test]
    fn speichert_eine_eingabe_der_oberflache_ohne_die_geheimnisse_zu_verlieren() {
        let (_dir, store) = store();
        store
            .write(&TelegramConfig {
                bot_token: TOKEN.to_string(),
                chats: vec![7],
                ..TelegramConfig::default()
            })
            .unwrap();

        let masked = store.read().unwrap().masked();
        let saved = store
            .save_update(&json!({ "enabled": true, "botToken": masked.bot_token, "chats": [] }))
            .unwrap();

        assert!(saved.enabled);
        assert_eq!(saved.bot_token, TOKEN);
        assert_eq!(saved.chats, vec![7]);
        assert_eq!(store.read().unwrap(), saved);
    }

    #[cfg(unix)]
    #[test]
    fn schreibt_die_datei_nur_fur_den_benutzer() {
        use std::os::unix::fs::PermissionsExt;

        let (dir, store) = store();
        store.write(&TelegramConfig::default()).unwrap();

        let mode = fs::metadata(dir.path().join("telegram.yaml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{mode:o}");
    }
}
