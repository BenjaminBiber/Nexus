//! Pfade eines Claude-Accounts, wie Claude Code sie bildet; portiert aus
//! `packages/core/src/accounts/AccountPaths.ts`.
//!
//! Der Standard-Account folgt der Umgebung des Servers (`CLAUDE_CONFIG_DIR`, falls gesetzt,
//! sonst `~/.claude`); jeder andere seinem eigenen Ordner.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::install::plugin_switch::{LEGACY_PLUGIN_ID, PLUGIN_ID};

/// Dienstname, unter dem Claude Code sein Token im macOS-Schlüsselbund ablegt
/// (ohne `CLAUDE_CONFIG_DIR`)
const KEYCHAIN_BASE: &str = "Claude Code-credentials";

/// Name des Plugins in `plugins/installed_plugins.json` (vor dem @ steht der Plugin-Name)
fn plugin_names() -> [&'static str; 2] {
    [name_of(PLUGIN_ID), name_of(LEGACY_PLUGIN_ID)]
}

/// Der Plugin-Name vor dem `@`
fn name_of(id: &str) -> &str {
    id.split('@').next().unwrap_or(id)
}

/// Dateien eines Accounts, aus denen Anmeldung, Identität und Plugin gelesen werden
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountFiles {
    /// Wirksamer Konfigurationsordner von Claude Code
    pub config_dir: PathBuf,
    /// Wert für `CLAUDE_CONFIG_DIR`; `None` = die Variable der Umgebung des Servers bleibt,
    /// wie sie ist
    pub override_dir: Option<PathBuf>,
    /// `.credentials.json` (Windows, Linux)
    pub credentials_file: PathBuf,
    /// `.claude.json` mit `oauthAccount`
    pub identity_file: PathBuf,
    /// Dienstname im macOS-Schlüsselbund
    pub keychain_service: String,
    /// Plugin-Liste von Claude Code
    pub plugins_file: PathBuf,
}

/// Bildet die Pfade eines Claude-Accounts.
#[derive(Debug, Clone)]
pub struct AccountPaths {
    env: HashMap<String, String>,
    home: PathBuf,
}

impl AccountPaths {
    /// Pfade aus einer Umgebung und einem Benutzerordner (für Tests)
    pub fn new(env: HashMap<String, String>, home: impl Into<PathBuf>) -> Self {
        Self {
            env,
            home: home.into(),
        }
    }

    /// Pfade aus der Umgebung des Prozesses und dem Benutzerordner des Rechners
    pub fn from_process_env() -> Self {
        #[allow(deprecated)]
        let home = std::env::home_dir().unwrap_or_default();
        Self::new(std::env::vars().collect(), home)
    }

    /// `~` bzw. `~/…` zum Benutzerordner auflösen
    pub fn expand(&self, path: &str) -> PathBuf {
        if path == "~" {
            return self.home.clone();
        }
        if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
            return self.home.join(rest);
        }
        PathBuf::from(path)
    }

    /// Die Dateien dieses Accounts; `config_dir` ist der eingegebene Ordner (`None` = Standard)
    pub fn files(&self, config_dir: Option<&str>) -> AccountFiles {
        let override_dir = config_dir
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
            .map(|dir| self.expand(dir));
        let server_dir = self
            .env
            .get("CLAUDE_CONFIG_DIR")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from);
        let dir = override_dir.clone().or(server_dir);
        let config_dir = dir.clone().unwrap_or_else(|| self.home.join(".claude"));
        AccountFiles {
            credentials_file: config_dir.join(".credentials.json"),
            // Ohne eigenen Ordner liegt `.claude.json` neben `~/.claude`, nicht darin
            identity_file: match &dir {
                Some(dir) => dir.join(".claude.json"),
                None => self.home.join(".claude.json"),
            },
            keychain_service: keychain_service(dir.as_deref()),
            plugins_file: config_dir.join("plugins").join("installed_plugins.json"),
            config_dir,
            override_dir,
        }
    }
}

/// Schlüsselbund-Dienst: ohne Ordner der feste Name, sonst mit den ersten 8 Hex-Zeichen von
/// SHA-256 des Ordners (wie Claude Code)
pub fn keychain_service(config_dir: Option<&Path>) -> String {
    let Some(dir) = config_dir else {
        return KEYCHAIN_BASE.to_string();
    };
    let digest = Sha256::digest(dir.to_string_lossy().as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{KEYCHAIN_BASE}-{}", &hex[..8])
}

/// Steht das Nexus-Plugin (oder sein alter Name) in der Plugin-Liste?
/// (Inhalt von `installed_plugins.json`)
pub fn has_plugin(installed_plugins_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(installed_plugins_json) else {
        return false;
    };
    let Some(plugins) = value.get("plugins").and_then(serde_json::Value::as_object) else {
        return false;
    };
    plugins
        .keys()
        .any(|key| plugin_names().contains(&name_of(key)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/ich";

    fn paths(env: &[(&str, &str)]) -> AccountPaths {
        AccountPaths::new(
            env.iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect(),
            HOME,
        )
    }

    #[test]
    fn standard_ohne_claude_config_dir_nutzt_punkt_claude_und_den_festen_schluesselbund() {
        let files = paths(&[]).files(None);
        assert_eq!(files.config_dir, Path::new(HOME).join(".claude"));
        assert_eq!(files.override_dir, None);
        assert_eq!(
            files.credentials_file,
            Path::new(HOME).join(".claude").join(".credentials.json")
        );
        assert_eq!(files.identity_file, Path::new(HOME).join(".claude.json"));
        assert_eq!(files.keychain_service, "Claude Code-credentials");
        assert_eq!(
            files.plugins_file,
            Path::new(HOME)
                .join(".claude")
                .join("plugins")
                .join("installed_plugins.json")
        );
    }

    #[test]
    fn standard_folgt_dem_claude_config_dir_des_servers_ohne_es_selbst_zu_setzen() {
        let files = paths(&[("CLAUDE_CONFIG_DIR", "/srv/claude")]).files(None);
        assert_eq!(files.config_dir, Path::new("/srv/claude"));
        assert_eq!(files.override_dir, None);
        assert_eq!(
            files.identity_file,
            Path::new("/srv/claude").join(".claude.json")
        );
    }

    #[test]
    fn eigener_ordner_loest_die_tilde_auf_und_bekommt_einen_eigenen_schluesselbund() {
        let files =
            paths(&[("CLAUDE_CONFIG_DIR", "/srv/claude")]).files(Some("~/.mcacc/max/claude"));
        let dir = Path::new(HOME).join(".mcacc/max/claude");
        assert_eq!(files.config_dir, dir);
        assert_eq!(files.override_dir, Some(dir.clone()));
        assert_eq!(files.identity_file, dir.join(".claude.json"));
        assert_eq!(files.keychain_service, keychain_service(Some(&dir)));
        assert!(files
            .keychain_service
            .starts_with("Claude Code-credentials-"));
        assert_eq!(
            files.keychain_service.len(),
            "Claude Code-credentials".len() + 9
        );
    }

    #[test]
    fn die_tilde_allein_ist_der_benutzerordner() {
        assert_eq!(paths(&[]).expand("~"), Path::new(HOME));
        assert_eq!(paths(&[]).expand("/woanders"), Path::new("/woanders"));
    }

    #[test]
    fn erkennt_das_nexus_plugin_und_seinen_alten_namen() {
        assert!(has_plugin(r#"{"plugins":{"nexus@nexus-local":[]}}"#));
        assert!(has_plugin(
            r#"{"plugins":{"ai-harness@ai-harness-local":[]}}"#
        ));
        assert!(!has_plugin(r#"{"plugins":{"other@x":[]}}"#));
        assert!(!has_plugin("{kaputt"));
    }
}
