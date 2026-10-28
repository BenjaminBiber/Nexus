//! Alle Rules, die `harness init` und das Anlegen eines Worktrees ins Projekt schreiben.
//! Portiert aus `packages/core/src/install/ProjectRules.ts`.

use std::path::Path;

use crate::error::Result;
use crate::install::project_rule::{ProjectRule, BOARD_RULE_PATH, DOCS_RULE_PATH};
use crate::registry::HarnessHome;

/// Orte aller Rules, die der Harness im Projekt anlegt; sie zählen bei Sauberkeitsprüfungen
/// nie als Änderung
pub const PROJECT_RULE_PATHS: &[&str] = &[DOCS_RULE_PATH, BOARD_RULE_PATH];

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

    /// Schreibt alle fehlenden oder veralteten Rules; liefert die Orte der neu geschriebenen
    pub fn ensure(&self, project_dir: &Path) -> Result<Vec<String>> {
        let mut written = Vec::new();
        for rule in self.all() {
            if rule.ensure(project_dir)? {
                written.push(rule.path().to_string());
            }
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kennt_die_orte_beider_rules() {
        assert_eq!(
            PROJECT_RULE_PATHS,
            [
                ".claude/rules/ai-harness-docs.md",
                ".claude/rules/ai-harness-board.md"
            ]
        );
        let rules = ProjectRules::new(HarnessHome::at("/x/.harness"));
        let orte: Vec<&str> = rules.all().iter().map(|rule| rule.path()).collect();
        assert_eq!(orte, PROJECT_RULE_PATHS);
    }
}
