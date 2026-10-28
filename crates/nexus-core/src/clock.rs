//! Die Uhr, die den Diensten hineingereicht wird. In TypeScript ist das der Parameter
//! `clock: () => Date`; hier derselbe Gedanke als teilbarer Funktionszeiger, damit Tests
//! feste Zeiten setzen und vorstellen können.

use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, SecondsFormat, Utc};

/// Liefert die aktuelle Zeit; geteilt (`Arc`), weil mehrere Dienste dieselbe Uhr benutzen
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>);

impl Clock {
    /// Die Uhr des Rechners
    pub fn system() -> Self {
        Self::from_fn(Utc::now)
    }

    /// Eine Uhr, die immer denselben Zeitpunkt liefert
    pub fn fixed(at: DateTime<Utc>) -> Self {
        Self::from_fn(move || at)
    }

    /// Eine Uhr aus einer eigenen Funktion (für Tests mit verstellbarer Zeit)
    pub fn from_fn(now: impl Fn() -> DateTime<Utc> + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }

    /// Der aktuelle Zeitpunkt
    pub fn now(&self) -> DateTime<Utc> {
        (self.0)()
    }

    /// Der aktuelle Zeitpunkt als ISO-Text mit Millisekunden und `Z`, wortgleich mit
    /// `new Date().toISOString()` in JavaScript – so steht er in allen Spalten des Boards
    pub fn iso(&self) -> String {
        iso(self.now())
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::system()
    }
}

impl fmt::Debug for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Clock")
    }
}

/// Ein Zeitpunkt als ISO-Text, wie `Date.toISOString()` ihn schreibt
pub fn iso(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn schreibt_den_zeitpunkt_wie_tojsonstring_in_javascript() {
        let at: DateTime<Utc> = "2026-10-13T10:00:00.000Z".parse().unwrap();
        assert_eq!(Clock::fixed(at).iso(), "2026-10-13T10:00:00.000Z");
    }

    #[test]
    fn eine_eigene_funktion_lasst_die_zeit_vorstellen() {
        let now: Arc<Mutex<DateTime<Utc>>> =
            Arc::new(Mutex::new("2026-10-13T10:00:00.000Z".parse().unwrap()));
        let clock = Clock::from_fn({
            let now = Arc::clone(&now);
            move || *now.lock().unwrap()
        });

        assert_eq!(clock.iso(), "2026-10-13T10:00:00.000Z");
        *now.lock().unwrap() = "2026-10-13T10:05:00.000Z".parse().unwrap();
        assert_eq!(clock.iso(), "2026-10-13T10:05:00.000Z");
    }

    #[test]
    fn die_uhr_des_rechners_lauft_vorwarts() {
        let clock = Clock::system();
        assert!(clock.now() <= Clock::default().now());
    }
}
