//! Einrichten von Rechner und Projekt: Rules im Projekt, Plugin-Schalter, Freigabe der Importe
//! in Claude Code, PATH-Eintrag des Benutzers, Startdateien und die Installation des Binaries.
//! Spiegelt `packages/core/src/install`.

pub mod claude_plugin;
pub mod cli_installer;
pub mod import_approval;
pub mod launcher_writer;
pub mod plugin_switch;
pub mod project_rule;
pub mod project_rules;
pub mod server_stopper;
pub mod shell_profile_path;
pub mod user_path;
// Nur unter Windows: der Eintrag läuft über ein PowerShell-Skript auf der Registry
#[cfg(windows)]
pub mod windows_user_path;

pub use claude_plugin::{
    ClaudeCli, ClaudeOutput, ClaudePluginInstall, PluginInstallResult, SystemClaudeCli, MARKETPLACE,
};
pub use cli_installer::{CliInstaller, InstallOptions, InstallResult};
pub use import_approval::{ApprovalResult, ApprovalStatus, ClaudeImportApproval};
pub use launcher_writer::{LauncherWriter, BINARY_NAME};
pub use plugin_switch::{ProjectPluginSwitch, SwitchResult, LEGACY_PLUGIN_ID, PLUGIN_ID};
pub use project_rule::{
    ProjectRule, ProjectRuleSpec, BOARD_RULE_PATH, DOCS_RULE_PATH, LEGACY_BOARD_RULE_PATH,
    LEGACY_DOCS_RULE_PATH, LEGACY_RULE_MARKER, RULE_MARKER,
};
pub use project_rules::{ProjectRules, RulesResult, PROJECT_RULE_PATHS};
pub use server_stopper::{ServerStopper, StopDeps};
pub use shell_profile_path::ShellProfilePath;
pub use user_path::UserPath;
#[cfg(windows)]
pub use windows_user_path::WindowsUserPath;

/// Der PATH-Eintrag, der auf dieser Plattform gilt: Registry unter Windows, sonst die
/// Shell-Profile
pub fn platform_user_path() -> Box<dyn UserPath> {
    #[cfg(windows)]
    {
        Box::new(WindowsUserPath::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(ShellProfilePath::new())
    }
}
