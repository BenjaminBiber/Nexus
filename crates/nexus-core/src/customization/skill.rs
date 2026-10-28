//! Portiert `packages/core/src/customization/SkillCustomization.ts`.

use std::path::Path;

use crate::error::{Error, Result};

use super::base_snapshot::{copy_path, remove_path, BaseSnapshot};
use super::front_matter::FrontMatter;
use super::name_rules::NameRules;
use super::overview::{CustomizationEntry, EntryKind, EntryState, ExtensionPoint, Problem};
use super::paths::CustomizationPaths;

/// Eigene Skills in custom/skills/<name>/: prüfen und über die Skills des gebauten Plugins legen.
#[derive(Debug, Clone)]
pub struct SkillCustomization<'a> {
    paths: &'a CustomizationPaths,
    snapshot: BaseSnapshot,
    rules: NameRules,
    front: FrontMatter,
}

impl<'a> SkillCustomization<'a> {
    /// Prüfer für diese Orte
    pub fn new(paths: &'a CustomizationPaths) -> Self {
        Self {
            paths,
            snapshot: BaseSnapshot::new(),
            rules: NameRules::new(),
            front: FrontMatter::new(),
        }
    }

    /// Ein Eintrag je Ordner in custom/skills, nach Name sortiert
    pub fn check(&self) -> Vec<CustomizationEntry> {
        let dir = self.paths.skills_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names.iter().map(|name| self.safe_entry(name)).collect()
    }

    /// Aktive Skills ersetzen den gleichnamigen Standard-Ordner vollständig oder kommen neu dazu
    pub fn apply(&self, entries: &[CustomizationEntry], plugin_dir: &Path) -> Result<()> {
        for entry in entries.iter().filter(|entry| {
            entry.point == ExtensionPoint::Skill && entry.state == EntryState::Active
        }) {
            let target = plugin_dir.join("skills").join(&entry.name);
            remove_path(&target)?;
            copy_path(
                &self.paths.custom_of(ExtensionPoint::Skill, &entry.name),
                &target,
            )?;
        }
        Ok(())
    }

    /// Wie `entry`, aber ein unerwarteter Fehler betrifft nur diesen Skill und führt zum Rückfall
    fn safe_entry(&self, name: &str) -> CustomizationEntry {
        match self.entry(name) {
            Ok(entry) => entry,
            Err(error) => {
                let file = self
                    .paths
                    .display(self.paths.custom_of(ExtensionPoint::Skill, name));
                CustomizationEntry {
                    state: EntryState::Fallback,
                    problems: vec![Problem::at(
                        "check-failed",
                        format!("Prüfung fehlgeschlagen: {error}"),
                        file,
                    )],
                    ..CustomizationEntry::new(ExtensionPoint::Skill, name, self.kind(name))
                }
            }
        }
    }

    /// Ein Standard-Skill gleichen Namens wird ersetzt, sonst ist der Skill neu
    fn kind(&self, name: &str) -> EntryKind {
        let standard = self
            .paths
            .standard_of(ExtensionPoint::Skill, name)
            .expect("Skills haben immer einen Standard-Pfad");
        if standard.join("SKILL.md").exists() {
            EntryKind::Replace
        } else {
            EntryKind::New
        }
    }

    /// Bewertet einen Skill: Rückfall bei Problemen, sonst aktiv; ersetzte Skills mit Basisvergleich
    fn entry(&self, name: &str) -> Result<CustomizationEntry> {
        let standard = self
            .paths
            .standard_of(ExtensionPoint::Skill, name)
            .expect("Skills haben immer einen Standard-Pfad");
        let kind = self.kind(name);
        let problems = self.problems(name)?;
        let leer = problems.is_empty();
        let entry = CustomizationEntry {
            state: if leer {
                EntryState::Active
            } else {
                EntryState::Fallback
            },
            problems,
            ..CustomizationEntry::new(ExtensionPoint::Skill, name, kind)
        };
        if !leer || kind == EntryKind::New {
            return Ok(entry);
        }
        let base = self
            .snapshot
            .check(&self.paths.base_of(ExtensionPoint::Skill, name), &standard)?;
        let file = self
            .paths
            .display(self.paths.custom_of(ExtensionPoint::Skill, name));
        Ok(CustomizationEntry {
            base: base.state.into(),
            warnings: base
                .warnings
                .into_iter()
                .map(|warning| warning.with_file(&file))
                .collect(),
            diff: base.diff,
            ..entry
        })
    }

    /// Name zulässig, SKILL.md vorhanden, Kopf lesbar, description gesetzt, name im Kopf
    /// gleich dem Ordner
    fn problems(&self, name: &str) -> Result<Vec<Problem>> {
        let dir = self.paths.custom_of(ExtensionPoint::Skill, name);
        let file = dir.join("SKILL.md");
        if !self.rules.is_valid(name) {
            return Ok(vec![Problem::at(
                "name-invalid",
                format!("Der Name {name} ist nicht zulässig (Kleinbuchstaben, Ziffern, einzelne Bindestriche, höchstens 64 Zeichen)."),
                self.paths.display(&dir),
            )]);
        }
        if !file.exists() {
            return Ok(vec![Problem::at(
                "skill-file-missing",
                "SKILL.md fehlt.",
                self.paths.display(&dir),
            )]);
        }
        let text = std::fs::read_to_string(&file).map_err(Error::unexpected)?;
        let at = self.paths.display(&file);
        let result = self.front.parse(&text);
        // Ein kaputtes YAML nicht als fehlende description ausgeben, sondern als das, was es ist
        if let Some(error) = result.error {
            return Ok(vec![Problem::at(
                "front-matter-invalid",
                format!("Der Kopf ist kein gültiges YAML: {error}"),
                at,
            )]);
        }
        let mut problems = Vec::new();
        if self.front.text(&result.fields, "description").is_none() {
            problems.push(Problem::at(
                "description-missing",
                "Im Kopf fehlt description.",
                &at,
            ));
        }
        if let Some(declared) = self.front.text(&result.fields, "name") {
            if declared != name {
                problems.push(Problem::at(
                    "name-mismatch",
                    format!("name im Kopf ({declared}) passt nicht zum Ordner {name}."),
                    &at,
                ));
            }
        }
        Ok(problems)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::customization::overview::{BaseState, EntryBase};

    struct Beispiel {
        dir: tempfile::TempDir,
        paths: CustomizationPaths,
    }

    impl Beispiel {
        /// Dieselben Skills wie im TypeScript-Test
        fn neu() -> Self {
            let dir = tempfile::tempdir().expect("Temp-Ordner");
            let root = dir.path().to_path_buf();
            let paths = CustomizationPaths::new(root.join("custom"), root.join("std"));
            let beispiel = Self { dir, paths };
            beispiel.skill(
                &beispiel.root().join("std").join("skills"),
                "harness-grill",
                Some("name: harness-grill\ndescription: Standard"),
            );
            std::fs::write(
                beispiel
                    .root()
                    .join("std")
                    .join("skills")
                    .join("harness-grill")
                    .join("alt.md"),
                "alt",
            )
            .unwrap();
            let own = beispiel.root().join("custom").join("skills");
            beispiel.skill(
                &own,
                "harness-grill",
                Some("name: harness-grill\ndescription: Eigen"),
            );
            beispiel.skill(&own, "mein-review", Some("description: Neu"));
            beispiel.skill(&own, "kaputt", None);
            beispiel.skill(&own, "Gross", Some("description: x"));
            beispiel.skill(&own, "ohne-text", Some("name: ohne-text"));
            beispiel.skill(&own, "falsch", Some("name: anders\ndescription: x"));
            beispiel
        }

        fn root(&self) -> PathBuf {
            self.dir.path().to_path_buf()
        }

        fn skill(&self, base: &Path, name: &str, head: Option<&str>) {
            let dir = base.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            if let Some(head) = head {
                std::fs::write(
                    dir.join("SKILL.md"),
                    format!("---\n{head}\n---\nText von {name}"),
                )
                .unwrap();
            }
        }

        fn eintraege(&self) -> Vec<CustomizationEntry> {
            SkillCustomization::new(&self.paths).check()
        }

        fn eintrag(&self, name: &str) -> CustomizationEntry {
            self.eintraege()
                .into_iter()
                .find(|entry| entry.name == name)
                .unwrap_or_else(|| panic!("Eintrag {name} fehlt"))
        }
    }

    fn codes(problems: &[Problem]) -> String {
        problems
            .iter()
            .map(|problem| problem.code.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }

    #[test]
    fn pruft_jeden_eigenen_skill() {
        let beispiel = Beispiel::neu();
        let entries = beispiel.eintraege();
        let summary: Vec<(String, EntryKind, EntryState, EntryBase, String)> = entries
            .iter()
            .map(|entry| {
                (
                    entry.name.clone(),
                    entry.kind,
                    entry.state,
                    entry.base,
                    codes(&entry.problems),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                (
                    "Gross".to_string(),
                    EntryKind::New,
                    EntryState::Fallback,
                    EntryBase::None,
                    "name-invalid".to_string()
                ),
                (
                    "falsch".to_string(),
                    EntryKind::New,
                    EntryState::Fallback,
                    EntryBase::None,
                    "name-mismatch".to_string()
                ),
                (
                    "harness-grill".to_string(),
                    EntryKind::Replace,
                    EntryState::Active,
                    EntryBase::Unknown,
                    String::new()
                ),
                (
                    "kaputt".to_string(),
                    EntryKind::New,
                    EntryState::Fallback,
                    EntryBase::None,
                    "skill-file-missing".to_string()
                ),
                (
                    "mein-review".to_string(),
                    EntryKind::New,
                    EntryState::Active,
                    EntryBase::None,
                    String::new()
                ),
                (
                    "ohne-text".to_string(),
                    EntryKind::New,
                    EntryState::Fallback,
                    EntryBase::None,
                    "description-missing".to_string()
                ),
            ]
        );
        assert_eq!(
            beispiel.eintrag("kaputt").problems[0].file.as_deref(),
            Some("custom/skills/kaputt")
        );
        assert!(beispiel.eintrag("mein-review").warnings.is_empty());
    }

    #[test]
    fn meldet_ein_kaputtes_yaml_im_kopf_als_solches() {
        let beispiel = Beispiel::neu();
        beispiel.skill(
            &beispiel.root().join("custom").join("skills"),
            "yaml-kaputt",
            Some("description: Review: prüft vor dem Abschluss"),
        );
        let entry = beispiel.eintrag("yaml-kaputt");
        assert_eq!(entry.state, EntryState::Fallback);
        assert_eq!(entry.problems.len(), 1);
        assert_eq!(entry.problems[0].code, "front-matter-invalid");
        assert_eq!(
            entry.problems[0].file.as_deref(),
            Some("custom/skills/yaml-kaputt/SKILL.md")
        );
        assert!(entry.problems[0]
            .message
            .starts_with("Der Kopf ist kein gültiges YAML: "));
        assert!(!entry.problems[0].message.contains('\n'));
    }

    #[test]
    fn lasst_einen_unerwarteten_fehler_die_anderen_nicht_verwerfen() {
        let beispiel = Beispiel::neu();
        // SKILL.md als Ordner: die Datei gibt es scheinbar, das Lesen scheitert
        std::fs::create_dir_all(
            beispiel
                .root()
                .join("custom")
                .join("skills")
                .join("ordner-skill")
                .join("SKILL.md"),
        )
        .unwrap();
        let broken = beispiel.eintrag("ordner-skill");
        assert_eq!(broken.point, ExtensionPoint::Skill);
        assert_eq!(broken.kind, EntryKind::New);
        assert_eq!(broken.state, EntryState::Fallback);
        assert_eq!(broken.base, EntryBase::None);
        assert!(broken.warnings.is_empty());
        assert_eq!(broken.problems.len(), 1);
        assert_eq!(broken.problems[0].code, "check-failed");
        assert!(broken.problems[0]
            .message
            .starts_with("Prüfung fehlgeschlagen: "));
        assert_eq!(
            broken.problems[0].file.as_deref(),
            Some("custom/skills/ordner-skill")
        );
        // Die übrigen Skills werden weiter bewertet
        assert_eq!(beispiel.eintrag("mein-review").state, EntryState::Active);
        assert_eq!(beispiel.eintrag("harness-grill").kind, EntryKind::Replace);
        assert_eq!(beispiel.eintrag("harness-grill").state, EntryState::Active);
        assert_eq!(codes(&beispiel.eintrag("falsch").problems), "name-mismatch");
    }

    #[test]
    fn legt_gultige_skills_uber_das_plugin_und_ubergeht_ungultige() {
        let beispiel = Beispiel::neu();
        let plugin = beispiel.root().join("plugin");
        beispiel.skill(
            &plugin.join("skills"),
            "harness-grill",
            Some("description: Standard"),
        );
        std::fs::write(
            plugin.join("skills").join("harness-grill").join("alt.md"),
            "alt",
        )
        .unwrap();
        let custom = SkillCustomization::new(&beispiel.paths);
        custom.apply(&custom.check(), &plugin).unwrap();
        let text =
            std::fs::read_to_string(plugin.join("skills").join("harness-grill").join("SKILL.md"))
                .unwrap();
        assert!(text.contains("Eigen"));
        assert!(!plugin
            .join("skills")
            .join("harness-grill")
            .join("alt.md")
            .exists());
        assert!(plugin
            .join("skills")
            .join("mein-review")
            .join("SKILL.md")
            .exists());
        assert!(!plugin.join("skills").join("kaputt").exists());
    }

    #[test]
    fn liefert_ohne_ordner_nichts() {
        let beispiel = Beispiel::neu();
        std::fs::remove_dir_all(beispiel.root().join("custom")).unwrap();
        assert!(beispiel.eintraege().is_empty());
    }

    #[test]
    fn vergleicht_die_basis_ersetzter_skills() {
        let beispiel = Beispiel::neu();
        let snapshot = BaseSnapshot::new();
        // Ohne Basis: unbekannt mit Hinweis
        let fresh = beispiel.eintrag("harness-grill");
        assert_eq!(fresh.base, EntryBase::Unknown);
        assert_eq!(codes(&fresh.warnings), "base-unknown");
        assert_eq!(
            fresh.warnings[0].file.as_deref(),
            Some("custom/skills/harness-grill")
        );
        // Basis als Kopie des Standards: aktuell, ohne Warnung und ohne Diff
        let standard = beispiel
            .root()
            .join("std")
            .join("skills")
            .join("harness-grill");
        snapshot
            .save(
                &standard,
                &beispiel
                    .paths
                    .base_of(ExtensionPoint::Skill, "harness-grill"),
            )
            .unwrap();
        let current = beispiel.eintrag("harness-grill");
        assert_eq!(current.base, EntryBase::Current);
        assert!(current.warnings.is_empty());
        assert_eq!(current.diff, None);
        // Standard geändert: veraltet mit Warnung, Ort und Diff
        std::fs::write(
            standard.join("SKILL.md"),
            "---\nname: harness-grill\ndescription: Standard neu\n---\n",
        )
        .unwrap();
        let outdated = beispiel.eintrag("harness-grill");
        assert_eq!(outdated.base, EntryBase::Outdated);
        assert_eq!(codes(&outdated.warnings), "base-outdated");
        assert_eq!(
            outdated.warnings[0].file.as_deref(),
            Some("custom/skills/harness-grill")
        );
        assert!(outdated
            .diff
            .unwrap()
            .contains("+description: Standard neu"));
        assert_eq!(
            snapshot
                .state(
                    &beispiel
                        .paths
                        .base_of(ExtensionPoint::Skill, "harness-grill"),
                    &standard
                )
                .unwrap(),
            BaseState::Outdated
        );
    }
}
