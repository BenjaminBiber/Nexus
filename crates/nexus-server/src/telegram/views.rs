//! Die Antworten der lesenden Befehle als Telegram-HTML.
//!
//! Hier steht nur Formatierung: reine Funktionen über schon gelesenen Daten, damit die Tests
//! ohne Server und ohne Netz prüfen können, was im Chat ankommt. Jeder fremde Text
//! (Ticket-Titel, Kommentare, Board-Namen) geht durch die Bausteine aus
//! [`nexus_core::telegram::html`], sonst zerreißt er das Markup.
//!
//! Zeitpunkte erscheinen immer als Abstand zu „jetzt“ („vor 2 h“, „noch 29 min“) und nie als
//! Ortszeit: der Server kennt die Zeitzone des Chats nicht.

use chrono::{DateTime, Utc};
use nexus_core::board::basics::Priority;
use nexus_core::board::inbox::types::{InboxItem, InboxKind, InboxList};
use nexus_core::board::ticket_service::TicketDetail;
use nexus_core::board::view_service::BoardView;
use nexus_core::telegram::{bold, escape, CallbackAction};
use nexus_core::text::{utf16_len, utf16_prefix};

use crate::accounts::AccountUsageEntryDto;
use crate::dispatch::status::{DispatchStateKind, DispatchStatus};
use crate::telegram::api::{InlineButton, InlineKeyboard};

/// So viele Tickets zeigt `/board` je Spalte
pub const BOARD_TICKETS_PER_COLUMN: usize = 5;

/// So viele Zeichen der Beschreibung zeigt `/ticket`
pub const TICKET_TEXT_LENGTH: usize = 600;

/// So viele Kommentare zeigt `/ticket`
pub const TICKET_COMMENTS: usize = 3;

/// So viele Einträge zeigt `/aufgaben` auf einer Seite
pub const INBOX_PAGE_SIZE: usize = 10;

/// Antwort auf `/ticket` ohne ID
pub const TICKET_WITHOUT_ID: &str = "Welches Ticket? Zum Beispiel <code>/ticket NXS-0042</code>";

/// Antwort auf `/aufgaben`, wenn nichts wartet
pub const INBOX_EMPTY: &str = "Nichts zu tun – „Meine Aufgaben“ ist leer.";

/// Antwort auf `/board`, wenn weder eine Angabe noch ein aktives Board da ist
pub const NO_ACTIVE_BOARD: &str =
    "Es ist kein Board aktiv. Gib eins an, z. B. <code>/board Sprint</code>, oder wähle es unten.";

/// Alles, was `/status` zeigt; der Befehl liest es aus denselben Diensten wie die Routen
#[derive(Debug, Clone, Copy)]
pub struct StatusInput<'a> {
    pub status: &'a DispatchStatus,
    /// Name des Boards, auf das sich der Zustand bezieht; `None` = keins ausgewählt
    pub board_name: Option<&'a str>,
    /// Auslastung der aktiven Accounts; leer, wenn die Einstellung sie abschaltet
    pub accounts: &'a [AccountUsageEntryDto],
    /// Fragt Nexus die Abo-Limits überhaupt ab?
    pub accounts_enabled: bool,
    /// „Meine Aufgaben“; `None` = die Liste war nicht lesbar
    pub inbox: Option<&'a InboxList>,
    pub now: DateTime<Utc>,
}

/// Die Antwort auf `/status`
pub fn status_message(input: StatusInput<'_>) -> String {
    let status = input.status;
    let mut out = String::from("<b>Dispatcher</b>");
    out.push_str(&format!("\nZustand: {}", escape(state_label(status.state))));
    if !status.message.trim().is_empty() {
        out.push_str(&format!("\n{}", escape(&status.message)));
    }
    if let Some(until) = &status.paused_until {
        out.push_str(&format!("\nPause {}", until_text(until, input.now)));
    }
    out.push_str(&format!(
        "\nBoard: {}",
        match (input.board_name, status.board_id) {
            (Some(name), Some(id)) => format!("{} ({id})", escape(name)),
            (Some(name), None) => escape(name),
            (None, Some(id)) => format!("{id}"),
            (None, None) => "keins".to_string(),
        }
    ));

    out.push_str(&format!(
        "\n\n{}",
        bold(&format!("Läufe ({})", status.runs.len()))
    ));
    if status.runs.is_empty() {
        out.push_str("\nkeine");
    }
    for run in &status.runs {
        let name = format!("{}{}", status.agent_prefix, run.ticket_id);
        let mut line = format!("\n{}", escape(&name));
        line.push_str(&format!(" · {}", since_text(&run.started_at, input.now)));
        if let Some(account) = &run.account {
            line.push_str(&format!(" · {}", escape(account)));
        }
        out.push_str(&line);
    }

    out.push_str(&format!("\n\n{}", bold("Accounts")));
    if !input.accounts_enabled {
        out.push_str("\nAuslastung ist abgeschaltet");
    } else if input.accounts.is_empty() {
        out.push_str("\nkeine");
    }
    for account in input.accounts {
        out.push_str(&format!(
            "\n{} · {}",
            escape(&account.name),
            usage_text(account)
        ));
    }

    match input.inbox {
        None => out.push_str(&format!("\n\n{}\nnicht lesbar", bold("Meine Aufgaben"))),
        Some(inbox) => {
            out.push_str(&format!(
                "\n\n{}",
                bold(&format!("Meine Aufgaben ({})", inbox.total))
            ));
            let parts: Vec<String> = InboxKind::ALL
                .into_iter()
                .filter(|kind| inbox.counts.get(*kind) > 0)
                .map(|kind| format!("{} {}", escape(kind.label()), inbox.counts.get(kind)))
                .collect();
            out.push_str(&format!(
                "\n{}",
                match parts.is_empty() {
                    true => "nichts offen".to_string(),
                    false => parts.join(" · "),
                }
            ));
        }
    }
    out
}

/// Die Auslastung eines Accounts in einer Zeile
fn usage_text(account: &AccountUsageEntryDto) -> String {
    if !account.usage.available {
        return format!(
            "keine Werte ({})",
            escape(account.usage.reason.as_deref().unwrap_or("unbekannt"))
        );
    }
    let window = |value: &Option<crate::usage::UsageWindowDto>| match value {
        Some(window) => format!("{} %", window.percent),
        None => "–".to_string(),
    };
    format!(
        "5 h {} · Woche {}",
        window(&account.usage.five_hour),
        window(&account.usage.seven_day)
    )
}

/// Die Antwort auf `/board`: Spalten mit Anzahl und je Spalte die ersten Tickets
pub fn board_message(name: &str, view: &BoardView) -> String {
    let mut out = format!(
        "{} (Board {})",
        bold(&format!("Board {name}")),
        view.board_id
    );
    for column in &view.columns {
        out.push_str(&format!(
            "\n\n{}",
            bold(&format!("{} ({})", column.title, column.tickets.len()))
        ));
        if column.tickets.is_empty() {
            out.push_str("\n–");
            continue;
        }
        for card in column.tickets.iter().take(BOARD_TICKETS_PER_COLUMN) {
            let mut line = format!("\n{} {}", escape(&card.id), escape(&card.title));
            if !card.labels.is_empty() {
                line.push_str(&format!(" [{}]", escape(&card.labels.join(", "))));
            }
            out.push_str(&line);
        }
        let rest = column
            .tickets
            .len()
            .saturating_sub(BOARD_TICKETS_PER_COLUMN);
        if rest > 0 {
            out.push_str(&format!("\n… und {rest} weitere"));
        }
    }
    out
}

/// Ein Board für die Knöpfe unter `/board`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardChoice {
    pub id: i32,
    pub name: String,
}

/// Knöpfe zum Umschalten des aktiven Boards; `None`, wenn es nichts umzuschalten gibt.
/// Das gezeigte Board trägt ein Häkchen und bleibt als Knopf stehen, damit die Reihenfolge
/// über alle Antworten gleich bleibt
pub fn board_keyboard(boards: &[BoardChoice], shown: Option<i32>) -> Option<InlineKeyboard> {
    if boards.is_empty() {
        return None;
    }
    let rows = boards
        .chunks(2)
        .map(|chunk| {
            chunk
                .iter()
                .map(|board| {
                    let mark = match Some(board.id) == shown {
                        true => "✓ ",
                        false => "",
                    };
                    InlineButton::callback(
                        format!("{mark}{}", board.name),
                        CallbackAction::Board(board.id).encode(),
                    )
                })
                .collect()
        })
        .collect();
    Some(InlineKeyboard::new(rows))
}

/// Die Antwort auf `/ticket <ID>`
pub fn ticket_message(detail: &TicketDetail, now: DateTime<Utc>) -> String {
    let ticket = &detail.view.ticket;
    let mut out = bold(&format!("{} · {}", ticket.id, ticket.title));
    out.push_str(&format!(
        "\nStatus: {} · Priorität: {}",
        escape(ticket.status.label()),
        escape(priority_label(ticket.priority))
    ));
    if detail.view.waiting {
        out.push_str(&format!(
            "\nWartet auf: {}",
            escape(&detail.view.depends_open.join(", "))
        ));
    }
    if let Some(reason) = ticket
        .blocked_reason
        .as_deref()
        .filter(|reason| !reason.trim().is_empty())
    {
        out.push_str(&format!("\nBlockiert: {}", escape(reason)));
    }
    if !ticket.labels.is_empty() {
        out.push_str(&format!("\nLabels: {}", escape(&ticket.labels.join(", "))));
    }
    if let Some(assignee) = ticket
        .assignee
        .as_deref()
        .filter(|name| !name.trim().is_empty())
    {
        out.push_str(&format!("\nÜbernommen von: {}", escape(assignee)));
    }
    out.push_str(&section("Beschreibung", &ticket.description));
    out.push_str(&section("Abnahme", &ticket.acceptance));

    out.push_str(&format!("\n\n{}", bold("Letzte Kommentare")));
    let comments: Vec<_> = detail.comments.iter().rev().take(TICKET_COMMENTS).collect();
    if comments.is_empty() {
        out.push_str("\nkeine");
    }
    // `rev` hat die neuesten zuerst; im Chat sollen sie wie im Ticket aufeinander folgen
    for comment in comments.into_iter().rev() {
        out.push_str(&format!(
            "\n<i>{} · {}</i>\n{}",
            escape(&comment.author_name),
            since_text(&comment.created_at, now),
            escape(&shortened(&comment.body, TICKET_TEXT_LENGTH))
        ));
    }
    out
}

/// Antwort auf `/ticket` mit einer ID, die es nicht gibt
pub fn unknown_ticket(id: &str) -> String {
    format!(
        "Das Ticket {} gibt es nicht.",
        escape(&shortened(id.trim(), 40))
    )
}

/// Ein Abschnitt mit Überschrift; leerer Text ergibt nichts
fn section(title: &str, text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    format!(
        "\n\n{}\n{}",
        bold(title),
        escape(&shortened(text.trim(), TICKET_TEXT_LENGTH))
    )
}

/// Ein Eintrag aus „Meine Aufgaben“ als eigene Nachricht samt seinen Knöpfen
/// ([`inbox_item_keyboard`]).
pub fn inbox_item_message(
    item: &InboxItem,
    now: DateTime<Utc>,
) -> (String, Option<InlineKeyboard>) {
    let mut out = bold(item.kind.label());
    if let Some(id) = item.ticket_id.as_deref().or(item.question_id.as_deref()) {
        out.push_str(&format!(" · {}", escape(id)));
    }
    out.push_str(&format!("\n{}", escape(&item.title)));
    if let Some(detail) = item
        .detail
        .as_deref()
        .filter(|detail| !detail.trim().is_empty())
    {
        out.push_str(&format!(
            "\n{}",
            escape(&shortened(detail.trim(), TICKET_TEXT_LENGTH))
        ));
    }
    let board = item.board_name.as_deref().unwrap_or(&item.project_key);
    out.push_str(&format!(
        "\n<i>{} · {}</i>",
        escape(board),
        since_text(&item.since, now)
    ));
    let keyboard = inbox_item_keyboard(item);
    (out, keyboard)
}

/// Die Knöpfe eines Eintrags aus „Meine Aufgaben“, je nach Art. Fragen und gescheiterte
/// Einrichtungen bekommen hier keine: eine Frage zeigt ihre Optionen samt Knöpfen über
/// [`crate::telegram::question_views::question_message`], und eine Einrichtung lässt sich nur
/// am Rechner richten
pub fn inbox_item_keyboard(item: &InboxItem) -> Option<InlineKeyboard> {
    let buttons = match item.kind {
        InboxKind::Accept => with_ticket(item, accept_buttons),
        InboxKind::Merge => with_ticket(item, merge_buttons),
        InboxKind::Blocked => with_ticket(item, blocked_buttons),
        InboxKind::Planning => with_ticket(item, |id| {
            vec![InlineButton::callback(
                "Planen starten",
                CallbackAction::Replan(id.to_string()).encode(),
            )]
        }),
        InboxKind::Note => item.note_id.map(|note_id| {
            vec![
                InlineButton::callback(
                    "Ticket daraus",
                    CallbackAction::NoteToTicket(note_id).encode(),
                ),
                InlineButton::callback("Verwerfen", CallbackAction::NoteDrop(note_id).encode()),
            ]
        }),
        InboxKind::Question | InboxKind::Setup => None,
    };
    Some(rows_of_two(buttons?))
}

/// Die Knöpfe einer Abnahme: übernehmen, zurück an den Agenten, geänderte Dateien
pub fn accept_buttons(id: &str) -> Vec<InlineButton> {
    vec![
        InlineButton::callback(
            "Übernehmen",
            CallbackAction::Accept(id.to_string()).encode(),
        ),
        InlineButton::callback(
            "Zurück an den Agenten",
            CallbackAction::Return(id.to_string()).encode(),
        ),
        InlineButton::callback(
            "Geänderte Dateien",
            CallbackAction::Diff(id.to_string()).encode(),
        ),
    ]
}

/// Die Knöpfe eines gescheiterten Merges: erneut mergen, Grund
pub fn merge_buttons(id: &str) -> Vec<InlineButton> {
    vec![
        InlineButton::callback(
            "Erneut mergen",
            CallbackAction::Merge(id.to_string()).encode(),
        ),
        InlineButton::callback(
            "Grund",
            CallbackAction::BlockedReason(id.to_string()).encode(),
        ),
    ]
}

/// Die Knöpfe eines blockierten Tickets: Grund, Kommentar, zurück nach Geplant
pub fn blocked_buttons(id: &str) -> Vec<InlineButton> {
    vec![
        InlineButton::callback(
            "Grund",
            CallbackAction::BlockedReason(id.to_string()).encode(),
        ),
        InlineButton::callback(
            "Kommentar schreiben",
            CallbackAction::Comment(id.to_string()).encode(),
        ),
        InlineButton::callback(
            "Zurück nach Geplant",
            CallbackAction::Return(id.to_string()).encode(),
        ),
    ]
}

/// Die Knöpfe zu je zwei in einer Zeile
pub fn rows_of_two(buttons: Vec<InlineButton>) -> InlineKeyboard {
    InlineKeyboard::new(buttons.chunks(2).map(<[InlineButton]>::to_vec).collect())
}

/// Die Knöpfe eines Ticket-Eintrags; ohne Ticket-ID gibt es keine
fn with_ticket(
    item: &InboxItem,
    buttons: impl FnOnce(&str) -> Vec<InlineButton>,
) -> Option<Vec<InlineButton>> {
    item.ticket_id.as_deref().map(buttons)
}

/// Knöpfe „Mehr“ unter der letzten Nachricht von `/aufgaben`: je Art, von der noch Einträge
/// fehlen, ein Knopf auf deren nächste Seite. `remaining` nennt je Art, wie viele fehlen;
/// `None`, wenn alles gezeigt wurde
pub fn more_keyboard(remaining: &[(InboxKind, i32)], next_page: u32) -> Option<InlineKeyboard> {
    let buttons: Vec<InlineButton> = InboxKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let rest = remaining
                .iter()
                .find(|(known, _)| *known == kind)
                .map(|(_, rest)| *rest)
                .unwrap_or(0);
            if rest <= 0 {
                return None;
            }
            Some(InlineButton::callback(
                format!("Mehr: {} ({rest})", kind.label()),
                CallbackAction::More(kind, next_page).encode(),
            ))
        })
        .collect();
    if buttons.is_empty() {
        return None;
    }
    Some(InlineKeyboard::new(
        buttons.chunks(2).map(<[InlineButton]>::to_vec).collect(),
    ))
}

/// Was nach den gezeigten Einträgen je Art noch offen ist (für [`more_keyboard`])
pub fn remaining_per_kind(shown: &[InboxItem], list: &InboxList) -> Vec<(InboxKind, i32)> {
    InboxKind::ALL
        .into_iter()
        .map(|kind| {
            let gezeigt = shown.iter().filter(|item| item.kind == kind).count() as i32;
            (kind, list.counts.get(kind) - gezeigt)
        })
        .collect()
}

/// Kopfzeile einer Seite von `/aufgaben` ab Seite 2
pub fn more_page_title(kind: InboxKind, page: u32) -> String {
    bold(&format!("{} – Seite {page}", kind.label()))
}

/// Antwort auf „Mehr“, wenn die Seite leer ist (die Liste hat sich inzwischen geändert)
pub fn no_more_items(kind: InboxKind) -> String {
    format!(
        "Keine weiteren Einträge der Art „{}“.",
        escape(kind.label())
    )
}

/// Antwort auf `/pause`
pub fn paused(minutes: i64) -> String {
    format!("Dispatcher pausiert – {}.", escape(&minutes_text(minutes)))
}

/// Antwort auf `/weiter`
pub const RESUMED: &str = "Dispatcher läuft wieder.";

/// Antwort auf `/pause` mit einer Angabe, die keine Minutenzahl ist
pub const BAD_MINUTES: &str = "Die Pause braucht eine ganze Zahl von 1 bis 1440 Minuten, \
z. B. <code>/pause 30</code>.";

/// „30 min“ bzw. „2 h 30 min“
fn minutes_text(minutes: i64) -> String {
    match (minutes / 60, minutes % 60) {
        (0, rest) => format!("{rest} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, rest) => format!("{hours} h {rest} min"),
    }
}

/// „vor 12 min“ zu einem ISO-Zeitpunkt in der Vergangenheit; unlesbare Zeitpunkte ergeben „?“
pub(crate) fn since_text(iso: &str, now: DateTime<Utc>) -> String {
    match seconds_between(iso, now) {
        None => "?".to_string(),
        Some(seconds) if seconds < 0 => format!("in {}", duration_text(-seconds)),
        Some(seconds) => format!("vor {}", duration_text(seconds)),
    }
}

/// „noch 29 min“ zu einem ISO-Zeitpunkt in der Zukunft
fn until_text(iso: &str, now: DateTime<Utc>) -> String {
    match seconds_between(iso, now) {
        None => "bis ?".to_string(),
        Some(seconds) if seconds > 0 => "ist vorbei".to_string(),
        Some(seconds) => format!("noch {}", duration_text(-seconds)),
    }
}

/// Sekunden von `iso` bis `now`; `None`, wenn `iso` kein Zeitpunkt ist
fn seconds_between(iso: &str, now: DateTime<Utc>) -> Option<i64> {
    let at = iso.parse::<DateTime<Utc>>().ok()?;
    Some((now - at).num_seconds())
}

/// Eine Dauer in Sekunden in Worten, immer mit der größten passenden Einheit
fn duration_text(seconds: i64) -> String {
    match seconds {
        ..=59 => format!("{} s", seconds.max(0)),
        60..=3599 => format!("{} min", seconds / 60),
        3600..=86399 => match (seconds / 3600, (seconds % 3600) / 60) {
            (hours, 0) => format!("{hours} h"),
            (hours, minutes) => format!("{hours} h {minutes} min"),
        },
        _ => match (seconds / 86400, (seconds % 86400) / 3600) {
            (days, 0) => format!("{days} d"),
            (days, hours) => format!("{days} d {hours} h"),
        },
    }
}

/// Kürzt `text` auf `max` UTF-16-Einheiten und hängt ein Auslassungszeichen an
pub(crate) fn shortened(text: &str, max: usize) -> String {
    if utf16_len(text) <= max {
        return text.to_string();
    }
    format!("{}…", utf16_prefix(text, max))
}

/// Priorität eines Tickets auf Deutsch (die Anzeige im Browser nutzt dieselben Wörter)
fn priority_label(priority: Priority) -> &'static str {
    match priority {
        Priority::Low => "niedrig",
        Priority::Normal => "normal",
        Priority::High => "hoch",
        Priority::Urgent => "dringend",
    }
}

/// Zustand des Dispatchers in einem Wort
fn state_label(state: DispatchStateKind) -> &'static str {
    match state {
        DispatchStateKind::Off => "aus",
        DispatchStateKind::Idle => "wartet auf Arbeit",
        DispatchStateKind::Running => "läuft",
        DispatchStateKind::Paused => "pausiert",
        DispatchStateKind::Outside => "außerhalb des Zeitfensters",
        DispatchStateKind::Error => "Einstellung fehlerhaft",
    }
}

#[cfg(test)]
mod tests {
    use nexus_core::board::basics::TicketStatus;
    use nexus_core::board::ticket_ordering::BoardColumn;
    use nexus_core::board::view_service::{BoardView as View, BoardViewColumn};
    use nexus_core::telegram::MAX_MESSAGE_LEN;

    use super::*;

    /// Der Zeitpunkt, auf den alle Tests ihre Uhr stellen
    fn now() -> DateTime<Utc> {
        "2026-11-03T12:00:00Z".parse().expect("Zeitpunkt")
    }

    /// Ein Zustand ohne Läufe, mit dem die Tests nur das Nötige setzen
    fn status() -> DispatchStatus {
        DispatchStatus {
            enabled: true,
            state: DispatchStateKind::Running,
            message: "2 von 3 Plätzen belegt".into(),
            paused_until: None,
            account_pauses: std::collections::BTreeMap::new(),
            wip: 3,
            active: 0,
            planning: 0,
            runs: Vec::new(),
            research: crate::dispatch::status::DispatchResearchInfo {
                auto: false,
                wip: 1,
                active: 0,
                max_per_day: 5,
                today: 0,
                since: None,
                retry_at: None,
                error: None,
            },
            planning_auto: crate::dispatch::status::DispatchPlanningInfo {
                auto: false,
                wip: 1,
                active: 0,
                since: None,
                retry_at: None,
                error: None,
            },
            merges: Vec::new(),
            board_id: Some(3),
            board_in_scope: true,
            tickets: Vec::new(),
            tickets_error: None,
            last_tick: None,
            last_errors: Vec::new(),
            config_file: "dispatch.yaml".into(),
            config_error: None,
            agent_prefix: "junior-".into(),
        }
    }

    /// Ein laufender Ticket-Lauf
    fn run(ticket: &str, started_at: &str, account: Option<&str>) -> super::DispatchStatus {
        let mut status = status();
        status.runs.push(crate::dispatch::status::DispatchRunInfo {
            id: "r1".into(),
            kind: nexus_core::dispatch::RunKind::Work,
            ticket_id: ticket.into(),
            title: "Egal".into(),
            profile: "standard".into(),
            model: None,
            effort: None,
            started_at: started_at.into(),
            log: "r1.log".into(),
            question_id: None,
            account: account.map(str::to_string),
        });
        status
    }

    /// Eine Auslastung mit diesen Prozenten
    fn account(name: &str, five: Option<i64>, week: Option<i64>) -> AccountUsageEntryDto {
        AccountUsageEntryDto {
            id: name.into(),
            name: name.into(),
            fallback: false,
            limit_five_hour: None,
            limit_week: None,
            usage: crate::usage::ClaudeUsageDto {
                available: true,
                reason: None,
                plan: Some("max".into()),
                five_hour: five.map(|percent| crate::usage::UsageWindowDto {
                    percent,
                    resets_at: None,
                }),
                seven_day: week.map(|percent| crate::usage::UsageWindowDto {
                    percent,
                    resets_at: None,
                }),
                fetched_at: None,
            },
        }
    }

    /// „Meine Aufgaben“ mit diesen Zählern
    fn inbox(
        items: Vec<InboxItem>,
        counts: nexus_core::board::inbox::types::InboxCounts,
    ) -> InboxList {
        let total = InboxKind::ALL
            .into_iter()
            .map(|kind| counts.get(kind))
            .sum();
        InboxList {
            items,
            counts,
            total,
        }
    }

    /// Zähler, in denen nur diese Arten vorkommen
    fn counts(
        merge: i32,
        question: i32,
        note: i32,
    ) -> nexus_core::board::inbox::types::InboxCounts {
        nexus_core::board::inbox::types::InboxCounts {
            merge,
            setup: 0,
            question,
            accept: 0,
            blocked: 0,
            planning: 0,
            note,
        }
    }

    /// Ein Eintrag der Arbeitsliste
    fn item(kind: InboxKind, title: &str, since: &str) -> InboxItem {
        InboxItem {
            key: format!("ticket:{title}"),
            kind,
            title: title.into(),
            detail: None,
            board_id: Some(3),
            board_name: Some("Sprint".into()),
            project_key: "NXS".into(),
            ticket_id: Some("NXS-0042".into()),
            question_id: None,
            note_id: None,
            waiting_tickets: 0,
            priority: None,
            since: since.into(),
            rank: 0,
        }
    }

    /// Die Knöpfe einer Tastatur als Paare (Beschriftung, Callback-Daten)
    fn buttons(keyboard: &Option<InlineKeyboard>) -> Vec<(String, String)> {
        keyboard
            .iter()
            .flat_map(|keyboard| keyboard.inline_keyboard.iter().flatten())
            .map(|button| {
                (
                    button.text.clone(),
                    button.callback_data.clone().unwrap_or_default(),
                )
            })
            .collect()
    }

    #[test]
    fn zeigt_zustand_laufe_accounts_und_aufgaben() {
        let status = run("NXS-0119", "2026-11-03T11:48:00Z", Some("standard"));
        let liste = inbox(Vec::new(), counts(1, 2, 1));

        let text = status_message(StatusInput {
            status: &status,
            board_name: Some("Sprint"),
            accounts: &[account("standard", Some(36), Some(12))],
            accounts_enabled: true,
            inbox: Some(&liste),
            now: now(),
        });

        assert!(text.contains("Zustand: läuft"), "{text}");
        assert!(text.contains("2 von 3 Plätzen belegt"), "{text}");
        assert!(text.contains("Board: Sprint (3)"), "{text}");
        assert!(
            text.contains("junior-NXS-0119 · vor 12 min · standard"),
            "{text}"
        );
        assert!(text.contains("standard · 5 h 36 % · Woche 12 %"), "{text}");
        assert!(text.contains("Meine Aufgaben (4)"), "{text}");
        assert!(
            text.contains("Merge gescheitert 1 · Frage 2 · Notiz 1"),
            "{text}"
        );
    }

    #[test]
    fn nennt_die_restliche_pause_statt_einer_ortszeit() {
        let mut status = status();
        status.state = DispatchStateKind::Paused;
        status.paused_until = Some("2026-11-03T12:29:00Z".into());

        let text = status_message(StatusInput {
            status: &status,
            board_name: None,
            accounts: &[],
            accounts_enabled: false,
            inbox: None,
            now: now(),
        });

        assert!(text.contains("Zustand: pausiert"), "{text}");
        assert!(text.contains("Pause noch 29 min"), "{text}");
        assert!(text.contains("Auslastung ist abgeschaltet"), "{text}");
        assert!(text.contains("nicht lesbar"), "{text}");
    }

    #[test]
    fn meldet_einen_account_ohne_werte_mit_seinem_grund() {
        let ohne = AccountUsageEntryDto {
            usage: crate::usage::ClaudeUsageDto::unavailable("no-login", None),
            ..account("arbeit", None, None)
        };

        let text = status_message(StatusInput {
            status: &status(),
            board_name: None,
            accounts: &[ohne],
            accounts_enabled: true,
            inbox: None,
            now: now(),
        });

        assert!(text.contains("arbeit · keine Werte (no-login)"), "{text}");
        assert!(text.contains("<b>Läufe (0)</b>\nkeine"), "{text}");
    }

    /// Eine Spalte mit so vielen Tickets
    fn column(id: BoardColumn, count: usize) -> BoardViewColumn {
        BoardViewColumn {
            id,
            title: id.label().to_string(),
            tickets: (1..=count).map(|number| card(number as i32)).collect(),
        }
    }

    /// Eine Karte mit Nummer, Titel und einem Label
    fn card(number: i32) -> nexus_core::board::view_service::BoardCard {
        nexus_core::board::view_service::BoardCard {
            id: format!("NXS-{number:04}"),
            project_id: 1,
            board_id: 3,
            number,
            title: format!("Ticket {number} <b>"),
            status: TicketStatus::Backlog,
            priority: Priority::Normal,
            labels: vec!["telegram".into()],
            depends_on: Vec::new(),
            sort_order: number,
            assignee: None,
            needs_planning: true,
            spec: None,
            spec_task: None,
            branch: None,
            worktree: None,
            target_branch: None,
            touches: Vec::new(),
            consumes: Vec::new(),
            produces: Vec::new(),
            blocked_reason: None,
            planned_at: None,
            planned_by: None,
            planning_claim_by: None,
            planning_claim_at: None,
            created_at: "2026-11-01T10:00:00.000Z".into(),
            created_by_id: "u".into(),
            created_by_name: "Ben".into(),
            updated_at: "2026-11-01T10:00:00.000Z".into(),
            rev: 1,
            column: BoardColumn::Backlog,
            waiting: false,
            depends_open: Vec::new(),
            depends_unmerged: Vec::new(),
            dependents: Vec::new(),
            unlocks: Vec::new(),
            waiting_on_questions: Vec::new(),
            steps_done: 0,
            steps_total: 0,
            sessions: Vec::new(),
            comment_count: 0,
            has_description: false,
            has_acceptance: false,
            has_plan: false,
            has_handbook: false,
            has_diagram: false,
        }
    }

    #[test]
    fn zeigt_je_spalte_anzahl_und_hoechstens_funf_tickets() {
        let view = View {
            board_id: 3,
            columns: vec![
                column(BoardColumn::Backlog, 7),
                column(BoardColumn::Done, 0),
            ],
        };

        let text = board_message("Sprint", &view);

        assert!(text.contains("<b>Board Sprint</b> (Board 3)"), "{text}");
        assert!(text.contains("<b>Backlog (7)</b>"), "{text}");
        assert_eq!(text.matches("NXS-000").count(), 5, "{text}");
        assert!(text.contains("… und 2 weitere"), "{text}");
        assert!(text.contains("<b>Erledigt (0)</b>\n–"), "{text}");
        assert!(text.contains("Ticket 1 &lt;b&gt; [telegram]"), "{text}");
    }

    #[test]
    fn baut_je_board_einen_knopf_und_markiert_das_gezeigte() {
        let keyboard = board_keyboard(
            &[
                BoardChoice {
                    id: 3,
                    name: "Sprint".into(),
                },
                BoardChoice {
                    id: 4,
                    name: "Ideen".into(),
                },
            ],
            Some(3),
        )
        .expect("Knöpfe");

        assert_eq!(keyboard.inline_keyboard.len(), 1);
        assert_eq!(keyboard.inline_keyboard[0][0].text, "✓ Sprint");
        assert_eq!(
            keyboard.inline_keyboard[0][0].callback_data.as_deref(),
            Some("brd:3")
        );
        assert_eq!(keyboard.inline_keyboard[0][1].text, "Ideen");
        assert!(board_keyboard(&[], None).is_none());
    }

    #[test]
    fn nennt_bei_mehr_je_fehlende_art_einen_knopf() {
        let gezeigt = vec![item(InboxKind::Question, "Erste", "2026-11-03T10:00:00Z")];
        let liste = inbox(Vec::new(), counts(1, 3, 0));

        let keyboard = more_keyboard(&remaining_per_kind(&gezeigt, &liste), 2).expect("Knöpfe");

        let knoepfe: Vec<(String, Option<String>)> = keyboard
            .inline_keyboard
            .iter()
            .flatten()
            .map(|button| (button.text.clone(), button.callback_data.clone()))
            .collect();
        assert_eq!(
            knoepfe,
            vec![
                (
                    "Mehr: Merge gescheitert (1)".to_string(),
                    Some("mo:merge:2".to_string())
                ),
                (
                    "Mehr: Frage (2)".to_string(),
                    Some("mo:question:2".to_string())
                ),
            ]
        );
    }

    #[test]
    fn laesst_die_knoepfe_weg_wenn_alles_gezeigt_ist() {
        let gezeigt = vec![item(InboxKind::Note, "Einzige", "2026-11-03T10:00:00Z")];
        let liste = inbox(Vec::new(), counts(0, 0, 1));

        assert!(more_keyboard(&remaining_per_kind(&gezeigt, &liste), 2).is_none());
    }

    #[test]
    fn zeigt_einen_eintrag_mit_art_titel_detail_board_und_seit() {
        let mut eintrag = item(InboxKind::Blocked, "Login hängt", "2026-11-01T12:00:00Z");
        eintrag.detail = Some("Zugang fehlt & wartet".into());

        let (text, keyboard) = inbox_item_message(&eintrag, now());

        assert!(text.contains("<b>Blockiert</b> · NXS-0042"), "{text}");
        assert!(text.contains("Login hängt"), "{text}");
        assert!(text.contains("Zugang fehlt &amp; wartet"), "{text}");
        assert!(text.contains("<i>Sprint · vor 2 d</i>"), "{text}");
        assert_eq!(
            buttons(&keyboard),
            vec![
                ("Grund".to_string(), "blk:NXS-0042".to_string()),
                (
                    "Kommentar schreiben".to_string(),
                    "cmt:NXS-0042".to_string()
                ),
                (
                    "Zurück nach Geplant".to_string(),
                    "ret:NXS-0042".to_string()
                ),
            ]
        );
    }

    #[test]
    fn jede_art_aus_meine_aufgaben_bekommt_ihre_eigenen_knoepfe() {
        let abnahme = buttons(&inbox_item_keyboard(&item(
            InboxKind::Accept,
            "Fertig",
            "2026-11-03T10:00:00Z",
        )));
        assert_eq!(
            abnahme
                .iter()
                .map(|(_, data)| data.as_str())
                .collect::<Vec<_>>(),
            vec!["acc:NXS-0042", "ret:NXS-0042", "dif:NXS-0042"]
        );

        let merge = buttons(&inbox_item_keyboard(&item(
            InboxKind::Merge,
            "Konflikt",
            "2026-11-03T10:00:00Z",
        )));
        assert_eq!(merge[0], ("Erneut mergen".into(), "mrg:NXS-0042".into()));

        let planung = buttons(&inbox_item_keyboard(&item(
            InboxKind::Planning,
            "Plan fehlt",
            "2026-11-03T10:00:00Z",
        )));
        assert_eq!(
            planung,
            vec![("Planen starten".into(), "rpl:NXS-0042".into())]
        );

        let mut eintrag = item(InboxKind::Note, "Cache", "2026-11-03T10:00:00Z");
        eintrag.ticket_id = None;
        eintrag.note_id = Some(7);
        assert_eq!(
            buttons(&inbox_item_keyboard(&eintrag)),
            vec![
                ("Ticket daraus".to_string(), "ntt:7".to_string()),
                ("Verwerfen".to_string(), "ndr:7".to_string()),
            ]
        );

        for ohne in [InboxKind::Question, InboxKind::Setup] {
            let eintrag = item(ohne, "egal", "2026-11-03T10:00:00Z");
            assert!(
                inbox_item_keyboard(&eintrag).is_none(),
                "{ohne} bekommt keine Knöpfe"
            );
        }
    }

    #[test]
    fn ein_eintrag_ohne_id_bekommt_keine_knoepfe() {
        let mut eintrag = item(InboxKind::Accept, "Fertig", "2026-11-03T10:00:00Z");
        eintrag.ticket_id = None;

        assert!(inbox_item_keyboard(&eintrag).is_none());
    }

    #[test]
    fn kuerzt_beschreibung_und_abnahme_und_zeigt_die_letzten_drei_kommentare() {
        let detail = ticket_detail();

        let text = ticket_message(&detail, now());

        assert!(text.contains("<b>NXS-0042 · Login bauen</b>"), "{text}");
        assert!(
            text.contains("Status: In Arbeit · Priorität: hoch"),
            "{text}"
        );
        assert!(text.contains("Labels: telegram, backend"), "{text}");
        assert!(text.contains("<b>Beschreibung</b>"), "{text}");
        assert!(text.contains('…'), "die lange Beschreibung ist gekürzt");
        assert!(text.contains("<b>Abnahme</b>\nLäuft durch"), "{text}");
        assert!(!text.contains("Kommentar 1"), "{text}");
        for nummer in 2..=4 {
            assert!(text.contains(&format!("Kommentar {nummer}")), "{text}");
        }
        assert!(text.contains("<i>Ben · vor 1 h</i>"), "{text}");
    }

    #[test]
    fn teilt_eine_lange_board_antwort_an_zeilengrenzen() {
        // Acht Spalten mit je fünf Karten reißen die Grenze von 4096 Zeichen, sobald die
        // Titel lang sind; geteilt wird nur an Zeilengrenzen, damit kein Tag zerreißt
        let columns = nexus_core::board::ticket_ordering::COLUMN_ORDER
            .iter()
            .map(|id| BoardViewColumn {
                id: *id,
                title: id.label().to_string(),
                tickets: (1..=5)
                    .map(|number| nexus_core::board::view_service::BoardCard {
                        title: "T".repeat(150),
                        ..card(number)
                    })
                    .collect(),
            })
            .collect();

        let text = board_message(
            "Sprint",
            &View {
                board_id: 3,
                columns,
            },
        );
        let stuecke = nexus_core::telegram::split_message(&text, MAX_MESSAGE_LEN);

        assert!(stuecke.len() > 1, "{} Stücke", stuecke.len());
        for stueck in &stuecke {
            assert!(utf16_len(stueck) <= MAX_MESSAGE_LEN, "{}", stueck.len());
        }
    }

    #[test]
    fn meldet_eine_unbekannte_id_verstaendlich() {
        assert_eq!(
            unknown_ticket(" NXS-9999 "),
            "Das Ticket NXS-9999 gibt es nicht."
        );
        assert!(unknown_ticket("<b>x</b>").contains("&lt;b&gt;"));
    }

    #[test]
    fn nennt_die_laenge_der_pause_in_worten() {
        assert_eq!(paused(30), "Dispatcher pausiert – 30 min.");
        assert_eq!(paused(60), "Dispatcher pausiert – 1 h.");
        assert_eq!(paused(90), "Dispatcher pausiert – 1 h 30 min.");
    }

    /// Ein Ticket mit langer Beschreibung, Abnahme und vier Kommentaren
    fn ticket_detail() -> TicketDetail {
        use nexus_core::board::ticket_service::TicketView;
        use nexus_core::board::types::Ticket;

        let ticket = Ticket {
            id: "NXS-0042".into(),
            title: "Login bauen".into(),
            status: TicketStatus::InProgress,
            priority: Priority::High,
            labels: vec!["telegram".into(), "backend".into()],
            description: "a".repeat(TICKET_TEXT_LENGTH + 50),
            acceptance: "Läuft durch".into(),
            ..blank_ticket()
        };
        TicketDetail {
            view: TicketView {
                ticket,
                column: BoardColumn::InProgress,
                waiting: false,
                depends_open: Vec::new(),
                depends_unmerged: Vec::new(),
                dependents: Vec::new(),
                unlocks: Vec::new(),
                waiting_on_questions: Vec::new(),
                steps_done: 0,
                steps_total: 0,
                sessions: Vec::new(),
            },
            comments: (1..=4)
                .map(|number| {
                    comment(
                        number,
                        &format!("Kommentar {number}"),
                        "2026-11-03T11:00:00Z",
                    )
                })
                .collect(),
            history: Vec::new(),
            steps: Vec::new(),
        }
    }

    /// Ein Kommentar
    fn comment(id: i64, body: &str, created_at: &str) -> nexus_core::board::types::Comment {
        nexus_core::board::types::Comment {
            id,
            ticket_id: "NXS-0042".into(),
            author_id: "u".into(),
            author_name: "Ben".into(),
            body: body.into(),
            created_at: created_at.into(),
            read_by_agent_at: None,
        }
    }

    /// Ein Ticket mit lauter Vorgaben; die Tests setzen nur, was sie prüfen
    fn blank_ticket() -> nexus_core::board::types::Ticket {
        nexus_core::board::types::Ticket {
            id: String::new(),
            project_id: 1,
            board_id: 3,
            number: 42,
            title: String::new(),
            status: TicketStatus::Backlog,
            priority: Priority::Normal,
            labels: Vec::new(),
            depends_on: Vec::new(),
            sort_order: 1,
            assignee: None,
            needs_planning: false,
            spec: None,
            spec_task: None,
            branch: None,
            worktree: None,
            target_branch: None,
            touches: Vec::new(),
            consumes: Vec::new(),
            produces: Vec::new(),
            blocked_reason: None,
            description: String::new(),
            acceptance: String::new(),
            plan: String::new(),
            handbook: String::new(),
            diagram: String::new(),
            planned_at: None,
            planned_by: None,
            planning_claim_by: None,
            planning_claim_at: None,
            created_at: "2026-11-01T10:00:00.000Z".into(),
            created_by_id: "u".into(),
            created_by_name: "Ben".into(),
            updated_at: "2026-11-01T10:00:00.000Z".into(),
            rev: 1,
        }
    }
}
