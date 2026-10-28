//! Portiert `packages/core/src/customization/OutputStyleCustomization.ts`.

use std::path::Path;

use crate::error::{Error, Result};

use super::base_snapshot::{copy_path, remove_path, BaseSnapshot};
use super::front_matter::FrontMatter;
use super::name_rules::NameRules;
use super::overview::{CustomizationEntry, EntryKind, EntryState, ExtensionPoint, Problem};
use super::paths::CustomizationPaths;

/// Eigene Output-Styles in custom/output-styles/<name>.md: prüfen und über die des gebauten
/// Plugins legen.
#[derive(Debug, Clone)]
pub struct OutputStyleCustomization<'a> {
    paths: &'a CustomizationPaths,
    snapshot: BaseSnapshot,
    rules: NameRules,
    front: FrontMatter,
}

impl<'a> OutputStyleCustomization<'a> {
    /// Prüfer für diese Orte
    pub fn new(paths: &'a CustomizationPaths) -> Self {
        Self {
            paths,
            snapshot: BaseSnapshot::new(),
            rules: NameRules::new(),
            front: FrontMatter::new(),
        }
    }

    /// Ein Eintrag je .md-Datei, nach Name sortiert; andere Dateien zählen nicht
    pub fn check(&self) -> Vec<CustomizationEntry> {
        let Ok(entries) = std::fs::read_dir(self.paths.output_styles_dir()) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter_map(|file| file.strip_suffix(".md").map(ToString::to_string))
            .collect();
        names.sort();
        names.iter().map(|name| self.safe_entry(name)).collect()
    }

    /// Aktive Output-Styles ersetzen die gleichnamige Datei oder kommen neu dazu
    pub fn apply(&self, entries: &[CustomizationEntry], plugin_dir: &Path) -> Result<()> {
        for entry in entries.iter().filter(|entry| {
            entry.point == ExtensionPoint::OutputStyle && entry.state == EntryState::Active
        }) {
            let dir = plugin_dir.join("output-styles");
            let target = dir.join(format!("{}.md", entry.name));
            std::fs::create_dir_all(&dir).map_err(Error::unexpected)?;
            remove_path(&target)?;
            copy_path(
                &self
                    .paths
                    .custom_of(ExtensionPoint::OutputStyle, &entry.name),
                &target,
            )?;
        }
        Ok(())
    }

    /// Wie `entry`, aber ein unerwarteter Fehler betrifft nur diesen Output-Style und führt
    /// zum Rückfall
    fn safe_entry(&self, name: &str) -> CustomizationEntry {
        match self.entry(name) {
            Ok(entry) => entry,
            Err(error) => {
                let file = self
                    .paths
                    .display(self.paths.custom_of(ExtensionPoint::OutputStyle, name));
                CustomizationEntry {
                    state: EntryState::Fallback,
                    problems: vec![Problem::at(
                        "check-failed",
                        format!("Prüfung fehlgeschlagen: {error}"),
                        file,
                    )],
                    ..CustomizationEntry::new(ExtensionPoint::OutputStyle, name, self.kind(name))
                }
            }
        }
    }

    /// Ein Standard-Output-Style gleichen Namens wird ersetzt, sonst ist er neu
    fn kind(&self, name: &str) -> EntryKind {
        if self
            .paths
            .standard_of(ExtensionPoint::OutputStyle, name)
            .is_some_and(|standard| standard.exists())
        {
            EntryKind::Replace
        } else {
            EntryKind::New
        }
    }

    /// Bewertet einen Output-Style: Rückfall bei Problemen, sonst aktiv; ersetzte mit
    /// Basisvergleich
    fn entry(&self, name: &str) -> Result<CustomizationEntry> {
        let standard = self
            .paths
            .standard_of(ExtensionPoint::OutputStyle, name)
            .expect("Output-Styles haben immer einen Standard-Pfad");
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
            ..CustomizationEntry::new(ExtensionPoint::OutputStyle, name, kind)
        };
        if !leer || kind == EntryKind::New {
            return Ok(entry);
        }
        let base = self.snapshot.check(
            &self.paths.base_of(ExtensionPoint::OutputStyle, name),
            &standard,
        )?;
        let file = self
            .paths
            .display(self.paths.custom_of(ExtensionPoint::OutputStyle, name));
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

    /// Name zulässig, Kopf lesbar, description gesetzt, name im Kopf gleich dem Dateinamen
    fn problems(&self, name: &str) -> Result<Vec<Problem>> {
        let file = self.paths.custom_of(ExtensionPoint::OutputStyle, name);
        let at = self.paths.display(&file);
        if !self.rules.is_valid(name) {
            return Ok(vec![Problem::at(
                "name-invalid",
                format!("Der Name {name} ist nicht zulässig (Kleinbuchstaben, Ziffern, einzelne Bindestriche, höchstens 64 Zeichen)."),
                at,
            )]);
        }
        let text = std::fs::read_to_string(&file).map_err(Error::unexpected)?;
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
                    format!("name im Kopf ({declared}) passt nicht zum Dateinamen {name}."),
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
    use crate::customization::overview::EntryBase;

    struct Beispiel {
        dir: tempfile::TempDir,
        paths: CustomizationPaths,
    }

    impl Beispiel {
        /// Dieselben Output-Styles wie im TypeScript-Test
        fn neu() -> Self {
            let dir = tempfile::tempdir().expect("Temp-Ordner");
            let root = dir.path().to_path_buf();
            let paths = CustomizationPaths::new(root.join("custom"), root.join("std"));
            let beispiel = Self { dir, paths };
            beispiel.style(
                &beispiel.root().join("std").join("output-styles"),
                "software-engineer.md",
                "---\nname: software-engineer\ndescription: Standard\n---\n",
            );
            let own = beispiel.root().join("custom").join("output-styles");
            beispiel.style(
                &own,
                "software-engineer.md",
                "---\nname: software-engineer\ndescription: Eigen\n---\nEigener Stil",
            );
            beispiel.style(&own, "kurz.md", "---\ndescription: Kurz\n---\n");
            beispiel.style(&own, "leer.md", "---\nname: leer\n---\n");
            beispiel.style(&own, "anders.md", "---\nname: x\ndescription: d\n---\n");
            beispiel.style(&own, "notiz.txt", "egal");
            beispiel
        }

        fn root(&self) -> PathBuf {
            self.dir.path().to_path_buf()
        }

        fn style(&self, dir: &Path, file: &str, text: &str) {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(file), text).unwrap();
        }

        fn eintraege(&self) -> Vec<CustomizationEntry> {
            OutputStyleCustomization::new(&self.paths).check()
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
    fn pruft_jeden_eigenen_output_style() {
        let beispiel = Beispiel::neu();
        let summary: Vec<(String, EntryKind, EntryState, EntryBase, String)> = beispiel
            .eintraege()
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
                    "anders".to_string(),
                    EntryKind::New,
                    EntryState::Fallback,
                    EntryBase::None,
                    "name-mismatch".to_string()
                ),
                (
                    "kurz".to_string(),
                    EntryKind::New,
                    EntryState::Active,
                    EntryBase::None,
                    String::new()
                ),
                (
                    "leer".to_string(),
                    EntryKind::New,
                    EntryState::Fallback,
                    EntryBase::None,
                    "description-missing".to_string()
                ),
                (
                    "software-engineer".to_string(),
                    EntryKind::Replace,
                    EntryState::Active,
                    EntryBase::Unknown,
                    String::new()
                ),
            ]
        );
    }

    #[test]
    fn meldet_ein_kaputtes_yaml_im_kopf_als_solches() {
        let beispiel = Beispiel::neu();
        beispiel.style(
            &beispiel.root().join("custom").join("output-styles"),
            "yaml-kaputt.md",
            "---\ndescription: Review: prüft vor dem Abschluss\n---\n",
        );
        let entry = beispiel.eintrag("yaml-kaputt");
        assert_eq!(entry.state, EntryState::Fallback);
        assert_eq!(entry.problems.len(), 1);
        assert_eq!(entry.problems[0].code, "front-matter-invalid");
        assert_eq!(
            entry.problems[0].file.as_deref(),
            Some("custom/output-styles/yaml-kaputt.md")
        );
        assert!(entry.problems[0]
            .message
            .starts_with("Der Kopf ist kein gültiges YAML: "));
        assert!(!entry.problems[0].message.contains('\n'));
    }

    #[test]
    fn lasst_einen_unerwarteten_fehler_die_anderen_nicht_verwerfen() {
        let beispiel = Beispiel::neu();
        // ordner-stil.md als Ordner: wird gelistet, das Lesen scheitert
        std::fs::create_dir_all(
            beispiel
                .root()
                .join("custom")
                .join("output-styles")
                .join("ordner-stil.md"),
        )
        .unwrap();
        let broken = beispiel.eintrag("ordner-stil");
        assert_eq!(broken.point, ExtensionPoint::OutputStyle);
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
            Some("custom/output-styles/ordner-stil.md")
        );
        // Die übrigen Output-Styles werden weiter bewertet
        assert_eq!(beispiel.eintrag("kurz").state, EntryState::Active);
        assert_eq!(
            beispiel.eintrag("software-engineer").kind,
            EntryKind::Replace
        );
        assert_eq!(
            beispiel.eintrag("software-engineer").state,
            EntryState::Active
        );
        assert_eq!(codes(&beispiel.eintrag("anders").problems), "name-mismatch");
    }

    #[test]
    fn legt_gultige_output_styles_uber_das_plugin() {
        let beispiel = Beispiel::neu();
        let plugin = beispiel.root().join("plugin");
        beispiel.style(
            &plugin.join("output-styles"),
            "software-engineer.md",
            "Standard",
        );
        let custom = OutputStyleCustomization::new(&beispiel.paths);
        custom.apply(&custom.check(), &plugin).unwrap();
        let text =
            std::fs::read_to_string(plugin.join("output-styles").join("software-engineer.md"))
                .unwrap();
        assert!(text.contains("Eigener Stil"));
        assert!(plugin.join("output-styles").join("kurz.md").exists());
        assert!(!plugin.join("output-styles").join("leer.md").exists());
    }

    #[test]
    fn vergleicht_die_basis_ersetzter_output_styles() {
        let beispiel = Beispiel::neu();
        let snapshot = BaseSnapshot::new();
        let standard = beispiel
            .root()
            .join("std")
            .join("output-styles")
            .join("software-engineer.md");
        // Ohne Basis: unbekannt mit Hinweis
        let fresh = beispiel.eintrag("software-engineer");
        assert_eq!(fresh.base, EntryBase::Unknown);
        assert_eq!(codes(&fresh.warnings), "base-unknown");
        assert_eq!(
            fresh.warnings[0].file.as_deref(),
            Some("custom/output-styles/software-engineer.md")
        );
        // Basis als Kopie des Standards: aktuell, ohne Warnung und ohne Diff
        snapshot
            .save(
                &standard,
                &beispiel
                    .paths
                    .base_of(ExtensionPoint::OutputStyle, "software-engineer"),
            )
            .unwrap();
        let current = beispiel.eintrag("software-engineer");
        assert_eq!(current.base, EntryBase::Current);
        assert!(current.warnings.is_empty());
        assert_eq!(current.diff, None);
        // Standard geändert: veraltet mit Warnung, Ort und Diff
        std::fs::write(
            &standard,
            "---\nname: software-engineer\ndescription: Standard neu\n---\n",
        )
        .unwrap();
        let outdated = beispiel.eintrag("software-engineer");
        assert_eq!(outdated.base, EntryBase::Outdated);
        assert_eq!(codes(&outdated.warnings), "base-outdated");
        assert_eq!(
            outdated.warnings[0].file.as_deref(),
            Some("custom/output-styles/software-engineer.md")
        );
        assert!(outdated
            .diff
            .unwrap()
            .contains("+description: Standard neu"));
    }
}
