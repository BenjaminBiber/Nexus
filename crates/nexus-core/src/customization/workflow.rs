//! Portiert `packages/core/src/customization/WorkflowCustomization.ts`.

use serde_json::Value;

use crate::error::{Error, Result};
use crate::workflow::workflow_phases;

use super::base_snapshot::{BaseCheck, BaseSnapshot};
use super::overview::{CustomizationEntry, EntryKind, EntryState, ExtensionPoint, Problem};
use super::paths::CustomizationPaths;
use super::tools::WorkflowTools;
use super::workflow_validator::WorkflowValidator;

/// Ergebnis der Prüfung von `custom/workflow.ts`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkflowResult {
    /// None, wenn es keinen eigenen Workflow gibt
    pub entry: Option<CustomizationEntry>,
    /// Geladene Phasen bei aktivem eigenen Workflow, sonst None (Standard gilt)
    pub phases: Option<Value>,
}

/// Bewertet `custom/workflow.ts`: Typprüfung, Laden, Regeln, Basis; aktiv oder Rückfall auf
/// den Standard.
pub struct WorkflowCustomization<'a> {
    paths: &'a CustomizationPaths,
    tools: &'a dyn WorkflowTools,
    validator: WorkflowValidator,
    snapshot: BaseSnapshot,
}

impl<'a> WorkflowCustomization<'a> {
    /// Prüfer für diese Orte mit diesen Werkzeugen
    pub fn new(paths: &'a CustomizationPaths, tools: &'a dyn WorkflowTools) -> Self {
        Self {
            paths,
            tools,
            validator: WorkflowValidator::new(),
            snapshot: BaseSnapshot::new(),
        }
    }

    /// Ohne Datei kein Eintrag; ein unerwarteter Fehler in der Prüfung führt zum Rückfall
    /// statt zum Abbruch
    pub fn check(&self, skills: &[String]) -> WorkflowResult {
        if !self.paths.workflow_file().exists() {
            return WorkflowResult::default();
        }
        match self.evaluate(skills) {
            Ok(result) => result,
            Err(error) => self.fallback(vec![Problem::new(
                "check-failed",
                format!("Prüfung fehlgeschlagen: {error}"),
            )]),
        }
    }

    /// Prüft in Reihenfolge; das erste Problem einer Stufe führt zum Rückfall
    fn evaluate(&self, skills: &[String]) -> Result<WorkflowResult> {
        let file = self.paths.workflow_file();
        let type_problems = self.tools.typecheck(&file)?;
        if !type_problems.is_empty() {
            return Ok(self.fallback(type_problems));
        }

        let value = match self.tools.load(&file) {
            Ok(value) => value,
            Err(message) => {
                return Ok(self.fallback(vec![Problem::new(
                    "load-failed",
                    format!("Der Workflow ließ sich nicht laden: {message}"),
                )]))
            }
        };

        let validation = self.validator.validate(&value, skills);
        if !validation.problems.is_empty() {
            return Ok(self.fallback(validation.problems));
        }

        // Basis nur prüfen, wenn der Workflow gültig ist
        let base = self.base()?;
        let mut warnings = validation.warnings;
        warnings.extend(base.warnings);
        let entry = CustomizationEntry {
            base: base.state.into(),
            warnings: self.located(warnings),
            diff: base.diff,
            ..self.entry(EntryState::Active, Vec::new())
        };
        Ok(WorkflowResult {
            entry: Some(entry),
            phases: Some(value),
        })
    }

    /// Rückfall: Standard gilt, Gründe mit Ort
    fn fallback(&self, problems: Vec<Problem>) -> WorkflowResult {
        WorkflowResult {
            entry: Some(self.entry(EntryState::Fallback, self.located(problems))),
            phases: None,
        }
    }

    /// Grundgerüst des Eintrags; ein eigener Workflow ersetzt immer den Standard
    fn entry(&self, state: EntryState, problems: Vec<Problem>) -> CustomizationEntry {
        CustomizationEntry {
            state,
            problems,
            ..CustomizationEntry::new(ExtensionPoint::Workflow, "workflow", EntryKind::Replace)
        }
    }

    /// Ort relativ zum Clone; ohne Ort gilt `custom/workflow.ts`
    fn located(&self, problems: Vec<Problem>) -> Vec<Problem> {
        problems
            .into_iter()
            .map(|problem| {
                let file = problem
                    .file
                    .clone()
                    .unwrap_or_else(|| self.paths.workflow_file().to_string_lossy().into_owned());
                problem.with_file(self.paths.display(file))
            })
            .collect()
    }

    /// Vergleicht die Basis mit dem aktuellen Standard (als formatiertes JSON in einer
    /// temporären Datei)
    fn base(&self) -> Result<BaseCheck> {
        let dir = tempfile::Builder::new()
            .prefix("harness-workflow-std-")
            .tempdir()
            .map_err(Error::unexpected)?;
        let standard = dir.path().join("workflow.json");
        std::fs::write(&standard, BaseSnapshot::format_json(&workflow_phases())?)
            .map_err(Error::unexpected)?;
        self.snapshot.check(
            &self.paths.base_of(ExtensionPoint::Workflow, "workflow"),
            &standard,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    #[cfg(unix)]
    use std::path::PathBuf;

    use super::*;
    use crate::customization::overview::EntryBase;
    use crate::error::HarnessError;

    /// Werkzeuge ohne tsc und esbuild: liefern vorgegebene Typprobleme und Werte
    struct Fake {
        value: Value,
        type_problems: Vec<Problem>,
        load_error: Option<String>,
        typecheck_error: Option<String>,
    }

    impl Fake {
        fn mit_wert(value: Value) -> Self {
            Self {
                value,
                type_problems: Vec::new(),
                load_error: None,
                typecheck_error: None,
            }
        }
    }

    impl WorkflowTools for Fake {
        fn typecheck(&self, _file: &Path) -> Result<Vec<Problem>> {
            if let Some(message) = &self.typecheck_error {
                return Err(HarnessError::new("type-tools-failed", message.clone()).into());
            }
            Ok(self.type_problems.clone())
        }

        fn load(&self, _file: &Path) -> std::result::Result<Value, String> {
            match &self.load_error {
                Some(message) => Err(message.clone()),
                None => Ok(self.value.clone()),
            }
        }
    }

    struct Beispiel {
        // Hält den Temp-Ordner am Leben; gelesen wird es nur im Unix-Test mit Symlink
        #[cfg_attr(not(unix), allow(dead_code))]
        dir: tempfile::TempDir,
        paths: CustomizationPaths,
    }

    impl Beispiel {
        fn neu() -> Self {
            let dir = tempfile::tempdir().expect("Temp-Ordner");
            let root = dir.path().to_path_buf();
            let paths = CustomizationPaths::new(root.join("custom"), root.join("std"));
            std::fs::create_dir_all(root.join("custom")).unwrap();
            std::fs::write(
                paths.workflow_file(),
                "// Inhalt egal, die Werkzeuge sind ersetzt",
            )
            .unwrap();
            Self { dir, paths }
        }

        #[cfg(unix)]
        fn root(&self) -> PathBuf {
            self.dir.path().to_path_buf()
        }
    }

    fn standard() -> Value {
        serde_json::to_value(workflow_phases()).unwrap()
    }

    fn skills() -> Vec<String> {
        [
            "harness-grill",
            "harness-wayfinder",
            "harness-tasks",
            "harness-implement",
        ]
        .iter()
        .map(ToString::to_string)
        .collect()
    }

    fn codes(problems: &[Problem]) -> Vec<&str> {
        problems.iter().map(|p| p.code.as_str()).collect()
    }

    #[test]
    fn gibt_ohne_datei_keinen_eintrag_und_den_standard() {
        let beispiel = Beispiel::neu();
        std::fs::remove_file(beispiel.paths.workflow_file()).unwrap();
        let tools = Fake::mit_wert(standard());
        assert_eq!(
            WorkflowCustomization::new(&beispiel.paths, &tools).check(&skills()),
            WorkflowResult::default()
        );
    }

    #[test]
    fn fallt_bei_typfehlern_zuruck_und_nennt_datei_und_zeile_relativ_zum_clone() {
        let beispiel = Beispiel::neu();
        let type_problem = Problem {
            line: Some(3),
            ..Problem::at(
                "type-error",
                "TS2322: x",
                beispiel
                    .paths
                    .workflow_file()
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        let tools = Fake {
            type_problems: vec![type_problem.clone()],
            ..Fake::mit_wert(Value::Null)
        };
        let result = WorkflowCustomization::new(&beispiel.paths, &tools).check(&skills());
        assert_eq!(result.phases, None);
        let entry = result.entry.unwrap();
        assert_eq!(entry.point, ExtensionPoint::Workflow);
        assert_eq!(entry.name, "workflow");
        assert_eq!(entry.kind, EntryKind::Replace);
        assert_eq!(entry.state, EntryState::Fallback);
        assert_eq!(entry.base, EntryBase::None);
        assert_eq!(
            entry.problems,
            vec![Problem {
                file: Some("custom/workflow.ts".to_string()),
                ..type_problem
            }]
        );
    }

    #[test]
    fn fallt_zuruck_wenn_das_laden_scheitert() {
        let beispiel = Beispiel::neu();
        let tools = Fake {
            load_error: Some("kaputt".to_string()),
            ..Fake::mit_wert(Value::Null)
        };
        let result = WorkflowCustomization::new(&beispiel.paths, &tools).check(&skills());
        let entry = result.entry.unwrap();
        assert_eq!(entry.problems[0].code, "load-failed");
        assert!(entry.problems[0].message.contains("kaputt"));
    }

    #[test]
    fn fallt_zuruck_wenn_die_pruefung_selbst_unerwartet_scheitert() {
        let beispiel = Beispiel::neu();
        let tools = Fake {
            typecheck_error: Some("tsc nicht da".to_string()),
            ..Fake::mit_wert(standard())
        };
        let result = WorkflowCustomization::new(&beispiel.paths, &tools).check(&skills());
        assert_eq!(result.phases, None);
        let entry = result.entry.unwrap();
        assert_eq!(entry.state, EntryState::Fallback);
        assert_eq!(entry.base, EntryBase::None);
        assert_eq!(
            entry.problems,
            vec![Problem::at(
                "check-failed",
                "Prüfung fehlgeschlagen: tsc nicht da",
                "custom/workflow.ts"
            )]
        );
    }

    /// Nur unter Unix: eine Basis, die nicht gelesen werden kann (unter Windows lässt sich
    /// eine solche Datei ohne Weiteres nicht herstellen)
    #[cfg(unix)]
    #[test]
    fn fallt_zuruck_wenn_der_basisvergleich_unerwartet_scheitert() {
        let beispiel = Beispiel::neu();
        // Die Basis ist ein Ordner mit einem ins Nichts zeigenden Verweis: er wird gelistet,
        // aber nicht gelesen
        let base = beispiel.paths.base_of(ExtensionPoint::Workflow, "workflow");
        std::fs::create_dir_all(&base).unwrap();
        std::os::unix::fs::symlink(beispiel.root().join("gibt-es-nicht"), base.join("weg.json"))
            .unwrap();
        let tools = Fake::mit_wert(standard());
        let result = WorkflowCustomization::new(&beispiel.paths, &tools).check(&skills());
        assert_eq!(result.phases, None);
        let entry = result.entry.unwrap();
        assert_eq!(entry.state, EntryState::Fallback);
        assert_eq!(codes(&entry.problems), vec!["check-failed"]);
        assert_eq!(
            entry.problems[0].file.as_deref(),
            Some("custom/workflow.ts")
        );
    }

    #[test]
    fn fallt_ohne_pflicht_phase_zuruck() {
        let beispiel = Beispiel::neu();
        let nur_erste = Value::Array(vec![standard()[0].clone()]);
        let tools = Fake::mit_wert(nur_erste);
        let result = WorkflowCustomization::new(&beispiel.paths, &tools).check(&skills());
        let entry = result.entry.unwrap();
        assert_eq!(entry.state, EntryState::Fallback);
        assert_eq!(
            entry.problems,
            vec![Problem::at(
                "required-phase-missing",
                "Die Pflicht-Phase umsetzen fehlt.",
                "custom/workflow.ts"
            )]
        );
    }

    #[test]
    fn ist_gultig_aktiv_ohne_basis_mit_hinweis() {
        let beispiel = Beispiel::neu();
        let tools = Fake::mit_wert(standard());
        let result = WorkflowCustomization::new(&beispiel.paths, &tools).check(&skills());
        assert_eq!(result.phases, Some(standard()));
        let entry = result.entry.unwrap();
        assert_eq!(entry.state, EntryState::Active);
        assert_eq!(entry.base, EntryBase::Unknown);
        assert_eq!(codes(&entry.warnings), vec!["base-unknown"]);
    }

    #[test]
    fn erkennt_aktuelle_und_veraltete_basis() {
        let beispiel = Beispiel::neu();
        let tools = Fake::mit_wert(standard());
        let base = beispiel.paths.base_of(ExtensionPoint::Workflow, "workflow");
        let snapshot = BaseSnapshot::new();
        snapshot
            .save_text(
                &BaseSnapshot::format_json(&workflow_phases()).unwrap(),
                &base,
            )
            .unwrap();
        let entry = WorkflowCustomization::new(&beispiel.paths, &tools)
            .check(&skills())
            .entry
            .unwrap();
        assert_eq!(entry.base, EntryBase::Current);
        assert!(entry.warnings.is_empty());

        snapshot.save_text("[]\n", &base).unwrap();
        let outdated = WorkflowCustomization::new(&beispiel.paths, &tools)
            .check(&skills())
            .entry
            .unwrap();
        assert_eq!(outdated.base, EntryBase::Outdated);
        assert_eq!(codes(&outdated.warnings), vec!["base-outdated"]);
        assert!(!outdated.diff.unwrap_or_default().is_empty());
    }
}
