//! Der Zeitraum von `GET /api/db/stats/dispatch`: Kalendertage in Ortszeit.
//!
//! Die Rechnung steckt hier und nicht im Handler, damit sie ohne Server und ohne Datenbank
//! mit festen Tagen geprüft werden kann. In TypeScript (`DbStatsRoutes.ts`) rechnet sie mit
//! `Date` in Ortszeit; mit [`NaiveDate`] zählen die Tage als Kalendertage, so dass eine
//! Zeitumstellung im Zeitraum nichts verschiebt.

use chrono::{Duration, NaiveDate};
use nexus_core::board::dispatch_run_types::DispatchRunKind;
use nexus_core::{HarnessError, Result};

/// Standard-Zeitraum: die letzten 8 Wochen (56 Tage inkl. heute)
pub const DEFAULT_DAYS: i64 = 56;
/// Längster erlaubter Zeitraum in Tagen
pub const MAX_DAYS: i64 = 366;

/// Die Abfrage-Werte des Zeitraums, so wie sie in der Adresse stehen
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatsQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub kind: Option<String>,
}

impl StatsQuery {
    /// Steht `from=config` in der Adresse? Nur dann wird die Fassung der Einstellung geholt
    pub fn wants_config(&self) -> bool {
        self.from.as_deref().map(str::trim) == Some("config")
    }
}

/// Der gerechnete Zeitraum; `to` ist einschließlich, die Abfrage selbst endet erst mit
/// Mitternacht danach
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub kind: DispatchRunKind,
}

impl StatsRange {
    /// Rechnet den Zeitraum aus: `to` (Standard heute), `from` (Standard 56 Tage, `config`
    /// ab dem Tag der neuesten Fassung) und die Lauf-Art.
    ///
    /// `config_changed` ist der Ortszeit-Tag der neuesten Fassung der Dispatcher-Einstellung,
    /// falls `from=config` angefragt wurde und es eine Fassung gibt.
    pub fn resolve(
        query: &StatsQuery,
        today: NaiveDate,
        config_changed: Option<NaiveDate>,
    ) -> Result<StatsRange> {
        let to = match text(&query.to) {
            Some(raw) => day(raw, "to")?,
            None => today,
        };
        let fallback = to - Duration::days(DEFAULT_DAYS - 1);
        let earliest = to - Duration::days(MAX_DAYS - 1);
        let from = match (config_changed, text(&query.from)) {
            // Der Tag der Fassung, aber nie weiter zurück als die Höchstdauer
            (Some(changed), _) => changed.max(earliest),
            (None, Some(raw)) if raw.trim() != "config" => day(raw, "from")?,
            _ => fallback,
        };
        if from > to {
            return Err(invalid("„from“ liegt nach „to“."));
        }
        let days = (to - from).num_days() + 1;
        if days > MAX_DAYS {
            return Err(invalid(format!(
                "Zeitraum ist zu lang ({days} Tage, höchstens {MAX_DAYS})."
            )));
        }
        Ok(StatsRange {
            from,
            to,
            kind: kind(text(&query.kind))?,
        })
    }

    /// Der erste Tag nach dem Zeitraum; die Abfrage läuft bis zu seiner Mitternacht
    pub fn end(&self) -> NaiveDate {
        self.to + Duration::days(1)
    }
}

/// Ein Tag als `JJJJ-MM-TT`, wie ihn die Antwort schreibt
pub fn text(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|raw| !raw.is_empty())
}

/// Ein Tag als Text der Antwort
pub fn day_text(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

/// `JJJJ-MM-TT` als Kalendertag; alles andere (auch der 30. Februar) wird abgelehnt
fn day(raw: &str, key: &str) -> Result<NaiveDate> {
    let trimmed = raw.trim();
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .ok()
        // Rundlauf wie in TypeScript: nur was genauso wieder herauskommt, ist ein Tag
        .filter(|day| day_text(*day) == trimmed)
        .ok_or_else(|| invalid(format!("„{key}“ muss ein Datum JJJJ-MM-TT sein.")))
}

/// Die Lauf-Art; ohne Angabe `work`
fn kind(raw: Option<&str>) -> Result<DispatchRunKind> {
    let wanted = raw.map(str::trim).filter(|kind| !kind.is_empty());
    let Some(wanted) = wanted else {
        return Ok(DispatchRunKind::Work);
    };
    DispatchRunKind::ALL
        .into_iter()
        .find(|kind| kind.as_str() == wanted)
        .ok_or_else(|| {
            let all = DispatchRunKind::ALL
                .map(|kind| kind.as_str().to_string())
                .join(", ");
            invalid(format!("„kind“ muss {all} sein."))
        })
}

/// Fehler einer ungültigen Eingabe
fn invalid(message: impl Into<String>) -> nexus_core::Error {
    HarnessError::new("input-invalid", message).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein fester Tag
    fn tag(text: &str) -> NaiveDate {
        text.parse().expect("Tag")
    }

    /// Die Abfrage aus `from`, `to` und `kind`
    fn query(from: Option<&str>, to: Option<&str>, kind: Option<&str>) -> StatsQuery {
        StatsQuery {
            from: from.map(str::to_string),
            to: to.map(str::to_string),
            kind: kind.map(str::to_string),
        }
    }

    /// Die Meldung eines Fehlers
    fn error(result: Result<StatsRange>) -> String {
        result
            .unwrap_err()
            .harness()
            .expect("erwartbar")
            .message
            .clone()
    }

    #[test]
    fn ohne_angabe_sind_es_die_letzten_56_tage_bis_heute() {
        let range = StatsRange::resolve(&query(None, None, None), tag("2026-03-10"), None)
            .expect("Zeitraum");
        assert_eq!(range.to, tag("2026-03-10"));
        assert_eq!(range.from, tag("2026-01-14"));
        assert_eq!((range.to - range.from).num_days() + 1, DEFAULT_DAYS);
        assert_eq!(range.kind, DispatchRunKind::Work);
        assert_eq!(range.end(), tag("2026-03-11"));
    }

    #[test]
    fn eine_zeitumstellung_im_zeitraum_verschiebt_die_tage_nicht() {
        // In Europa liegt die Umstellung Ende März im Standard-Zeitraum; gezählt wird in
        // Kalendertagen, nicht in 86 400 Sekunden
        let range = StatsRange::resolve(&query(None, None, None), tag("2026-04-10"), None)
            .expect("Zeitraum");
        assert_eq!(range.from, tag("2026-02-14"));
    }

    #[test]
    fn from_config_beginnt_am_tag_der_fassung_hochstens_366_tage_zuruck() {
        let heute = tag("2026-03-10");
        let seit = StatsRange::resolve(
            &query(Some("config"), None, None),
            heute,
            Some(tag("2026-03-07")),
        )
        .expect("Zeitraum");
        assert_eq!(seit.from, tag("2026-03-07"));

        // Eine sehr alte Fassung wird auf die Höchstdauer gekürzt
        let alt = StatsRange::resolve(
            &query(Some("config"), None, None),
            heute,
            Some(tag("2020-01-01")),
        )
        .expect("Zeitraum");
        assert_eq!(alt.from, tag("2025-03-10"));
        assert_eq!((alt.to - alt.from).num_days() + 1, MAX_DAYS);

        // Ohne Fassung gilt der Standard-Zeitraum
        let ohne =
            StatsRange::resolve(&query(Some("config"), None, None), heute, None).expect("Zeitraum");
        assert_eq!(ohne.from, tag("2026-01-14"));
    }

    #[test]
    fn prufft_monatsgrenzen_und_die_schreibweise_der_tage() {
        let ende = StatsRange::resolve(
            &query(Some("2026-02-28"), Some("2026-03-01"), None),
            tag("2026-03-10"),
            None,
        )
        .expect("Zeitraum");
        assert_eq!(day_text(ende.from), "2026-02-28");
        assert_eq!(day_text(ende.to), "2026-03-01");

        let schalttag = StatsRange::resolve(
            &query(Some("2024-02-29"), Some("2024-03-01"), None),
            tag("2024-03-10"),
            None,
        )
        .expect("Zeitraum");
        assert_eq!(day_text(schalttag.from), "2024-02-29");

        for raw in ["2026-02-30", "2026-13-01", "2026-1-01", "heute", ""] {
            let result =
                StatsRange::resolve(&query(None, Some(raw), None), tag("2026-03-10"), None);
            match raw {
                // Ein leerer Wert zählt wie „nicht angegeben“
                "" => assert_eq!(result.expect("Zeitraum").to, tag("2026-03-10")),
                _ => assert_eq!(error(result), "„to“ muss ein Datum JJJJ-MM-TT sein."),
            }
        }
    }

    #[test]
    fn lehnt_verdrehte_und_zu_lange_zeitraume_ab() {
        let heute = tag("2026-03-10");
        assert_eq!(
            error(StatsRange::resolve(
                &query(Some("2026-02-01"), Some("2026-01-01"), None),
                heute,
                None
            )),
            "„from“ liegt nach „to“."
        );
        assert_eq!(
            error(StatsRange::resolve(
                &query(Some("2025-01-01"), Some("2026-01-02"), None),
                heute,
                None
            )),
            "Zeitraum ist zu lang (367 Tage, höchstens 366)."
        );
        // Genau die Höchstdauer geht noch
        assert!(StatsRange::resolve(
            &query(Some("2025-01-02"), Some("2026-01-02"), None),
            heute,
            None
        )
        .is_ok());
    }

    #[test]
    fn liest_die_lauf_art() {
        let heute = tag("2026-03-10");
        for (raw, expected) in [
            ("work", DispatchRunKind::Work),
            ("merge", DispatchRunKind::Merge),
            ("research", DispatchRunKind::Research),
        ] {
            let range =
                StatsRange::resolve(&query(None, None, Some(raw)), heute, None).expect("Zeitraum");
            assert_eq!(range.kind, expected);
        }
        assert_eq!(
            StatsRange::resolve(&query(None, None, Some(" ")), heute, None)
                .expect("Zeitraum")
                .kind,
            DispatchRunKind::Work
        );
        assert_eq!(
            error(StatsRange::resolve(
                &query(None, None, Some("plan")),
                heute,
                None
            )),
            "„kind“ muss work, merge, research sein."
        );
    }

    #[test]
    fn erkennt_die_anfrage_nach_der_konfig_fassung() {
        assert!(StatsQuery {
            from: Some(" config ".into()),
            ..StatsQuery::default()
        }
        .wants_config());
        assert!(!StatsQuery::default().wants_config());
        assert!(!StatsQuery {
            from: Some("2026-01-01".into()),
            ..StatsQuery::default()
        }
        .wants_config());
    }
}
