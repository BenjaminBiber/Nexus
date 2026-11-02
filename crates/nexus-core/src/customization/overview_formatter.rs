//! Portiert `packages/core/src/customization/OverviewFormatter.ts`
//! (Format: contracts/customize-script.md).

use super::overview::{
    CustomizationEntry, CustomizationOverview, EntryKind, EntryState, ExtensionPoint, Problem,
    WorkflowState, CUSTOMIZE_COMMAND,
};

/// So viele Zeilen eines Diffs zeigt die Übersicht in `setup`
pub const DEFAULT_MAX_DIFF_LINES: usize = 40;

/// Anzeigename eines Erweiterungspunkts mit Name, z. B. „Skill harness-grill“
pub fn label(point: ExtensionPoint, name: &str) -> String {
    match point {
        ExtensionPoint::Workflow => "Workflow".to_string(),
        ExtensionPoint::Skill => format!("Skill {name}"),
        ExtensionPoint::OutputStyle => format!("Output-Style {name}"),
    }
}

/// Text der Übersicht für `setup` und `customize check`.
#[derive(Debug, Clone, Copy, Default)]
pub struct OverviewFormatter;

impl OverviewFormatter {
    /// Neuer Formatierer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Ganze Übersicht; Diffs je Warnung auf `max_diff_lines` Zeilen gekürzt (None: alle)
    pub fn format(
        &self,
        overview: &CustomizationOverview,
        max_diff_lines: Option<usize>,
    ) -> String {
        if overview.entries.is_empty() {
            return format!(
                "Keine Anpassungen, überall gilt der Standard.\nÄndern: {CUSTOMIZE_COMMAND}"
            );
        }
        let mut lines = vec![format!("Anpassungen (custom/ in {})", overview.custom_dir)];
        lines.extend(self.summary(overview));
        let warned: Vec<&CustomizationEntry> = overview
            .entries
            .iter()
            .filter(|entry| !entry.warnings.is_empty())
            .collect();
        if !warned.is_empty() {
            lines.push(String::new());
            lines.push("Warnungen".to_string());
            for entry in warned {
                lines.extend(self.warnings(entry, max_diff_lines));
            }
        }
        let fallbacks: Vec<&CustomizationEntry> = overview
            .entries
            .iter()
            .filter(|entry| entry.state == EntryState::Fallback)
            .collect();
        if !fallbacks.is_empty() {
            lines.push(String::new());
            lines.push("Rückfälle auf den Standard".to_string());
            for entry in fallbacks {
                for problem in &entry.problems {
                    lines.push(format!(
                        "  ✗ {}: {}{}",
                        self.name(entry),
                        problem.message,
                        self.where_of(problem)
                    ));
                }
            }
        }
        lines.push(String::new());
        lines.push(format!("Ändern: {CUSTOMIZE_COMMAND}"));
        lines.join("\n")
    }

    /// Je Erweiterungspunkt eine Zeile
    fn summary(&self, overview: &CustomizationOverview) -> Vec<String> {
        let workflow = match overview.workflow {
            WorkflowState::Standard => "Standard",
            WorkflowState::Custom => "angepasst",
            WorkflowState::Fallback => "Rückfall auf Standard",
        };
        vec![
            format!("  Workflow        {workflow}"),
            format!(
                "  Skills          {}",
                self.names(overview, ExtensionPoint::Skill)
            ),
            format!(
                "  Output-Styles   {}",
                self.names(overview, ExtensionPoint::OutputStyle)
            ),
        ]
    }

    /// Aktive Anpassungen eines Punkts mit Art, sonst „Standard“
    fn names(&self, overview: &CustomizationOverview, point: ExtensionPoint) -> String {
        let active: Vec<String> = overview
            .entries
            .iter()
            .filter(|entry| entry.point == point && entry.state == EntryState::Active)
            .map(|entry| {
                let kind = if entry.kind == EntryKind::Replace {
                    "ersetzt"
                } else {
                    "neu"
                };
                format!("{} ({kind})", entry.name)
            })
            .collect();
        if active.is_empty() {
            "Standard".to_string()
        } else {
            active.join(", ")
        }
    }

    /// Warnungen eines Eintrags, darunter der Diff eingerückt und gekürzt
    fn warnings(&self, entry: &CustomizationEntry, max: Option<usize>) -> Vec<String> {
        let mut lines: Vec<String> = entry
            .warnings
            .iter()
            .map(|warning| format!("  ! {}: {}", self.name(entry), warning.message))
            .collect();
        let Some(diff) = &entry.diff else {
            return lines;
        };
        let diff: Vec<&str> = diff.split('\n').collect();
        let max = max.unwrap_or(diff.len());
        lines.extend(diff.iter().take(max).map(|line| format!("      {line}")));
        if diff.len() > max {
            lines.push(format!(
                "      … {} weitere Zeilen: nexus customize check",
                diff.len() - max
            ));
        }
        lines
    }

    /// Anzeigename eines Eintrags
    fn name(&self, entry: &CustomizationEntry) -> String {
        label(entry.point, &entry.name)
    }

    /// Ort in Klammern, mit Zeile falls bekannt
    fn where_of(&self, problem: &Problem) -> String {
        let Some(file) = &problem.file else {
            return String::new();
        };
        match problem.line {
            Some(line) => format!(" ({file}:{line})"),
            None => format!(" ({file})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::customization::overview::EntryBase;

    fn basis() -> CustomizationOverview {
        CustomizationOverview {
            version: 1,
            created_at: "2026-10-14T10:00:00.000Z".to_string(),
            clone_dir: "/h".to_string(),
            custom_dir: "/h/custom".to_string(),
            harness_version: "0.2.0".to_string(),
            workflow: WorkflowState::Standard,
            entries: Vec::new(),
        }
    }

    fn eintrag(name: &str) -> CustomizationEntry {
        CustomizationEntry::new(ExtensionPoint::Skill, name, EntryKind::New)
    }

    #[test]
    fn meldet_wenn_es_keine_anpassungen_gibt() {
        assert_eq!(
            OverviewFormatter::new().format(&basis(), Some(DEFAULT_MAX_DIFF_LINES)),
            "Keine Anpassungen, überall gilt der Standard.\nÄndern: /nexus:harness-customize"
        );
    }

    #[test]
    fn zeigt_je_punkt_eine_zeile_warnungen_mit_gekurztem_diff_und_ruckfalle_mit_ort() {
        let diff = (0..50)
            .map(|i| format!("+zeile {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let overview = CustomizationOverview {
            workflow: WorkflowState::Custom,
            entries: vec![
                CustomizationEntry {
                    base: EntryBase::Current,
                    ..CustomizationEntry::new(
                        ExtensionPoint::Workflow,
                        "workflow",
                        EntryKind::Replace,
                    )
                },
                CustomizationEntry {
                    kind: EntryKind::Replace,
                    base: EntryBase::Outdated,
                    warnings: vec![Problem::new(
                        "base-outdated",
                        "Der Standard hat sich seit deiner Kopie geändert.",
                    )],
                    diff: Some(diff),
                    ..eintrag("harness-implement")
                },
                eintrag("mein-review"),
                CustomizationEntry {
                    point: ExtensionPoint::OutputStyle,
                    state: EntryState::Fallback,
                    problems: vec![Problem::at(
                        "description-missing",
                        "Im Kopf fehlt description.",
                        "custom/output-styles/kurz.md",
                    )],
                    ..eintrag("kurz")
                },
            ],
            ..basis()
        };
        let text = OverviewFormatter::new().format(&overview, Some(40));
        assert!(text.contains("Anpassungen (custom/ in /h/custom)"));
        assert!(text.contains("  Workflow        angepasst"));
        assert!(text.contains("  Skills          harness-implement (ersetzt), mein-review (neu)"));
        assert!(text.contains("  Output-Styles   Standard"));
        assert!(text.contains(
            "  ! Skill harness-implement: Der Standard hat sich seit deiner Kopie geändert."
        ));
        assert!(text.contains("      +zeile 39"));
        assert!(!text.contains("+zeile 40"));
        assert!(text.contains("      … 10 weitere Zeilen: nexus customize check"));
        assert!(text.contains(
            "  ✗ Output-Style kurz: Im Kopf fehlt description. (custom/output-styles/kurz.md)"
        ));
        assert!(text.ends_with("Ändern: /nexus:harness-customize"));
    }

    #[test]
    fn zeigt_alle_diff_zeilen_ohne_grenze() {
        let diff = (0..50)
            .map(|i| format!("+zeile {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let overview = CustomizationOverview {
            entries: vec![CustomizationEntry {
                warnings: vec![Problem::new("base-outdated", "geändert")],
                diff: Some(diff),
                ..eintrag("x")
            }],
            ..basis()
        };
        let text = OverviewFormatter::new().format(&overview, None);
        assert!(text.contains("      +zeile 49"));
        assert!(!text.contains("weitere Zeilen"));
    }

    #[test]
    fn nennt_die_zeile_bei_typfehlern_und_benennt_erweiterungspunkte() {
        let overview = CustomizationOverview {
            workflow: WorkflowState::Fallback,
            entries: vec![CustomizationEntry {
                point: ExtensionPoint::Workflow,
                kind: EntryKind::Replace,
                state: EntryState::Fallback,
                problems: vec![Problem {
                    line: Some(3),
                    ..Problem::at("type-error", "TS2322: x", "custom/workflow.ts")
                }],
                ..eintrag("workflow")
            }],
            ..basis()
        };
        let text = OverviewFormatter::new().format(&overview, Some(DEFAULT_MAX_DIFF_LINES));
        assert!(text.contains("  Workflow        Rückfall auf Standard"));
        assert!(text.contains("  ✗ Workflow: TS2322: x (custom/workflow.ts:3)"));
        assert_eq!(label(ExtensionPoint::Skill, "x"), "Skill x");
        assert_eq!(label(ExtensionPoint::OutputStyle, "y"), "Output-Style y");
        assert_eq!(label(ExtensionPoint::Workflow, "workflow"), "Workflow");
    }
}
