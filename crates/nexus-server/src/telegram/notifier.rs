//! Telegram als dritter Meldeweg: schickt dieselbe Meldung, die als Systemmeldung und über
//! ntfy hinausgeht, in die gekoppelten Chats – mit denselben Knöpfen wie `/aufgaben`.
//!
//! Die Texte der Knöpfe kommen aus [`crate::telegram::views`] und
//! [`crate::telegram::question_views`]; hier entsteht nur, welcher Anlass welche Knöpfe
//! bekommt, ob der Anlass eingeschaltet ist und ob gerade Nachtruhe ist.

use chrono::{DateTime, Local, Timelike, Utc};
use nexus_core::board::question_types::BoardQuestionOption;
use nexus_core::clock::Clock;
use nexus_core::telegram::{
    bold, escape, QuietHours, TelegramConfig, TelegramConfigStore, TelegramNotify,
};
use nexus_core::HarnessHome;

use crate::dispatch::notifier::{DispatchNotice, DispatchNoticeKind};
use crate::log::ServerLog;
use crate::notifications::model::ToastModel;
use crate::telegram::api::{InlineKeyboard, TelegramApi};
use crate::telegram::bot::{reqwest_factory, TelegramApiFactory};
use crate::telegram::question_views::question_buttons;
use crate::telegram::views::{accept_buttons, blocked_buttons, merge_buttons, rows_of_two};

/// Vorsatz eines Blockierungsgrundes, hinter dem ein gescheiterter Merge steht
pub const MERGE_PREFIX: &str = "Merge:";

/// Vorsatz eines Blockierungsgrundes, hinter dem eine Abnahme durch einen Menschen steht
pub const REVIEW_PREFIX: &str = "Human Interaction: Review";

/// Wozu eine Meldung gehört; daran hängen die Knöpfe und der Schalter in `notify.*`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occasion {
    /// Ein Lauf wartet auf Abnahme
    Review,
    /// Ein Ticket ist blockiert
    Blocked,
    /// Eine Frage wartet auf eine Entscheidung
    Question,
    /// Ein Merge ist gescheitert
    Merge,
    /// Ein Merge ist durch
    Merged,
    /// Der Dispatcher pausiert
    Paused,
}

impl Occasion {
    /// Ob dieser Anlass eingeschaltet ist
    pub fn allowed(self, notify: &TelegramNotify) -> bool {
        match self {
            Occasion::Review => notify.review,
            Occasion::Blocked => notify.blocked,
            Occasion::Question => notify.question,
            Occasion::Merge => notify.merge,
            Occasion::Merged => notify.merged,
            Occasion::Paused => notify.paused,
        }
    }
}

/// Was Telegram über die Meldung hinaus braucht: woran die Knöpfe hängen
#[derive(Debug, Clone, Copy)]
pub enum TelegramNotice<'a> {
    /// Ein Ereignis des Dispatchers
    Dispatch(&'a DispatchNotice),
    /// Eine neue Frage oder eine fertige Recherche samt den Optionen der neuesten Recherche
    Question {
        id: &'a str,
        options: &'a [BoardQuestionOption],
    },
}

impl TelegramNotice<'_> {
    /// Der Anlass dieser Meldung; ein blockiertes Ticket entscheidet sich am Grund
    pub fn occasion(&self) -> Occasion {
        let notice = match self {
            TelegramNotice::Question { .. } => return Occasion::Question,
            TelegramNotice::Dispatch(notice) => notice,
        };
        match notice.kind {
            DispatchNoticeKind::Review => Occasion::Review,
            DispatchNoticeKind::Question => Occasion::Question,
            DispatchNoticeKind::Merged => Occasion::Merged,
            DispatchNoticeKind::Paused => Occasion::Paused,
            // Eine Sackgasse im Autopilot: Schalter und Knöpfe wie bei einem blockierten Ticket
            DispatchNoticeKind::Stuck => Occasion::Blocked,
            DispatchNoticeKind::Blocked => match notice.text.trim() {
                reason if reason.starts_with(MERGE_PREFIX) => Occasion::Merge,
                reason if reason.starts_with(REVIEW_PREFIX) => Occasion::Review,
                _ => Occasion::Blocked,
            },
        }
    }

    /// Die Knöpfe unter der Meldung; ohne Ticket bzw. ohne Knöpfe `None`
    pub fn keyboard(&self) -> Option<InlineKeyboard> {
        match self {
            TelegramNotice::Question { id, options } => Some(question_buttons(id, options)),
            TelegramNotice::Dispatch(notice) => {
                let id = notice.ticket_id.as_deref()?;
                let buttons = match self.occasion() {
                    Occasion::Review => accept_buttons(id),
                    Occasion::Merge => merge_buttons(id),
                    Occasion::Blocked => blocked_buttons(id),
                    // Zu einer Frage, einem Merge und einer Pause gibt es hier nichts zu
                    // entscheiden: die Frage trägt ihre Knöpfe an ihrer eigenen Meldung
                    Occasion::Question | Occasion::Merged | Occasion::Paused => return None,
                };
                Some(rows_of_two(buttons))
            }
        }
    }
}

/// Schickt Meldungen in die gekoppelten Chats des Telegram-Bots.
pub struct TelegramNotifier {
    config: TelegramConfigStore,
    api: TelegramApiFactory,
    clock: Clock,
    log: ServerLog,
}

impl std::fmt::Debug for TelegramNotifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramNotifier").finish_non_exhaustive()
    }
}

impl TelegramNotifier {
    /// Melder auf dem Benutzerordner; ohne Angabe geht es über reqwest an Telegram, in Tests
    /// über die Attrappe
    pub fn new(home: HarnessHome, clock: Clock, api: Option<TelegramApiFactory>) -> Self {
        Self {
            config: TelegramConfigStore::new(home.clone()),
            api: api.unwrap_or_else(reqwest_factory),
            clock,
            log: ServerLog::new(&home),
        }
    }

    /// Ob Telegram überhaupt melden kann: eingeschaltet, mit Token und mit gekoppeltem Chat
    pub fn wanted(&self) -> bool {
        self.read().is_some_and(|config| Self::ready(&config))
    }

    /// Schickt die Meldung an jeden gekoppelten Chat; Fehler landen nur im Protokoll
    pub async fn send(&self, model: &ToastModel, notice: &TelegramNotice<'_>) {
        let Some(config) = self.read() else {
            return;
        };
        if !Self::ready(&config) || !notice.occasion().allowed(&config.notify) {
            return;
        }
        if let Some(hours) = &config.quiet_hours {
            if quiet_now(hours, self.clock.now()) {
                return;
            }
        }
        let api = (self.api)(&config);
        let html = message(model);
        let keyboard = notice.keyboard();
        for chat in &config.chats {
            self.send_one(api.as_ref(), *chat, &html, keyboard.clone())
                .await;
        }
    }

    /// Eine Nachricht an einen Chat; ein Fehler hält die übrigen Chats nicht auf
    async fn send_one(
        &self,
        api: &dyn TelegramApi,
        chat: i64,
        html: &str,
        keyboard: Option<InlineKeyboard>,
    ) {
        if let Err(error) = api.send_message(chat, html, keyboard, None).await {
            self.log
                .write(&format!("Telegram-Meldung fehlgeschlagen: {error}"));
        }
    }

    /// Die Einstellung; ist sie kaputt, meldet Telegram nichts
    fn read(&self) -> Option<TelegramConfig> {
        match self.config.read() {
            Ok(config) => Some(config),
            Err(error) => {
                self.log
                    .write(&format!("Telegram-Einstellung nicht lesbar: {error}"));
                None
            }
        }
    }

    /// Eingeschaltet, mit Token und mit mindestens einem gekoppelten Chat
    fn ready(config: &TelegramConfig) -> bool {
        config.enabled && !config.bot_token.is_empty() && !config.chats.is_empty()
    }
}

/// Die Meldung als Telegram-HTML: Titel fett, darunter die beiden Zeilen
pub fn message(model: &ToastModel) -> String {
    let mut out = bold(&model.title);
    for line in [model.line1.as_str(), model.line2.as_str()] {
        if !line.trim().is_empty() {
            out.push_str(&format!("\n{}", escape(line)));
        }
    }
    out
}

/// Ob `at` in der Nachtruhe liegt; gerechnet wird in Ortszeit
pub fn quiet_now(hours: &QuietHours, at: DateTime<Utc>) -> bool {
    let local = at.with_timezone(&Local);
    let now = local.hour() * 60 + local.minute();
    let (Some(from), Some(to)) = (minutes(&hours.from), minutes(&hours.to)) else {
        return false;
    };
    in_window(now, from, to)
}

/// Ob die Minute des Tages im Fenster von `from` (einschließlich) bis `to` (ausschließlich)
/// liegt; liegt `to` vor `from`, geht das Fenster über Mitternacht
fn in_window(now: u32, from: u32, to: u32) -> bool {
    match from <= to {
        true => now >= from && now < to,
        false => now >= from || now < to,
    }
}

/// „HH:MM“ als Minute des Tages; was nicht passt, ergibt `None`
fn minutes(hhmm: &str) -> Option<u32> {
    let (hours, minutes) = hhmm.split_once(':')?;
    let hours: u32 = hours.parse().ok()?;
    let minutes: u32 = minutes.parse().ok()?;
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::model::ToastKind;

    /// Eine Meldung des Dispatchers dieser Art mit diesem Text
    fn notice(kind: DispatchNoticeKind, text: &str) -> DispatchNotice {
        DispatchNotice {
            kind,
            ticket_id: Some("NXS-0042".into()),
            title: "Login bauen".into(),
            text: text.into(),
            repo_path: None,
        }
    }

    /// Die Beschriftungen der Knöpfe, Zeile für Zeile aneinandergereiht
    fn labels(keyboard: &InlineKeyboard) -> Vec<String> {
        keyboard
            .inline_keyboard
            .iter()
            .flatten()
            .map(|button| button.text.clone())
            .collect()
    }

    #[test]
    fn der_anlass_eines_blockierten_tickets_haengt_am_grund() {
        assert_eq!(
            TelegramNotice::Dispatch(&notice(DispatchNoticeKind::Blocked, "Merge: Konflikt"))
                .occasion(),
            Occasion::Merge
        );
        assert_eq!(
            TelegramNotice::Dispatch(&notice(
                DispatchNoticeKind::Blocked,
                "Human Interaction: Review der Oberfläche"
            ))
            .occasion(),
            Occasion::Review
        );
        assert_eq!(
            TelegramNotice::Dispatch(&notice(DispatchNoticeKind::Blocked, "Zugang fehlt"))
                .occasion(),
            Occasion::Blocked
        );
    }

    #[test]
    fn stuck_zaehlt_wie_blockiert_und_traegt_die_knoepfe_dazu() {
        // Auch ein Grund mit „Merge:“ dahinter bleibt hier ein blockiertes Ticket
        let stuck = notice(
            DispatchNoticeKind::Stuck,
            "Autopilot steckt fest: NXS-0042 – Merge: Konflikt",
        );
        assert_eq!(
            TelegramNotice::Dispatch(&stuck).occasion(),
            Occasion::Blocked
        );
        assert_eq!(
            labels(&TelegramNotice::Dispatch(&stuck).keyboard().expect("Knöpfe")),
            ["Grund", "Kommentar schreiben", "Zurück nach Geplant"]
        );
    }

    #[test]
    fn jeder_anlass_bekommt_seine_knoepfe() {
        let review = notice(DispatchNoticeKind::Review, "wartet auf Freigabe");
        let keyboard = TelegramNotice::Dispatch(&review)
            .keyboard()
            .expect("Knöpfe");
        assert_eq!(
            labels(&keyboard),
            ["Übernehmen", "Zurück an den Agenten", "Geänderte Dateien"]
        );

        let merge = notice(DispatchNoticeKind::Blocked, "Merge: Konflikt");
        assert_eq!(
            labels(&TelegramNotice::Dispatch(&merge).keyboard().expect("Knöpfe")),
            ["Erneut mergen", "Grund"]
        );

        let blocked = notice(DispatchNoticeKind::Blocked, "Zugang fehlt");
        assert_eq!(
            labels(
                &TelegramNotice::Dispatch(&blocked)
                    .keyboard()
                    .expect("Knöpfe")
            ),
            ["Grund", "Kommentar schreiben", "Zurück nach Geplant"]
        );
    }

    #[test]
    fn merged_paused_und_frage_tragen_keine_knoepfe() {
        for kind in [
            DispatchNoticeKind::Merged,
            DispatchNoticeKind::Paused,
            DispatchNoticeKind::Question,
        ] {
            let notice = notice(kind, "egal");
            assert!(
                TelegramNotice::Dispatch(&notice).keyboard().is_none(),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn eine_frage_traegt_die_fragen_knoepfe() {
        let keyboard = TelegramNotice::Question {
            id: "NXS-F0003",
            options: &[],
        }
        .keyboard()
        .expect("Knöpfe");

        assert_eq!(
            labels(&keyboard),
            ["Eigener Text", "Recherche starten", "Verwerfen"]
        );
    }

    #[test]
    fn die_nachricht_nennt_titel_und_zeilen_und_laesst_leere_weg() {
        let model = ToastModel {
            kind: ToastKind::Wartet,
            title: "Dispatcher: blockiert".into(),
            line1: "NXS-0042 Login <bauen>".into(),
            line2: String::new(),
            open_url: "http://127.0.0.1:7777".into(),
        };

        assert_eq!(
            message(&model),
            "<b>Dispatcher: blockiert</b>\nNXS-0042 Login &lt;bauen&gt;"
        );
    }

    #[test]
    fn die_nachtruhe_geht_auch_ueber_mitternacht() {
        // 22:00 bis 07:00: alles dazwischen ist Nachtruhe, der Tag dazwischen nicht
        assert!(in_window(22 * 60, 22 * 60, 7 * 60));
        assert!(in_window(2 * 60, 22 * 60, 7 * 60));
        assert!(!in_window(7 * 60, 22 * 60, 7 * 60));
        assert!(!in_window(12 * 60, 22 * 60, 7 * 60));
        // 08:00 bis 12:00 liegt innerhalb eines Tages
        assert!(in_window(9 * 60, 8 * 60, 12 * 60));
        assert!(!in_window(13 * 60, 8 * 60, 12 * 60));
    }

    #[test]
    fn eine_unlesbare_uhrzeit_haelt_keine_meldung_auf() {
        let hours = QuietHours {
            from: "abends".into(),
            to: "07:00".into(),
        };
        assert!(!quiet_now(&hours, Utc::now()));
        assert_eq!(minutes("24:00"), None);
        assert_eq!(minutes("07:30"), Some(450));
    }
}
