//! Formuliert die Texte der Meldungen (siehe `docs/design/toast/toast-spec.md`); portiert aus
//! `packages/server/src/notifications/ToastTexts.ts`.

use nexus_core::board::question_types::BoardQuestionOption;

use crate::dispatch::notifier::{DispatchNotice, DispatchNoticeKind};
use crate::notifications::attention::{AttentionKind, PendingSession};
use crate::notifications::model::{ToastKind, ToastModel};

/// Länge, ab der eine Zeile mit „…“ gekürzt wird
const MAX_LINE: usize = 80;

/// Ein Ticket in der Meldung
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketRef {
    pub id: String,
    pub title: String,
}

/// Eine Einzelmeldung zu einer Session
#[derive(Debug, Clone)]
pub struct SingleInput<'a> {
    pub kind: AttentionKind,
    pub project_name: &'a str,
    pub ticket: Option<&'a TicketRef>,
    pub session_title: Option<&'a str>,
    pub session_model: Option<&'a str>,
    pub open_url: &'a str,
}

/// Eine Meldung zu einer Frage
#[derive(Debug, Clone)]
pub struct QuestionInput<'a> {
    pub project_name: &'a str,
    pub question_id: &'a str,
    pub question_title: &'a str,
    pub occasion: Option<&'a str>,
    pub research_count: usize,
    pub open_url: &'a str,
}

/// Formuliert die Texte der Meldungen
#[derive(Debug, Clone, Copy, Default)]
pub struct ToastTexts;

impl ToastTexts {
    /// Einzelmeldung: Titel, `Projekt · Ticket`, `Sessiontitel · Modell`
    pub fn single(&self, input: SingleInput<'_>) -> ToastModel {
        let line1 = match input.ticket {
            Some(ticket) => format!("{} · {} {}", input.project_name, ticket.id, ticket.title)
                .trim()
                .to_string(),
            None => input.project_name.to_string(),
        };
        let line2 = join(
            " · ",
            [input.session_title, input.session_model]
                .into_iter()
                .flatten(),
        );
        let line2 = if line2.is_empty() {
            "Zurück zur Session".to_string()
        } else {
            line2
        };
        let (kind, title) = match input.kind {
            AttentionKind::Bereit => (ToastKind::Bereit, "Session ist fertig"),
            AttentionKind::Wartet => (ToastKind::Wartet, "Session braucht deine Antwort"),
        };
        ToastModel {
            kind,
            title: title.to_string(),
            line1,
            line2,
            open_url: input.open_url.to_string(),
        }
    }

    /// Neue Frage eines Agenten: `Projekt · F-Nr Titel`,
    /// `Anlass: … · noch keine Recherche` (oder „Recherche liegt vor“)
    pub fn question_new(&self, input: &QuestionInput<'_>) -> ToastModel {
        let occasion = input.occasion.unwrap_or("nicht angegeben");
        let research = match input.research_count {
            0 => "noch keine Recherche",
            _ => "Recherche liegt vor",
        };
        ToastModel {
            kind: ToastKind::FrageNeu,
            title: "Neue Frage von einer Session".to_string(),
            line1: self.question_line(input),
            line2: shorten(format!("Anlass: {occasion} · {research}")),
            open_url: input.open_url.to_string(),
        }
    }

    /// Neue Recherche eines Agenten: Zahl und Bereich der Optionen, Empfehlung; ohne Optionen
    /// „Kontext ergänzt“
    pub fn research(
        &self,
        input: &QuestionInput<'_>,
        options: &[BoardQuestionOption],
    ) -> ToastModel {
        ToastModel {
            kind: ToastKind::Recherche,
            title: "Recherche ist da".to_string(),
            line1: self.question_line(input),
            line2: Self::options_line(options),
            open_url: input.open_url.to_string(),
        }
    }

    /// Meldung des Dispatchers: Titel je Art, `ID Titel`, Text (gemergt, blockiert, Frage,
    /// Pause, Review)
    pub fn dispatch(&self, notice: &DispatchNotice, open_url: &str) -> ToastModel {
        let title = match notice.kind {
            DispatchNoticeKind::Merged => "Dispatcher: gemergt",
            DispatchNoticeKind::Blocked => "Dispatcher: blockiert",
            DispatchNoticeKind::Question => "Dispatcher: Frage vom Agent",
            DispatchNoticeKind::Paused => "Dispatcher pausiert",
            DispatchNoticeKind::Review => "Dispatcher: wartet auf Review",
        };
        let kind = match notice.kind {
            DispatchNoticeKind::Merged => ToastKind::Bereit,
            _ => ToastKind::Wartet,
        };
        let ticket = notice.ticket_id.as_deref().unwrap_or_default();
        ToastModel {
            kind,
            title: title.to_string(),
            line1: shorten(format!("{ticket} {}", notice.title).trim()),
            line2: shorten(&notice.text),
            open_url: open_url.to_string(),
        }
    }

    /// Sammelmeldung für zwei oder mehr wartende Sessions
    pub fn collected(&self, pending: &[PendingSession], board_url: &str) -> ToastModel {
        let waiting = pending
            .iter()
            .filter(|session| session.kind == AttentionKind::Wartet)
            .count();
        let ready = pending.len() - waiting;
        let mut parts: Vec<String> = Vec::new();
        if waiting > 0 {
            let verb = if waiting == 1 { "braucht" } else { "brauchen" };
            parts.push(format!("{waiting} {verb} eine Antwort"));
        }
        if ready > 0 {
            let verb = if ready == 1 { "ist" } else { "sind" };
            parts.push(format!("{ready} {verb} fertig"));
        }
        let shown: Vec<&str> = pending
            .iter()
            .take(3)
            .map(|session| session.label.as_str())
            .collect();
        let rest = pending.len() - shown.len();
        let liste = shown.join(" · ");
        ToastModel {
            kind: ToastKind::Gesammelt,
            title: format!("{} Sessions warten auf dich", pending.len()),
            line1: parts.join(", "),
            line2: match rest {
                0 => liste,
                _ => format!("{liste} + {rest} weitere"),
            },
            open_url: board_url.to_string(),
        }
    }

    /// `Projekt · F-Nr Titel`, gekürzt
    fn question_line(&self, input: &QuestionInput<'_>) -> String {
        shorten(
            format!(
                "{} · {} {}",
                input.project_name, input.question_id, input.question_title
            )
            .trim(),
        )
    }

    /// `3 Optionen (O3 bis O5), O4 empfohlen`
    fn options_line(options: &[BoardQuestionOption]) -> String {
        let (Some(first), Some(last)) = (options.first(), options.last()) else {
            return "Kontext ergänzt".to_string();
        };
        let count = match options.len() {
            1 => "1 Option".to_string(),
            many => format!("{many} Optionen"),
        };
        let range = match options.len() {
            1 => first.id.clone(),
            _ => format!("{} bis {}", first.id, last.id),
        };
        let recommended = options
            .iter()
            .find(|option| option.recommended)
            .map(|option| format!(", {} empfohlen", option.id))
            .unwrap_or_default();
        format!("{count} ({range}){recommended}")
    }
}

/// Verbindet die nicht leeren Teile mit dem Trenner (wie `filter(Boolean).join(...)`)
fn join<'a>(separator: &str, parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Kürzt lange Zeilen mit „…“ (gezählt wird in Zeichen)
fn shorten(line: impl AsRef<str>) -> String {
    let line = line.as_ref();
    if line.chars().count() <= MAX_LINE {
        return line.to_string();
    }
    let kept: String = line.chars().take(MAX_LINE - 1).collect();
    format!("{kept}…")
}
