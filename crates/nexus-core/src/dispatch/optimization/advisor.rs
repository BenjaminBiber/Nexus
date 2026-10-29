//! Fester Regelkatalog der Optimierungs-Vorschläge; portiert aus
//! `packages/core/src/dispatch/optimization/OptimizationAdvisor.ts`.
//!
//! Kennzahlen je Profil und Regel → Vorschläge mit Begründung und Änderung. Die Texte und
//! Schwellen stehen wortgleich zu TypeScript hier, damit beide Stände dieselben
//! Vorschlags-IDs erzeugen.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::dispatch::config::DispatchConfig;
use crate::dispatch::optimization::config_patcher::js_stringify;
use crate::dispatch::optimization::types::{
    AdvisorResult, ConfigPatchOp, InsufficientData, OptimizationFacts, ProfileFacts, RuleFacts,
    Suggestion, SuggestionEvidence, SuggestionKind, SuggestionSource, SuggestionTarget, MIN_RUNS,
};
use crate::dispatch::optimization::types::StartSource;
use crate::text::locale_cmp;

/// Ab diesem Anteil Abbrüche an den Schritten ist das Schrittlimit zu knapp
const TURNS_RATE: f64 = 0.2;
/// Ab diesem Anteil Abbrüche am Zeitlimit ist das Zeitlimit zu knapp
const TIMEOUT_RATE: f64 = 0.2;
/// Ab diesem Anteil Läufen mit vollem Kontext gibt es einen Hinweis
const CONTEXT_RATE: f64 = 0.15;
/// Ab diesem Anteil eskalierter Starts gilt ein Profil als überholt
const OVERTAKEN_RATE: f64 = 0.5;
/// Ab diesem Anteil gescheiterter Starts führt eine Regel in Abbrüche
const RULE_FAIL_RATE: f64 = 0.5;
/// Ab dieser Erstversuch-Quote kann ein Profil zu groß sein
const OVERSIZED_SUCCESS: f64 = 0.9;
/// … solange die Läufe höchstens diesen Anteil der Schritte brauchen
const OVERSIZED_TURNS: f64 = 0.4;
/// So lange ohne Lauf gilt ein Profil außerhalb von Leiter und Regeln als ungenutzt
const UNUSED_DAYS: i64 = 30;
/// Millisekunden eines Tages
const DAY_MS: i64 = 86_400_000;

/// Kurze, stabile ID eines Vorschlags aus seinem Fingerabdruck
pub fn suggestion_id(fingerprint: &str) -> String {
    let digest = Sha256::digest(fingerprint.as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    hex[..16].to_string()
}

/// Anteil als gerundeter Prozentsatz, wie `Math.round(rate * 100)`
fn pct(rate: f64) -> String {
    format!("{} %", js_round(rate * 100.0))
}

/// `Math.round` aus JavaScript (halbe Werte gehen nach oben)
fn js_round(value: f64) -> i64 {
    (value + 0.5).floor() as i64
}

/// 1,5-fach, auf 5 aufgerundet
fn raise(value: i64) -> i64 {
    (((value as f64 * 1.5) / 5.0).ceil() as i64) * 5
}

/// Die Limits eines Profils als Text für den Fingerabdruck
fn limits(facts: &ProfileFacts) -> String {
    format!(
        "maxTurns={},timeoutMinutes={}",
        facts.max_turns, facts.timeout_minutes
    )
}

/// Vorschlag vor ID und Gewicht; `values` geht bei Hinweisen in den Fingerabdruck
struct Draft {
    kind: SuggestionKind,
    title: String,
    reason: String,
    target: SuggestionTarget,
    patch: Option<Vec<ConfigPatchOp>>,
    evidence: Option<SuggestionEvidence>,
    values: String,
}

/// Fester Regelkatalog der Optimierungs-Vorschläge.
#[derive(Debug, Clone, Copy, Default)]
pub struct OptimizationAdvisor;

impl OptimizationAdvisor {
    /// Neuer Berater (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Vorschläge und Profile mit zu wenig Daten
    pub fn advise(
        &self,
        facts: &OptimizationFacts,
        config: &DispatchConfig,
        now: DateTime<Utc>,
    ) -> AdvisorResult {
        let used = used(config);
        let mut drafts: Vec<Draft> = vec![];
        let mut insufficient: Vec<InsufficientData> = vec![];
        for profile in &facts.profiles {
            drafts.extend(unused(profile, &used, now));
            if profile.runs < MIN_RUNS {
                if used.contains(&profile.profile) {
                    insufficient.push(InsufficientData {
                        profile: profile.profile.clone(),
                        runs: profile.runs,
                        needed: MIN_RUNS,
                    });
                }
                continue;
            }
            drafts.extend(self.limits(profile));
            drafts.extend(self.overtaken(profile, config));
            drafts.extend(self.dominates(profile, facts, config));
            drafts.extend(self.oversized(profile, config));
        }
        for rule in &facts.rules {
            drafts.extend(self.rule_fails(rule, config));
        }
        let suggestions: Vec<Suggestion> = drafts.iter().map(finish).collect();
        AdvisorResult {
            suggestions: Self::sort(dedupe(suggestions)),
            insufficient,
        }
    }

    /// Übernehmbare zuerst, dann nach Gewicht, dann nach Titel
    pub fn sort(list: Vec<Suggestion>) -> Vec<Suggestion> {
        let mut list = list;
        list.sort_by(|a, b| {
            b.patch
                .is_some()
                .cmp(&a.patch.is_some())
                .then_with(|| {
                    b.weight
                        .partial_cmp(&a.weight)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| locale_cmp(&a.title, &b.title))
        });
        list
    }

    /// Schritt-, Zeitlimit- und Kontext-Abbrüche eines Profils
    fn limits(&self, f: &ProfileFacts) -> Vec<Draft> {
        let mut out: Vec<Draft> = vec![];
        let runs = f.runs as f64;
        let turns = f.aborts.get("max-turns").copied().unwrap_or(0);
        if turns as f64 / runs >= TURNS_RATE {
            let rate = turns as f64 / runs;
            out.push(Draft {
                kind: SuggestionKind::TurnsTooLow,
                title: format!("Schritte zu knapp · {}", f.profile),
                reason: format!(
                    "{turns} von {} Läufen ({}) mit „{}“ brachen ab, weil {} Schritte nicht reichten.",
                    f.runs,
                    pct(rate),
                    f.profile,
                    f.max_turns
                ),
                target: SuggestionTarget {
                    profile: Some(f.profile.clone()),
                    rule_index: None,
                },
                patch: Some(vec![ConfigPatchOp {
                    path: format!("profiles.{}.maxTurns", f.profile),
                    from: Some(json!(f.max_turns)),
                    to: json!(raise(f.max_turns)),
                }]),
                evidence: Some(SuggestionEvidence {
                    runs: f.runs,
                    hits: turns,
                    rate,
                }),
                values: limits(f),
            });
        }
        let timeouts = f.aborts.get("timeout").copied().unwrap_or(0);
        if timeouts as f64 / runs >= TIMEOUT_RATE {
            let rate = timeouts as f64 / runs;
            out.push(Draft {
                kind: SuggestionKind::TimeoutTooLow,
                title: format!("Zeitlimit zu knapp · {}", f.profile),
                reason: format!(
                    "{timeouts} von {} Läufen ({}) mit „{}“ brachen nach {} Minuten am Zeitlimit ab.",
                    f.runs,
                    pct(rate),
                    f.profile,
                    f.timeout_minutes
                ),
                target: SuggestionTarget {
                    profile: Some(f.profile.clone()),
                    rule_index: None,
                },
                patch: Some(vec![ConfigPatchOp {
                    path: format!("profiles.{}.timeoutMinutes", f.profile),
                    from: Some(json!(f.timeout_minutes)),
                    to: json!(raise(f.timeout_minutes)),
                }]),
                evidence: Some(SuggestionEvidence {
                    runs: f.runs,
                    hits: timeouts,
                    rate,
                }),
                values: limits(f),
            });
        }
        let context = f.aborts.get("context").copied().unwrap_or(0);
        if context as f64 / runs >= CONTEXT_RATE {
            let rate = context as f64 / runs;
            out.push(Draft {
                kind: SuggestionKind::ContextFull,
                title: format!("Kontext läuft voll · {}", f.profile),
                reason: format!(
                    "{context} von {} Läufen ({}) mit „{}“ liefen in die Grenze des Kontextfensters. \
Ein Modell mit größerem Kontext (z. B. „opus[1m]“) oder kleiner geschnittene Tickets helfen.",
                    f.runs,
                    pct(rate),
                    f.profile
                ),
                target: SuggestionTarget {
                    profile: Some(f.profile.clone()),
                    rule_index: None,
                },
                patch: None,
                evidence: Some(SuggestionEvidence {
                    runs: f.runs,
                    hits: context,
                    rate,
                }),
                values: limits(f),
            });
        }
        out
    }

    /// Ein Profil, dessen Starts meist eskaliert werden
    fn overtaken(&self, f: &ProfileFacts, config: &DispatchConfig) -> Vec<Draft> {
        if f.starts < MIN_RUNS {
            return vec![];
        }
        let rate = f.escalated_starts as f64 / f.starts as f64;
        if rate < OVERTAKEN_RATE {
            return vec![];
        }
        let next = next_step(config, &f.profile);
        let way = next
            .as_deref()
            .and_then(|next| redirect(&f.start_sources, &f.profile, next, config));
        let mut reason = format!(
            "{} von {} Tickets ({}), die mit „{}“ starteten, mussten eskaliert werden.",
            f.escalated_starts,
            f.starts,
            pct(rate),
            f.profile
        );
        match (&next, &way) {
            (None, _) => reason.push_str(&format!(
                " Über „{}“ gibt es keine höhere Stufe in der Leiter.",
                f.profile
            )),
            (Some(next), Some(way)) if way.rule_index.is_some() => reason.push_str(&format!(
                " Sie kamen meist über Regel {} – sie könnte gleich „{next}“ wählen.",
                way.rule_index.unwrap_or_default()
            )),
            (Some(next), Some(_)) => reason.push_str(&format!(
                " „{}“ ist das Standardprofil – „{next}“ als Standard spart die Umwege.",
                f.profile
            )),
            (Some(_), None) => {}
        }
        vec![Draft {
            kind: SuggestionKind::ProfileOvertaken,
            title: format!("Profil wird meist überholt · {}", f.profile),
            reason,
            target: SuggestionTarget {
                profile: Some(f.profile.clone()),
                rule_index: way.as_ref().and_then(|way| way.rule_index),
            },
            patch: way.map(|way| way.patch),
            evidence: Some(SuggestionEvidence {
                runs: f.starts,
                hits: f.escalated_starts,
                rate,
            }),
            values: limits(f),
        }]
    }

    /// Ein Eskalationsziel, das öfter per Eskalation als direkt genutzt wird
    fn dominates(
        &self,
        f: &ProfileFacts,
        facts: &OptimizationFacts,
        config: &DispatchConfig,
    ) -> Vec<Draft> {
        let Some(top_origin) = f.top_origin.clone() else {
            return vec![];
        };
        if f.via_escalation < MIN_RUNS || f.via_escalation <= f.starts {
            return vec![];
        }
        let origin = facts
            .profiles
            .iter()
            .find(|profile| profile.profile == top_origin);
        let way = origin.and_then(|origin| {
            redirect(&origin.start_sources, &origin.profile, &f.profile, config)
        });
        let rate = f.via_escalation as f64 / (f.via_escalation + f.starts) as f64;
        let mut reason = format!(
            "„{}“ wurde {}-mal per Eskalation erreicht, aber nur {}-mal direkt gewählt – meist von „{top_origin}“ aus.",
            f.profile, f.via_escalation, f.starts
        );
        match &way {
            Some(way) if way.rule_index.is_some() => reason.push_str(&format!(
                " Regel {} könnte gleich „{}“ wählen.",
                way.rule_index.unwrap_or_default(),
                f.profile
            )),
            Some(_) => reason.push_str(&format!(
                " „{}“ als Standardprofil spart die Umwege.",
                f.profile
            )),
            None => {}
        }
        vec![Draft {
            kind: SuggestionKind::EscalationTargetDominates,
            title: format!(
                "Eskalationsziel öfter genutzt als direkt gewählt · {}",
                f.profile
            ),
            reason,
            target: SuggestionTarget {
                profile: Some(f.profile.clone()),
                rule_index: way.as_ref().and_then(|way| way.rule_index),
            },
            patch: way.map(|way| way.patch),
            evidence: Some(SuggestionEvidence {
                runs: f.via_escalation + f.starts,
                hits: f.via_escalation,
                rate,
            }),
            values: format!("origin={top_origin}"),
        }]
    }

    /// Ein höheres Profil, das fast immer im ersten Versuch und mit wenig Schritten gelingt
    fn oversized(&self, f: &ProfileFacts, config: &DispatchConfig) -> Vec<Draft> {
        let index = config
            .ladder
            .iter()
            .position(|profile| *profile == f.profile);
        let (Some(index), Some(turns_p90)) = (index, f.turns_p90) else {
            return vec![];
        };
        if index == 0 || f.starts < MIN_RUNS {
            return vec![];
        }
        let rate = f.first_try_success as f64 / f.starts as f64;
        if rate < OVERSIZED_SUCCESS || turns_p90 as f64 > f.max_turns as f64 * OVERSIZED_TURNS {
            return vec![];
        }
        vec![Draft {
            kind: SuggestionKind::ProfileOversized,
            title: format!("Profil eventuell zu groß · {}", f.profile),
            reason: format!(
                "{} der Tickets mit „{}“ gelingen im ersten Versuch, 90 % der Läufe brauchen höchstens {turns_p90} von \
{} Schritten. Ein Versuch mit „{}“ oder weniger Schritten könnte Kosten sparen.",
                pct(rate),
                f.profile,
                f.max_turns,
                config.ladder[index - 1]
            ),
            target: SuggestionTarget {
                profile: Some(f.profile.clone()),
                rule_index: None,
            },
            patch: None,
            evidence: Some(SuggestionEvidence {
                runs: f.starts,
                hits: f.first_try_success,
                rate,
            }),
            values: limits(f),
        }]
    }

    /// Eine Regel, deren Tickets meist eskaliert werden oder scheitern
    fn rule_fails(&self, r: &RuleFacts, config: &DispatchConfig) -> Vec<Draft> {
        if r.runs < MIN_RUNS {
            return vec![];
        }
        let rate = r.failed as f64 / r.runs as f64;
        if rate < RULE_FAIL_RATE {
            return vec![];
        }
        let next = next_step(config, &r.profile);
        let reason = format!(
            "{} von {} Tickets ({}) aus Regel {} („{}“) wurden eskaliert oder scheiterten endgültig.{}",
            r.failed,
            r.runs,
            pct(rate),
            r.index,
            r.profile,
            match &next {
                Some(next) => format!(" Die nächste Stufe „{next}“ wäre ein besserer Start."),
                None => format!(
                    " Über „{}“ gibt es keine höhere Stufe in der Leiter.",
                    r.profile
                ),
            }
        );
        vec![Draft {
            kind: SuggestionKind::RuleFails,
            title: format!("Regel {} führt in Abbrüche", r.index),
            reason,
            target: SuggestionTarget {
                profile: Some(r.profile.clone()),
                rule_index: Some(r.index),
            },
            patch: next.map(|next| {
                vec![ConfigPatchOp {
                    path: format!("rules.{}.profile", r.index - 1),
                    from: Some(json!(r.profile)),
                    to: json!(next),
                }]
            }),
            evidence: Some(SuggestionEvidence {
                runs: r.runs,
                hits: r.failed,
                rate,
            }),
            values: format!("profile={}", r.profile),
        }]
    }
}

/// Profile, die der Dispatcher von sich aus wählen kann; research und planning gelten als
/// genutzt, weil sie ohne Einstellung greifen
fn used(config: &DispatchConfig) -> HashSet<String> {
    let mut out: Vec<String> = vec![config.default_profile.clone()];
    out.extend(config.ladder.iter().cloned());
    out.extend(config.rules.iter().map(|rule| rule.profile.clone()));
    out.extend(config.research.profile.clone());
    out.extend(config.planning.profile.clone());
    out.push("research".to_string());
    out.push("planning".to_string());
    out.into_iter().filter(|name| !name.is_empty()).collect()
}

/// Ein Profil außerhalb von Leiter und Regeln, das lange nicht lief
fn unused(f: &ProfileFacts, used: &HashSet<String>, now: DateTime<Utc>) -> Vec<Draft> {
    if used.contains(&f.profile) {
        return vec![];
    }
    if let Some(last) = f.last_run_at.as_deref() {
        let last = DateTime::parse_from_rfc3339(last)
            .map(|time| time.with_timezone(&Utc))
            .ok();
        // Ein unlesbarer Zeitpunkt ist wie `NaN` in TypeScript: der Vergleich trifft nicht zu
        if let Some(last) = last {
            if now.timestamp_millis() - last.timestamp_millis() < UNUSED_DAYS * DAY_MS {
                return vec![];
            }
        }
    }
    vec![Draft {
        kind: SuggestionKind::ProfileUnused,
        title: format!("Profil ungenutzt · {}", f.profile),
        reason: format!(
            "„{}“ steht weder in der Leiter noch in einer Regel und lief seit über {UNUSED_DAYS} Tagen nicht. Es kann entfernt werden.",
            f.profile
        ),
        target: SuggestionTarget {
            profile: Some(f.profile.clone()),
            rule_index: None,
        },
        patch: None,
        evidence: None,
        values: String::new(),
    }]
}

/// Nächste Stufe der Leiter; `None` an der Spitze oder außerhalb der Leiter
fn next_step(config: &DispatchConfig, profile: &str) -> Option<String> {
    let index = config.ladder.iter().position(|name| name == profile)?;
    config.ladder.get(index + 1).cloned()
}

/// Eine Umlenkung: die Änderung und die Regel, über die sie geht
struct Redirect {
    patch: Vec<ConfigPatchOp>,
    rule_index: Option<i64>,
}

/// Änderung, die die Wahl von `from` auf `to` umlenkt: über die Regel, aus der seine Starts
/// meist kommen, sonst über das Standardprofil
fn redirect(
    start_sources: &[StartSource],
    from: &str,
    to: &str,
    config: &DispatchConfig,
) -> Option<Redirect> {
    let source = start_sources.first();
    if let Some(source) = source {
        if source.reason == Some(crate::board::DispatchProfileReason::Rule) {
            if let Some(rule_index) = source.rule_index {
                let index = rule_index - 1;
                let rule = usize::try_from(index)
                    .ok()
                    .and_then(|index| config.rules.get(index));
                if rule.map(|rule| rule.profile.as_str()) != Some(from) {
                    return None;
                }
                return Some(Redirect {
                    patch: vec![ConfigPatchOp {
                        path: format!("rules.{index}.profile"),
                        from: Some(json!(from)),
                        to: json!(to),
                    }],
                    rule_index: Some(rule_index),
                });
            }
        }
    }
    if config.default_profile == from {
        return Some(Redirect {
            patch: vec![ConfigPatchOp {
                path: "defaultProfile".to_string(),
                from: Some(json!(from)),
                to: json!(to),
            }],
            rule_index: None,
        });
    }
    None
}

/// Fingerabdruck aus Regel, Ziel und betroffenen Werten; Gewicht = betroffene Läufe × Quote
fn finish(draft: &Draft) -> Suggestion {
    let target = format!(
        "{}{}",
        draft.target.profile.clone().unwrap_or_default(),
        match draft.target.rule_index {
            Some(index) if index != 0 => format!("#{index}"),
            _ => String::new(),
        }
    );
    let values = match &draft.patch {
        Some(ops) => ops
            .iter()
            .map(|op| format!("{}={}", op.path, js_stringify(op.from.as_ref())))
            .collect::<Vec<String>>()
            .join(","),
        None => draft.values.clone(),
    };
    let kind = kind_text(draft.kind);
    let fingerprint = format!("rule:{kind}:{target}:{values}");
    Suggestion {
        id: suggestion_id(&fingerprint),
        fingerprint,
        source: SuggestionSource::Rule,
        kind: draft.kind,
        title: draft.title.clone(),
        reason: draft.reason.clone(),
        target: draft.target.clone(),
        patch: draft.patch.clone(),
        weight: draft
            .evidence
            .map(|evidence| evidence.hits as f64 * evidence.rate)
            .unwrap_or(0.0),
        evidence: draft.evidence,
    }
}

/// Die Regel als Text, wie sie in JSON steht
fn kind_text(kind: SuggestionKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Gleiche Änderung aus zwei Regeln (z. B. „überholt“ und „Eskalationsziel“): nur die
/// gewichtigere bleibt
fn dedupe(list: Vec<Suggestion>) -> Vec<Suggestion> {
    let hints: Vec<Suggestion> = list
        .iter()
        .filter(|s| s.patch.is_none())
        .cloned()
        .collect();
    let mut order: Vec<String> = vec![];
    let mut best: HashMap<String, Suggestion> = HashMap::new();
    for suggestion in list.into_iter().filter(|s| s.patch.is_some()) {
        let key = serde_json::to_string(&suggestion.patch).unwrap_or_default();
        match best.get(&key) {
            Some(previous) if suggestion.weight <= previous.weight => {}
            Some(_) => {
                best.insert(key, suggestion);
            }
            None => {
                order.push(key.clone());
                best.insert(key, suggestion);
            }
        }
    }
    let mut out: Vec<Suggestion> = order
        .into_iter()
        .filter_map(|key| best.remove(&key))
        .collect();
    out.extend(hints);
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::board::DispatchProfileReason;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;
    use crate::dispatch::optimization::types::OptimizationBasis;

    /// Limits der Profile der Testeinstellung
    const LIMITS: [(&str, i64, i64); 4] = [
        ("klein", 20, 30),
        ("standard", 30, 60),
        ("gross", 80, 120),
        ("alt", 10, 10),
    ];

    /// Die aufgelöste Testeinstellung
    fn config() -> DispatchConfig {
        let normalizer = DispatchConfigNormalizer::new();
        let mut raw = normalizer.example();
        let object = raw.as_object_mut().unwrap();
        object.insert(
            "profiles".to_string(),
            json!({
                "klein": { "model": "sonnet", "maxTurns": 20, "timeoutMinutes": 30 },
                "standard": { "model": "opus", "maxTurns": 30, "timeoutMinutes": 60 },
                "gross": { "model": "opus", "maxTurns": 80, "timeoutMinutes": 120 },
                "alt": { "model": "sonnet", "maxTurns": 10, "timeoutMinutes": 10 },
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
        normalizer.normalize(&raw).unwrap()
    }

    /// Der Zeitpunkt der Auswertung
    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-22T12:00:00.000Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    /// Unauffälliges Profil mit 10 Läufen; einzelne Werte überschreibbar
    fn pf(profile: &str, change: impl FnOnce(&mut ProfileFacts)) -> ProfileFacts {
        let (_, max_turns, timeout_minutes) = LIMITS
            .iter()
            .find(|(name, _, _)| *name == profile)
            .copied()
            .expect("bekanntes Profil");
        let mut facts = ProfileFacts {
            profile: profile.to_string(),
            max_turns,
            timeout_minutes,
            runs: 10,
            starts: 10,
            escalated_starts: 0,
            via_escalation: 0,
            top_origin: None,
            aborts: BTreeMap::new(),
            first_try_success: 5,
            turns_p90: Some(js_round(max_turns as f64 * 0.8)),
            start_sources: vec![StartSource {
                reason: Some(DispatchProfileReason::Default),
                rule_index: None,
                count: 10,
            }],
            last_run_at: Some("2026-10-20T00:00:00.000Z".to_string()),
        };
        change(&mut facts);
        facts
    }

    /// Kennzahlen mit diesen Profilen und Regeln; fehlende Profile sind unauffällig
    fn facts(profiles: Vec<ProfileFacts>, rules: Vec<RuleFacts>) -> OptimizationFacts {
        let all = ["klein", "standard", "gross", "alt"]
            .iter()
            .map(|name| {
                profiles
                    .iter()
                    .find(|profile| profile.profile == *name)
                    .cloned()
                    .unwrap_or_else(|| pf(name, |_| {}))
            })
            .collect();
        OptimizationFacts {
            basis: OptimizationBasis {
                runs: 40,
                projects: 2,
                since: "2026-04-25T00:00:00.000Z".to_string(),
            },
            profiles: all,
            rules,
            paths: vec![],
        }
    }

    /// Der Regelkatalog über diese Kennzahlen
    fn advise(facts: &OptimizationFacts) -> AdvisorResult {
        OptimizationAdvisor::new().advise(facts, &config(), now())
    }

    /// Die Regeln der Vorschläge in ihrer Reihenfolge
    fn kinds(facts: &OptimizationFacts) -> Vec<SuggestionKind> {
        advise(facts)
            .suggestions
            .iter()
            .map(|suggestion| suggestion.kind)
            .collect()
    }

    #[test]
    fn schlagt_bei_20_prozent_abbruchen_wegen_schritten_15_fache_schritte_vor_auf_5_gerundet() {
        let mit_abbruchen = facts(
            vec![pf("standard", |f| {
                f.aborts = BTreeMap::from([("max-turns".to_string(), 2)]);
            })],
            vec![],
        );
        let result = advise(&mit_abbruchen);
        let s = &result.suggestions[0];
        assert_eq!(s.kind, SuggestionKind::TurnsTooLow);
        assert_eq!(s.source, SuggestionSource::Rule);
        assert_eq!(s.title, "Schritte zu knapp · standard");
        assert_eq!(s.target.profile.as_deref(), Some("standard"));
        assert_eq!(
            s.patch,
            Some(vec![ConfigPatchOp {
                path: "profiles.standard.maxTurns".to_string(),
                from: Some(json!(30)),
                to: json!(45),
            }])
        );
        assert_eq!(
            s.evidence,
            Some(SuggestionEvidence {
                runs: 10,
                hits: 2,
                rate: 0.2
            })
        );
        assert!(s.reason.contains("2 von 10 Läufen (20 %)"), "{}", s.reason);
        assert_eq!(s.id.len(), 16);
        assert!(s.id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        let knapp = facts(
            vec![pf("standard", |f| {
                f.aborts = BTreeMap::from([("max-turns".to_string(), 1)]);
            })],
            vec![],
        );
        assert!(!kinds(&knapp).contains(&SuggestionKind::TurnsTooLow));
    }

    #[test]
    fn schlagt_bei_20_prozent_zeitlimit_abbruchen_ein_langeres_zeitlimit_vor() {
        let result = advise(&facts(
            vec![pf("standard", |f| {
                f.aborts = BTreeMap::from([("timeout".to_string(), 3)]);
            })],
            vec![],
        ));
        assert_eq!(result.suggestions[0].kind, SuggestionKind::TimeoutTooLow);
        assert_eq!(
            result.suggestions[0].patch,
            Some(vec![ConfigPatchOp {
                path: "profiles.standard.timeoutMinutes".to_string(),
                from: Some(json!(60)),
                to: json!(90),
            }])
        );
    }

    #[test]
    fn meldet_vollen_kontext_ab_15_prozent_nur_als_hinweis() {
        let result = advise(&facts(
            vec![pf("standard", |f| {
                f.aborts = BTreeMap::from([("context".to_string(), 2)]);
            })],
            vec![],
        ));
        assert_eq!(result.suggestions[0].kind, SuggestionKind::ContextFull);
        assert_eq!(result.suggestions[0].patch, None);
        let knapp = facts(
            vec![pf("standard", |f| {
                f.aborts = BTreeMap::from([("context".to_string(), 1)]);
            })],
            vec![],
        );
        assert!(!kinds(&knapp).contains(&SuggestionKind::ContextFull));
    }

    #[test]
    fn urteilt_unter_5_laufen_nicht_und_meldet_genutzte_profile_als_zu_wenig_daten() {
        let result = advise(&facts(
            vec![pf("standard", |f| {
                f.runs = 4;
                f.starts = 4;
                f.aborts = BTreeMap::from([("max-turns".to_string(), 4)]);
            })],
            vec![],
        ));
        assert!(!result
            .suggestions
            .iter()
            .any(|s| s.kind == SuggestionKind::TurnsTooLow));
        assert_eq!(
            result.insufficient,
            vec![InsufficientData {
                profile: "standard".to_string(),
                runs: 4,
                needed: 5
            }]
        );
    }

    #[test]
    fn lenkt_ein_meist_uberholtes_standardprofil_auf_die_nachste_stufe() {
        let result = advise(&facts(
            vec![pf("standard", |f| f.escalated_starts = 5)],
            vec![],
        ));
        assert_eq!(
            result.suggestions[0].kind,
            SuggestionKind::ProfileOvertaken
        );
        assert_eq!(
            result.suggestions[0].patch,
            Some(vec![ConfigPatchOp {
                path: "defaultProfile".to_string(),
                from: Some(json!("standard")),
                to: json!("gross"),
            }])
        );
    }

    #[test]
    fn lenkt_ein_meist_uberholtes_profil_uber_seine_regel_um() {
        let result = advise(&facts(
            vec![pf("klein", |f| {
                f.escalated_starts = 6;
                f.start_sources = vec![StartSource {
                    reason: Some(DispatchProfileReason::Rule),
                    rule_index: Some(1),
                    count: 10,
                }];
            })],
            vec![],
        ));
        let s = &result.suggestions[0];
        assert_eq!(s.kind, SuggestionKind::ProfileOvertaken);
        assert_eq!(s.target.profile.as_deref(), Some("klein"));
        assert_eq!(s.target.rule_index, Some(1));
        assert_eq!(
            s.patch,
            Some(vec![ConfigPatchOp {
                path: "rules.0.profile".to_string(),
                from: Some(json!("klein")),
                to: json!("standard"),
            }])
        );
    }

    #[test]
    fn gibt_fur_die_oberste_stufe_der_leiter_nur_einen_hinweis() {
        let result = advise(&facts(vec![pf("gross", |f| f.escalated_starts = 8)], vec![]));
        assert_eq!(
            result.suggestions[0].kind,
            SuggestionKind::ProfileOvertaken
        );
        assert_eq!(result.suggestions[0].patch, None);
    }

    #[test]
    fn macht_ein_eskalationsziel_zum_start_und_fasst_gleiche_anderungen_zusammen() {
        let result = advise(&facts(
            vec![
                pf("gross", |f| {
                    f.runs = 8;
                    f.starts = 2;
                    f.via_escalation = 6;
                    f.top_origin = Some("standard".to_string());
                }),
                pf("standard", |f| f.escalated_starts = 5),
            ],
            vec![],
        ));
        let patched: Vec<&Suggestion> = result
            .suggestions
            .iter()
            .filter(|s| {
                s.patch
                    .as_ref()
                    .and_then(|ops| ops.first())
                    .is_some_and(|op| op.path == "defaultProfile")
            })
            .collect();
        assert_eq!(patched.len(), 1);
        assert_eq!(patched[0].kind, SuggestionKind::EscalationTargetDominates);
        assert_eq!(
            patched[0].patch,
            Some(vec![ConfigPatchOp {
                path: "defaultProfile".to_string(),
                from: Some(json!("standard")),
                to: json!("gross"),
            }])
        );
    }

    #[test]
    fn hebt_das_ziel_einer_regel_mit_vielen_abbruchen_an() {
        let result = advise(&facts(
            vec![],
            vec![RuleFacts {
                index: 1,
                profile: "klein".to_string(),
                runs: 6,
                failed: 3,
            }],
        ));
        let s = &result.suggestions[0];
        assert_eq!(s.kind, SuggestionKind::RuleFails);
        assert_eq!(s.title, "Regel 1 führt in Abbrüche");
        assert_eq!(s.target.profile.as_deref(), Some("klein"));
        assert_eq!(s.target.rule_index, Some(1));
        assert_eq!(
            s.patch,
            Some(vec![ConfigPatchOp {
                path: "rules.0.profile".to_string(),
                from: Some(json!("klein")),
                to: json!("standard"),
            }])
        );
        let zu_wenig = facts(
            vec![],
            vec![RuleFacts {
                index: 1,
                profile: "klein".to_string(),
                runs: 4,
                failed: 4,
            }],
        );
        assert!(!kinds(&zu_wenig).contains(&SuggestionKind::RuleFails));
    }

    #[test]
    fn halt_ein_hoheres_profil_mit_glattem_ersten_versuch_fur_eventuell_zu_gross() {
        let result = advise(&facts(
            vec![pf("gross", |f| {
                f.first_try_success = 9;
                f.turns_p90 = Some(32);
            })],
            vec![],
        ));
        assert_eq!(
            result.suggestions[0].kind,
            SuggestionKind::ProfileOversized
        );
        assert_eq!(result.suggestions[0].patch, None);
        let zu_viele_schritte = facts(
            vec![pf("gross", |f| {
                f.first_try_success = 9;
                f.turns_p90 = Some(33);
            })],
            vec![],
        );
        assert!(!kinds(&zu_viele_schritte).contains(&SuggestionKind::ProfileOversized));
        let unterste_stufe = facts(
            vec![pf("klein", |f| {
                f.first_try_success = 10;
                f.turns_p90 = Some(1);
            })],
            vec![],
        );
        assert!(!kinds(&unterste_stufe).contains(&SuggestionKind::ProfileOversized));
    }

    #[test]
    fn meldet_ein_profil_ausserhalb_von_leiter_und_regeln_ohne_lauf_als_ungenutzt() {
        let ohne_lauf = facts(
            vec![pf("alt", |f| {
                f.runs = 0;
                f.starts = 0;
                f.last_run_at = None;
            })],
            vec![],
        );
        let result = advise(&ohne_lauf);
        assert_eq!(result.suggestions[0].kind, SuggestionKind::ProfileUnused);
        assert_eq!(result.suggestions[0].patch, None);
        assert_eq!(result.insufficient, vec![]);
        let kurzlich = facts(
            vec![pf("alt", |f| {
                f.runs = 0;
                f.starts = 0;
                f.last_run_at = Some("2026-10-17T00:00:00.000Z".to_string());
            })],
            vec![],
        );
        assert!(!kinds(&kurzlich).contains(&SuggestionKind::ProfileUnused));
    }

    /// Referenzwerte aus dem TypeScript-Stand; erzeugt mit einem einmaligen tsx-Skript, das
    /// `OptimizationAdvisor.advise` über dieselben Kennzahlen laufen ließ und je Vorschlag
    /// Regel, Fingerabdruck und ID ausgab
    #[test]
    fn erzeugt_dieselben_fingerabdrucke_und_ids_wie_typescript() {
        let szenarien: Vec<(OptimizationFacts, Vec<(&str, &str, &str)>)> = vec![
            (
                facts(
                    vec![pf("standard", |f| {
                        f.aborts = BTreeMap::from([
                            ("max-turns".to_string(), 2),
                            ("timeout".to_string(), 3),
                            ("context".to_string(), 5),
                        ]);
                    })],
                    vec![],
                ),
                vec![
                    (
                        "timeout-too-low",
                        "rule:timeout-too-low:standard:profiles.standard.timeoutMinutes=60",
                        "b92106225b2f8db8",
                    ),
                    (
                        "turns-too-low",
                        "rule:turns-too-low:standard:profiles.standard.maxTurns=30",
                        "ab518721e45623aa",
                    ),
                    (
                        "context-full",
                        "rule:context-full:standard:maxTurns=30,timeoutMinutes=60",
                        "77d597fca10821af",
                    ),
                ],
            ),
            (
                facts(
                    vec![pf("klein", |f| {
                        f.escalated_starts = 6;
                        f.start_sources = vec![StartSource {
                            reason: Some(DispatchProfileReason::Rule),
                            rule_index: Some(1),
                            count: 10,
                        }];
                    })],
                    vec![],
                ),
                vec![(
                    "profile-overtaken",
                    "rule:profile-overtaken:klein#1:rules.0.profile=\"klein\"",
                    "9fb75945a6840055",
                )],
            ),
            (
                facts(
                    vec![
                        pf("gross", |f| {
                            f.runs = 8;
                            f.starts = 2;
                            f.via_escalation = 6;
                            f.top_origin = Some("standard".to_string());
                        }),
                        pf("standard", |f| f.escalated_starts = 5),
                    ],
                    vec![],
                ),
                vec![(
                    "escalation-target-dominates",
                    "rule:escalation-target-dominates:gross:defaultProfile=\"standard\"",
                    "3d258a08168a46ec",
                )],
            ),
            (
                facts(
                    vec![pf("gross", |f| {
                        f.first_try_success = 9;
                        f.turns_p90 = Some(32);
                    })],
                    vec![],
                ),
                vec![(
                    "profile-oversized",
                    "rule:profile-oversized:gross:maxTurns=80,timeoutMinutes=120",
                    "2692bc711db9492f",
                )],
            ),
            (
                facts(
                    vec![pf("alt", |f| {
                        f.runs = 0;
                        f.starts = 0;
                        f.last_run_at = None;
                    })],
                    vec![],
                ),
                vec![("profile-unused", "rule:profile-unused:alt:", "4e9deb44a6a4cf40")],
            ),
            (
                facts(
                    vec![],
                    vec![RuleFacts {
                        index: 1,
                        profile: "klein".to_string(),
                        runs: 6,
                        failed: 3,
                    }],
                ),
                vec![(
                    "rule-fails",
                    "rule:rule-fails:klein#1:rules.0.profile=\"klein\"",
                    "b50e1334ee0916ee",
                )],
            ),
        ];
        for (kennzahlen, erwartet) in szenarien {
            let suggestions = advise(&kennzahlen).suggestions;
            assert_eq!(
                suggestions
                    .iter()
                    .map(|s| (kind_text(s.kind), s.fingerprint.clone(), s.id.clone()))
                    .collect::<Vec<(String, String, String)>>(),
                erwartet
                    .iter()
                    .map(|(kind, fingerprint, id)| (
                        kind.to_string(),
                        fingerprint.to_string(),
                        id.to_string()
                    ))
                    .collect::<Vec<(String, String, String)>>()
            );
        }
    }

    #[test]
    fn sortiert_ubernehmbares_vor_hinweisen_und_andert_die_id_mit_dem_profil() {
        let result = advise(&facts(
            vec![pf("standard", |f| {
                f.aborts = BTreeMap::from([
                    ("context".to_string(), 5),
                    ("max-turns".to_string(), 2),
                ]);
            })],
            vec![],
        ));
        assert_eq!(
            result
                .suggestions
                .iter()
                .map(|s| s.kind)
                .collect::<Vec<SuggestionKind>>(),
            vec![SuggestionKind::TurnsTooLow, SuggestionKind::ContextFull]
        );
        let vorher = result.suggestions[0].id.clone();
        let geandert = advise(&facts(
            vec![pf("standard", |f| {
                f.max_turns = 45;
                f.aborts = BTreeMap::from([("max-turns".to_string(), 2)]);
            })],
            vec![],
        ));
        assert_ne!(geandert.suggestions[0].id, vorher);
    }
}
