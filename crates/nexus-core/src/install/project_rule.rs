//! Eine Rule im Projekt, die nur einen Import auf eine zentrale Regeldatei unter
//! `~/.nexus/rules` enthält. Portiert aus `packages/core/src/install/ProjectRule.ts`,
//! `ProjectDocsRule.ts` und `ProjectBoardRule.ts`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::git::{GitExclude, GitPaths};
use crate::locking::AtomicWriter;
use crate::registry::{HarnessHome, HARNESS_DIR, NEXUS_DIR};

/// Ort der Doku-Rule im Projekt, relativ zur Wurzel (Hauptordner oder Worktree)
pub const DOCS_RULE_PATH: &str = ".claude/rules/nexus-docs.md";

/// Ort der Board-Rule im Projekt, relativ zur Wurzel (Hauptordner oder Worktree)
pub const BOARD_RULE_PATH: &str = ".claude/rules/nexus-board.md";

/// Ort der Doku-Rule, die der Harness früher angelegt hat; `nexus init` löst sie ab
pub const LEGACY_DOCS_RULE_PATH: &str = ".claude/rules/ai-harness-docs.md";

/// Ort der Board-Rule, die der Harness früher angelegt hat; `nexus init` löst sie ab
pub const LEGACY_BOARD_RULE_PATH: &str = ".claude/rules/ai-harness-board.md";

/// Kopfzeile, an der eine von Nexus angelegte Rule zu erkennen ist
pub const RULE_MARKER: &str = "# Angelegt von `nexus init`";

/// Kopfzeile der Rules, die der Harness angelegt hat; nur solche Dateien darf `nexus init`
/// löschen, von Hand geschriebene bleiben liegen
pub const LEGACY_RULE_MARKER: &str = "# Angelegt von `harness init`";

/// Was eine Rule ausmacht: Ort im Projekt, zentrale Datei und optionale Eingrenzung
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuleSpec {
    /// Ort der Rule im Projekt, relativ zur Wurzel (Hauptordner oder Worktree), mit Schrägstrichen
    pub path: String,
    /// Datei unter `~/.harness/rules`, die die Rule importiert (z. B. `docs.md`)
    pub import_file: String,
    /// Optionale Eingrenzung (`paths` im Frontmatter); ohne sie lädt Claude Code die Rule immer
    pub paths: Vec<String>,
}

/// Eine Rule im Projekt, die nur einen Import auf eine zentrale Regeldatei enthält.
/// Den Inhalt dort aktualisiert `npm run setup`. Die Rule ist lokal wie die Plugin-Einstellung
/// und wird über `info/exclude` aus Git herausgenommen.
#[derive(Debug, Clone)]
pub struct ProjectRule {
    spec: ProjectRuleSpec,
    home: HarnessHome,
    writer: AtomicWriter,
    exclude: GitExclude,
    git: GitPaths,
}

impl ProjectRule {
    /// Rule zu einer Beschreibung und einem Benutzerordner
    pub fn new(spec: ProjectRuleSpec, home: HarnessHome) -> Self {
        Self {
            spec,
            home,
            writer: AtomicWriter::new(),
            exclude: GitExclude::new(),
            git: GitPaths::new(),
        }
    }

    /// Rule, die beim Zugriff auf einen `docs/`-Ordner die zentralen Doku-Regeln
    /// (`~/.nexus/rules/docs.md`) lädt
    pub fn docs(home: HarnessHome) -> Self {
        Self::new(
            ProjectRuleSpec {
                path: DOCS_RULE_PATH.to_string(),
                import_file: "docs.md".to_string(),
                paths: vec!["**/docs/**".to_string()],
            },
            home,
        )
    }

    /// Rule ohne Eingrenzung (immer geladen), die die zentrale Board-Regel
    /// (`~/.nexus/rules/board.md`) lädt: Tickets, Board, Fragen und Notizen meinen immer
    /// das Nexus-Board
    pub fn board(home: HarnessHome) -> Self {
        Self::new(
            ProjectRuleSpec {
                path: BOARD_RULE_PATH.to_string(),
                import_file: "board.md".to_string(),
                paths: Vec::new(),
            },
            home,
        )
    }

    /// Beschreibung der Rule
    pub fn spec(&self) -> &ProjectRuleSpec {
        &self.spec
    }

    /// Ort der Rule relativ zur Projektwurzel
    pub fn path(&self) -> &str {
        &self.spec.path
    }

    /// Datei der Rule im Projekt
    pub fn file(&self, project_dir: &Path) -> PathBuf {
        let mut file = project_dir.to_path_buf();
        for part in self.spec.path.split('/') {
            file.push(part);
        }
        file
    }

    /// Importziel: beim Standardordner `~/…`, sonst der absolute Pfad mit Schrägstrichen und
    /// maskierten Leerzeichen
    pub fn import_target(&self) -> String {
        let dir = absolute(self.home.dir());
        for name in [NEXUS_DIR, HARNESS_DIR] {
            if dir == absolute(&home_dir().join(name)) {
                return format!("~/{name}/rules/{}", self.spec.import_file);
            }
        }
        self.home
            .rules_dir()
            .join(&self.spec.import_file)
            .to_string_lossy()
            .replace('\\', "/")
            .replace(' ', "\\ ")
    }

    /// Inhalt der Rule: Frontmatter (mit Eingrenzung, falls angegeben), dann der Import
    pub fn content(&self) -> String {
        let mut lines = vec![
            "---".to_string(),
            format!("{RULE_MARKER}, nicht versioniert. Die Regeln liegen zentral und kommen mit `npm run setup`."),
        ];
        if !self.spec.paths.is_empty() {
            lines.push("paths:".to_string());
            lines.extend(self.spec.paths.iter().map(|path| format!("  - \"{path}\"")));
        }
        lines.push("---".to_string());
        lines.push(format!("@{}", self.import_target()));
        lines.push(String::new());
        lines.join("\n")
    }

    /// Schreibt die Rule, falls sie fehlt oder veraltet ist, und trägt sie in `info/exclude` ein.
    /// Liefert true, wenn die Rule neu geschrieben wurde.
    pub fn ensure(&self, project_dir: &Path) -> Result<bool> {
        let common_dir = self.git.common_dir(project_dir)?;
        self.exclude
            .add(Path::new(&common_dir), &format!("/{}", self.spec.path))?;
        let file = self.file(project_dir);
        let content = self.content();
        if fs::read_to_string(&file).is_ok_and(|old| old == content) {
            return Ok(false);
        }
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(Error::unexpected)?;
        }
        self.writer.write(&file, &content)?;
        Ok(true)
    }
}

/// Absoluter Pfad ohne Auflösen von Verknüpfungen, wie `path.resolve` in Node
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Benutzerordner des angemeldeten Benutzers, wie `os.homedir()` in Node
fn home_dir() -> PathBuf {
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule mit eigenem Benutzerordner
    fn rule(home: &str, docs: bool) -> ProjectRule {
        let home = HarnessHome::at(home);
        if docs {
            ProjectRule::docs(home)
        } else {
            ProjectRule::board(home)
        }
    }

    #[test]
    fn die_doku_rule_grenzt_auf_docs_ein_und_importiert_die_zentralen_regeln() {
        let text = rule("/x/.harness", true).content();
        assert!(text.starts_with("---\n"), "{text}");
        assert!(text.contains("paths:\n  - \"**/docs/**\"\n"), "{text}");
        assert!(
            text.ends_with("\n---\n@/x/.harness/rules/docs.md\n"),
            "{text}"
        );
    }

    #[test]
    fn die_board_rule_hat_keine_eingrenzung() {
        let text = rule("/x/.harness", false).content();
        assert!(!text.contains("paths:"), "{text}");
        assert!(
            text.ends_with("---\n@/x/.harness/rules/board.md\n"),
            "{text}"
        );
    }

    #[test]
    fn importiert_bei_eigenem_harness_home_den_absoluten_pfad_leerzeichen_maskiert() {
        let home = HarnessHome::at("C:\\Users\\Max Muster\\.harness");
        assert_eq!(
            ProjectRule::docs(home.clone()).import_target(),
            "C:/Users/Max\\ Muster/.harness/rules/docs.md"
        );
        assert_eq!(
            ProjectRule::board(home).import_target(),
            "C:/Users/Max\\ Muster/.harness/rules/board.md"
        );
    }

    #[test]
    fn importiert_beim_standardordner_mit_tilde() {
        let home = HarnessHome::at(home_dir().join(".nexus"));
        assert_eq!(
            ProjectRule::docs(home).import_target(),
            "~/.nexus/rules/docs.md"
        );
    }

    #[test]
    fn importiert_beim_alten_standardordner_ebenfalls_mit_tilde() {
        let home = HarnessHome::at(home_dir().join(".harness"));
        assert_eq!(
            ProjectRule::docs(home).import_target(),
            "~/.harness/rules/docs.md"
        );
    }

    #[test]
    fn kennt_die_orte_beider_rules() {
        assert_eq!(rule("/x", true).path(), ".claude/rules/nexus-docs.md");
        assert_eq!(rule("/x", false).path(), ".claude/rules/nexus-board.md");
        assert_eq!(
            rule("/x", true).file(Path::new("p")),
            Path::new("p")
                .join(".claude")
                .join("rules")
                .join("nexus-docs.md")
        );
    }

    #[test]
    fn die_kopfzeile_nennt_nexus_init() {
        let text = rule("/x/.nexus", true).content();
        assert!(text.contains(RULE_MARKER), "{text}");
        assert!(!text.contains(LEGACY_RULE_MARKER), "{text}");
    }
}
