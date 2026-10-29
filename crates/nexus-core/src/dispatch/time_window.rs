//! Prüft, ob ein Zeitpunkt (Ortszeit) im Zeitfenster liegt; portiert aus
//! `packages/core/src/dispatch/TimeWindowCheck.ts`. Über Mitternacht erlaubt, ohne Fenster immer.

use chrono::{NaiveTime, Timelike};

use crate::dispatch::config::TimeWindow;

/// Prüft das Startfenster des Dispatchers.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeWindowCheck;

impl TimeWindowCheck {
    /// Neue Prüfung (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// true, wenn neue Worker jetzt starten dürfen
    pub fn contains(&self, now: NaiveTime, window: Option<&TimeWindow>) -> bool {
        let Some(window) = window else {
            return true;
        };
        let t = now.hour() as i64 * 60 + now.minute() as i64;
        let from = minutes(&window.from);
        let to = minutes(&window.to);
        if from == to {
            return true;
        }
        if from < to {
            t >= from && t < to
        } else {
            t >= from || t < to
        }
    }
}

/// Minuten seit Mitternacht aus HH:MM
fn minutes(hhmm: &str) -> i64 {
    let mut parts = hhmm.split(':');
    let hours: i64 = parts.next().unwrap_or_default().parse().unwrap_or(0);
    let minutes: i64 = parts.next().unwrap_or_default().parse().unwrap_or(0);
    hours * 60 + minutes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ortszeit HH:MM
    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    /// Fenster aus zwei Zeiten
    fn window(from: &str, to: &str) -> TimeWindow {
        TimeWindow {
            from: from.to_string(),
            to: to.to_string(),
        }
    }

    #[test]
    fn ohne_fenster_immer() {
        assert!(TimeWindowCheck::new().contains(at(12, 0), None));
    }

    #[test]
    fn am_tag() {
        let check = TimeWindowCheck::new();
        let tag = window("08:00", "17:00");
        assert!(check.contains(at(9, 0), Some(&tag)));
        assert!(!check.contains(at(17, 0), Some(&tag)));
    }

    #[test]
    fn uber_mitternacht() {
        let check = TimeWindowCheck::new();
        let nacht = window("18:00", "07:00");
        assert!(check.contains(at(23, 0), Some(&nacht)));
        assert!(check.contains(at(6, 59), Some(&nacht)));
        assert!(!check.contains(at(12, 0), Some(&nacht)));
    }

    #[test]
    fn gleiches_from_und_to_heisst_immer() {
        assert!(TimeWindowCheck::new().contains(at(3, 0), Some(&window("05:00", "05:00"))));
    }
}
