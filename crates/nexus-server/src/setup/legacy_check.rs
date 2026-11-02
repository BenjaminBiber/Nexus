//! Erkennt Repos, die noch für den alten AI Harness eingerichtet sind.

use std::fs;
use std::path::Path;

use nexus_core::install::{
    LEGACY_BOARD_RULE_PATH, LEGACY_DOCS_RULE_PATH, LEGACY_PLUGIN_ID, PLUGIN_ID,
};
use serde::Serialize;
use serde_json::{Map, Value};

/// Lokale Einstellungen von Claude Code im Projekt
const SETTINGS: &str = ".claude/settings.local.json";

/// Antwort von `GET /api/db/boards/{id}/migration`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationCheck {
    /// Das Repo sollte umgestellt werden
    pub needed: bool,
    /// Repo des Boards auf diesem Rechner
    pub repo_path: Option<String>,
    /// `legacy-plugin` und/oder `legacy-rules`
    pub reasons: Vec<String>,
}

/// Prüft das Repo eines Boards; ohne Repo, ohne Ordner oder bei unlesbarer Einstellung ist
/// nichts nötig (die unlesbare Datei meldet erst die Migration selbst)
pub fn check_repo(repo: Option<&str>) -> MigrationCheck {
    let none = |repo_path: Option<String>| MigrationCheck {
        needed: false,
        repo_path,
        reasons: Vec::new(),
    };
    let Some(repo) = repo else {
        return none(None);
    };
    let dir = Path::new(repo);
    if !dir.is_dir() {
        return none(Some(repo.to_string()));
    }
    let Some(plugins) = enabled_plugins(dir) else {
        return none(Some(repo.to_string()));
    };
    if plugins.get(PLUGIN_ID) == Some(&Value::Bool(true)) {
        return none(Some(repo.to_string()));
    }
    let mut reasons = Vec::new();
    if plugins.contains_key(LEGACY_PLUGIN_ID) {
        reasons.push("legacy-plugin".to_string());
    }
    if [LEGACY_DOCS_RULE_PATH, LEGACY_BOARD_RULE_PATH]
        .iter()
        .any(|path| dir.join(path).is_file())
    {
        reasons.push("legacy-rules".to_string());
    }
    MigrationCheck {
        needed: !reasons.is_empty(),
        repo_path: Some(repo.to_string()),
        reasons,
    }
}

/// `enabledPlugins` der lokalen Einstellungen; fehlende Datei: leer, unlesbar: `None`
fn enabled_plugins(dir: &Path) -> Option<Map<String, Value>> {
    let text = match fs::read_to_string(dir.join(SETTINGS)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some(Map::new()),
        Err(_) => return None,
    };
    let value: Value = serde_json::from_str(&text).ok()?;
    Some(
        value
            .get("enabledPlugins")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("nexus-legacy-")
            .tempdir()
            .unwrap()
    }

    fn write(dir: &Path, path: &str, text: &str) {
        let file = dir.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    fn check(dir: &Path) -> MigrationCheck {
        check_repo(Some(&dir.to_string_lossy()))
    }

    #[test]
    fn erkennt_das_alte_plugin() {
        let dir = repo();
        write(
            dir.path(),
            SETTINGS,
            &format!(r#"{{"enabledPlugins":{{"{LEGACY_PLUGIN_ID}":true}}}}"#),
        );
        let result = check(dir.path());
        assert!(result.needed);
        assert_eq!(result.reasons, vec!["legacy-plugin"]);
    }

    #[test]
    fn erkennt_alte_regeln() {
        let dir = repo();
        write(dir.path(), LEGACY_BOARD_RULE_PATH, "alt");
        assert_eq!(check(dir.path()).reasons, vec!["legacy-rules"]);
    }

    #[test]
    fn nennt_beide_gruende() {
        let dir = repo();
        write(
            dir.path(),
            SETTINGS,
            &format!(r#"{{"enabledPlugins":{{"{LEGACY_PLUGIN_ID}":false}}}}"#),
        );
        write(dir.path(), LEGACY_DOCS_RULE_PATH, "alt");
        assert_eq!(
            check(dir.path()).reasons,
            vec!["legacy-plugin", "legacy-rules"]
        );
    }

    #[test]
    fn ein_schon_migriertes_projekt_braucht_nichts() {
        let dir = repo();
        write(
            dir.path(),
            SETTINGS,
            &format!(r#"{{"enabledPlugins":{{"{PLUGIN_ID}":true}}}}"#),
        );
        write(dir.path(), LEGACY_DOCS_RULE_PATH, "von Hand geändert");
        assert!(!check(dir.path()).needed);
    }

    #[test]
    fn ohne_repo_oder_ordner_braucht_es_nichts() {
        assert!(!check_repo(None).needed);
        let dir = repo();
        assert!(!check(&dir.path().join("fehlt")).needed);
    }

    #[test]
    fn eine_unlesbare_einstellung_meldet_erst_die_migration() {
        let dir = repo();
        write(dir.path(), SETTINGS, "{kaputt");
        write(dir.path(), LEGACY_BOARD_RULE_PATH, "alt");
        assert!(!check(dir.path()).needed);
    }

    #[test]
    fn serialisiert_wie_die_oberflaeche_es_erwartet() {
        assert_eq!(
            serde_json::to_value(check_repo(None)).unwrap(),
            serde_json::json!({ "needed": false, "repoPath": null, "reasons": [] })
        );
    }
}
