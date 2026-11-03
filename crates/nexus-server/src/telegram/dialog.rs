//! Offene Dialoge je Chat: ein Knopf fragt nach einem Text, die nächste Nachricht ist die
//! Antwort.
//!
//! Je Chat wartet höchstens ein Dialog; ein neuer verdrängt den alten. Wer nach
//! [`DIALOG_TIMEOUT_MINUTES`] antwortet, bekommt den Hinweis, dass der Dialog abgelaufen
//! ist – der Text löst dann nichts mehr aus. `/abbrechen` beendet ihn von Hand.
//!
//! Die Antwort kommt als [`DialogInput`] herein, damit ein späteres Ticket ein
//! Sprach-Transkript an dieselbe Stelle geben kann.

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, TimeDelta, Utc};
use nexus_core::clock::Clock;

/// So lange wartet ein Dialog auf die Antwort
pub const DIALOG_TIMEOUT_MINUTES: i64 = 15;

/// Worauf ein offener Dialog wartet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    /// Rückmeldung für „Zurück an den Agenten“ bzw. „Zurück nach Geplant“
    ReturnFeedback,
    /// Kommentar an ein Ticket
    Comment,
    /// Eigener Text als Entscheidung einer Frage
    QuestionText,
    /// Begründung, mit der eine Frage verworfen wird (Pflicht)
    QuestionDiscard,
}

/// Die Antwort auf einen Dialog. Eine Sprachnachricht kommt später als Transkript
/// ebenfalls hier an
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogInput {
    Text(String),
}

impl DialogInput {
    /// Die Antwort als Text
    pub fn text(&self) -> &str {
        match self {
            DialogInput::Text(text) => text,
        }
    }
}

/// Ein offener Dialog eines Chats
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    pub kind: DialogKind,
    /// Ticket oder Frage, um das bzw. die es geht
    pub target: String,
    /// Seit wann der Dialog wartet
    pub since: DateTime<Utc>,
}

/// Was eine Nachricht in einem Chat vorfindet
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogAnswer {
    /// Kein Dialog offen; die Nachricht gehört jemand anderem
    None,
    /// Der Dialog hat zu lange gewartet und ist verworfen
    Expired(Dialog),
    /// Der Dialog wartete auf diese Nachricht und ist damit abgeschlossen
    Open(Dialog),
}

/// Höchstens ein offener Dialog je Chat.
pub struct Dialogs {
    clock: Clock,
    chats: Mutex<HashMap<i64, Dialog>>,
}

impl std::fmt::Debug for Dialogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dialogs")
            .field("offen", &self.chats.lock().map(|chats| chats.len()).ok())
            .finish_non_exhaustive()
    }
}

impl Dialogs {
    /// Dialoge mit dieser Uhr (Tests lassen die Zeit ablaufen)
    pub fn with_clock(clock: Clock) -> Self {
        Self {
            clock,
            chats: Mutex::new(HashMap::new()),
        }
    }

    /// Öffnet einen Dialog; ein noch offener desselben Chats wird verdrängt und
    /// zurückgegeben
    pub fn open(&self, chat_id: i64, kind: DialogKind, target: &str) -> Option<Dialog> {
        let dialog = Dialog {
            kind,
            target: target.to_string(),
            since: self.clock.now(),
        };
        let replaced = self.chats().insert(chat_id, dialog);
        replaced.filter(|dialog| !self.expired(dialog))
    }

    /// Nimmt die Antwort ab: der Dialog ist danach in jedem Fall zu
    pub fn take(&self, chat_id: i64) -> DialogAnswer {
        match self.chats().remove(&chat_id) {
            None => DialogAnswer::None,
            Some(dialog) if self.expired(&dialog) => DialogAnswer::Expired(dialog),
            Some(dialog) => DialogAnswer::Open(dialog),
        }
    }

    /// Beendet den offenen Dialog (`/abbrechen`, „Abbrechen“); `None`, wenn keiner wartete
    /// oder er schon abgelaufen war
    pub fn cancel(&self, chat_id: i64) -> Option<Dialog> {
        self.chats()
            .remove(&chat_id)
            .filter(|dialog| !self.expired(dialog))
    }

    /// Wartet der Dialog zu lange?
    fn expired(&self, dialog: &Dialog) -> bool {
        self.clock.now() - dialog.since >= TimeDelta::minutes(DIALOG_TIMEOUT_MINUTES)
    }

    /// Die offenen Dialoge; eine vergiftete Sperre wäre ein Fehler im Server
    fn chats(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Dialog>> {
        self.chats.lock().expect("Sperre der offenen Dialoge")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;

    use super::*;

    /// Beginn aller Tests
    const START: &str = "2026-11-03T12:00:00Z";

    /// Dialoge mit einer Uhr, die der Test in Minuten vorstellt
    fn dialogs() -> (Dialogs, Arc<AtomicI64>) {
        let minutes = Arc::new(AtomicI64::new(0));
        let read = minutes.clone();
        let start: DateTime<Utc> = START.parse().expect("Zeitpunkt");
        let clock =
            Clock::from_fn(move || start + TimeDelta::minutes(read.load(Ordering::Relaxed)));
        (Dialogs::with_clock(clock), minutes)
    }

    #[test]
    fn die_naechste_nachricht_beantwortet_den_offenen_dialog() {
        let (dialogs, _) = dialogs();

        assert_eq!(dialogs.open(42, DialogKind::Comment, "NXS-0042"), None);

        assert_eq!(
            dialogs.take(42),
            DialogAnswer::Open(Dialog {
                kind: DialogKind::Comment,
                target: "NXS-0042".into(),
                since: START.parse().expect("Zeitpunkt"),
            })
        );
        assert_eq!(
            dialogs.take(42),
            DialogAnswer::None,
            "die Antwort schließt den Dialog"
        );
    }

    #[test]
    fn jeder_chat_hat_seinen_eigenen_dialog() {
        let (dialogs, _) = dialogs();

        dialogs.open(42, DialogKind::Comment, "NXS-0042");
        dialogs.open(99, DialogKind::ReturnFeedback, "NXS-0007");

        let DialogAnswer::Open(zweiter) = dialogs.take(99) else {
            panic!("der zweite Chat wartet");
        };
        assert_eq!(zweiter.target, "NXS-0007");
        let DialogAnswer::Open(erster) = dialogs.take(42) else {
            panic!("der erste Chat wartet weiter");
        };
        assert_eq!(erster.target, "NXS-0042");
    }

    #[test]
    fn ein_neuer_dialog_verdraengt_den_alten() {
        let (dialogs, _) = dialogs();

        dialogs.open(42, DialogKind::Comment, "NXS-0042");
        let verdraengt = dialogs.open(42, DialogKind::ReturnFeedback, "NXS-0007");

        assert_eq!(
            verdraengt.map(|dialog| dialog.target),
            Some("NXS-0042".into())
        );
        let DialogAnswer::Open(dialog) = dialogs.take(42) else {
            panic!("der neue Dialog wartet");
        };
        assert_eq!(dialog.kind, DialogKind::ReturnFeedback);
    }

    #[test]
    fn nach_fuenfzehn_minuten_ist_der_dialog_abgelaufen() {
        let (dialogs, minuten) = dialogs();
        dialogs.open(42, DialogKind::Comment, "NXS-0042");

        minuten.store(DIALOG_TIMEOUT_MINUTES - 1, Ordering::Relaxed);
        assert!(matches!(dialogs.take(42), DialogAnswer::Open(_)));

        dialogs.open(42, DialogKind::Comment, "NXS-0042");
        minuten.store(2 * DIALOG_TIMEOUT_MINUTES, Ordering::Relaxed);
        assert!(matches!(dialogs.take(42), DialogAnswer::Expired(_)));
        assert_eq!(
            dialogs.take(42),
            DialogAnswer::None,
            "auch der abgelaufene Dialog ist danach weg"
        );
    }

    #[test]
    fn abbrechen_beendet_nur_einen_wartenden_dialog() {
        let (dialogs, minuten) = dialogs();

        assert_eq!(dialogs.cancel(42), None);

        dialogs.open(42, DialogKind::Comment, "NXS-0042");
        assert_eq!(
            dialogs.cancel(42).map(|dialog| dialog.target),
            Some("NXS-0042".into())
        );
        assert_eq!(dialogs.take(42), DialogAnswer::None);

        dialogs.open(42, DialogKind::Comment, "NXS-0042");
        minuten.store(DIALOG_TIMEOUT_MINUTES, Ordering::Relaxed);
        assert_eq!(dialogs.cancel(42), None, "der abgelaufene zählt nicht mehr");
    }
}
