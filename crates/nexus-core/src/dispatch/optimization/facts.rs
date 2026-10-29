//! Verdichtet die Arbeitsläufe zu Kennzahlen je Profil, je Regel und je Eskalationsweg;
//! portiert aus `packages/core/src/dispatch/optimization/OptimizationFactsCollector.ts`.
//!
//! Die Kennzahlen sind die Grundlage der Optimierungs-Vorschläge.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::{Map, Value};

use crate::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchProfileReason, DispatchRun, DispatchRunKind, DispatchRunOutcome,
};
use crate::dispatch::config::DispatchConfig;
use crate::dispatch::optimization::types::{
    EscalationPath, OptimizationBasis, OptimizationFacts, ProfileFacts, RuleFacts, StartSource,
};
use crate::text::locale_cmp;

/// Eingabe der Auswertung
#[derive(Debug, Clone)]
pub struct FactsInput<'a> {
    pub runs: &'a [DispatchRun],
    pub config: &'a DispatchConfig,
    /// Fassungen der Einstellung (für die Zuordnung der Regeln)
    pub versions: &'a [DispatchConfigVersion],
    /// Beginn des Zeitraums (ISO)
    pub since: String,
}

/// Ergebnisse, die als Erfolg im ersten Versuch zählen
const SUCCESS: [DispatchRunOutcome; 2] = [DispatchRunOutcome::Done, DispatchRunOutcome::Review];

/// JSON mit sortierten Schlüsseln – vergleicht Regeln unabhängig von der Reihenfolge ihrer
/// Felder
pub fn sorted_json(value: &Value) -> String {
    serde_json::to_string(&sorted(value)).unwrap_or_default()
}

/// Objekte mit sortierten Schlüsseln neu aufbauen
fn sorted(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        Value::Object(object) => {
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for key in keys {
                out.insert(key.clone(), sorted(&object[key]));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Erster Lauf eines Tickets auf seinem Basisprofil (keine Eskalation, erster Versuch)
fn is_start(run: &DispatchRun) -> bool {
    run.escalated_from.is_none() && run.attempt.unwrap_or(1) <= 1
}

/// Verdichtet die Arbeitsläufe zu Kennzahlen.
#[derive(Debug, Clone, Copy, Default)]
pub struct OptimizationFactsCollector;

impl OptimizationFactsCollector {
    /// Neuer Sammler (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Kennzahlen je Profil, je Regel und je Eskalationsweg
    pub fn collect(&self, input: &FactsInput<'_>) -> OptimizationFacts {
        let work: Vec<&DispatchRun> = input
            .runs
            .iter()
            .filter(|run| run.kind == DispatchRunKind::Work)
            .collect();
        let escalated = escalated_subjects(&work);
        let projects: HashSet<i32> = work.iter().map(|run| run.project_id).collect();
        OptimizationFacts {
            basis: OptimizationBasis {
                runs: work.len() as i64,
                projects: projects.len() as i64,
                since: input.since.clone(),
            },
            profiles: input
                .config
                .profiles
                .iter()
                .map(|(name, profile)| {
                    self.profile(
                        name,
                        profile.max_turns,
                        profile.timeout_minutes,
                        &work,
                        &escalated,
                    )
                })
                .collect(),
            rules: self.rules(input, &work, &escalated),
            paths: paths(&work),
        }
    }

    /// Kennzahlen eines Profils; gezählt werden nur Läufe mit seinen aktuellen Limits
    fn profile(
        &self,
        name: &str,
        max_turns: i64,
        timeout_minutes: i64,
        work: &[&DispatchRun],
        escalated: &HashMap<String, HashSet<String>>,
    ) -> ProfileFacts {
        let all: Vec<&DispatchRun> = work
            .iter()
            .copied()
            .filter(|run| run.profile == name)
            .collect();
        let current: Vec<&DispatchRun> = all
            .iter()
            .copied()
            .filter(|run| {
                run.max_turns.map(i64::from) == Some(max_turns)
                    && run.timeout_minutes.map(i64::from) == Some(timeout_minutes)
            })
            .collect();
        let starts: Vec<&DispatchRun> = current.iter().copied().filter(|r| is_start(r)).collect();
        let empty = HashSet::new();
        let away = escalated.get(name).unwrap_or(&empty);
        let via: Vec<&DispatchRun> = current
            .iter()
            .copied()
            .filter(|run| run.escalated_from.is_some())
            .collect();
        let mut aborts: BTreeMap<String, i64> = BTreeMap::new();
        for run in &current {
            if let Some(category) = run.abort_category.as_deref().filter(|c| !c.is_empty()) {
                *aborts.entry(category.to_string()).or_insert(0) += 1;
            }
        }
        let mut turns: Vec<i64> = current
            .iter()
            .filter_map(|run| run.num_turns.map(i64::from))
            .collect();
        turns.sort_unstable();
        let turns_p90 = match turns.len() {
            0 => None,
            len => {
                let index = (len as f64 * 0.9).ceil() as usize;
                turns.get(index.saturating_sub(1)).copied()
            }
        };
        ProfileFacts {
            profile: name.to_string(),
            max_turns,
            timeout_minutes,
            runs: current.len() as i64,
            starts: starts.len() as i64,
            escalated_starts: starts
                .iter()
                .filter(|run| away.contains(&run.subject_id))
                .count() as i64,
            via_escalation: via.len() as i64,
            top_origin: top(via
                .iter()
                .filter_map(|run| run.escalated_from.clone())
                .collect()),
            aborts,
            first_try_success: starts
                .iter()
                .filter(|run| SUCCESS.contains(&run.outcome))
                .count() as i64,
            turns_p90,
            start_sources: sources(&starts),
            // Läufe kommen nach Start sortiert
            last_run_at: all.last().map(|run| run.started_at.clone()),
        }
    }

    /// Starts je Regel – nur Läufe, deren Konfig-Fassung an dieser Stelle dieselbe Regel hatte
    fn rules(
        &self,
        input: &FactsInput<'_>,
        work: &[&DispatchRun],
        escalated: &HashMap<String, HashSet<String>>,
    ) -> Vec<RuleFacts> {
        let mut rules_of: HashMap<&str, Vec<Value>> = HashMap::new();
        for version in input.versions {
            let rules = serde_json::from_str::<Value>(&version.content)
                .ok()
                .and_then(|parsed| parsed.get("rules").cloned())
                .and_then(|rules| rules.as_array().cloned())
                .unwrap_or_default();
            rules_of.insert(version.hash.as_str(), rules);
        }
        input
            .config
            .rules
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                let key = sorted_json(&serde_json::to_value(rule).unwrap_or(Value::Null));
                let same_rule = |hash: Option<&String>| match hash {
                    None => false,
                    Some(hash) => rules_of
                        .get(hash.as_str())
                        .and_then(|rules| rules.get(index))
                        .is_some_and(|old| sorted_json(old) == key),
                };
                let runs: Vec<&DispatchRun> = work
                    .iter()
                    .copied()
                    .filter(|run| {
                        run.profile_reason == Some(DispatchProfileReason::Rule)
                            && run.rule_index.map(i64::from) == Some(index as i64 + 1)
                            && is_start(run)
                            && same_rule(run.config_hash.as_ref())
                    })
                    .collect();
                let failed = runs
                    .iter()
                    .filter(|run| {
                        run.outcome == DispatchRunOutcome::AbortFinal
                            || escalated
                                .get(&run.profile)
                                .is_some_and(|away| away.contains(&run.subject_id))
                    })
                    .count() as i64;
                RuleFacts {
                    index: index as i64 + 1,
                    profile: rule.profile.clone(),
                    runs: runs.len() as i64,
                    failed,
                }
            })
            .collect()
    }
}

/// Basisprofil → Tickets, die von ihm weg eskaliert wurden
fn escalated_subjects(work: &[&DispatchRun]) -> HashMap<String, HashSet<String>> {
    let mut out: HashMap<String, HashSet<String>> = HashMap::new();
    for run in work {
        if let Some(from) = run.escalated_from.as_deref() {
            out.entry(from.to_string())
                .or_default()
                .insert(run.subject_id.clone());
        }
    }
    out
}

/// Herkunft der Starts (Grund und Regel), häufigste zuerst
fn sources(starts: &[&DispatchRun]) -> Vec<StartSource> {
    let mut order: Vec<(Option<DispatchProfileReason>, Option<i64>)> = vec![];
    let mut counts: HashMap<(Option<String>, Option<i64>), i64> = HashMap::new();
    for run in starts {
        let rule_index = run.rule_index.map(i64::from);
        let key = (
            run.profile_reason.map(|r| r.as_str().to_string()),
            rule_index,
        );
        let entry = counts.entry(key).or_insert(0);
        if *entry == 0 {
            order.push((run.profile_reason, rule_index));
        }
        *entry += 1;
    }
    let mut out: Vec<StartSource> = order
        .into_iter()
        .map(|(reason, rule_index)| StartSource {
            count: counts[&(reason.map(|r| r.as_str().to_string()), rule_index)],
            reason,
            rule_index,
        })
        .collect();
    // Häufigste zuerst, bei Gleichstand nach dem Grund (`String(null)` = „null“)
    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| locale_cmp(&reason_text(a.reason), &reason_text(b.reason)))
    });
    out
}

/// `String(reason)` aus JavaScript: der Grund als Text, `null` ohne Grund
fn reason_text(reason: Option<DispatchProfileReason>) -> String {
    match reason {
        Some(reason) => reason.as_str().to_string(),
        None => "null".to_string(),
    }
}

/// Eskalationswege Basisprofil → Zielprofil, häufigste zuerst
fn paths(work: &[&DispatchRun]) -> Vec<EscalationPath> {
    let mut order: Vec<(String, String)> = vec![];
    let mut counts: HashMap<(String, String), i64> = HashMap::new();
    for run in work {
        let Some(from) = run.escalated_from.clone() else {
            continue;
        };
        let key = (from, run.profile.clone());
        let entry = counts.entry(key.clone()).or_insert(0);
        if *entry == 0 {
            order.push(key);
        }
        *entry += 1;
    }
    let mut out: Vec<EscalationPath> = order
        .into_iter()
        .map(|key| EscalationPath {
            count: counts[&key],
            from: key.0,
            to: key.1,
        })
        .collect();
    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| locale_cmp(&a.from, &b.from))
    });
    out
}

/// Häufigster Wert; bei Gleichstand alphabetisch der erste
fn top(values: Vec<String>) -> Option<String> {
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_insert(0) += 1;
    }
    let mut entries: Vec<(String, i64)> = counts.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| locale_cmp(&a.0, &b.0)));
    entries.into_iter().next().map(|(value, _)| value)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;
    use crate::dispatch::fingerprint::DispatchConfigFingerprint;
    use crate::testing::fixtures::run_fixture;

    /// Limits der Profile der Testeinstellung
    const LIMITS: [(&str, i32, i32); 3] =
        [("standard", 30, 60), ("klein", 20, 30), ("gross", 80, 120)];

    /// Rohe Einstellung der TypeScript-Tests
    fn raw() -> Value {
        let mut raw = DispatchConfigNormalizer::new().example();
        let object = raw.as_object_mut().unwrap();
        object.insert(
            "profiles".to_string(),
            json!({
                "klein": { "model": "sonnet", "maxTurns": 20, "timeoutMinutes": 30 },
                "standard": { "model": "opus", "maxTurns": 30, "timeoutMinutes": 60 },
                "gross": { "model": "opus", "maxTurns": 80, "timeoutMinutes": 120 },
            }),
        );
        object.insert("defaultProfile".to_string(), json!("standard"));
        object.insert("escalate".to_string(), json!(true));
        object.insert(
            "escalation".to_string(),
            json!(["klein", "standard", "gross"]),
        );
        object.insert(
            "rules".to_string(),
            json!([{ "when": { "labels": ["doku"] }, "profile": "klein" }]),
        );
        object.insert(
            "research".to_string(),
            json!({ "auto": false, "profile": null }),
        );
        object.insert("planning".to_string(), json!({ "profile": null }));
        raw
    }

    /// Die aufgelöste Testeinstellung
    fn config() -> DispatchConfig {
        DispatchConfigNormalizer::new().normalize(&raw()).unwrap()
    }

    /// Lauf mit den aktuellen Limits seines Profils
    fn run(id: &str, change: impl FnOnce(&mut DispatchRun)) -> DispatchRun {
        let mut run = run_fixture(id, id, 1);
        change(&mut run);
        if run.max_turns.is_none() || run.timeout_minutes.is_none() {
            let (_, max_turns, timeout) = LIMITS
                .iter()
                .find(|(name, _, _)| *name == run.profile)
                .copied()
                .expect("bekanntes Profil");
            run.max_turns = run.max_turns.or(Some(max_turns));
            run.timeout_minutes = run.timeout_minutes.or(Some(timeout));
        }
        run
    }

    /// Kennzahlen zu diesen Läufen
    fn collect(runs: Vec<DispatchRun>, versions: Vec<DispatchConfigVersion>) -> OptimizationFacts {
        OptimizationFactsCollector::new().collect(&FactsInput {
            runs: &runs,
            config: &config(),
            versions: &versions,
            since: "2026-04-01T00:00:00.000Z".to_string(),
        })
    }

    /// Die Kennzahlen eines Profils
    fn of(facts: &OptimizationFacts, profile: &str) -> ProfileFacts {
        facts
            .profiles
            .iter()
            .find(|p| p.profile == profile)
            .cloned()
            .expect("Profil")
    }

    #[test]
    fn zahlt_je_profil_nur_laufe_mit_den_aktuellen_limits_den_letzten_lauf_aber_unabhangig_davon() {
        let facts = collect(
            vec![
                run("a", |r| {
                    r.started_at = "2026-10-01T00:00:00.000Z".to_string()
                }),
                run("b", |r| {
                    r.max_turns = Some(20);
                    r.started_at = "2026-10-02T00:00:00.000Z".to_string();
                }),
            ],
            vec![],
        );
        let standard = of(&facts, "standard");
        assert_eq!(standard.runs, 1);
        assert_eq!(standard.starts, 1);
        assert_eq!(standard.max_turns, 30);
        assert_eq!(standard.timeout_minutes, 60);
        assert_eq!(
            standard.last_run_at.as_deref(),
            Some("2026-10-02T00:00:00.000Z")
        );
    }

    #[test]
    fn verfolgt_eskalationen_uberholte_starts_ankunfte_herkunft_und_wege() {
        let facts = collect(
            vec![
                run("T1", |_| {}),
                run("T1-2", |r| {
                    r.subject_id = "T1".to_string();
                    r.profile = "gross".to_string();
                    r.escalated_from = Some("standard".to_string());
                    r.attempt = Some(2);
                }),
                run("T2", |_| {}),
            ],
            vec![],
        );
        let standard = of(&facts, "standard");
        assert_eq!((standard.starts, standard.escalated_starts), (2, 1));
        let gross = of(&facts, "gross");
        assert_eq!(
            (
                gross.runs,
                gross.starts,
                gross.via_escalation,
                gross.top_origin
            ),
            (1, 0, 1, Some("standard".to_string()))
        );
        assert_eq!(
            facts.paths,
            vec![EscalationPath {
                from: "standard".to_string(),
                to: "gross".to_string(),
                count: 1,
            }]
        );
    }

    #[test]
    fn zahlt_abbruchgrunde_erfolg_im_ersten_versuch_p90_und_herkunft_der_starts() {
        let runs: Vec<DispatchRun> = (0..10)
            .map(|i| {
                run(&format!("r{i}"), |r| {
                    r.num_turns = Some(i + 1);
                    r.outcome = if i < 7 {
                        DispatchRunOutcome::Done
                    } else {
                        DispatchRunOutcome::Abort
                    };
                    r.abort_category = (i >= 7).then(|| "max-turns".to_string());
                    r.profile_reason = Some(if i < 2 {
                        DispatchProfileReason::Rule
                    } else {
                        DispatchProfileReason::Default
                    });
                    r.rule_index = (i < 2).then_some(1);
                })
            })
            .collect();
        let standard = of(&collect(runs, vec![]), "standard");
        assert_eq!(standard.runs, 10);
        assert_eq!(
            standard.aborts,
            BTreeMap::from([("max-turns".to_string(), 3)])
        );
        assert_eq!(standard.first_try_success, 7);
        assert_eq!(standard.turns_p90, Some(9));
        assert_eq!(
            standard.start_sources,
            vec![
                StartSource {
                    reason: Some(DispatchProfileReason::Default),
                    rule_index: None,
                    count: 8,
                },
                StartSource {
                    reason: Some(DispatchProfileReason::Rule),
                    rule_index: Some(1),
                    count: 2,
                },
            ]
        );
    }

    #[test]
    fn wertet_regeln_nur_fur_laufe_mit_derselben_fassung_der_regel_aus() {
        let fingerprint = DispatchConfigFingerprint::new();
        let normalizer = DispatchConfigNormalizer::new();
        let same = DispatchConfigVersion {
            hash: "same".to_string(),
            dispatcher: "d".to_string(),
            first_seen_at: "2026-09-01T00:00:00.000Z".to_string(),
            content: fingerprint.content(&config()),
        };
        let mut other_raw = raw();
        other_raw.as_object_mut().unwrap().insert(
            "rules".to_string(),
            json!([{ "when": { "labels": ["alt"] }, "profile": "klein" }]),
        );
        let changed = DispatchConfigVersion {
            hash: "old".to_string(),
            dispatcher: "d".to_string(),
            first_seen_at: "2026-08-01T00:00:00.000Z".to_string(),
            content: fingerprint.content(&normalizer.normalize(&other_raw).unwrap()),
        };
        // Lauf, den die Regel 1 gestartet hat
        let via_rule = |r: &mut DispatchRun| {
            r.profile = "klein".to_string();
            r.profile_reason = Some(DispatchProfileReason::Rule);
            r.rule_index = Some(1);
        };
        let facts = collect(
            vec![
                run("A", |r| {
                    via_rule(r);
                    r.config_hash = Some("same".to_string());
                    r.outcome = DispatchRunOutcome::AbortFinal;
                }),
                run("B", |r| {
                    via_rule(r);
                    r.config_hash = Some("same".to_string());
                }),
                run("B-2", |r| {
                    r.subject_id = "B".to_string();
                    r.escalated_from = Some("klein".to_string());
                    r.attempt = Some(2);
                }),
                run("C", |r| {
                    via_rule(r);
                    r.config_hash = Some("same".to_string());
                }),
                run("D", |r| {
                    via_rule(r);
                    r.config_hash = Some("old".to_string());
                }),
                run("E", |r| {
                    via_rule(r);
                    r.config_hash = None;
                }),
            ],
            vec![same, changed],
        );
        assert_eq!(
            facts.rules,
            vec![RuleFacts {
                index: 1,
                profile: "klein".to_string(),
                runs: 3,
                failed: 2,
            }]
        );
    }

    #[test]
    fn beschreibt_die_basis_nur_arbeitslaufe_projekte_und_zeitraum() {
        let facts = collect(
            vec![
                run("a", |_| {}),
                run("b", |r| r.project_id = 2),
                run("m", |r| {
                    r.kind = DispatchRunKind::Merge;
                    r.project_id = 3;
                }),
            ],
            vec![],
        );
        assert_eq!(
            facts.basis,
            OptimizationBasis {
                runs: 2,
                projects: 2,
                since: "2026-04-01T00:00:00.000Z".to_string(),
            }
        );
    }

    #[test]
    fn vergleicht_objekte_unabhangig_von_der_reihenfolge_der_schlussel() {
        assert_eq!(
            sorted_json(&json!({ "b": 1, "a": { "d": 2, "c": 3 } })),
            sorted_json(&json!({ "a": { "c": 3, "d": 2 }, "b": 1 }))
        );
    }
}
