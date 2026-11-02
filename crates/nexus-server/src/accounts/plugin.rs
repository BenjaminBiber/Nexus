//! Richtet das Nexus-Plugin im Ordner eines Claude-Accounts ein; portiert aus
//! `packages/server/src/usage/AccountPluginInstaller.ts`.
//!
//! Statt die Befehlsfolge noch einmal zu bauen, läuft die vorhandene Einrichtung
//! ([`ClaudePluginInstall`]) mit dem `CLAUDE_CONFIG_DIR` des Accounts: lokalen Marktplatz
//! anmelden, Plugin installieren und auf Benutzerebene ausschalten – eingeschaltet wird es je
//! Projekt über `.claude/settings.local.json`, das für jeden Account gilt.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use nexus_core::accounts::{AccountPaths, ClaudeAccount};
use nexus_core::install::claude_plugin::MARKETPLACE;
use nexus_core::install::plugin_switch::PLUGIN_ID;
use nexus_core::install::{ClaudeCli, ClaudeOutput, ClaudePluginInstall};
use nexus_core::{HarnessError, HarnessHome, Result};

/// Baut die Kommandozeile von Claude Code für den Ordner eines Accounts
pub type AccountCli =
    Arc<dyn Fn(&str, Option<&Path>) -> Arc<dyn ClaudeCli + Send + Sync> + Send + Sync>;

/// Richtet das Nexus-Plugin in Account-Ordnern ein.
pub struct AccountPluginInstaller {
    home: HarnessHome,
    paths: AccountPaths,
    /// Baut die Kommandozeile für einen Account-Ordner; Tests setzen eine Attrappe ein
    cli: AccountCli,
}

impl std::fmt::Debug for AccountPluginInstaller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountPluginInstaller")
            .finish_non_exhaustive()
    }
}

impl AccountPluginInstaller {
    /// Einrichtung mit dem echten `claude`
    pub fn new(home: HarnessHome, paths: AccountPaths) -> Self {
        Self::with_cli(
            home,
            paths,
            Arc::new(|command, dir| {
                Arc::new(AccountClaudeCli::new(command, dir)) as Arc<dyn ClaudeCli + Send + Sync>
            }),
        )
    }

    /// Einrichtung mit eigener Kommandozeile (Tests)
    pub fn with_cli(home: HarnessHome, paths: AccountPaths, cli: AccountCli) -> Self {
        Self { home, paths, cli }
    }

    /// Ordner des lokalen Marktplatzes: der mitgelieferte unter `~/.nexus/plugin`, sonst der,
    /// den der Standard-Account kennt; `None` = Plugin nie eingerichtet
    pub fn marketplace(&self) -> Option<PathBuf> {
        let staged = self.home.plugin_dir();
        if staged
            .join(".claude-plugin")
            .join("marketplace.json")
            .is_file()
        {
            return Some(staged);
        }
        let file = self
            .paths
            .files(None)
            .config_dir
            .join("plugins")
            .join("known_marketplaces.json");
        let text = std::fs::read_to_string(file).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        let entry = json.get(MARKETPLACE)?;
        let path = entry
            .get("source")
            .and_then(|source| source.get("path"))
            .or_else(|| entry.get("installLocation"))?
            .as_str()
            .filter(|path| !path.is_empty())?;
        Some(PathBuf::from(path))
    }

    /// Befehle zum Einrichten von Hand (ohne `CLAUDE_CONFIG_DIR` davor); leer ohne bekannten
    /// Marktplatz
    pub fn commands(&self) -> Vec<String> {
        let Some(marketplace) = self.marketplace() else {
            return Vec::new();
        };
        vec![
            format!(
                "claude plugin marketplace add \"{}\"",
                marketplace.display()
            ),
            format!("claude plugin install {PLUGIN_ID}"),
            format!("claude plugin disable {PLUGIN_ID} --scope user"),
        ]
    }

    /// Richtet das Plugin im Ordner des Accounts ein; scheitert es, `plugin-install-failed`
    /// mit der Ausgabe des gescheiterten Schritts
    pub fn install(&self, account: &ClaudeAccount, command: &str) -> Result<()> {
        let Some(marketplace) = self.marketplace() else {
            return Err(HarnessError::new(
                "plugin-install-failed",
                "Der lokale Marktplatz von Nexus ist nicht eingerichtet – zuerst die \
Einrichtung („Plugin installieren“) ausführen.",
            )
            .into());
        };
        let files = self.paths.files(account.config_dir.as_deref());
        let cli = (self.cli)(command, files.override_dir.as_deref());
        let result = ClaudePluginInstall::new(&*cli, Some(files.config_dir)).install(&marketplace);
        if !result.installed {
            let hints = result.hints.join(" ");
            return Err(HarnessError::new(
                "plugin-install-failed",
                format!(
                    "Das Nexus-Plugin ließ sich für Account „{}“ nicht einrichten: {}",
                    account.id,
                    if hints.is_empty() {
                        "Claude Code hat den Befehl nicht ausgeführt.".to_string()
                    } else {
                        hints
                    }
                ),
            )
            .into());
        }
        Ok(())
    }
}

/// `claude …` im Ordner eines Accounts: wie [`nexus_core::install::SystemClaudeCli`], nur mit
/// gesetztem `CLAUDE_CONFIG_DIR` und ohne die API-Schlüssel des Servers
#[derive(Debug, Clone)]
pub struct AccountClaudeCli {
    command: String,
    env: BTreeMap<String, String>,
}

impl AccountClaudeCli {
    /// Kommandozeile für diesen Account-Ordner (`None` = Umgebung des Servers)
    pub fn new(command: &str, config_dir: Option<&Path>) -> Self {
        Self {
            command: command.to_string(),
            env: account_env(&std::env::vars().collect(), config_dir),
        }
    }
}

impl ClaudeCli for AccountClaudeCli {
    fn run(&self, args: &[&str], cwd: Option<&Path>) -> ClaudeOutput {
        // Unter Windows ist `claude` eine Batchdatei, die nur `cmd` findet
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(&self.command).args(args);
            command
        } else {
            let mut command = Command::new(&self.command);
            command.args(args);
            command
        };
        command.env_clear().envs(&self.env).stdin(Stdio::null());
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
        nexus_core::process::hide_console(&mut command);
        let output = match std::process::Command::output(&mut command) {
            Ok(output) => output,
            Err(error) => {
                return ClaudeOutput {
                    ok: false,
                    out: error.to_string(),
                }
            }
        };
        ClaudeOutput {
            ok: output.status.success(),
            out: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }
}

/// Umgebung eines Account-Aufrufs: `CLAUDE_CONFIG_DIR` des Accounts, ohne API-Schlüssel und
/// ohne `CLAUDECODE` (die Anmeldung gehört zum Abo, nicht zu einem Schlüssel des Servers)
pub fn account_env(
    base: &BTreeMap<String, String>,
    config_dir: Option<&Path>,
) -> BTreeMap<String, String> {
    let mut env = base.clone();
    env.remove("ANTHROPIC_API_KEY");
    env.remove("ANTHROPIC_AUTH_TOKEN");
    env.remove("CLAUDECODE");
    if let Some(dir) = config_dir {
        env.insert(
            "CLAUDE_CONFIG_DIR".to_string(),
            dir.to_string_lossy().into_owned(),
        );
    }
    env
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn die_umgebung_setzt_den_ordner_und_entfernt_die_schluessel() {
        let base: BTreeMap<String, String> = [
            ("PATH".to_string(), "/bin".to_string()),
            ("ANTHROPIC_API_KEY".to_string(), "geheim".to_string()),
            ("CLAUDECODE".to_string(), "1".to_string()),
        ]
        .into_iter()
        .collect();

        let env = account_env(&base, Some(Path::new("/accounts/max")));

        assert_eq!(
            env.get("CLAUDE_CONFIG_DIR").map(String::as_str),
            Some("/accounts/max")
        );
        assert_eq!(env.get("PATH").map(String::as_str), Some("/bin"));
        assert!(!env.contains_key("ANTHROPIC_API_KEY"));
        assert!(!env.contains_key("CLAUDECODE"));
    }

    #[test]
    fn ohne_eigenen_ordner_bleibt_claude_config_dir_wie_es_ist() {
        let base: BTreeMap<String, String> =
            [("CLAUDE_CONFIG_DIR".to_string(), "/srv/claude".to_string())]
                .into_iter()
                .collect();

        let env = account_env(&base, None);

        assert_eq!(
            env.get("CLAUDE_CONFIG_DIR").map(String::as_str),
            Some("/srv/claude")
        );
    }
}
