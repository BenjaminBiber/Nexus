//! Portiert `packages/core/src/customization/CustomizationPaths.ts`.

use std::path::{Component, Path, PathBuf};

use super::overview::ExtensionPoint;

/// Orte der Anpassungen in custom/, ihrer Basis in custom/.base/ und des Standards
/// (Manifest oder gebautes Plugin).
#[derive(Debug, Clone)]
pub struct CustomizationPaths {
    custom_dir: PathBuf,
    standard_dir: PathBuf,
}

impl CustomizationPaths {
    /// Orte für diesen Ordner der Anpassungen und diesen Standard
    pub fn new(custom_dir: impl Into<PathBuf>, standard_dir: impl Into<PathBuf>) -> Self {
        Self {
            custom_dir: custom_dir.into(),
            standard_dir: standard_dir.into(),
        }
    }

    /// Ordner der Anpassungen (custom/)
    pub fn custom_dir(&self) -> &Path {
        &self.custom_dir
    }

    /// Ordner des Standards (Manifest oder gebautes Plugin)
    pub fn standard_dir(&self) -> &Path {
        &self.standard_dir
    }

    /// Datei des Workflows in custom/
    pub fn workflow_file(&self) -> PathBuf {
        self.custom_dir.join("workflow.ts")
    }

    /// Ordner der Skills in custom/
    pub fn skills_dir(&self) -> PathBuf {
        self.custom_dir.join("skills")
    }

    /// Ordner der Output-Styles in custom/
    pub fn output_styles_dir(&self) -> PathBuf {
        self.custom_dir.join("output-styles")
    }

    /// Ordner der Basis in custom/.base/
    pub fn base_dir(&self) -> PathBuf {
        self.custom_dir.join(".base")
    }

    /// Ort der Anpassung in custom/
    pub fn custom_of(&self, point: ExtensionPoint, name: &str) -> PathBuf {
        match point {
            ExtensionPoint::Workflow => self.workflow_file(),
            ExtensionPoint::Skill => self.skills_dir().join(name),
            ExtensionPoint::OutputStyle => self.output_styles_dir().join(format!("{name}.md")),
        }
    }

    /// Ort der Basis in custom/.base/; die Basis hat den gleichen relativen Pfad, der
    /// Workflow liegt als JSON vor
    pub fn base_of(&self, point: ExtensionPoint, name: &str) -> PathBuf {
        match point {
            ExtensionPoint::Workflow => self.base_dir().join("workflow.json"),
            ExtensionPoint::Skill => self.base_dir().join("skills").join(name),
            ExtensionPoint::OutputStyle => self
                .base_dir()
                .join("output-styles")
                .join(format!("{name}.md")),
        }
    }

    /// Ort des Standards, None beim Workflow: den gibt es nur als Daten (`workflow_phases`)
    pub fn standard_of(&self, point: ExtensionPoint, name: &str) -> Option<PathBuf> {
        match point {
            ExtensionPoint::Workflow => None,
            ExtensionPoint::Skill => Some(self.standard_dir.join("skills").join(name)),
            ExtensionPoint::OutputStyle => Some(
                self.standard_dir
                    .join("output-styles")
                    .join(format!("{name}.md")),
            ),
        }
    }

    /// Pfad relativ zum Clone mit `/`, für Meldungen
    pub fn display(&self, file: impl AsRef<Path>) -> String {
        // Windows liefert `\`, Meldungen brauchen immer `/`
        let clone = self.custom_dir.parent().unwrap_or(Path::new(""));
        relative(clone, file.as_ref())
    }
}

/// Pfad von `from` nach `to` mit `/` wie `path.relative` in Node
fn relative(from: &Path, to: &Path) -> String {
    let from = parts(from);
    let to = parts(to);
    let gemeinsam = from
        .iter()
        .zip(to.iter())
        .take_while(|(a, b)| gleich(a, b))
        .count();
    let hoch = std::iter::repeat_n("..".to_string(), from.len() - gemeinsam);
    let runter = to.iter().skip(gemeinsam).cloned();
    hoch.chain(runter).collect::<Vec<_>>().join("/")
}

/// Bestandteile eines Pfades als Texte; Wurzel und Laufwerk bleiben ein eigener Teil.
/// `.` und `..` werden dabei aufgelöst, wie `path.resolve` es tut: Meldungen von tsc nennen
/// Pfade auch mit `..`.
fn parts(path: &Path) -> Vec<String> {
    let mut teile: Vec<String> = Vec::new();
    for teil in path.components() {
        match teil {
            Component::Prefix(prefix) => {
                teile.push(prefix.as_os_str().to_string_lossy().into_owned())
            }
            Component::RootDir => teile.push("/".to_string()),
            Component::Normal(name) => teile.push(name.to_string_lossy().into_owned()),
            Component::CurDir => {}
            // Ein `..` hebt den Teil davor auf; über der Wurzel bleibt es stehen
            Component::ParentDir => match teile.last().map(String::as_str) {
                Some("..") | None => teile.push("..".to_string()),
                Some("/") => {}
                Some(_) => {
                    teile.pop();
                }
            },
        }
    }
    teile
}

/// Gleicher Pfadteil? Unter Windows ohne Rücksicht auf Groß- und Kleinschreibung
fn gleich(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pfade() -> (PathBuf, CustomizationPaths) {
        let root = PathBuf::from(if cfg!(windows) { "C:\\c" } else { "/c" });
        let paths = CustomizationPaths::new(root.join("custom"), root.join("std"));
        (root, paths)
    }

    #[test]
    fn nennt_die_orte_der_anpassungen_in_custom() {
        let (root, paths) = pfade();
        assert_eq!(
            paths.custom_of(ExtensionPoint::Workflow, "workflow"),
            root.join("custom").join("workflow.ts")
        );
        assert_eq!(
            paths.custom_of(ExtensionPoint::Skill, "x"),
            root.join("custom").join("skills").join("x")
        );
        assert_eq!(
            paths.custom_of(ExtensionPoint::OutputStyle, "y"),
            root.join("custom").join("output-styles").join("y.md")
        );
    }

    #[test]
    fn nennt_die_orte_der_basis_in_custom_base() {
        let (root, paths) = pfade();
        let base = root.join("custom").join(".base");
        assert_eq!(
            paths.base_of(ExtensionPoint::Workflow, "workflow"),
            base.join("workflow.json")
        );
        assert_eq!(
            paths.base_of(ExtensionPoint::Skill, "x"),
            base.join("skills").join("x")
        );
        assert_eq!(
            paths.base_of(ExtensionPoint::OutputStyle, "y"),
            base.join("output-styles").join("y.md")
        );
    }

    #[test]
    fn nennt_den_standard_den_workflow_gibt_es_nur_als_daten() {
        let (root, paths) = pfade();
        assert_eq!(
            paths.standard_of(ExtensionPoint::Skill, "x"),
            Some(root.join("std").join("skills").join("x"))
        );
        assert_eq!(
            paths.standard_of(ExtensionPoint::OutputStyle, "y"),
            Some(root.join("std").join("output-styles").join("y.md"))
        );
        assert_eq!(
            paths.standard_of(ExtensionPoint::Workflow, "workflow"),
            None
        );
    }

    #[test]
    fn zeigt_pfade_relativ_zum_clone_mit_schragstrich() {
        let (root, paths) = pfade();
        assert_eq!(
            paths.display(
                root.join("custom")
                    .join("skills")
                    .join("x")
                    .join("SKILL.md")
            ),
            "custom/skills/x/SKILL.md"
        );
    }

    #[test]
    fn loest_punkte_im_pfad_auf_wie_path_resolve() {
        let (root, paths) = pfade();
        // So nennt tsc eine Datei, die neben dem Clone liegt
        assert_eq!(
            paths.display(root.join("..").join("fremd").join("x.ts")),
            "../fremd/x.ts"
        );
        assert_eq!(
            paths.display(root.join("custom").join(".").join("workflow.ts")),
            "custom/workflow.ts"
        );
    }

    #[test]
    fn zeigt_pfade_ausserhalb_des_clones_mit_doppelpunkt() {
        let (root, paths) = pfade();
        assert_eq!(paths.display(root.join("woanders.md")), "woanders.md");
        assert_eq!(
            paths.display(root.parent().unwrap().join("fremd.md")),
            "../fremd.md"
        );
    }
}
