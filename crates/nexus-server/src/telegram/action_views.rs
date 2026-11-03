//! Die Texte der Knöpfe an „Meine Aufgaben“: Sicherheitsfragen, Ergebnisse, Hinweise,
//! Diff-Übersicht, Blockierungsgrund und die Dialoge für Rückmeldung und Kommentar.
//!
//! Wie in [`crate::telegram::views`] steht hier nur Formatierung über schon gelesenen
//! Daten; jeder fremde Text geht durch die Bausteine aus [`nexus_core::telegram::html`].

use chrono::{DateTime, Local, Utc};
use nexus_core::board::types::Ticket;
use nexus_core::dispatch::git::ReviewFile;
use nexus_core::telegram::{bold, escape, CallbackAction};

use crate::dispatch::review_service::ReviewView;
use crate::telegram::api::{InlineButton, InlineKeyboard};
use crate::telegram::dialog::DialogKind;

/// So viele Dateien zeigt die Diff-Übersicht
pub const DIFF_FILES: usize = 20;

/// Zusatz an jedem Kommentar, den der Bot schreibt
pub const VIA_TELEGRAM: &str = "(über Telegram)";

/// Antwort auf `/abbrechen` und auf „Abbrechen“ mit offenem Dialog
pub const DIALOG_CANCELLED: &str = "Abgebrochen.";

/// Antwort auf `/abbrechen` ohne offenen Dialog
pub const NO_DIALOG: &str = "Da ist gerade kein Dialog offen.";

/// Antwort auf einen Text, dessen Dialog zu lange gewartet hat
pub const DIALOG_EXPIRED: &str =
    "Der Dialog ist abgelaufen – nichts gespeichert. Drück den Knopf noch einmal.";

/// Was hinter einer Meldung steht, die nichts verändert hat
pub const NOTHING_CHANGED: &str = " – nichts geändert.";

/// Hängt an einen Kommentar den Zusatz [`VIA_TELEGRAM`]
pub fn via_telegram(text: &str) -> String {
    format!("{} {VIA_TELEGRAM}", text.trim())
}

/// Sicherheitsfrage vor dem Übernehmen samt „Ja“ und „Abbrechen“
pub fn accept_question(id: &str) -> (String, InlineKeyboard) {
    question(
        &format!("{id} wirklich übernehmen und mergen?"),
        "Ja, übernehmen",
        CallbackAction::AcceptConfirm(id.to_string()),
    )
}

/// Sicherheitsfrage vor dem erneuten Merge samt „Ja“ und „Abbrechen“
pub fn merge_question(id: &str) -> (String, InlineKeyboard) {
    question(
        &format!("{id} wirklich erneut mergen?"),
        "Ja, mergen",
        CallbackAction::MergeConfirm(id.to_string()),
    )
}

/// Eine Sicherheitsfrage: fetter Text, ein Knopf für „Ja“ und einer für den Abbruch
pub fn question(text: &str, yes: &str, action: CallbackAction) -> (String, InlineKeyboard) {
    let keyboard = InlineKeyboard::row(vec![
        InlineButton::callback(yes, action.encode()),
        InlineButton::callback("Abbrechen", CallbackAction::Cancel.encode()),
    ]);
    (bold(text), keyboard)
}

/// Neuer Text der Nachricht nach einer gelaufenen Aktion; `detail` ist das Ergebnis des
/// Dienstes. Die Uhrzeit ist die des Servers – dieselbe Ortszeit, nach der auch die
/// Zeitfenster des Dispatchers gehen
pub fn done_message(what: &str, detail: Option<&str>, now: DateTime<Utc>) -> String {
    let mut out = format!(
        "✅ {} über Telegram, {}",
        escape(what),
        now.with_timezone(&Local).format("%H:%M")
    );
    if let Some(detail) = detail.map(str::trim).filter(|text| !text.is_empty()) {
        out.push_str(&format!("\n{}", escape(detail)));
    }
    out
}

/// Eine Meldung als neuer Text der Nachricht; denselben Satz zeigt das Hinweisfenster des
/// Knopfes
pub fn hint_message(plain: &str) -> String {
    format!("⚠️ {}", escape(plain))
}

/// Die Dateien des Ticket-Branches gegen das Ziel, je Datei die Zahl der Zeilen
pub fn diff_message(view: &ReviewView) -> String {
    let mut out = bold(&format!("Geänderte Dateien · {}", view.ticket_id));
    if !view.available {
        let reason = view
            .reason
            .as_deref()
            .unwrap_or("Kein Diff zu diesem Ticket.");
        out.push_str(&format!("\n{}", escape(reason)));
        return out;
    }
    if let (Some(branch), Some(target)) = (view.branch.as_deref(), view.target.as_deref()) {
        out.push_str(&format!("\n{} → {}", escape(branch), escape(target)));
    }
    if view.files.is_empty() {
        out.push_str("\nKeine Datei geändert.");
        return out;
    }
    for file in view.files.iter().take(DIFF_FILES) {
        out.push_str(&format!("\n{}", file_line(file)));
    }
    if view.files.len() > DIFF_FILES {
        out.push_str(&format!(
            "\n… und {} weitere",
            view.files.len() - DIFF_FILES
        ));
    }
    let additions: u64 = view.files.iter().map(|file| file.additions).sum();
    let deletions: u64 = view.files.iter().map(|file| file.deletions).sum();
    out.push_str(&format!(
        "\n<i>{} Dateien, +{additions} −{deletions}</i>",
        view.files.len()
    ));
    out
}

/// Eine Zeile der Dateiliste: Art der Änderung, Pfad und die Zahl der Zeilen
fn file_line(file: &ReviewFile) -> String {
    let path = match file.old_path.as_deref() {
        Some(old) => format!("{old} → {}", file.path),
        None => file.path.clone(),
    };
    let numbers = if file.binary {
        "binär".to_string()
    } else {
        format!("+{} −{}", file.additions, file.deletions)
    };
    format!(
        "<code>{}</code> {} {}",
        escape(&file.status),
        escape(&path),
        escape(&numbers)
    )
}

/// Der Grund, aus dem ein Ticket blockiert ist
pub fn reason_message(ticket: &Ticket) -> String {
    let reason = ticket
        .blocked_reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .unwrap_or("Kein Grund eingetragen.");
    format!(
        "{}\n{}",
        bold(&format!("{} · Grund", ticket.id)),
        escape(reason)
    )
}

/// Der Text, mit dem ein Dialog beginnt: was der Bot erwartet und wie man abbricht
pub fn dialog_message(kind: DialogKind, target: &str) -> String {
    let what = match kind {
        DialogKind::ReturnFeedback => {
            format!("Was soll der Agent an {target} anders machen?")
        }
        DialogKind::Comment => format!("Was soll als Kommentar an {target}?"),
        DialogKind::QuestionText => format!("Wie ist {target} entschieden?"),
        DialogKind::QuestionDiscard => format!("Warum soll {target} verworfen werden?"),
    };
    format!(
        "✍️ {}\nSchick es als Nachricht; /abbrechen beendet den Dialog.",
        escape(&what)
    )
}

#[cfg(test)]
mod tests {
    use nexus_core::board::basics::{Priority, TicketStatus};

    use super::*;

    /// Ein Zeitpunkt für die Ergebnisse
    fn now() -> DateTime<Utc> {
        "2026-11-03T12:34:56Z".parse().expect("Zeitpunkt")
    }

    /// Eine Datei des Reviews
    fn file(path: &str, status: &str, additions: u64, deletions: u64) -> ReviewFile {
        ReviewFile {
            path: path.into(),
            old_path: None,
            status: status.into(),
            additions,
            deletions,
            binary: false,
        }
    }

    /// Eine Ansicht mit diesen Dateien
    fn view(files: Vec<ReviewFile>) -> ReviewView {
        ReviewView {
            ticket_id: "NXS-0042".into(),
            branch: Some("ticket/NXS-0042".into()),
            target: Some("telegram-bot".into()),
            base: Some("abc1234".into()),
            files,
            tests: Vec::new(),
            tests_at: None,
            run_log: None,
            available: true,
            reason: None,
        }
    }

    /// Ein blockiertes Ticket mit diesem Grund
    fn blocked(reason: Option<&str>) -> Ticket {
        Ticket {
            id: "NXS-0042".into(),
            project_id: 1,
            board_id: 3,
            number: 42,
            title: "Login bauen".into(),
            description: String::new(),
            status: TicketStatus::Blocked,
            priority: Priority::Normal,
            labels: Vec::new(),
            depends_on: Vec::new(),
            sort_order: 1,
            assignee: None,
            needs_planning: false,
            acceptance: String::new(),
            plan: String::new(),
            handbook: String::new(),
            diagram: String::new(),
            spec: None,
            spec_task: None,
            branch: None,
            worktree: None,
            target_branch: None,
            touches: Vec::new(),
            consumes: Vec::new(),
            produces: Vec::new(),
            blocked_reason: reason.map(str::to_string),
            planned_at: None,
            planned_by: None,
            planning_claim_by: None,
            planning_claim_at: None,
            created_at: "2026-11-03T10:00:00.000Z".into(),
            created_by_id: "u".into(),
            created_by_name: "Ben".into(),
            updated_at: "2026-11-03T10:00:00.000Z".into(),
            rev: 1,
        }
    }

    #[test]
    fn die_sicherheitsfrage_bietet_ja_und_abbrechen() {
        let (text, keyboard) = accept_question("NXS-0042");

        assert_eq!(text, "<b>NXS-0042 wirklich übernehmen und mergen?</b>");
        assert_eq!(
            keyboard,
            InlineKeyboard::row(vec![
                InlineButton::callback("Ja, übernehmen", "accc:NXS-0042"),
                InlineButton::callback("Abbrechen", "x"),
            ])
        );
        let (_, keyboard) = merge_question("NXS-0042");
        assert_eq!(
            keyboard.inline_keyboard[0][0].callback_data.as_deref(),
            Some("mrgc:NXS-0042")
        );
    }

    #[test]
    fn das_ergebnis_nennt_die_aktion_die_uhrzeit_und_das_detail() {
        let text = done_message("Übernommen", Some("NXS-0042 gemergt in main."), now());

        let uhrzeit = now().with_timezone(&Local).format("%H:%M").to_string();
        assert_eq!(
            text,
            format!("✅ Übernommen über Telegram, {uhrzeit}\nNXS-0042 gemergt in main.")
        );
        assert_eq!(
            done_message("Verworfen", Some("  "), now()),
            format!("✅ Verworfen über Telegram, {uhrzeit}"),
            "ein leeres Detail fällt weg"
        );
    }

    #[test]
    fn die_diff_uebersicht_zeigt_je_datei_die_zeilen_und_kuerzt_lange_listen() {
        let dateien = (0..DIFF_FILES + 2)
            .map(|index| file(&format!("src/a{index}.rs"), "M", 2, 1))
            .collect();

        let text = diff_message(&view(dateien));

        assert!(
            text.starts_with("<b>Geänderte Dateien · NXS-0042</b>\nticket/NXS-0042 → telegram-bot"),
            "{text}"
        );
        assert!(text.contains("<code>M</code> src/a0.rs +2 −1"), "{text}");
        assert!(text.contains("… und 2 weitere"), "{text}");
        assert!(
            text.contains(&format!("<i>{} Dateien, +44 −22</i>", DIFF_FILES + 2)),
            "{text}"
        );
    }

    #[test]
    fn die_diff_uebersicht_nennt_den_grund_wenn_es_keinen_diff_gibt() {
        let ohne = ReviewView {
            available: false,
            reason: Some("Am Ticket ist kein Branch eingetragen.".into()),
            ..view(Vec::new())
        };

        let text = diff_message(&ohne);

        assert!(
            text.contains("Am Ticket ist kein Branch eingetragen."),
            "{text}"
        );
        assert!(diff_message(&view(Vec::new())).contains("Keine Datei geändert."));
    }

    #[test]
    fn der_grund_kommt_escaped_und_fehlt_er_steht_das_auch_da() {
        let text = reason_message(&blocked(Some("Merge: Konflikt in <a>")));

        assert_eq!(
            text,
            "<b>NXS-0042 · Grund</b>\nMerge: Konflikt in &lt;a&gt;"
        );
        assert!(reason_message(&blocked(None)).contains("Kein Grund eingetragen."));
    }

    #[test]
    fn der_dialog_sagt_was_er_erwartet_und_wie_man_abbricht() {
        let text = dialog_message(DialogKind::ReturnFeedback, "NXS-0042");

        assert!(
            text.contains("Was soll der Agent an NXS-0042 anders machen?"),
            "{text}"
        );
        assert!(text.contains("/abbrechen"), "{text}");
        assert!(
            dialog_message(DialogKind::Comment, "NXS-0042").contains("als Kommentar an NXS-0042"),
        );
    }

    #[test]
    fn ein_kommentar_traegt_den_zusatz_ueber_telegram() {
        assert_eq!(
            via_telegram("  Bitte nachbessern  "),
            "Bitte nachbessern (über Telegram)"
        );
    }
}
