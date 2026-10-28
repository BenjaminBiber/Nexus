//! Portiert `packages/core/src/customization/WorkflowValidator.ts`.

use serde_json::Value;

use super::overview::Problem;

/// Vorsilbe eines Harness-Skills in einem Befehl
const PREFIX: &str = "/ai-harness:";

/// Ergebnis der Prüfung: Probleme führen zum Rückfall, Warnungen nicht
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkflowValidation {
    /// Führen zum Rückfall auf den Standard
    pub problems: Vec<Problem>,
    /// Hinweise, der Workflow bleibt aktiv
    pub warnings: Vec<Problem>,
}

/// Prüft einen geladenen Workflow ohne Dateizugriff: Aufbau, Pflicht-Phasen (Leitplanke),
/// Verweise auf Harness-Skills.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkflowValidator;

impl WorkflowValidator {
    /// Phasen, die jeder Workflow haben muss
    pub const REQUIRED: [&'static str; 2] = ["planen", "umsetzen"];
    /// Phasen-IDs, die die Anleitung im Board selbst belegt (EXTRA_PHASES in guideContent.ts)
    pub const RESERVED: [&'static str; 2] = ["fragen", "nebenbei"];

    /// Neuer Prüfer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Aufbau zuerst; nur ein korrekt aufgebauter Workflow wird auf IDs und Verweise geprüft
    pub fn validate(&self, value: &Value, skills: &[String]) -> WorkflowValidation {
        let problems = self.shape(value);
        if !problems.is_empty() {
            return WorkflowValidation {
                problems,
                warnings: Vec::new(),
            };
        }
        let phases = value.as_array().expect("der Aufbau ist geprüft");
        let mut problems = self.duplicates(phases);
        problems.extend(self.reserved(phases));
        problems.extend(self.required(phases));
        WorkflowValidation {
            problems,
            warnings: self.references(phases, skills),
        }
    }

    /// Liste von Phasen mit id, title, hint und steps; Ort im Wert als Pfad wie
    /// `phases[1].steps[0].when`
    fn shape(&self, value: &Value) -> Vec<Problem> {
        let Some(phases) = value.as_array() else {
            return vec![problem(
                "shape-invalid",
                "Der Workflow muss eine Liste von Phasen sein.",
            )];
        };
        let mut problems = Vec::new();
        for (p, phase) in phases.iter().enumerate() {
            let at = format!("phases[{p}]");
            problems.extend(texts(phase, &at, &["id", "title", "hint"], &[]));
            let steps = phase.get("steps");
            let Some(steps) = steps.and_then(Value::as_array) else {
                problems.push(problem(
                    "shape-invalid",
                    format!("{at}.steps muss eine Liste sein."),
                ));
                continue;
            };
            for (s, step) in steps.iter().enumerate() {
                problems.extend(self.step(step, &format!("{at}.steps[{s}]")));
            }
        }
        problems
    }

    /// Ein Schritt: step und when Pflicht, command und place optional, options als Liste
    /// von {label, command, when}
    fn step(&self, step: &Value, at: &str) -> Vec<Problem> {
        let mut problems = texts(step, at, &["step", "when"], &["command", "place"]);
        let options = step.get("options");
        let Some(options) = options else {
            return problems;
        };
        let Some(options) = options.as_array() else {
            problems.push(problem(
                "shape-invalid",
                format!("{at}.options muss eine Liste sein."),
            ));
            return problems;
        };
        for (o, option) in options.iter().enumerate() {
            problems.extend(texts(
                option,
                &format!("{at}.options[{o}]"),
                &["label", "command", "when"],
                &[],
            ));
        }
        problems
    }

    /// Jede Phasen-ID darf nur einmal vorkommen
    fn duplicates(&self, phases: &[Value]) -> Vec<Problem> {
        let mut seen: Vec<&str> = Vec::new();
        let mut problems = Vec::new();
        for id in phases.iter().map(phase_id) {
            if seen.contains(&id) {
                problems.push(problem(
                    "phase-id-duplicate",
                    format!("Die Phase {id} kommt mehrfach vor."),
                ));
            }
            seen.push(id);
        }
        problems
    }

    /// Die IDs der Anleitung im Board dürfen nicht für eigene Phasen verwendet werden
    fn reserved(&self, phases: &[Value]) -> Vec<Problem> {
        phases
            .iter()
            .map(phase_id)
            .filter(|id| Self::RESERVED.contains(id))
            .map(|id| {
                problem(
                    "phase-id-reserved",
                    format!("Die Phasen-ID {id} ist für die Anleitung reserviert."),
                )
            })
            .collect()
    }

    /// Leitplanke: Pflicht-Phasen planen und umsetzen
    fn required(&self, phases: &[Value]) -> Vec<Problem> {
        let ids: Vec<&str> = phases.iter().map(phase_id).collect();
        Self::REQUIRED
            .iter()
            .filter(|id| !ids.contains(id))
            .map(|id| {
                problem(
                    "required-phase-missing",
                    format!("Die Pflicht-Phase {id} fehlt."),
                )
            })
            .collect()
    }

    /// `/ai-harness:<name>` muss einen verfügbaren Skill nennen; je Schritt und Name eine
    /// Warnung; andere Befehle ungeprüft
    fn references(&self, phases: &[Value], skills: &[String]) -> Vec<Problem> {
        let mut warnings = Vec::new();
        let steps = phases
            .iter()
            .filter_map(|phase| phase.get("steps").and_then(Value::as_array))
            .flatten();
        for step in steps {
            let mut names: Vec<&str> = Vec::new();
            let befehle = std::iter::once(step.get("command"))
                .chain(
                    step.get("options")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|option| option.get("command")),
                )
                .flatten();
            for name in befehle.filter_map(skill_name) {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            for name in names {
                if !skills.iter().any(|skill| skill == name) {
                    warnings.push(problem(
                        "skill-ref-missing",
                        format!(
                            "{PREFIX}{name} gibt es nicht (Schritt „{}“).",
                            step.get("step").and_then(Value::as_str).unwrap_or_default()
                        ),
                    ));
                }
            }
        }
        warnings
    }
}

/// Pflichtfelder müssen Texte sein, optionale Felder Texte oder fehlen
fn texts(value: &Value, at: &str, required: &[&str], optional: &[&str]) -> Vec<Problem> {
    if !value.is_object() {
        return vec![problem(
            "shape-invalid",
            format!("{at} muss ein Objekt sein."),
        )];
    }
    let mut problems = Vec::new();
    for key in required {
        if !value.get(key).is_some_and(Value::is_string) {
            problems.push(problem(
                "shape-invalid",
                format!("{at}.{key} muss ein Text sein."),
            ));
        }
    }
    for key in optional {
        if value.get(key).is_some_and(|value| !value.is_string()) {
            problems.push(problem(
                "shape-invalid",
                format!("{at}.{key} muss ein Text sein."),
            ));
        }
    }
    problems
}

/// ID einer Phase; nach der Aufbauprüfung ist sie immer ein Text
fn phase_id(phase: &Value) -> &str {
    phase.get("id").and_then(Value::as_str).unwrap_or_default()
}

/// Name des Harness-Skills in einem Befehl, sonst None
fn skill_name(command: &Value) -> Option<&str> {
    let rest = command.as_str()?.strip_prefix(PREFIX)?;
    let name = rest.split_whitespace().next().unwrap_or_default();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Baut ein Problem ohne Ort (den ergänzt der Aufrufer)
fn problem(code: &str, message: impl Into<String>) -> Problem {
    Problem::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::workflow_phases;

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

    fn standard() -> Value {
        serde_json::to_value(workflow_phases()).unwrap()
    }

    fn codes(problems: &[Problem]) -> Vec<&str> {
        problems.iter().map(|p| p.code.as_str()).collect()
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

    #[test]
    fn nimmt_den_standard_ohne_befund_an() {
        assert_eq!(
            WorkflowValidator::new().validate(&standard(), &skills()),
            WorkflowValidation::default()
        );
    }

    #[test]
    fn verlangt_eine_liste_von_phasen_mit_texten() {
        let validator = WorkflowValidator::new();
        assert_eq!(
            validator
                .validate(&serde_json::json!({}), &skills())
                .problems[0]
                .message,
            "Der Workflow muss eine Liste von Phasen sein."
        );
        let result = validator.validate(
            &serde_json::json!([{ "id": "planen", "hint": "h", "steps": [] }]),
            &skills(),
        );
        assert!(codes(&result.problems).contains(&"shape-invalid"));
        assert!(result.problems[0].message.contains("phases[0].title"));
    }

    #[test]
    fn verlangt_die_pflicht_phasen_und_eindeutige_ids() {
        let validator = WorkflowValidator::new();
        let nur_planen = Value::Array(
            standard()
                .as_array()
                .unwrap()
                .iter()
                .filter(|phase| phase["id"] == "planen")
                .cloned()
                .collect(),
        );
        assert_eq!(
            validator.validate(&nur_planen, &skills()).problems,
            vec![Problem::new(
                "required-phase-missing",
                "Die Pflicht-Phase umsetzen fehlt."
            )]
        );
        let mut doppelt = standard();
        let erste = doppelt[0].clone();
        doppelt.as_array_mut().unwrap().push(erste);
        assert_eq!(
            codes(&validator.validate(&doppelt, &skills()).problems),
            vec!["phase-id-duplicate"]
        );
    }

    #[test]
    fn reserviert_die_phasen_ids_der_anleitung_im_board() {
        let validator = WorkflowValidator::new();
        let extra = |id: &str| serde_json::json!({ "id": id, "title": "Eigene Phase", "hint": "h", "steps": [] });
        for id in ["fragen", "nebenbei"] {
            let mut phases = standard();
            phases.as_array_mut().unwrap().push(extra(id));
            assert_eq!(
                validator.validate(&phases, &skills()).problems,
                vec![Problem::new(
                    "phase-id-reserved",
                    format!("Die Phasen-ID {id} ist für die Anleitung reserviert.")
                )]
            );
        }
        // Andere eigene IDs bleiben erlaubt
        let mut phases = standard();
        phases.as_array_mut().unwrap().push(extra("qualitaet"));
        assert!(validator.validate(&phases, &skills()).problems.is_empty());
    }

    #[test]
    fn warnt_einmal_je_fehlendem_harness_skill_und_prueft_fremde_befehle_nicht() {
        let validator = WorkflowValidator::new();
        let result = validator.validate(
            &mit_schritt(serde_json::json!({
                "step": "Review",
                "command": "/ai-harness:mein-review",
                "when": "Vorher.",
                "options": [{ "label": "R", "command": "/ai-harness:mein-review", "when": "x" }],
            })),
            &skills(),
        );
        assert!(result.problems.is_empty());
        assert_eq!(
            result.warnings,
            vec![Problem::new(
                "skill-ref-missing",
                "/ai-harness:mein-review gibt es nicht (Schritt „Review“)."
            )]
        );
        let fremd = validator.validate(
            &mit_schritt(
                serde_json::json!({ "step": "Ideen", "command": "/superpowers:brainstorming", "when": "x" }),
            ),
            &skills(),
        );
        assert!(fremd.warnings.is_empty());
    }

    #[test]
    fn verlangt_listen_fur_steps_und_options() {
        let validator = WorkflowValidator::new();
        let ohne_steps = serde_json::json!([{ "id": "planen", "title": "t", "hint": "h" }]);
        assert_eq!(
            validator.validate(&ohne_steps, &skills()).problems[0].message,
            "phases[0].steps muss eine Liste sein."
        );
        let options_kein_array = serde_json::json!([{
            "id": "planen", "title": "t", "hint": "h",
            "steps": [{ "step": "s", "when": "w", "options": 3 }],
        }]);
        assert!(validator
            .validate(&options_kein_array, &skills())
            .problems
            .iter()
            .any(|p| p.message == "phases[0].steps[0].options muss eine Liste sein."));
    }
}
