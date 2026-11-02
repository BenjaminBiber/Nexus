//! Portiert `packages/core/src/customization/CustomizationApplier.ts`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::clock::Clock;
use crate::error::{Error, Result};

use super::base_snapshot::{remove_path, BaseSnapshot};
use super::output_style::OutputStyleCustomization;
use super::overview::{
    CustomizationEntry, CustomizationOverview, EntryState, ExtensionPoint, WorkflowState,
};
use super::paths::CustomizationPaths;
use super::skill::SkillCustomization;
use super::tools::WorkflowTools;
use super::workflow::WorkflowCustomization;

/// Was geprüft werden soll: Clone, Anpassungen, Standard, Fassung und die Werkzeuge für
/// `custom/workflow.ts`
pub struct CustomizationInput<'a> {
    pub clone_dir: PathBuf,
    pub custom_dir: PathBuf,
    /// Ordner mit skills/ und output-styles/ des Standards (Manifest oder frisch gebautes Plugin)
    pub standard_dir: PathBuf,
    pub harness_version: String,
    pub tools: &'a dyn WorkflowTools,
    /// Uhr für `createdAt`; Tests setzen eine feste Zeit
    pub clock: Clock,
}

/// Ergebnis der Prüfung: Übersicht und, bei aktivem eigenem Workflow, dessen Phasen
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub overview: CustomizationOverview,
    /// Phasen des aktiven eigenen Workflows, sonst None
    pub phases: Option<Value>,
}

/// Ablauf für `setup` und `customize check`: alle Anpassungen prüfen, Übersicht bauen,
/// ins Plugin schreiben.
#[derive(Debug, Clone, Copy, Default)]
pub struct CustomizationApplier;

impl CustomizationApplier {
    /// Neuer Ablauf (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Prüft alle Anpassungen, ohne etwas zu schreiben; aktive eigene Skills gelten für die
    /// Verweise im Workflow
    pub fn check(&self, input: &CustomizationInput<'_>) -> CheckResult {
        let paths = CustomizationPaths::new(&input.custom_dir, &input.standard_dir);
        let skills = SkillCustomization::new(&paths).check();
        let styles = OutputStyleCustomization::new(&paths).check();
        let mut available = standard_skills(&input.standard_dir);
        available.extend(
            skills
                .iter()
                .filter(|entry| entry.state == EntryState::Active)
                .map(|entry| entry.name.clone()),
        );
        let workflow = WorkflowCustomization::new(&paths, input.tools).check(&available);
        let mut entries: Vec<CustomizationEntry> = workflow.entry.into_iter().collect();
        entries.extend(skills);
        entries.extend(styles);
        CheckResult {
            overview: self.overview(input, entries),
            phases: workflow.phases,
        }
    }

    /// Wendet die Anpassungen auf das gebaute Plugin an und schreibt customizations/ neu
    pub fn apply(
        &self,
        input: &CustomizationInput<'_>,
        plugin_dir: &Path,
    ) -> Result<CustomizationOverview> {
        let paths = CustomizationPaths::new(&input.custom_dir, &input.standard_dir);
        // Ein kaputter Ordner der Anpassungen (z. B. custom/skills als Datei) darf nicht
        // stillschweigend als „keine Anpassungen“ durchgehen: der Aufrufer fällt dann sichtbar
        // auf den Standard zurück, statt die Anpassungen des Devs einfach zu verlieren
        muss_ordner_sein(&paths.skills_dir())?;
        muss_ordner_sein(&paths.output_styles_dir())?;
        let CheckResult { overview, phases } = self.check(input);
        // Gültige Skills und Output-Styles überlagern das gebaute Plugin
        SkillCustomization::new(&paths).apply(&overview.entries, plugin_dir)?;
        OutputStyleCustomization::new(&paths).apply(&overview.entries, plugin_dir)?;
        // Alter Ordner aus einem früheren Lauf wird vollständig ersetzt
        let dir = plugin_dir.join("customizations");
        remove_path(&dir)?;
        std::fs::create_dir_all(&dir).map_err(Error::unexpected)?;
        if let Some(phases) = phases {
            std::fs::write(
                dir.join("workflow.json"),
                BaseSnapshot::format_json(&phases)?,
            )
            .map_err(Error::unexpected)?;
        }
        std::fs::write(
            dir.join("overview.json"),
            BaseSnapshot::format_json(&overview)?,
        )
        .map_err(Error::unexpected)?;
        Ok(overview)
    }

    /// Übersicht mit Zustand des Workflows
    fn overview(
        &self,
        input: &CustomizationInput<'_>,
        entries: Vec<CustomizationEntry>,
    ) -> CustomizationOverview {
        let workflow = entries
            .iter()
            .find(|entry| entry.point == ExtensionPoint::Workflow);
        CustomizationOverview {
            version: 1,
            created_at: input.clock.iso(),
            clone_dir: input.clone_dir.to_string_lossy().into_owned(),
            custom_dir: input.custom_dir.to_string_lossy().into_owned(),
            harness_version: input.harness_version.clone(),
            workflow: match workflow {
                None => WorkflowState::Standard,
                Some(entry) if entry.state == EntryState::Active => WorkflowState::Custom,
                Some(_) => WorkflowState::Fallback,
            },
            entries,
        }
    }
}

/// Fehler, wenn der Pfad vorhanden, aber kein Ordner ist; fehlt er, ist alles in Ordnung
fn muss_ordner_sein(dir: &Path) -> Result<()> {
    if dir.exists() && !dir.is_dir() {
        return Err(Error::unexpected(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            format!("{} ist kein Ordner", dir.display()),
        )));
    }
    Ok(())
}

/// Namen der mitgelieferten Skills (Ordner mit SKILL.md)
fn standard_skills(standard_dir: &Path) -> Vec<String> {
    let dir = standard_dir.join("skills");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| dir.join(name).join("SKILL.md").exists())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::customization::overview::Problem;
    use crate::workflow::workflow_phases;

    /// Werkzeuge ohne tsc und esbuild: liefern einen vorgegebenen Wert
    struct Fake(Value);

    impl WorkflowTools for Fake {
        fn typecheck(&self, _file: &Path) -> Result<Vec<Problem>> {
            Ok(Vec::new())
        }

        fn load(&self, _file: &Path) -> std::result::Result<Value, String> {
            Ok(self.0.clone())
        }
    }

    /// Die vier Skills, auf die der Standard-Workflow verweist
    const STANDARD_SKILLS: [&str; 4] = [
        "harness-grill",
        "harness-wayfinder",
        "harness-tasks",
        "harness-implement",
    ];

    fn jetzt() -> Clock {
        Clock::fixed(
            chrono::DateTime::parse_from_rfc3339("2026-10-14T10:00:00Z")
                .unwrap()
                .into(),
        )
    }

    struct Beispiel {
        dir: tempfile::TempDir,
    }

    impl Beispiel {
        fn neu() -> Self {
            let beispiel = Self {
                dir: tempfile::tempdir().expect("Temp-Ordner"),
            };
            for name in STANDARD_SKILLS {
                let dir = beispiel.plugin().join("skills").join(name);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(
                    dir.join("SKILL.md"),
                    format!("---\nname: {name}\ndescription: d\n---\n"),
                )
                .unwrap();
            }
            beispiel
        }

        fn root(&self) -> PathBuf {
            self.dir.path().to_path_buf()
        }

        fn plugin(&self) -> PathBuf {
            self.root().join("plugin")
        }

        fn eingabe<'a>(&self, tools: &'a Fake) -> CustomizationInput<'a> {
            CustomizationInput {
                clone_dir: self.root(),
                custom_dir: self.root().join("custom"),
                standard_dir: self.plugin(),
                harness_version: "0.2.0".to_string(),
                tools,
                clock: jetzt(),
            }
        }

        /// Legt `custom/workflow.ts` an; die Werkzeuge liefern `value` als Default-Export
        fn mit_workflow(&self) {
            std::fs::create_dir_all(self.root().join("custom")).unwrap();
            std::fs::write(self.root().join("custom").join("workflow.ts"), "// ersetzt").unwrap();
        }
    }

    fn standard() -> Value {
        serde_json::to_value(workflow_phases()).unwrap()
    }

    /// Der Standard mit einem zusätzlichen Schritt in der Phase „umsetzen“
    fn mit_schritt(step: Value) -> Value {
        let mut phases = standard();
        for phase in phases.as_array_mut().unwrap() {
            if phase["id"] == "umsetzen" {
                phase["steps"].as_array_mut().unwrap().push(step.clone());
            }
        }
        phases
    }

    fn codes(problems: &[Problem]) -> Vec<&str> {
        problems.iter().map(|p| p.code.as_str()).collect()
    }

    #[test]
    fn ein_kaputter_ordner_der_anpassungen_ist_ein_fehler_und_kein_leeres_ergebnis() {
        for name in ["skills", "output-styles"] {
            let beispiel = Beispiel::neu();
            std::fs::create_dir_all(beispiel.root().join("custom")).unwrap();
            // Statt eines Ordners liegt dort eine Datei
            std::fs::write(beispiel.root().join("custom").join(name), "kein Ordner").unwrap();
            let tools = Fake(Value::Null);
            let input = beispiel.eingabe(&tools);

            let fehler = CustomizationApplier::new().apply(&input, &beispiel.plugin());

            assert!(fehler.is_err(), "{name}");
        }
    }

    #[test]
    fn meldet_ohne_custom_den_standard_und_schreibt_nur_die_ubersicht() {
        let beispiel = Beispiel::neu();
        let tools = Fake(Value::Null);
        let input = beispiel.eingabe(&tools);
        let overview = CustomizationApplier::new()
            .apply(&input, &beispiel.plugin())
            .unwrap();

        assert_eq!(
            overview,
            CustomizationOverview {
                version: 1,
                created_at: "2026-10-14T10:00:00.000Z".to_string(),
                clone_dir: beispiel.root().to_string_lossy().into_owned(),
                custom_dir: beispiel
                    .root()
                    .join("custom")
                    .to_string_lossy()
                    .into_owned(),
                harness_version: "0.2.0".to_string(),
                workflow: WorkflowState::Standard,
                entries: Vec::new(),
            }
        );
        let geschrieben: CustomizationOverview = serde_json::from_str(
            &std::fs::read_to_string(
                beispiel
                    .plugin()
                    .join("customizations")
                    .join("overview.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(geschrieben, overview);
        assert!(!beispiel
            .plugin()
            .join("customizations")
            .join("workflow.json")
            .exists());
    }

    #[test]
    fn schreibt_einen_gultigen_eigenen_workflow() {
        let beispiel = Beispiel::neu();
        beispiel.mit_workflow();
        let phases = mit_schritt(serde_json::json!({
            "step": "Grill", "command": "/nexus:harness-grill", "when": "x",
        }));
        let tools = Fake(phases.clone());
        let input = beispiel.eingabe(&tools);

        let overview = CustomizationApplier::new()
            .apply(&input, &beispiel.plugin())
            .unwrap();

        assert_eq!(overview.workflow, WorkflowState::Custom);
        assert_eq!(codes(&overview.entries[0].warnings), vec!["base-unknown"]);
        assert_eq!(
            std::fs::read_to_string(
                beispiel
                    .plugin()
                    .join("customizations")
                    .join("workflow.json")
            )
            .unwrap(),
            BaseSnapshot::format_json(&phases).unwrap()
        );
    }

    #[test]
    fn fallt_bei_ungultigem_workflow_zuruck_und_ersetzt_alte_ergebnisse() {
        let beispiel = Beispiel::neu();
        let dir = beispiel.plugin().join("customizations");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("workflow.json"), "[]").unwrap();
        beispiel.mit_workflow();
        let tools = Fake(serde_json::json!({}));
        let input = beispiel.eingabe(&tools);

        let overview = CustomizationApplier::new()
            .apply(&input, &beispiel.plugin())
            .unwrap();

        assert_eq!(overview.workflow, WorkflowState::Fallback);
        assert!(!dir.join("workflow.json").exists());
    }

    #[test]
    fn pruft_ohne_zu_schreiben() {
        let beispiel = Beispiel::neu();
        beispiel.mit_workflow();
        let tools = Fake(standard());
        let input = beispiel.eingabe(&tools);

        let result = CustomizationApplier::new().check(&input);

        assert_eq!(result.overview.workflow, WorkflowState::Custom);
        assert_eq!(result.phases, Some(standard()));
        assert!(!beispiel.plugin().join("customizations").exists());
    }

    #[test]
    fn zahlt_aktive_eigene_skills_bei_den_verweisen_des_workflows() {
        let beispiel = Beispiel::neu();
        beispiel.mit_workflow();
        let phases = mit_schritt(serde_json::json!({
            "step": "Review", "command": "/nexus:mein-review", "when": "x",
        }));
        let tools = Fake(phases);
        let input = beispiel.eingabe(&tools);
        let skill = beispiel
            .root()
            .join("custom")
            .join("skills")
            .join("mein-review");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\ndescription: Neu\n---\n").unwrap();

        let ok = CustomizationApplier::new().check(&input).overview;
        assert_eq!(
            ok.entries
                .iter()
                .map(|entry| format!("{}:{}", entry.point.as_str(), entry.name))
                .collect::<Vec<_>>(),
            vec!["workflow:workflow", "skill:mein-review"]
        );
        assert_eq!(codes(&ok.entries[0].warnings), vec!["base-unknown"]);

        std::fs::write(skill.join("SKILL.md"), "ohne Kopf").unwrap();
        let missing = CustomizationApplier::new().check(&input).overview;
        assert!(codes(&missing.entries[0].warnings).contains(&"skill-ref-missing"));
    }

    #[test]
    fn legt_skills_und_output_styles_uber_das_plugin() {
        let beispiel = Beispiel::neu();
        let skill = beispiel
            .root()
            .join("custom")
            .join("skills")
            .join("harness-grill");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            "---\ndescription: Eigene Fassung\n---\n",
        )
        .unwrap();
        let styles = beispiel.root().join("custom").join("output-styles");
        std::fs::create_dir_all(&styles).unwrap();
        std::fs::write(styles.join("kurz.md"), "---\ndescription: Kurz\n---\n").unwrap();
        let tools = Fake(Value::Null);
        let input = beispiel.eingabe(&tools);

        let overview = CustomizationApplier::new()
            .apply(&input, &beispiel.plugin())
            .unwrap();

        assert_eq!(
            overview
                .entries
                .iter()
                .map(|entry| format!(
                    "{}:{}:{}",
                    entry.point.as_str(),
                    entry.name,
                    if entry.kind == super::super::overview::EntryKind::Replace {
                        "replace"
                    } else {
                        "new"
                    }
                ))
                .collect::<Vec<_>>(),
            vec!["skill:harness-grill:replace", "output-style:kurz:new"]
        );
        let text = std::fs::read_to_string(
            beispiel
                .plugin()
                .join("skills")
                .join("harness-grill")
                .join("SKILL.md"),
        )
        .unwrap();
        assert!(text.contains("Eigene Fassung"));
        assert!(beispiel
            .plugin()
            .join("output-styles")
            .join("kurz.md")
            .exists());
    }
}
