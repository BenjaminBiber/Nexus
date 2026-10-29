//! Portiert aus `packages/core/tests/unit/board/DispatchStatsCalculator.test.ts`.

use super::*;
use chrono::TimeZone;

use crate::testing::run_fixture;

/// Ortszeit, damit Wochen und Tage unabhängig von der Zeitzone des Rechners stimmen
fn at(year: i32, month: u32, day: u32, hour: u32) -> String {
    crate::clock::iso(local(year, month, day, hour))
}

/// Ein Zeitpunkt in Ortszeit
fn local(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
    Local
        .with_ymd_and_hms(year, month, day, hour, 0, 0)
        .single()
        .expect("der Zeitpunkt gibt es in jeder Zeitzone")
        .to_utc()
}

/// Ein Lauf mit den Standardwerten der Vorlage
fn run(id: &str, subject_id: &str, outcome: DispatchRunOutcome, started_at: String) -> DispatchRun {
    DispatchRun {
        project_id: 1,
        board_id: Some(5),
        profile: "klein".into(),
        outcome,
        started_at,
        ..run_fixture(id, subject_id, 1)
    }
}

/// Vier Tickets über den Jahreswechsel (Zeitraum 29.12.2025 bis 11.01.2026):
/// A: Zeitlimit in klein, eskaliert nach standard und erledigt
/// B: im ersten Lauf erledigt
/// C: Schrittlimit in klein, Kontext voll in standard (endgültig)
/// D: Frage, dann Pause wegen Nutzungslimit; dazu eine Merge-Sitzung und ein verschwundener
/// Merge-Lauf
fn facts() -> DispatchStatsFacts {
    DispatchStatsFacts {
        from: local(2025, 12, 29, 0),
        to: local(2026, 1, 12, 0),
        kind: DispatchRunKind::Work,
        runs_since: Some(at(2025, 12, 1, 10)),
        config_changed_at: Some(at(2026, 1, 2, 10)),
        runs: vec![
            DispatchRun {
                abort_category: Some("timeout".into()),
                ..run("a1", "A", DispatchRunOutcome::Abort, at(2025, 12, 30, 10))
            },
            DispatchRun {
                attempt: Some(2),
                profile: "standard".into(),
                escalated_from: Some("klein".into()),
                ..run("a2", "A", DispatchRunOutcome::Done, at(2026, 1, 1, 10))
            },
            run("b1", "B", DispatchRunOutcome::Review, at(2026, 1, 5, 10)),
            DispatchRun {
                abort_category: Some("max-turns".into()),
                ..run("c1", "C", DispatchRunOutcome::Abort, at(2026, 1, 5, 10))
            },
            DispatchRun {
                abort_category: Some("context".into()),
                attempt: Some(2),
                profile: "standard".into(),
                escalated_from: Some("klein".into()),
                ..run(
                    "c2",
                    "C",
                    DispatchRunOutcome::AbortFinal,
                    at(2026, 1, 6, 10),
                )
            },
            run("d1", "D", DispatchRunOutcome::Question, at(2026, 1, 6, 10)),
            run(
                "d2",
                "D",
                DispatchRunOutcome::PauseLimit,
                at(2026, 1, 7, 10),
            ),
            DispatchRun {
                kind: DispatchRunKind::Merge,
                ..run(
                    "m1",
                    "A",
                    DispatchRunOutcome::MergeSession,
                    at(2026, 1, 1, 11),
                )
            },
            DispatchRun {
                kind: DispatchRunKind::Merge,
                abort_category: Some("vanished".into()),
                ..run("m2", "B", DispatchRunOutcome::Abort, at(2026, 1, 5, 11))
            },
        ],
    }
}

#[test]
fn rechnet_die_kennzahlen_der_arbeitslaufe_wie_von_hand() {
    let stats = DispatchStatsCalculator::calculate(&facts());
    assert_eq!(stats.kind, DispatchRunKind::Work);
    assert_eq!(stats.config_changed_at, Some(at(2026, 1, 2, 10)));
    assert_eq!(
        stats.kpis,
        DispatchStatsKpis {
            runs: 7,
            subjects: 4,
            done: 2,
            aborted: 3,
            solved: 2,
            // A und B gelöst, C endgültig; D nicht abgeschlossen
            success_rate: Some(2.0 / 3.0),
            // A nach Eskalation, B im ersten Lauf, C endgültig; D nicht abgeschlossen
            first_try_rate: Some(1.0 / 3.0),
            // A und C
            escalation_rate: Some(2.0 / 4.0),
            final_failures: 1,
        }
    );
    assert_eq!(
        stats.ladder,
        DispatchLadder {
            finished: 3,
            first_try: Some(1.0 / 3.0),
            after_escalation: Some(1.0 / 3.0),
            final_failures: Some(1.0 / 3.0),
        }
    );
}

#[test]
fn gruppiert_die_ausgange_je_profil() {
    assert_eq!(
        DispatchStatsCalculator::calculate(&facts()).by_profile,
        [
            DispatchProfileRow {
                profile: "klein".into(),
                runs: 5,
                done: 1,
                escalated: 2,
                final_failures: 0,
                other: 2,
            },
            DispatchProfileRow {
                profile: "standard".into(),
                runs: 2,
                done: 1,
                escalated: 0,
                final_failures: 1,
                other: 0,
            },
        ]
    );
}

#[test]
fn zahlt_abbruchgrunde_und_den_anteil_den_die_einstellung_beeinflusst() {
    let stats = DispatchStatsCalculator::calculate(&facts());
    assert_eq!(
        stats.abort_reasons,
        [
            DispatchAbortReason {
                category: "context".into(),
                label: "Kontext voll".into(),
                count: 1,
            },
            DispatchAbortReason {
                category: "max-turns".into(),
                label: "Schrittlimit".into(),
                count: 1,
            },
            DispatchAbortReason {
                category: "timeout".into(),
                label: "Zeitlimit".into(),
                count: 1,
            },
        ]
    );
    assert_eq!(stats.configurable_abort_share, Some(1.0));
}

#[test]
fn zahlt_nutzungslimit_pausen_und_hangende_laufe_uber_alle_arten() {
    let stats = DispatchStatsCalculator::calculate(&facts());
    assert_eq!(stats.usage_pauses, 1);
    assert_eq!(stats.stale_runs, 1);
}

#[test]
fn wertet_eine_andere_lauf_art_aus_merge_sitzung_zahlt_als_erfolg() {
    let stats = DispatchStatsCalculator::calculate(&DispatchStatsFacts {
        kind: DispatchRunKind::Merge,
        ..facts()
    });
    // B nur abgebrochen, nicht endgültig: zählt nicht zur Erfolgsquote
    assert_eq!(stats.kpis.runs, 2);
    assert_eq!(stats.kpis.subjects, 2);
    assert_eq!(stats.kpis.done, 1);
    assert_eq!(stats.kpis.aborted, 1);
    assert_eq!(stats.kpis.solved, 1);
    assert_eq!(stats.kpis.success_rate, Some(1.0));
    assert_eq!(
        stats.abort_reasons,
        [DispatchAbortReason {
            category: "vanished".into(),
            label: "Worker verschwunden".into(),
            count: 1,
        }]
    );
    assert_eq!(stats.configurable_abort_share, Some(0.0));
}

#[test]
fn zahlt_bis_28_tage_je_tag_in_ortszeit_mit_fragen_und_blockiert_als_rest() {
    let stats = DispatchStatsCalculator::calculate(&facts());
    assert_eq!(stats.series.unit, DispatchSeriesUnit::Day);
    assert_eq!(stats.series.points.len(), 14);
    assert_eq!(stats.series.points[0].start, "2025-12-29");
    assert_eq!(
        stats
            .series
            .points
            .iter()
            .filter(|point| point.done > 0 || point.aborted > 0 || point.other > 0)
            .cloned()
            .collect::<Vec<_>>(),
        [
            DispatchSeriesPoint {
                start: "2025-12-30".into(),
                done: 0,
                aborted: 1,
                other: 0,
            },
            DispatchSeriesPoint {
                start: "2026-01-01".into(),
                done: 1,
                aborted: 0,
                other: 0,
            },
            DispatchSeriesPoint {
                start: "2026-01-05".into(),
                done: 1,
                aborted: 1,
                other: 0,
            },
            DispatchSeriesPoint {
                start: "2026-01-06".into(),
                done: 0,
                aborted: 1,
                other: 1,
            },
        ]
    );
    // Genau 28 Tage noch je Tag, 29 je Woche
    assert_eq!(
        DispatchStatsCalculator::calculate(&DispatchStatsFacts {
            to: local(2026, 1, 26, 0),
            ..facts()
        })
        .series
        .unit,
        DispatchSeriesUnit::Day
    );
    assert_eq!(
        DispatchStatsCalculator::calculate(&DispatchStatsFacts {
            to: local(2026, 1, 27, 0),
            ..facts()
        })
        .series
        .unit,
        DispatchSeriesUnit::Week
    );
}

#[test]
fn zahlt_uber_28_tage_je_iso_woche_montag_in_ortszeit_uber_den_jahreswechsel() {
    let stats = DispatchStatsCalculator::calculate(&DispatchStatsFacts {
        from: local(2025, 12, 1, 0),
        ..facts()
    });
    assert_eq!(stats.series.unit, DispatchSeriesUnit::Week);
    assert_eq!(
        stats
            .series
            .points
            .iter()
            .map(|point| point.start.as_str())
            .collect::<Vec<_>>(),
        [
            "2025-12-01",
            "2025-12-08",
            "2025-12-15",
            "2025-12-22",
            "2025-12-29",
            "2026-01-05",
        ]
    );
    assert_eq!(
        stats.series.points[stats.series.points.len() - 2..],
        [
            DispatchSeriesPoint {
                start: "2025-12-29".into(),
                done: 1,
                aborted: 1,
                other: 0,
            },
            DispatchSeriesPoint {
                start: "2026-01-05".into(),
                done: 1,
                aborted: 2,
                other: 1,
            },
        ]
    );
}

#[test]
fn liefert_fur_einen_leeren_zeitraum_null_statt_einer_division_durch_0() {
    let stats = DispatchStatsCalculator::calculate(&DispatchStatsFacts {
        runs: Vec::new(),
        runs_since: None,
        config_changed_at: None,
        ..facts()
    });
    assert_eq!(
        stats.kpis,
        DispatchStatsKpis {
            runs: 0,
            subjects: 0,
            done: 0,
            aborted: 0,
            solved: 0,
            success_rate: None,
            first_try_rate: None,
            escalation_rate: None,
            final_failures: 0,
        }
    );
    assert_eq!(
        stats.ladder,
        DispatchLadder {
            finished: 0,
            first_try: None,
            after_escalation: None,
            final_failures: None,
        }
    );
    assert_eq!(stats.configurable_abort_share, None);
    assert_eq!(
        stats
            .series
            .points
            .iter()
            .map(|point| point.done)
            .collect::<Vec<_>>(),
        [0; 14]
    );
    assert!(stats.by_profile.is_empty());
}

#[test]
fn zahlt_einen_wiederholungslauf_ohne_eskalation_nicht_als_erstversuch() {
    let stats = DispatchStatsCalculator::calculate(&DispatchStatsFacts {
        runs: vec![DispatchRun {
            attempt: Some(2),
            escalated_from: None,
            ..run("x2", "X", DispatchRunOutcome::Done, at(2026, 1, 6, 10))
        }],
        ..facts()
    });
    assert_eq!(stats.kpis.first_try_rate, Some(0.0));
    assert_eq!(stats.kpis.escalation_rate, Some(1.0));
}
