//! Typen der Übersicht über die Anpassungen, portiert aus
//! `packages/core/src/customization/CustomizationOverview.ts`. Die Feldnamen und ihre
//! Reihenfolge sind das Format von `customizations/overview.json`, das Hook, Server und
//! Oberfläche lesen.

use serde::{Deserialize, Serialize};

/// Befehl, mit dem Anpassungen geändert werden (steht in jeder Übersicht)
pub const CUSTOMIZE_COMMAND: &str = "/nexus:harness-customize";

/// Die drei Stellen, an denen ein Clone angepasst werden kann
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionPoint {
    Workflow,
    Skill,
    OutputStyle,
}

impl ExtensionPoint {
    /// Wert wie im JSON (`workflow`, `skill`, `output-style`)
    pub fn as_str(&self) -> &'static str {
        match self {
            ExtensionPoint::Workflow => "workflow",
            ExtensionPoint::Skill => "skill",
            ExtensionPoint::OutputStyle => "output-style",
        }
    }
}

/// Zustand der Basis einer Anpassung gegenüber dem aktuellen Standard
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BaseState {
    Current,
    Outdated,
    Unknown,
}

/// Zustand der Basis im Eintrag: wie [`BaseState`], dazu `none` für „nicht verglichen“
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryBase {
    None,
    Current,
    Outdated,
    Unknown,
}

impl From<BaseState> for EntryBase {
    fn from(state: BaseState) -> Self {
        match state {
            BaseState::Current => EntryBase::Current,
            BaseState::Outdated => EntryBase::Outdated,
            BaseState::Unknown => EntryBase::Unknown,
        }
    }
}

/// Art der Anpassung: ersetzt einen mitgelieferten Standard oder kommt neu dazu
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Replace,
    New,
}

/// Gilt die Anpassung oder gilt der Standard?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryState {
    Active,
    Fallback,
}

/// Zustand des Workflows in der Übersicht
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowState {
    Standard,
    Custom,
    Fallback,
}

/// Ein Grund für einen Rückfall oder ein Hinweis, mit Ort soweit bekannt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    pub code: String,
    pub message: String,
    /// Pfad relativ zum Clone, mit `/`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

impl Problem {
    /// Befund ohne Ort (den ergänzt der Aufrufer)
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            file: None,
            line: None,
        }
    }

    /// Befund mit Ort
    pub fn at(
        code: impl Into<String>,
        message: impl Into<String>,
        file: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            file: Some(file.into()),
            line: None,
        }
    }

    /// Derselbe Befund mit gesetztem Ort
    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }
}

/// Eine Anpassung mit ihrer Bewertung
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomizationEntry {
    pub point: ExtensionPoint,
    pub name: String,
    pub kind: EntryKind,
    pub state: EntryState,
    pub base: EntryBase,
    /// Gründe für den Rückfall auf den Standard
    pub problems: Vec<Problem>,
    /// Hinweise ohne Rückfall
    pub warnings: Vec<Problem>,
    /// Unterschiede Basis → Standard, nur bei veralteter Basis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

impl CustomizationEntry {
    /// Grundgerüst eines Eintrags ohne Befunde
    pub fn new(point: ExtensionPoint, name: impl Into<String>, kind: EntryKind) -> Self {
        Self {
            point,
            name: name.into(),
            kind,
            state: EntryState::Active,
            base: EntryBase::None,
            problems: Vec::new(),
            warnings: Vec::new(),
            diff: None,
        }
    }
}

/// Die ganze Übersicht; Feldreihenfolge wie in `overview.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomizationOverview {
    #[serde(default = "version_eins")]
    pub version: u32,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub clone_dir: String,
    #[serde(default)]
    pub custom_dir: String,
    #[serde(default)]
    pub harness_version: String,
    #[serde(default = "workflow_standard")]
    pub workflow: WorkflowState,
    pub entries: Vec<CustomizationEntry>,
}

/// Fassung des Formats, falls die Datei sie nicht nennt
fn version_eins() -> u32 {
    1
}

/// Zustand des Workflows, falls die Datei ihn nicht nennt
fn workflow_standard() -> WorkflowState {
    WorkflowState::Standard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_uebersicht_wird_wie_in_typescript_serialisiert() {
        let overview = CustomizationOverview {
            version: 1,
            created_at: "2026-10-14T10:00:00.000Z".to_string(),
            clone_dir: "/h".to_string(),
            custom_dir: "/h/custom".to_string(),
            harness_version: "0.2.0".to_string(),
            workflow: WorkflowState::Fallback,
            entries: vec![CustomizationEntry {
                point: ExtensionPoint::OutputStyle,
                problems: vec![Problem::at("x", "y", "custom/output-styles/k.md")],
                ..CustomizationEntry::new(ExtensionPoint::OutputStyle, "k", EntryKind::New)
            }],
        };
        let json = serde_json::to_string(&overview).unwrap();
        assert_eq!(
            json,
            r#"{"version":1,"createdAt":"2026-10-14T10:00:00.000Z","cloneDir":"/h","customDir":"/h/custom","harnessVersion":"0.2.0","workflow":"fallback","entries":[{"point":"output-style","name":"k","kind":"new","state":"active","base":"none","problems":[{"code":"x","message":"y","file":"custom/output-styles/k.md"}],"warnings":[]}]}"#
        );
    }
}
