//! Rechnet aus den Läufen die Kennzahlen des Dashboards (ohne Datenbank): Erfolg, Eskalation
//! entlang der Profile, Abbruchgründe. Portiert aus
//! `packages/core/src/board/DispatchStatsCalculator.ts`.
//!
//! Die Grenzen der Zeitreihe liegen in Ortszeit – wie in TypeScript, wo `new Date(y, m, d)`
//! und `getFullYear()` die Zeitzone des Rechners nehmen.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, TimeDelta, Utc};

use crate::board::dispatch_run_types::{
    DispatchAbortReason, DispatchLadder, DispatchProfileRow, DispatchRun, DispatchRunKind,
    DispatchRunOutcome, DispatchSeries, DispatchSeriesPoint, DispatchSeriesUnit, DispatchStats,
    DispatchStatsKpis,
};
use crate::clock::parse_millis;
use crate::dispatch::abort_reason::{AbortCategory, AbortReasonClassifier};

/// Erfolgreiche Ausgänge (Merge-Sitzung und Recherche zählen bei ihrer Art als Erfolg)
const SUCCESS: [DispatchRunOutcome; 5] = [
    DispatchRunOutcome::Done,
    DispatchRunOutcome::Review,
    DispatchRunOutcome::Human,
    DispatchRunOutcome::MergeSession,
    DispatchRunOutcome::Research,
];
/// Abgebrochene Ausgänge
const ABORTED: [DispatchRunOutcome; 2] =
    [DispatchRunOutcome::Abort, DispatchRunOutcome::AbortFinal];
/// Ausgänge, bei denen der Lauf auf einen Menschen wartet
const WAITING: [DispatchRunOutcome; 2] =
    [DispatchRunOutcome::Question, DispatchRunOutcome::Blocked];
/// Abbruchkategorien, die sich über die Einstellung beeinflussen lassen
const CONFIGURABLE: [AbortCategory; 3] = [
    AbortCategory::MaxTurns,
    AbortCategory::Timeout,
    AbortCategory::Context,
];
/// Bis zu so vielen Tagen zeigt die Zeitreihe Tage statt Wochen
const DAILY_MAX_DAYS: i64 = 28;

/// Rohfakten der Auswertung, wie sie die Repositories liefern
#[derive(Debug, Clone)]
pub struct DispatchStatsFacts {
    pub from: DateTime<Utc>,
    /// ausschließlich
    pub to: DateTime<Utc>,
    pub kind: DispatchRunKind,
    /// Läufe aller Arten mit Start im Zeitraum
    pub runs: Vec<DispatchRun>,
    pub runs_since: Option<String>,
    pub config_changed_at: Option<String>,
}

/// Rechnet die Kennzahlen der Dispatcher-Auswertung.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchStatsCalculator;

impl DispatchStatsCalculator {
    /// Alle Kennzahlen zu diesen Fakten
    pub fn calculate(facts: &DispatchStatsFacts) -> DispatchStats {
        let runs: Vec<&DispatchRun> = facts
            .runs
            .iter()
            .filter(|run| run.kind == facts.kind)
            .collect();
        let done = runs.iter().filter(|run| is_success(run)).count();
        let aborted = runs.iter().filter(|run| is_aborted(run)).count();
        let subjects = subjects(&runs);
        let ladder = ladder(&subjects);
        let solved = subjects
            .values()
            .filter(|list| list.iter().any(|run| is_success(run)))
            .count();
        let abort_reasons = abort_reasons(&runs);
        let configurable: i32 = abort_reasons
            .iter()
            .filter(|reason| {
                CONFIGURABLE
                    .iter()
                    .any(|known| known.as_str() == reason.category)
            })
            .map(|reason| reason.count)
            .sum();
        let escalated = subjects
            .values()
            .filter(|list| list.iter().any(|run| retried(run)))
            .count();

        DispatchStats {
            kind: facts.kind,
            runs_since: facts.runs_since.clone(),
            backfilled_runs: runs
                .iter()
                .filter(|run| {
                    run.source == crate::board::dispatch_run_types::DispatchRunSource::Comments
                })
                .count() as i32,
            config_changed_at: facts.config_changed_at.clone(),
            kpis: DispatchStatsKpis {
                runs: runs.len() as i32,
                subjects: subjects.len() as i32,
                done: done as i32,
                aborted: aborted as i32,
                solved: solved as i32,
                success_rate: rate(solved as i32, ladder.finished),
                first_try_rate: ladder.first_try,
                escalation_rate: rate(escalated as i32, subjects.len() as i32),
                final_failures: runs
                    .iter()
                    .filter(|run| run.outcome == DispatchRunOutcome::AbortFinal)
                    .count() as i32,
            },
            series: series(facts, &runs),
            by_profile: profiles(&runs),
            ladder,
            abort_reasons,
            configurable_abort_share: rate(configurable, aborted as i32),
            usage_pauses: facts
                .runs
                .iter()
                .filter(|run| run.outcome == DispatchRunOutcome::PauseLimit)
                .count() as i32,
            stale_runs: facts
                .runs
                .iter()
                .filter(|run| {
                    run.abort_category.as_deref() == Some(AbortCategory::Vanished.as_str())
                })
                .count() as i32,
        }
    }
}

/// Erfolgreicher Ausgang?
fn is_success(run: &DispatchRun) -> bool {
    SUCCESS.contains(&run.outcome)
}

/// Abgebrochener Ausgang?
fn is_aborted(run: &DispatchRun) -> bool {
    ABORTED.contains(&run.outcome)
}

/// Lauf nach einem früheren Abbruch: eskaliert oder wiederholt
fn retried(run: &DispatchRun) -> bool {
    run.escalated_from.is_some() || run.attempt.unwrap_or(1) > 1
}

/// Läufe je Ticket bzw. Frage, älteste zuerst
fn subjects<'a>(runs: &[&'a DispatchRun]) -> BTreeMap<&'a str, Vec<&'a DispatchRun>> {
    let mut sorted: Vec<&&DispatchRun> = runs.iter().collect();
    sorted.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    let mut map: BTreeMap<&str, Vec<&DispatchRun>> = BTreeMap::new();
    for run in sorted {
        map.entry(run.subject_id.as_str()).or_default().push(run);
    }
    map
}

/// Abgeschlossen ist ein Gegenstand mit Erfolg oder endgültigem Abbruch; Erstversuch = erster
/// Lauf erfolgreich und kein Wiederholungslauf
fn ladder(subjects: &BTreeMap<&str, Vec<&DispatchRun>>) -> DispatchLadder {
    let mut first_try = 0;
    let mut after_escalation = 0;
    let mut final_failures = 0;
    for list in subjects.values() {
        match list.iter().position(|run| is_success(run)) {
            Some(success) => {
                let clean = !list[..=success]
                    .iter()
                    .any(|run| is_aborted(run) || retried(run));
                if clean {
                    first_try += 1;
                } else {
                    after_escalation += 1;
                }
            }
            None if list
                .iter()
                .any(|run| run.outcome == DispatchRunOutcome::AbortFinal) =>
            {
                final_failures += 1;
            }
            None => {}
        }
    }
    let finished = first_try + after_escalation + final_failures;
    DispatchLadder {
        finished,
        first_try: rate(first_try, finished),
        after_escalation: rate(after_escalation, finished),
        final_failures: rate(final_failures, finished),
    }
}

/// Anteil; ohne Grundmenge `None` statt einer Division durch 0
fn rate(part: i32, total: i32) -> Option<f64> {
    (total != 0).then(|| f64::from(part) / f64::from(total))
}

/// Zeitreihe vom Anfang bis zum Ende des Zeitraums, auch leere Abschnitte: bis
/// [`DAILY_MAX_DAYS`] Tage je Tag, sonst je Woche (Montag); Grenzen in Ortszeit
fn series(facts: &DispatchStatsFacts, runs: &[&DispatchRun]) -> DispatchSeries {
    let last = (facts.to - TimeDelta::milliseconds(1))
        .with_timezone(&Local)
        .date_naive();
    let first = facts.from.with_timezone(&Local).date_naive();
    let days = (last - first).num_days() + 1;
    let unit = if days <= DAILY_MAX_DAYS {
        DispatchSeriesUnit::Day
    } else {
        DispatchSeriesUnit::Week
    };
    let step = if unit == DispatchSeriesUnit::Day {
        1
    } else {
        7
    };
    let start_of = |date: NaiveDate| {
        if unit == DispatchSeriesUnit::Day {
            date
        } else {
            monday(date)
        }
    };

    let mut buckets: BTreeMap<String, (i32, i32, i32)> = BTreeMap::new();
    let mut date = start_of(first);
    while date <= last {
        buckets.insert(day(date), (0, 0, 0));
        let Some(next) = date.checked_add_days(Days::new(step)) else {
            break;
        };
        date = next;
    }
    for run in runs {
        let Some(started) = parse_millis(&run.started_at)
            .and_then(DateTime::from_timestamp_millis)
            .map(|at| at.with_timezone(&Local).date_naive())
        else {
            continue;
        };
        let Some(bucket) = buckets.get_mut(&day(start_of(started))) else {
            continue;
        };
        if is_success(run) {
            bucket.0 += 1;
        } else if is_aborted(run) {
            bucket.1 += 1;
        } else if WAITING.contains(&run.outcome) {
            bucket.2 += 1;
        }
    }
    DispatchSeries {
        unit,
        points: buckets
            .into_iter()
            .map(|(start, (done, aborted, other))| DispatchSeriesPoint {
                start,
                done,
                aborted,
                other,
            })
            .collect(),
    }
}

/// Montag der Woche dieses Tages
fn monday(date: NaiveDate) -> NaiveDate {
    date - Days::new(u64::from(date.weekday().num_days_from_monday()))
}

/// JJJJ-MM-TT
fn day(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Ausgänge je Profil, nach Zahl der Läufe, dann Name (die Reihenfolge der Leiter kennt erst
/// die Anzeige)
fn profiles(runs: &[&DispatchRun]) -> Vec<DispatchProfileRow> {
    let mut rows: Vec<DispatchProfileRow> = Vec::new();
    for run in runs {
        let row = match rows.iter_mut().find(|row| row.profile == run.profile) {
            Some(row) => row,
            None => {
                rows.push(DispatchProfileRow {
                    profile: run.profile.clone(),
                    runs: 0,
                    done: 0,
                    escalated: 0,
                    final_failures: 0,
                    other: 0,
                });
                rows.last_mut().expect("gerade angelegt")
            }
        };
        row.runs += 1;
        if is_success(run) {
            row.done += 1;
        } else if run.outcome == DispatchRunOutcome::Abort {
            row.escalated += 1;
        } else if run.outcome == DispatchRunOutcome::AbortFinal {
            row.final_failures += 1;
        } else {
            row.other += 1;
        }
    }
    rows.sort_by(|a, b| b.runs.cmp(&a.runs).then_with(|| a.profile.cmp(&b.profile)));
    rows
}

/// Abbrüche je Kategorie, häufigste zuerst
fn abort_reasons(runs: &[&DispatchRun]) -> Vec<DispatchAbortReason> {
    let mut counts: HashMap<String, i32> = HashMap::new();
    for run in runs.iter().filter(|run| is_aborted(run)) {
        let category = run
            .abort_category
            .clone()
            .unwrap_or_else(|| AbortCategory::Other.as_str().to_string());
        *counts.entry(category).or_insert(0) += 1;
    }
    let mut reasons: Vec<DispatchAbortReason> = counts
        .into_iter()
        .map(|(category, count)| DispatchAbortReason {
            label: AbortReasonClassifier::label(&category),
            category,
            count,
        })
        .collect();
    reasons.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.category.cmp(&b.category))
    });
    reasons
}

#[cfg(test)]
mod tests;
