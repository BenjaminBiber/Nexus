//! Was die Spracherkennung zuletzt getan hat: die letzte erfolgreiche Erkennung und der letzte
//! Fehler, ob aus einer Sprachnachricht oder vom Test der Einstellungsseite.
//!
//! Die Einstellungsseite zeigt daraus „Zuletzt erkannt vor 20 min“. Der Stand lebt nur im
//! Speicher: nach einem Neustart weiß der Server noch nichts.

use std::sync::Mutex;
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use nexus_core::telegram::{SpeechBackend, SpeechStatusDto};

/// Woher eine Erkennung kam
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechVia {
    /// Eine Sprach- oder Audionachricht im Bot
    Voice,
    /// Der Knopf „Testen“ der Einstellungsseite
    Test,
}

impl SpeechVia {
    fn as_str(self) -> &'static str {
        match self {
            SpeechVia::Voice => "voice",
            SpeechVia::Test => "test",
        }
    }
}

/// Der zuletzt gemerkte Stand der Spracherkennung
#[derive(Debug, Default)]
pub struct SpeechActivity {
    last: Mutex<SpeechStatusDto>,
}

impl SpeechActivity {
    /// Ohne Erkennung und ohne Fehler
    pub fn new() -> Self {
        Self::default()
    }

    /// Merkt sich das Ergebnis eines Aufrufs der Erkennung. Eine abgeschaltete Erkennung zählt
    /// nicht: „ist ausgeschaltet“ ist kein Fehler des Dienstes.
    pub fn record<T>(
        &self,
        backend: SpeechBackend,
        via: SpeechVia,
        took: Duration,
        result: &nexus_core::Result<T>,
    ) {
        if backend == SpeechBackend::Off {
            return;
        }
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match result {
            Ok(_) => {
                last.last_success_at = Some(now);
                last.last_success_via = Some(via.as_str().to_string());
                last.last_millis = Some(took.as_millis() as u64);
            }
            Err(error) => {
                last.last_error_at = Some(now);
                last.last_error = Some(error.to_string());
            }
        }
    }

    /// Der Stand für `GET /api/speech/status`
    pub fn snapshot(&self) -> SpeechStatusDto {
        self.last
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::HarnessError;

    #[test]
    fn kennt_anfangs_weder_erfolg_noch_fehler() {
        assert_eq!(SpeechActivity::new().snapshot(), SpeechStatusDto::default());
    }

    #[test]
    fn merkt_sich_erfolg_und_fehler_getrennt() {
        let activity = SpeechActivity::new();
        let ok: nexus_core::Result<()> = Ok(());
        activity.record(
            SpeechBackend::OpenaiCompatible,
            SpeechVia::Voice,
            Duration::from_millis(420),
            &ok,
        );
        let failed: nexus_core::Result<()> = Err(HarnessError::new(
            "speech-failed",
            "Die Spracherkennung antwortete mit 401.",
        )
        .into());
        activity.record(
            SpeechBackend::OpenaiCompatible,
            SpeechVia::Test,
            Duration::ZERO,
            &failed,
        );

        let status = activity.snapshot();
        assert!(status.last_success_at.is_some());
        assert_eq!(status.last_success_via.as_deref(), Some("voice"));
        assert_eq!(status.last_millis, Some(420));
        assert!(status.last_error_at.is_some());
        assert_eq!(
            status.last_error.as_deref(),
            Some("Die Spracherkennung antwortete mit 401.")
        );
    }

    #[test]
    fn uebergeht_eine_abgeschaltete_erkennung() {
        let activity = SpeechActivity::new();
        let off: nexus_core::Result<()> =
            Err(HarnessError::new("speech-off", "Spracherkennung ist ausgeschaltet.").into());
        activity.record(SpeechBackend::Off, SpeechVia::Test, Duration::ZERO, &off);
        assert_eq!(activity.snapshot(), SpeechStatusDto::default());
    }
}
