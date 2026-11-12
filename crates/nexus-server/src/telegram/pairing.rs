//! Einmal-Codes für die Kopplung eines Telegram-Chats mit diesem Nexus.
//!
//! Die Oberfläche lässt sich einen Code geben und zeigt ihn an; wer ihn als
//! `/start <code>` an den Bot schickt, gibt damit seinen Chat frei. Ein Code gilt
//! [`CODE_TTL`] lang und nur ein einziges Mal.

use std::sync::Mutex;

use chrono::{DateTime, TimeDelta, Utc};
use nexus_core::clock::{iso, Clock};
use nexus_core::random;
use serde::{Deserialize, Serialize};

/// Zeichen eines Codes: Ziffern und Großbuchstaben ohne die verwechselbaren
/// `I`, `O`, `0`, `1` – der Code wird abgelesen und abgetippt
pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Länge eines Codes in Zeichen
pub const CODE_LEN: usize = 6;

/// So viele Minuten gilt ein Code
pub const CODE_TTL_MINUTES: i64 = 10;

/// So lange gilt ein Code
pub fn code_ttl() -> TimeDelta {
    TimeDelta::minutes(CODE_TTL_MINUTES)
}

/// Ein ausgegebener Code samt Ende seiner Gültigkeit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCode {
    /// Der Code, wie die Oberfläche ihn zeigt (Großbuchstaben und Ziffern)
    pub code: String,
    /// Zeitpunkt, ab dem der Code nicht mehr zieht (ISO)
    pub expires_at: String,
}

/// Die offenen Einmal-Codes.
///
/// Sie leben nur im Speicher: Nach einem Neustart des Servers ist jeder ausgegebene Code
/// verfallen, und die Oberfläche holt sich einen neuen.
pub struct PairingCodes {
    clock: Clock,
    open: Mutex<Vec<Open>>,
}

/// Ein offener Code
struct Open {
    code: String,
    expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for PairingCodes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairingCodes")
            .field("offen", &self.locked().len())
            .finish_non_exhaustive()
    }
}

impl PairingCodes {
    /// Codes mit der Systemuhr
    pub fn new() -> Self {
        Self::with_clock(Clock::system())
    }

    /// Codes mit eigener Uhr (Tests lassen Codes ablaufen)
    pub fn with_clock(clock: Clock) -> Self {
        Self {
            clock,
            open: Mutex::new(Vec::new()),
        }
    }

    /// Gibt einen neuen Code aus; er gilt [`CODE_TTL_MINUTES`] Minuten lang
    pub fn create(&self) -> PairingCode {
        let now = self.clock.now();
        let expires_at = now + code_ttl();
        let code = self.unused_code();
        let mut open = self.locked();
        open.retain(|entry| entry.expires_at > now);
        open.push(Open {
            code: code.clone(),
            expires_at,
        });
        PairingCode {
            code,
            expires_at: iso(expires_at),
        }
    }

    /// Löst einen Code ein: `true`, wenn er offen und noch gültig war. Danach ist er
    /// verbraucht – ein zweiter Chat kommt mit demselben Code nicht herein
    pub fn redeem(&self, code: &str) -> bool {
        let now = self.clock.now();
        let gesucht = code.trim().to_ascii_uppercase();
        let mut open = self.locked();
        // Abgelaufene fallen beim Einlösen weg; danach zählt nur noch, was gültig ist
        open.retain(|entry| entry.expires_at > now);
        match open.iter().position(|entry| entry.code == gesucht) {
            Some(index) => {
                open.remove(index);
                true
            }
            None => false,
        }
    }

    /// Zahl der offenen, noch gültigen Codes
    pub fn open_count(&self) -> usize {
        let now = self.clock.now();
        self.locked()
            .iter()
            .filter(|entry| entry.expires_at > now)
            .count()
    }

    /// Ein Code, der gerade nicht offen ist
    fn unused_code(&self) -> String {
        // Bei 32^6 Möglichkeiten und einer Handvoll offener Codes trifft das sofort; die
        // Schleife ist nur die Zusicherung, dass kein Code doppelt ausgegeben wird
        loop {
            let code = random_code();
            if !self.locked().iter().any(|entry| entry.code == code) {
                return code;
            }
        }
    }

    /// Die offenen Codes; eine vergiftete Sperre ist ein Fehler im Programm
    fn locked(&self) -> std::sync::MutexGuard<'_, Vec<Open>> {
        self.open.lock().expect("Sperre der Kopplungscodes")
    }
}

impl Default for PairingCodes {
    fn default() -> Self {
        Self::new()
    }
}

/// Ein Code aus [`CODE_LEN`] Zeichen des [`CODE_ALPHABET`], aus der Zufallsquelle des Systems
fn random_code() -> String {
    // 32 Zeichen im Alphabet: fünf Bits je Zeichen, deshalb ist die Restklasse gleichmäßig
    random::bytes(CODE_LEN)
        .into_iter()
        .map(|byte| CODE_ALPHABET[usize::from(byte) % CODE_ALPHABET.len()] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eine Uhr, die bei 12:00 beginnt, samt Hebel, der sie um Minuten vorstellt
    fn verstellbare_uhr() -> (Clock, std::sync::Arc<std::sync::atomic::AtomicI64>) {
        let start: DateTime<Utc> = "2026-11-03T12:00:00Z".parse().unwrap();
        let minuten = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
        let gelesen = minuten.clone();
        let clock = Clock::from_fn(move || {
            start + TimeDelta::minutes(gelesen.load(std::sync::atomic::Ordering::Relaxed))
        });
        (clock, minuten)
    }

    /// Stellt die Uhr aus [`verstellbare_uhr`] um Minuten vor
    fn vorstellen(minuten: &std::sync::atomic::AtomicI64, um: i64) {
        minuten.store(um, std::sync::atomic::Ordering::Relaxed);
    }

    #[test]
    fn gibt_einen_code_aus_dem_alphabet_aus() {
        let codes = PairingCodes::new();

        let code = codes.create().code;

        assert_eq!(code.chars().count(), CODE_LEN);
        assert!(
            code.bytes().all(|byte| CODE_ALPHABET.contains(&byte)),
            "{code}"
        );
    }

    #[test]
    fn nennt_das_ende_der_gultigkeit_zehn_minuten_spater() {
        let (clock, _) = verstellbare_uhr();
        let codes = PairingCodes::with_clock(clock);

        let code = codes.create();

        assert_eq!(code.expires_at, "2026-11-03T12:10:00.000Z");
    }

    #[test]
    fn lost_einen_gultigen_code_genau_einmal_ein() {
        let codes = PairingCodes::new();
        let code = codes.create().code;

        assert!(codes.redeem(&code));
        assert!(!codes.redeem(&code), "ein Code zieht nur einmal");
        assert_eq!(codes.open_count(), 0);
    }

    #[test]
    fn weist_einen_falschen_code_ab() {
        let codes = PairingCodes::new();
        codes.create();

        assert!(!codes.redeem("ABCDEF"));
        assert!(!codes.redeem(""));
        assert_eq!(codes.open_count(), 1, "der echte Code bleibt offen");
    }

    #[test]
    fn weist_einen_abgelaufenen_code_ab() {
        let (clock, minuten) = verstellbare_uhr();
        let codes = PairingCodes::with_clock(clock);
        let code = codes.create().code;

        vorstellen(&minuten, 11);

        assert!(!codes.redeem(&code));
        assert_eq!(codes.open_count(), 0);
    }

    #[test]
    fn liest_den_code_unabhangig_von_gross_und_kleinschreibung() {
        let codes = PairingCodes::new();
        let code = codes.create().code;

        assert!(codes.redeem(&format!("  {}  ", code.to_lowercase())));
    }

    #[test]
    fn gibt_zwei_verschiedene_codes_aus() {
        let codes = PairingCodes::new();

        assert_ne!(codes.create().code, codes.create().code);
        assert_eq!(codes.open_count(), 2);
    }

    #[test]
    fn raumt_abgelaufene_codes_beim_ausgeben_weg() {
        let (clock, minuten) = verstellbare_uhr();
        let codes = PairingCodes::with_clock(clock);
        codes.create();

        vorstellen(&minuten, 11);
        codes.create();

        assert_eq!(codes.open_count(), 1, "nur der neue Code ist noch offen");
    }
}
