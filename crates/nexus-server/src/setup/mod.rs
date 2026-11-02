//! Einrichtung aus der Oberfläche: Claude-Code-Plugin installieren und Projekte vom alten
//! AI Harness umstellen. Die Routen dazu stehen in [`crate::routes::setup`].

pub mod legacy_check;
pub mod migration;
pub mod plugin_install;
pub mod plugin_status;

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use nexus_core::install::{ClaudeCli, ClaudeImportApproval, ClaudeOutput, SystemClaudeCli};
use nexus_core::{HarnessError, Result};

/// Umgebung der Einrichtung: Kommandozeile von Claude Code, Claude-Ordner, Benutzerordner,
/// dazu die Sperre gegen gleichzeitige Läufe. Tests setzen Attrappe und Temp-Ordner ein.
#[derive(Clone)]
pub struct SetupContext {
    claude: Arc<dyn ClaudeCli + Send + Sync>,
    config_dir: Option<PathBuf>,
    user_home: PathBuf,
    found: Arc<OnceLock<bool>>,
    busy: Arc<AtomicBool>,
}

impl fmt::Debug for SetupContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetupContext")
            .field("config_dir", &self.config_dir)
            .field("user_home", &self.user_home)
            .finish_non_exhaustive()
    }
}

impl SetupContext {
    /// Umgebung mit ausdrücklich gesetzten Teilen; `config_dir` ist `CLAUDE_CONFIG_DIR`
    pub fn new(
        claude: Arc<dyn ClaudeCli + Send + Sync>,
        config_dir: Option<PathBuf>,
        user_home: PathBuf,
    ) -> Self {
        Self {
            claude,
            config_dir,
            user_home,
            found: Arc::new(OnceLock::new()),
            busy: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Umgebung des laufenden Prozesses: echtes `claude`, `CLAUDE_CONFIG_DIR`, Benutzerordner
    pub fn from_process_env() -> Self {
        let config_dir = std::env::var("CLAUDE_CONFIG_DIR")
            .ok()
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from);
        #[allow(deprecated)]
        let user_home = std::env::home_dir().unwrap_or_default();
        Self::new(Arc::new(SystemClaudeCli::new()), config_dir, user_home)
    }

    /// Kommandozeile von Claude Code
    pub fn claude(&self) -> &dyn ClaudeCli {
        &*self.claude
    }

    /// Ordner mit `plugins/installed_plugins.json`: `CLAUDE_CONFIG_DIR`, sonst `~/.claude`
    pub fn claude_dir(&self) -> PathBuf {
        self.config_dir
            .clone()
            .unwrap_or_else(|| self.user_home.join(".claude"))
    }

    /// Lief `claude --version`? Geprüft beim ersten Aufruf, gemerkt bis zum Neustart (blockiert)
    pub fn claude_found(&self) -> bool {
        *self
            .found
            .get_or_init(|| self.claude.run(&["--version"], None).ok)
    }

    /// Freigabe der Regel-Importe in `.claude.json` für diese Umgebung
    pub fn approval(&self) -> ClaudeImportApproval {
        ClaudeImportApproval::new(
            self.config_dir
                .as_deref()
                .map(|dir| dir.to_string_lossy().into_owned()),
            self.user_home.clone(),
            cfg!(windows),
        )
    }

    /// Beginnt einen Lauf; läuft schon einer, `setup-busy` (409)
    pub fn try_begin(&self) -> Result<RunGuard> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(HarnessError::new(
                "setup-busy",
                "Die Einrichtung läuft schon. Bitte warten, bis sie fertig ist.",
            )
            .into());
        }
        Ok(RunGuard {
            busy: self.busy.clone(),
        })
    }
}

/// Hält die Sperre eines Laufs; gibt sie beim Fallenlassen frei
#[derive(Debug)]
pub struct RunGuard {
    busy: Arc<AtomicBool>,
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        self.busy.store(false, Ordering::SeqCst);
    }
}

/// Kommandozeile, die es nicht gibt: jeder Aufruf scheitert (Tests ohne Claude)
#[derive(Debug, Clone, Copy, Default)]
pub struct NoClaude;

impl ClaudeCli for NoClaude {
    fn run(&self, _args: &[&str], _cwd: Option<&Path>) -> ClaudeOutput {
        ClaudeOutput {
            ok: false,
            out: "claude nicht gefunden".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erlaubt_nur_einen_lauf_gleichzeitig() {
        let setup = SetupContext::new(Arc::new(NoClaude), None, PathBuf::from("/x"));
        let first = setup.try_begin().unwrap();
        let second = setup.try_begin().unwrap_err();
        assert_eq!(
            second.harness().map(|h| h.code.as_str()),
            Some("setup-busy")
        );
        drop(first);
        assert!(setup.try_begin().is_ok());
    }

    #[test]
    fn der_claude_ordner_folgt_claude_config_dir_sonst_dem_benutzerordner() {
        let user = PathBuf::from("/home/u");
        let eigene = SetupContext::new(Arc::new(NoClaude), Some(PathBuf::from("/c")), user.clone());
        assert_eq!(eigene.claude_dir(), PathBuf::from("/c"));
        let standard = SetupContext::new(Arc::new(NoClaude), None, user.clone());
        assert_eq!(standard.claude_dir(), user.join(".claude"));
    }

    #[test]
    fn ohne_claude_ist_claude_nicht_gefunden() {
        let setup = SetupContext::new(Arc::new(NoClaude), None, PathBuf::from("/x"));
        assert!(!setup.claude_found());
    }
}
