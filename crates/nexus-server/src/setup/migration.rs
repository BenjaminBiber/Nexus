//! Stellt ein Repo vom alten AI Harness auf Nexus um: dieselben Schritte wie `nexus init`.

use std::path::Path;

use nexus_core::install::{
    ApprovalStatus, ClaudeImportApproval, ProjectPluginSwitch, ProjectRules,
};
use nexus_core::{HarnessHome, HarnessProject, Result};
use serde::Serialize;

/// Antwort von `POST /api/db/boards/{id}/migration`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationResult {
    /// Neu geschriebene Regeln
    pub written: Vec<String>,
    /// Gelöschte alte Regeln
    pub removed: Vec<String>,
    /// Alte Regeln, die von Hand geändert wurden und liegen bleiben
    pub kept: Vec<String>,
    /// Plugin-Eintrag im Projekt geändert (eingeschaltet oder vom alten umgestellt)
    pub plugin_switched: bool,
    /// Ergebnis der Freigabe der Regel-Importe
    pub import_approval: ApprovalStatus,
}

/// Führt `HarnessProject::init`, Plugin-Schalter, Regeln und Import-Freigabe für das Repo aus
pub fn migrate_repo(
    home: &HarnessHome,
    approval: &ClaudeImportApproval,
    dir: &Path,
) -> Result<MigrationResult> {
    let project = HarnessProject::init(dir)?;
    let root = Path::new(&project.root);
    let switched = ProjectPluginSwitch::new().enable(root)?;
    let rules = ProjectRules::new(home.clone()).ensure(root)?;
    let approved = approval.approve(&project.root)?;
    Ok(MigrationResult {
        written: rules.written,
        removed: rules.removed,
        kept: rules.kept,
        plugin_switched: switched.enabled || switched.migrated,
        import_approval: approved.status,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use nexus_core::install::{LEGACY_BOARD_RULE_PATH, LEGACY_DOCS_RULE_PATH, LEGACY_RULE_MARKER};

    use super::*;

    fn temp(prefix: &str) -> tempfile::TempDir {
        tempfile::Builder::new().prefix(prefix).tempdir().unwrap()
    }

    fn write(dir: &Path, path: &str, text: &str) {
        let file = dir.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    #[test]
    fn stellt_ein_altes_projekt_um_und_behaelt_von_hand_geaenderte_regeln() {
        let repo = temp("nexus-mig-repo-");
        let git = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(git.success());
        let home_dir = temp("nexus-mig-home-");
        let home = HarnessHome::at(home_dir.path());
        write(
            repo.path(),
            ".claude/settings.local.json",
            r#"{"enabledPlugins":{"ai-harness@ai-harness-local":true}}"#,
        );
        write(
            repo.path(),
            LEGACY_BOARD_RULE_PATH,
            &format!("{LEGACY_RULE_MARKER}\nalt"),
        );
        write(repo.path(), LEGACY_DOCS_RULE_PATH, "von Hand");
        // Ohne `.claude.json` legt die Freigabe nichts an
        let approval = ClaudeImportApproval::new(
            Some(home_dir.path().join("claude").to_string_lossy().into_owned()),
            home_dir.path().to_path_buf(),
            cfg!(windows),
        );

        let result = migrate_repo(&home, &approval, repo.path()).unwrap();

        assert!(result.plugin_switched);
        assert_eq!(result.removed, vec![LEGACY_BOARD_RULE_PATH.to_string()]);
        assert_eq!(result.kept, vec![LEGACY_DOCS_RULE_PATH.to_string()]);
        assert_eq!(result.written.len(), 2);
        assert_eq!(result.import_approval, ApprovalStatus::NoConfig);
        let settings =
            fs::read_to_string(repo.path().join(".claude/settings.local.json")).unwrap();
        assert!(settings.contains("nexus@nexus-local"));
        assert!(!settings.contains("ai-harness@ai-harness-local"));
        assert_eq!(
            serde_json::to_value(&result).unwrap()["importApproval"],
            "no-config"
        );
    }
}
