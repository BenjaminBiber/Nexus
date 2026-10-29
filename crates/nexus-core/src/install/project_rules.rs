//! Alle Rules, die `nexus init` und das Anlegen eines Worktrees ins Projekt schreiben, samt
//! Ablösung der alten Rules des Harness. Portiert aus
//! `packages/core/src/install/ProjectRules.ts`.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::install::project_rule::{
    ProjectRule, BOARD_RULE_PATH, DOCS_RULE_PATH, LEGACY_BOARD_RULE_PATH, LEGACY_DOCS_RULE_PATH,
    LEGACY_RULE_MARKER,
};
use crate::registry::HarnessHome;

/// Orte aller Rules, die Nexus im Projekt anlegt oder ablöst; sie zählen bei
/// Sauberkeitsprüfungen nie als Änderung. Die alten Namen stehen weiter darin, solange es
/// Projekte gibt, die sie noch tragen.
pub const PROJECT_RULE_PATHS: &[&str] = &[
    DOCS_RULE_PATH,
    BOARD_RULE_PATH,
    LEGACY_DOCS_RULE_PATH,
    LEGACY_BOARD_RULE_PATH,
];

/// Die alten Rules des Harness und die Rule, die sie jeweils ablöst
const LEGACY_RULE_PATHS: [&str; 2] = [LEGACY_DOCS_RULE_PATH, LEGACY_BOARD_RULE_PATH];

/// Was `ensure` getan hat: neu geschriebene Rules, abgelöste alte Rules und alte Rules, die
/// von Hand geändert wurden und deshalb liegen bleiben.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RulesResult {
    /// Orte der neu geschriebenen Rules
    pub written: Vec<String>,
    /// Orte der gelöschten alten Rules
    pub removed: Vec<String>,
    /// Orte der alten Rules, die liegen bleiben (Inhalt nicht von Nexus)
    pub kept: Vec<String>,
}

/// Alle Rules des Projekts in fester Reihenfolge.
#[derive(Debug, Clone)]
pub struct ProjectRules {
    /// Rule der Doku-Regeln (nur bei Zugriff auf `docs/`)
    pub docs: ProjectRule,
    /// Rule der Board-Regel (immer geladen)
    pub board: ProjectRule,
}

impl ProjectRules {
    /// Rules zu einem Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            docs: ProjectRule::docs(home.clone()),
            board: ProjectRule::board(home),
        }
    }

    /// Alle Rules in fester Reihenfolge
    pub fn all(&self) -> Vec<&ProjectRule> {
        vec![&self.docs, &self.board]
    }

    /// Schreibt alle fehlenden oder veralteten Rules und löst die alten Rules des Harness ab
    pub fn ensure(&self, project_dir: &Path) -> Result<RulesResult> {
        let mut result = RulesResult::default();
        for rule in self.all() {
            if rule.ensure(project_dir)? {
                result.written.push(rule.path().to_string());
            }
        }
        self.drop_legacy(project_dir, &mut result)?;
        Ok(result)
    }

    /// Löscht die alten Rules, aber nur die von `harness init` angelegten; eine von Hand
    /// geänderte Datei bleibt liegen und wird gemeldet
    fn drop_legacy(&self, project_dir: &Path, result: &mut RulesResult) -> Result<()> {
        for path in LEGACY_RULE_PATHS {
            let file = file_of(project_dir, path);
            let text = match fs::read_to_string(&file) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(Error::unexpected(error)),
            };
            if generated(&text) {
                fs::remove_file(&file).map_err(Error::unexpected)?;
                result.removed.push(path.to_string());
            } else {
                result.kept.push(path.to_string());
            }
        }
        Ok(())
    }
}

/// Datei einer Rule im Projekt (Pfad mit Schrägstrichen)
fn file_of(project_dir: &Path, path: &str) -> std::path::PathBuf {
    let mut file = project_dir.to_path_buf();
    for part in path.split('/') {
        file.push(part);
    }
    file
}

/// Trägt der Text die Kopfzeile von `harness init`? Nur dann hat Nexus die Datei geschrieben.
fn generated(text: &str) -> bool {
    text.lines()
        .take(3)
        .any(|line| line.starts_with(LEGACY_RULE_MARKER))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Schreibt eine alte Rule mit dem angegebenen Inhalt
    fn alte_rule(dir: &Path, path: &str, text: &str) -> std::path::PathBuf {
        let file = file_of(dir, path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, text).unwrap();
        file
    }

    #[test]
    fn kennt_die_orte_beider_rules_und_der_alten() {
        assert_eq!(
            PROJECT_RULE_PATHS,
            [
                ".claude/rules/nexus-docs.md",
                ".claude/rules/nexus-board.md",
                ".claude/rules/ai-harness-docs.md",
                ".claude/rules/ai-harness-board.md"
            ]
        );
        let rules = ProjectRules::new(HarnessHome::at("/x/.nexus"));
        let orte: Vec<&str> = rules.all().iter().map(|rule| rule.path()).collect();
        assert_eq!(orte, PROJECT_RULE_PATHS[..2]);
    }

    #[test]
    fn loscht_eine_alte_rule_nur_mit_der_kopfzeile_von_harness_init() {
        let dir = tempfile::Builder::new()
            .prefix("nexus-rules-")
            .tempdir()
            .unwrap();
        let rules = ProjectRules::new(HarnessHome::at("/x/.nexus"));
        let angelegt = alte_rule(
            dir.path(),
            LEGACY_DOCS_RULE_PATH,
            "---\n# Angelegt von `harness init`, nicht versioniert.\n---\n@~/.harness/rules/docs.md\n",
        );
        let eigen = alte_rule(dir.path(), LEGACY_BOARD_RULE_PATH, "# Meine eigene Regel\n");

        let mut result = RulesResult::default();
        rules.drop_legacy(dir.path(), &mut result).unwrap();

        assert!(!angelegt.exists());
        assert!(eigen.exists());
        assert_eq!(result.removed, [LEGACY_DOCS_RULE_PATH.to_string()]);
        assert_eq!(result.kept, [LEGACY_BOARD_RULE_PATH.to_string()]);
    }

    #[test]
    fn ohne_alte_rules_gibt_es_nichts_abzulosen() {
        let dir = tempfile::Builder::new()
            .prefix("nexus-rules-")
            .tempdir()
            .unwrap();
        let rules = ProjectRules::new(HarnessHome::at("/x/.nexus"));

        let mut result = RulesResult::default();
        rules.drop_legacy(dir.path(), &mut result).unwrap();

        assert!(result.removed.is_empty());
        assert!(result.kept.is_empty());
    }
}
