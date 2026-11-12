//! Schätzt den erwarteten Verbrauch eines Laufs aus den vergangenen Verbrauchsproben.
//!
//! Zuerst zählen Proben mit gleichem Profil, Modell und Effort, sonst nur die mit gleichem
//! Profil; je Fenster das 75. Perzentil der letzten 20 passenden Verbräuche, ab 5 Proben. Sonst
//! gilt der feste Puffer aus der Einstellung. Reine Logik – die Proben kommen von außen.

use chrono::DateTime;

use crate::dispatch::account_chooser::AccountReserve;
use crate::dispatch::state::UsageSample;

/// Ab so vielen passenden Proben gilt das Perzentil statt des Puffers
pub const MIN_SAMPLES: usize = 5;
/// So viele der jüngsten passenden Proben zählen höchstens
pub const MAX_SAMPLES: usize = 20;

/// Wofür geschätzt wird
#[derive(Debug, Clone, Copy)]
pub struct ReserveFor<'a> {
    pub profile: &'a str,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
}

/// Geschätzte Reserve samt Herkunft für die Begründung
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimatedReserve {
    pub reserve: AccountReserve,
    /// Woher die Reserve stammt, z. B. „aus 7 Läufen profil:klein/sonnet/high“ oder „Puffer“
    pub source: String,
}

/// Reserve eines Laufs aus vergangenen Proben (älteste zuerst); `fallback` = Puffer aus der
/// Einstellung
pub fn estimate_reserve(
    samples: &[UsageSample],
    run: &ReserveFor<'_>,
    fallback: AccountReserve,
) -> EstimatedReserve {
    let same_profile: Vec<&UsageSample> = samples
        .iter()
        .filter(|s| s.profile == run.profile)
        .collect();
    let same_all: Vec<&UsageSample> = same_profile
        .iter()
        .copied()
        .filter(|s| s.model.as_deref() == run.model && s.effort.as_deref() == run.effort)
        .collect();

    let five = estimate_window(&same_all, &same_profile, run, FIVE_HOUR, fallback.five_hour);
    let week = estimate_window(&same_all, &same_profile, run, WEEK, fallback.week);
    EstimatedReserve {
        reserve: AccountReserve {
            five_hour: five.0,
            week: week.0,
        },
        source: five.1,
    }
}

/// Wie der Verbrauch eines Fensters aus einer Probe gelesen wird
type Window = fn(&UsageSample) -> Option<i64>;

const FIVE_HOUR: Window = |s| {
    window_usage(
        s,
        (s.before.five_hour, s.after.five_hour),
        s.before.five_hour_resets_at.as_deref(),
    )
};
const WEEK: Window = |s| {
    window_usage(
        s,
        (s.before.seven_day, s.after.seven_day),
        s.before.seven_day_resets_at.as_deref(),
    )
};

/// Verbrauch einer Probe in einem Fenster; `None`, wenn ein Wert unbekannt oder der Verbrauch
/// negativ ist oder sich das Fenster während des Laufs zurückgesetzt hat (Rücksetzung fehlt,
/// nicht lesbar oder nicht nach dem Ende des Laufs)
fn window_usage(
    sample: &UsageSample,
    (before, after): (Option<i64>, Option<i64>),
    resets_at: Option<&str>,
) -> Option<i64> {
    let used = after? - before?;
    let resets_at = DateTime::parse_from_rfc3339(resets_at?).ok()?;
    let ended_at = DateTime::parse_from_rfc3339(&sample.ended_at).ok()?;
    (used >= 0 && resets_at > ended_at).then_some(used)
}

/// Reserve eines Fensters und die Herkunft: Stufe Profil/Modell/Effort, dann Profil, dann Puffer
fn estimate_window(
    same_all: &[&UsageSample],
    same_profile: &[&UsageSample],
    run: &ReserveFor<'_>,
    window: Window,
    fallback: i64,
) -> (i64, String) {
    let tiers = [
        (
            same_all,
            format!(
                "profil:{}/{}/{}",
                run.profile,
                run.model.unwrap_or("-"),
                run.effort.unwrap_or("-")
            ),
        ),
        (same_profile, format!("Profil {}", run.profile)),
    ];
    for (candidates, what) in tiers {
        let used: Vec<i64> = candidates.iter().filter_map(|s| window(s)).collect();
        let recent = &used[used.len().saturating_sub(MAX_SAMPLES)..];
        if recent.len() >= MIN_SAMPLES {
            return (
                percentile_75(recent),
                format!("aus {} Läufen {what}", recent.len()),
            );
        }
    }
    (fallback, "Puffer".to_string())
}

/// 75. Perzentil nach Nearest-Rank (aufgerundet); `values` darf nicht leer sein
fn percentile_75(values: &[i64]) -> i64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = (sorted.len() * 3).div_ceil(4);
    sorted[rank - 1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::state::UsageSnapshot;

    const ENDED: &str = "2026-11-03T11:00:00.000Z";
    const LATER: &str = "2026-11-03T14:00:00.000Z";

    fn sample(
        profile: &str,
        five: (Option<i64>, Option<i64>),
        week: (Option<i64>, Option<i64>),
    ) -> UsageSample {
        let snapshot = |five_hour, seven_day| UsageSnapshot {
            five_hour,
            seven_day,
            five_hour_resets_at: Some(LATER.to_string()),
            seven_day_resets_at: Some("2026-11-08T00:00:00.000Z".to_string()),
        };
        UsageSample {
            account: "a".to_string(),
            profile: profile.to_string(),
            model: Some("sonnet".to_string()),
            effort: Some("high".to_string()),
            started_at: "2026-11-03T10:00:00.000Z".to_string(),
            ended_at: ENDED.to_string(),
            before: snapshot(five.0, week.0),
            after: snapshot(five.1, week.1),
        }
    }

    /// Probe mit 5-Stunden-Verbrauch `five` und Wochenverbrauch `week`
    fn used(profile: &str, five: i64, week: i64) -> UsageSample {
        sample(
            profile,
            (Some(10), Some(10 + five)),
            (Some(20), Some(20 + week)),
        )
    }

    fn run() -> ReserveFor<'static> {
        ReserveFor {
            profile: "klein",
            model: Some("sonnet"),
            effort: Some("high"),
        }
    }

    const FALLBACK: AccountReserve = AccountReserve {
        five_hour: 10,
        week: 2,
    };

    #[test]
    fn few_samples_give_the_fallback() {
        let samples: Vec<_> = (1..=4).map(|n| used("klein", n, n)).collect();
        let estimate = estimate_reserve(&samples, &run(), FALLBACK);
        assert_eq!(estimate.reserve, FALLBACK);
        assert_eq!(estimate.source, "Puffer");
    }

    #[test]
    fn no_samples_give_the_fallback() {
        let estimate = estimate_reserve(&[], &run(), FALLBACK);
        assert_eq!(estimate.reserve, FALLBACK);
        assert_eq!(estimate.source, "Puffer");
    }

    #[test]
    fn eight_matching_samples_give_the_75th_percentile_per_window() {
        let samples: Vec<_> = (1..=8).map(|n| used("klein", n, 2 * n)).collect();
        let estimate = estimate_reserve(&samples, &run(), FALLBACK);
        assert_eq!(
            estimate.reserve,
            AccountReserve {
                five_hour: 6,
                week: 12
            }
        );
        assert_eq!(estimate.source, "aus 8 Läufen profil:klein/sonnet/high");
    }

    #[test]
    fn falls_back_from_model_and_effort_to_profile() {
        let mut samples: Vec<_> = (1..=6).map(|n| used("klein", n, n)).collect();
        for sample in &mut samples {
            sample.model = Some("opus".to_string());
        }
        samples.extend((1..=2).map(|n| used("klein", n, n)));
        let estimate = estimate_reserve(&samples, &run(), FALLBACK);
        // 8 Proben des Profils: 1..=6 und 1..=2 → sortiert 1,1,2,2,3,4,5,6 → 6. Wert = 4
        assert_eq!(estimate.reserve.five_hour, 4);
        assert_eq!(estimate.source, "aus 8 Läufen Profil klein");
    }

    #[test]
    fn other_profiles_do_not_count() {
        let samples: Vec<_> = (1..=8).map(|n| used("schwer", n, n)).collect();
        let estimate = estimate_reserve(&samples, &run(), FALLBACK);
        assert_eq!(estimate.reserve, FALLBACK);
        assert_eq!(estimate.source, "Puffer");
    }

    #[test]
    fn reset_during_the_run_and_negative_usage_are_ignored() {
        let mut samples: Vec<_> = (1..=5).map(|n| used("klein", n, n)).collect();
        // Rücksetzung des 5-Stunden-Fensters lag vor dem Ende des Laufs
        let mut reset = used("klein", 50, 1);
        reset.before.five_hour_resets_at = Some("2026-11-03T10:30:00.000Z".to_string());
        samples.push(reset);
        // negativer Verbrauch im 5-Stunden-Fenster
        samples.push(sample("klein", (Some(40), Some(30)), (Some(20), Some(21))));
        let estimate = estimate_reserve(&samples, &run(), FALLBACK);
        // 5 h: nur 1..=5 gültig → P75 = 4. Woche: alle 7 gültig → 1,1,1,2,3,4,5 → 6. Wert = 4
        assert_eq!(estimate.reserve.five_hour, 4);
        assert_eq!(estimate.reserve.week, 4);
    }

    #[test]
    fn unknown_or_unparsable_values_are_ignored() {
        let mut samples: Vec<_> = (1..=5).map(|n| used("klein", n, n)).collect();
        samples.push(sample("klein", (None, Some(99)), (Some(1), Some(2))));
        let mut no_reset = used("klein", 90, 1);
        no_reset.before.five_hour_resets_at = None;
        samples.push(no_reset);
        let mut broken = used("klein", 90, 1);
        broken.before.five_hour_resets_at = Some("morgen".to_string());
        samples.push(broken);
        let estimate = estimate_reserve(&samples, &run(), FALLBACK);
        assert_eq!(estimate.reserve.five_hour, 4);
    }

    #[test]
    fn a_window_without_enough_samples_uses_the_fallback_alone() {
        let mut samples: Vec<_> = (1..=8).map(|n| used("klein", n, n)).collect();
        for sample in samples.iter_mut().skip(3) {
            sample.before.seven_day = None;
        }
        let estimate = estimate_reserve(&samples, &run(), FALLBACK);
        assert_eq!(estimate.reserve.five_hour, 6);
        assert_eq!(estimate.reserve.week, FALLBACK.week);
        assert_eq!(estimate.source, "aus 8 Läufen profil:klein/sonnet/high");
    }

    #[test]
    fn only_the_last_twenty_matching_samples_count() {
        // 5 alte Ausreißer, dann 20 Proben mit Verbrauch 1..=20
        let mut samples: Vec<_> = (0..5).map(|_| used("klein", 90, 90)).collect();
        samples.extend((1..=20).map(|n| used("klein", n, n)));
        let estimate = estimate_reserve(&samples, &run(), FALLBACK);
        // P75 von 20 Werten = 15. Wert
        assert_eq!(estimate.reserve.five_hour, 15);
        assert_eq!(estimate.source, "aus 20 Läufen profil:klein/sonnet/high");
    }
}
