//! Portiert `packages/core/src/customization/CustomizationCopier.ts`.

use std::path::PathBuf;

use crate::error::{Error, HarnessError, Result};
use crate::workflow::workflow_phases;

use super::base_snapshot::{copy_path, remove_path, BaseSnapshot};
use super::name_rules::NameRules;
use super::overview::ExtensionPoint;
use super::overview_formatter::label;
use super::paths::CustomizationPaths;

/// Was kopiert oder zurückgesetzt werden soll
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyTarget {
    pub point: ExtensionPoint,
    pub name: String,
}

impl CopyTarget {
    /// Ziel mit Erweiterungspunkt und Name (beim Workflow immer „workflow“)
    pub fn new(point: ExtensionPoint, name: impl Into<String>) -> Self {
        Self {
            point,
            name: name.into(),
        }
    }
}

/// Vorgänge des Skills: Standard als Ausgangspunkt kopieren (mit Basis), nur die Basis
/// erneuern, Anpassung löschen.
#[derive(Debug, Clone)]
pub struct CustomizationCopier<'a> {
    paths: &'a CustomizationPaths,
    snapshot: BaseSnapshot,
    rules: NameRules,
}

impl<'a> CustomizationCopier<'a> {
    /// Kopierer für diese Orte
    pub fn new(paths: &'a CustomizationPaths) -> Self {
        Self {
            paths,
            snapshot: BaseSnapshot::new(),
            rules: NameRules::new(),
        }
    }

    /// Kopiert den aktuellen Standard nach custom/ und vermerkt die Basis; mit `rebase` nur
    /// die Basis erneuern
    pub fn copy(&self, target: &CopyTarget, rebase: bool) -> Result<String> {
        self.check_name(target)?;
        let custom = self.paths.custom_of(target.point, &target.name);
        let label = label(target.point, &target.name);
        // Nur Basis erneuern: die Anpassung bleibt unverändert
        if rebase {
            if !custom.exists() {
                return Err(HarnessError::new(
                    "customization-missing",
                    format!("{label} ist nicht angepasst."),
                )
                .into());
            }
            self.save_base(target)?;
            return Ok(format!(
                "Basis von {label} auf den aktuellen Standard gesetzt. Wirkt nach dem Bau des Plugins und nexus install."
            ));
        }
        if custom.exists() {
            return Err(HarnessError::new(
                "customization-exists",
                format!(
                    "{label} ist schon angepasst ({}). Nur die Basis erneuern: copy mit --rebase.",
                    self.paths.display(&custom)
                ),
            )
            .into());
        }
        // Standard zuerst prüfen, damit ein abgelehnter Vorgang keinen leeren Ordner hinterlässt
        let source = match target.point {
            ExtensionPoint::Workflow => None,
            _ => Some(self.standard(target)?),
        };
        if let Some(parent) = custom.parent() {
            std::fs::create_dir_all(parent).map_err(Error::unexpected)?;
        }
        match source {
            None => std::fs::write(&custom, self.workflow_source()?).map_err(Error::unexpected)?,
            Some(source) => copy_path(&source, &custom)?,
        }
        self.save_base(target)?;
        Ok(format!(
            "{label} nach {} kopiert. Wirkt nach dem Bau des Plugins und nexus install.",
            self.paths.display(&custom)
        ))
    }

    /// Löscht Anpassung und Basis; danach gilt nach dem Bau des Plugins wieder der Standard
    pub fn reset(&self, target: &CopyTarget) -> Result<String> {
        self.check_name(target)?;
        let custom = self.paths.custom_of(target.point, &target.name);
        let label = label(target.point, &target.name);
        if !custom.exists() {
            return Err(HarnessError::new(
                "customization-missing",
                format!("{label} ist nicht angepasst."),
            )
            .into());
        }
        remove_path(&custom)?;
        remove_path(&self.paths.base_of(target.point, &target.name))?;
        Ok(format!(
            "{label} gelöscht. Wirkt nach dem Bau des Plugins und nexus install: dann gilt wieder der Standard."
        ))
    }

    /// Verhindert Pfadausbrüche: Skill- und Output-Style-Namen müssen den Namensregeln
    /// genügen (Workflow ist fest)
    fn check_name(&self, target: &CopyTarget) -> Result<()> {
        if target.point == ExtensionPoint::Workflow || self.rules.is_valid(&target.name) {
            return Ok(());
        }
        Err(HarnessError::new(
            "customization-invalid-name",
            format!(
                "Der Name {} ist nicht zulässig (Kleinbuchstaben, Ziffern, einzelne Bindestriche, höchstens 64 Zeichen).",
                target.name
            ),
        )
        .into())
    }

    /// Basis: beim Workflow der Standard als JSON, sonst eine Kopie der Standard-Datei bzw.
    /// des Ordners
    fn save_base(&self, target: &CopyTarget) -> Result<()> {
        let base = self.paths.base_of(target.point, &target.name);
        if target.point == ExtensionPoint::Workflow {
            return self
                .snapshot
                .save_text(&BaseSnapshot::format_json(&workflow_phases())?, &base);
        }
        self.snapshot.save(&self.standard(target)?, &base)
    }

    /// Ort des Standards; fehlt er, ist es kein Ersatz, sondern etwas Neues
    fn standard(&self, target: &CopyTarget) -> Result<PathBuf> {
        match self.paths.standard_of(target.point, &target.name) {
            Some(standard) if standard.exists() => Ok(standard),
            _ => Err(HarnessError::new(
                "customization-no-standard",
                format!(
                    "Einen mitgelieferten {} gibt es nicht. Neue legst du direkt in {} an.",
                    label(target.point, &target.name),
                    self.paths
                        .display(self.paths.custom_of(target.point, &target.name))
                ),
            )
            .into()),
        }
    }

    /// Ausgangspunkt für `custom/workflow.ts`: der Standard als Literal mit Typprüfung
    fn workflow_source(&self) -> Result<String> {
        let phases = serde_json::to_string_pretty(&workflow_phases()).map_err(Error::unexpected)?;
        Ok([
            "import type { WorkflowPhase } from '@harness/core';",
            "",
            "// Eigener Workflow: ersetzt den Standard vollständig. Pflicht-Phasen: planen, umsetzen. Reserviert: fragen, nebenbei. Wirkt nach dem Bau des Plugins und nexus install.",
            &format!("export default {phases} satisfies WorkflowPhase[];"),
            "",
        ]
        .join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::customization::overview::BaseState;

    struct Beispiel {
        dir: tempfile::TempDir,
        paths: CustomizationPaths,
    }

    impl Beispiel {
        fn neu() -> Self {
            let dir = tempfile::tempdir().expect("Temp-Ordner");
            let root = dir.path().to_path_buf();
            let paths = CustomizationPaths::new(root.join("custom"), root.join("std"));
            let skill = root.join("std").join("skills").join("harness-grill");
            std::fs::create_dir_all(&skill).unwrap();
            std::fs::write(skill.join("SKILL.md"), "Standard").unwrap();
            Self { dir, paths }
        }

        fn root(&self) -> PathBuf {
            self.dir.path().to_path_buf()
        }
    }

    fn grill() -> CopyTarget {
        CopyTarget::new(ExtensionPoint::Skill, "harness-grill")
    }

    fn workflow() -> CopyTarget {
        CopyTarget::new(ExtensionPoint::Workflow, "workflow")
    }

    fn code<T>(result: Result<T>) -> Option<String> {
        result
            .err()
            .and_then(|error| error.code().map(ToString::to_string))
    }

    #[test]
    fn kopiert_den_workflow_als_typescript_mit_basis() {
        let beispiel = Beispiel::neu();
        let copier = CustomizationCopier::new(&beispiel.paths);
        assert!(copier
            .copy(&workflow(), false)
            .unwrap()
            .contains("Wirkt nach dem Bau des Plugins"));
        let source = std::fs::read_to_string(beispiel.paths.workflow_file()).unwrap();
        assert!(source.contains("import type { WorkflowPhase } from '@harness/core';"));
        assert!(source.contains("satisfies WorkflowPhase[]"));
        assert_eq!(
            std::fs::read_to_string(beispiel.paths.base_of(ExtensionPoint::Workflow, "workflow"))
                .unwrap(),
            BaseSnapshot::format_json(&workflow_phases()).unwrap()
        );
        assert_eq!(
            code(copier.copy(&workflow(), false)).as_deref(),
            Some("customization-exists")
        );
    }

    #[test]
    fn kopiert_einen_skill_mit_basis_und_verweigert_unbekanntes() {
        let beispiel = Beispiel::neu();
        let copier = CustomizationCopier::new(&beispiel.paths);
        copier.copy(&grill(), false).unwrap();
        assert_eq!(
            std::fs::read_to_string(
                beispiel
                    .paths
                    .custom_of(ExtensionPoint::Skill, "harness-grill")
                    .join("SKILL.md")
            )
            .unwrap(),
            "Standard"
        );
        assert!(beispiel
            .paths
            .base_of(ExtensionPoint::Skill, "harness-grill")
            .join("SKILL.md")
            .exists());
        assert_eq!(
            code(copier.copy(
                &CopyTarget::new(ExtensionPoint::Skill, "gibt-es-nicht"),
                false
            ))
            .as_deref(),
            Some("customization-no-standard")
        );
    }

    #[test]
    fn erneuert_nur_die_basis() {
        let beispiel = Beispiel::neu();
        let copier = CustomizationCopier::new(&beispiel.paths);
        copier.copy(&grill(), false).unwrap();
        std::fs::write(
            beispiel
                .paths
                .custom_of(ExtensionPoint::Skill, "harness-grill")
                .join("SKILL.md"),
            "Eigen",
        )
        .unwrap();
        let standard = beispiel
            .paths
            .standard_of(ExtensionPoint::Skill, "harness-grill")
            .unwrap();
        std::fs::write(standard.join("SKILL.md"), "Standard neu").unwrap();
        let snapshot = BaseSnapshot::new();
        let base = beispiel
            .paths
            .base_of(ExtensionPoint::Skill, "harness-grill");
        assert_eq!(
            snapshot.state(&base, &standard).unwrap(),
            BaseState::Outdated
        );
        copier.copy(&grill(), true).unwrap();
        assert_eq!(
            snapshot.state(&base, &standard).unwrap(),
            BaseState::Current
        );
        assert_eq!(
            std::fs::read_to_string(
                beispiel
                    .paths
                    .custom_of(ExtensionPoint::Skill, "harness-grill")
                    .join("SKILL.md")
            )
            .unwrap(),
            "Eigen"
        );
        assert_eq!(
            code(copier.copy(&CopyTarget::new(ExtensionPoint::OutputStyle, "x"), true)).as_deref(),
            Some("customization-missing")
        );
    }

    #[test]
    fn setzt_auf_den_standard_zuruck() {
        let beispiel = Beispiel::neu();
        let copier = CustomizationCopier::new(&beispiel.paths);
        copier.copy(&grill(), false).unwrap();
        assert!(copier
            .reset(&grill())
            .unwrap()
            .contains("Wirkt nach dem Bau des Plugins"));
        assert!(!beispiel
            .paths
            .custom_of(ExtensionPoint::Skill, "harness-grill")
            .exists());
        assert!(!beispiel
            .paths
            .base_of(ExtensionPoint::Skill, "harness-grill")
            .exists());
        assert_eq!(
            code(copier.reset(&grill())).as_deref(),
            Some("customization-missing")
        );
    }

    #[test]
    fn lehnt_unzulassige_namen_ab_bevor_etwas_geloscht_wird() {
        let beispiel = Beispiel::neu();
        let copier = CustomizationCopier::new(&beispiel.paths);
        copier.copy(&grill(), false).unwrap();
        std::fs::write(beispiel.root().join("marker.txt"), "x").unwrap();
        for target in [
            CopyTarget::new(ExtensionPoint::Skill, "../.."),
            CopyTarget::new(ExtensionPoint::Skill, ".."),
        ] {
            assert_eq!(
                code(copier.reset(&target)).as_deref(),
                Some("customization-invalid-name")
            );
        }
        assert_eq!(
            code(copier.copy(&CopyTarget::new(ExtensionPoint::OutputStyle, "../x"), false))
                .as_deref(),
            Some("customization-invalid-name")
        );
        assert!(beispiel.root().join("marker.txt").exists());
        assert!(beispiel
            .paths
            .custom_of(ExtensionPoint::Skill, "harness-grill")
            .exists());
    }

    #[test]
    fn legt_bei_unbekanntem_standard_keinen_ordner_an() {
        let beispiel = Beispiel::neu();
        let copier = CustomizationCopier::new(&beispiel.paths);
        assert_eq!(
            code(copier.copy(
                &CopyTarget::new(ExtensionPoint::Skill, "gibt-es-nicht"),
                false
            ))
            .as_deref(),
            Some("customization-no-standard")
        );
        assert!(!beispiel.paths.skills_dir().exists());
    }
}
