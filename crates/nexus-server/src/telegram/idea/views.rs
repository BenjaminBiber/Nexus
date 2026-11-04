//! Die Texte des Ideen-Dialogs: die Ansage während der Auswertung, die Bestätigungsnachricht
//! samt Knöpfen, die Board-Auswahl und die Antwort nach dem Anlegen.
//!
//! Wie in [`crate::telegram::views`] steht hier nur Formatierung über schon gelesenen Daten;
//! jeder fremde Text geht durch die Bausteine aus [`nexus_core::telegram::html`].

use nexus_core::telegram::idea::{IdeaAsk, IdeaDraft, IdeaKind};
use nexus_core::telegram::{bold, escape, italic, CallbackAction, IdeaTarget, Questionnaire};

use crate::telegram::api::{InlineButton, InlineKeyboard};
use crate::telegram::views::{rows_of_two, BoardChoice};

/// Die Ansage, solange Claude die Idee liest; sie wird später durch den Entwurf ersetzt
pub const THINKING: &str = "🤔 Ich schaue mir das Board an …";

/// Antwort auf `/idee` ohne Text
pub const IDEA_WITHOUT_TEXT: &str =
    "Was ist die Idee? Zum Beispiel <code>/idee Der Cache der Board-Ansicht läuft nie ab</code>";

/// Antwort auf eine Korrektur ohne Text
pub const EMPTY_CORRECTION: &str = "Da war kein Text dabei – der Entwurf bleibt, wie er war.";

/// Antwort auf einen Knopf, dessen Entwurf es nicht mehr gibt (angelegt, verworfen, abgelaufen)
pub const NO_DRAFT: &str = "Diesen Entwurf gibt es nicht mehr";

/// Neuer Text der Nachricht, wenn der Entwurf verworfen wurde
pub const DROPPED: &str = "🗑 Verworfen – nichts angelegt.";

/// Hinweis, sobald die Korrekturrunden aufgebraucht sind
pub const ROUNDS_SPENT: &str =
    "Mehr Korrekturen gehen nicht – jetzt nur noch anlegen oder verwerfen.";

/// So viele Zeichen der Details stehen in der Bestätigung
pub const DETAILS_PREVIEW: usize = 500;

/// Kommentar, mit dem ein Ticket aus einer Telegram-Idee beginnt
pub const FROM_TELEGRAM_VOICE: &str = "Aus Telegram-Sprachnotiz";

/// Vorspann der Eingabe in den Details einer Notiz und im Kommentar eines Tickets
pub const TRANSCRIPT: &str = "Transkript:";

/// Die Bestätigungsnachricht: Rückfragen zuerst, dann Zusammenfassung, Ziel, Titel, Details,
/// Bezüge und mögliche Dubletten. `board` ist der Name des Ziel-Boards, soweit bekannt
pub fn draft_message(draft: &IdeaDraft, board: Option<&str>) -> String {
    let mut out = String::new();
    if !draft.questions.is_empty() {
        out.push_str(&bold("Bevor ich es anlege:"));
        for question in &draft.questions {
            out.push_str(&format!("\n• {}", escape(&question.text)));
        }
        out.push_str("\n\n");
    }
    out.push_str(&escape(&draft.summary));
    out.push_str(&format!("\n\n{}", bold(&target_line(draft, board))));
    out.push_str(&format!("\n{}", escape(&draft.title)));
    if let Some(details) = preview(&draft.details) {
        out.push_str(&format!("\n{}", escape(&details)));
    }
    if !draft.related.is_empty() {
        out.push_str(&format!("\n\n{}", bold("Bezüge")));
        for related in &draft.related {
            out.push_str(&format!(
                "\n• {} – {}",
                escape(&related.ticket_id),
                escape(&related.why)
            ));
        }
    }
    if !draft.duplicates.is_empty() {
        out.push_str(&format!("\n\n{}", bold("Vielleicht schon da")));
        for duplicate in &draft.duplicates {
            out.push_str(&format!(
                "\n• {} – {}",
                escape(&duplicate.id),
                escape(&duplicate.why)
            ));
        }
    }
    out.push_str(&format!(
        "\n\n{}",
        italic(&format!("Runde {}", draft.rounds))
    ));
    out
}

/// Die Zeile, die das Ziel beschreibt: Art, Board und bei einem Kommentar das Ticket
fn target_line(draft: &IdeaDraft, board: Option<&str>) -> String {
    let where_ = match board {
        Some(name) if !name.trim().is_empty() => {
            format!("Board {} „{name}“", draft.target.board_id)
        }
        _ => format!("Board {}", draft.target.board_id),
    };
    match draft.target.kind {
        IdeaKind::Note => format!("Notiz auf {where_}"),
        IdeaKind::Ticket => format!("Ticket im Backlog von {where_}"),
        IdeaKind::Comment => format!(
            "Kommentar an {}",
            draft.target.ticket_id.as_deref().unwrap_or("?")
        ),
    }
}

/// Die Details in der Bestätigung: gekürzt, damit die Nachricht in ein Stück passt
fn preview(details: &str) -> Option<String> {
    let details = details.trim();
    if details.is_empty() {
        return None;
    }
    let short: String = details.chars().take(DETAILS_PREVIEW).collect();
    Some(match short.chars().count() < details.chars().count() {
        true => format!("{short}…"),
        false => short,
    })
}

/// Die Knöpfe unter der Bestätigung: Anlegen, das andere Ziel, anderes Board, Korrigieren,
/// Verwerfen. Sind die Korrekturrunden aufgebraucht, fällt „Korrigieren“ weg; Ziel und Board
/// bleiben umschaltbar, weil das keine Runde kostet
pub fn draft_keyboard(
    draft_id: &str,
    draft: &IdeaDraft,
    boards: usize,
    rounds_left: bool,
) -> InlineKeyboard {
    let mut buttons = vec![InlineButton::callback(
        "Anlegen",
        CallbackAction::IdeaCreate(draft_id.to_string()).encode(),
    )];
    // Ein Kommentar hängt an seinem Ticket; Notiz und Ticket lassen sich tauschen
    if let Some(other) = other_target(draft.target.kind) {
        buttons.push(InlineButton::callback(
            match other {
                IdeaTarget::Note => "Als Notiz",
                IdeaTarget::Ticket => "Als Ticket",
            },
            CallbackAction::IdeaTarget(draft_id.to_string(), other).encode(),
        ));
        if boards > 1 {
            buttons.push(InlineButton::callback(
                "Anderes Board",
                CallbackAction::IdeaBoardList(draft_id.to_string()).encode(),
            ));
        }
    }
    if rounds_left {
        buttons.push(InlineButton::callback(
            "Korrigieren",
            CallbackAction::IdeaEdit(draft_id.to_string()).encode(),
        ));
    }
    buttons.push(InlineButton::callback(
        "Verwerfen",
        CallbackAction::IdeaDrop(draft_id.to_string()).encode(),
    ));
    rows_of_two(buttons)
}

/// Das Ziel, auf das der Knopf umschaltet; ein Kommentar lässt sich nicht umschalten
fn other_target(kind: IdeaKind) -> Option<IdeaTarget> {
    match kind {
        IdeaKind::Note => Some(IdeaTarget::Ticket),
        IdeaKind::Ticket => Some(IdeaTarget::Note),
        IdeaKind::Comment => None,
    }
}

/// „Anderes Board“: die eigenen Boards zur Wahl, das aktuelle Ziel ohne Knopf
pub fn board_list(
    draft_id: &str,
    boards: &[BoardChoice],
    current: i32,
) -> (String, InlineKeyboard) {
    let buttons: Vec<InlineButton> = boards
        .iter()
        .filter(|board| board.id != current)
        .map(|board| {
            InlineButton::callback(
                &board.name,
                CallbackAction::IdeaBoard(draft_id.to_string(), board.id).encode(),
            )
        })
        .chain(std::iter::once(InlineButton::callback(
            "Abbrechen",
            CallbackAction::Cancel.encode(),
        )))
        .collect();
    (bold("Auf welches Board?"), rows_of_two(buttons))
}

/// Eine neue Idee bei noch offenem Entwurf: erst entscheiden, was mit dem alten wird
pub fn replace_question(draft_id: &str, open: &IdeaDraft) -> (String, InlineKeyboard) {
    let text = format!(
        "{}\n{}",
        bold("Es liegt noch ein Entwurf hier:"),
        escape(&open.title)
    );
    let keyboard = InlineKeyboard::row(vec![
        InlineButton::callback(
            "Alten verwerfen",
            CallbackAction::IdeaReplace(draft_id.to_string()).encode(),
        ),
        InlineButton::callback("Abbrechen", CallbackAction::Cancel.encode()),
    ]);
    (text, keyboard)
}

/// Der Text, mit dem die Korrekturrunde beginnt; offene Rückfragen stehen schon in der
/// Bestätigung
pub fn correction_message(questions: usize) -> String {
    let what = match questions {
        0 => "Was soll am Entwurf anders sein?",
        1 => "Beantworte die Rückfrage oder sag, was anders sein soll.",
        _ => "Beantworte die Rückfragen oder sag, was anders sein soll.",
    };
    format!(
        "✍️ {}\nSchick es als Text oder Sprachnachricht; /abbrechen beendet den Dialog.",
        escape(what)
    )
}

/// Was nach dem Anlegen im Chat steht: Art, Nummer und Titel
pub fn created_message(what: &str, id: &str, title: &str) -> String {
    format!("✅ {} {}\n{}", escape(what), escape(id), escape(title))
}

// ---------------------------------------------------------------------------------------------
// Die Rückfragen, einzeln
// ---------------------------------------------------------------------------------------------

/// Hinweis an einem Knopf einer Rückfrage, die nicht mehr dran ist
pub const ASK_STALE: &str = "Diese Frage ist schon beantwortet – nichts geändert.";

/// Hinweis an „Fertig“ ohne Häkchen
pub const NOTHING_TICKED: &str = "Erst mindestens eine Antwort ankreuzen.";

/// Hinweis an einer Möglichkeit, die es an der Frage nicht gibt
pub const UNKNOWN_ANSWER: &str = "Diese Antwort steht nicht mehr zur Wahl – nichts geändert.";

/// Antwort auf eine leere freie Antwort
pub const EMPTY_ANSWER: &str = "Da war kein Text dabei – die Frage bleibt offen.";

/// Marke an der empfohlenen Möglichkeit (wie bei den Board-Fragen)
pub const RECOMMENDED_MARK: &str = "★";

/// Die Nachricht zur aktuellen Rückfrage: Zusammenfassung, schon Beantwortetes, dann
/// „Frage n/m“ samt Möglichkeiten und Knöpfen
pub fn ask_message(
    draft_id: &str,
    draft: &IdeaDraft,
    questionnaire: &Questionnaire,
) -> (String, InlineKeyboard) {
    let asks = &draft.questions;
    let current = questionnaire.current.min(asks.len().saturating_sub(1));
    let mut out = escape(&draft.summary);
    let answered = answered_lines(asks, questionnaire, Some(current));
    if !answered.is_empty() {
        out.push_str(&format!("\n\n{answered}"));
    }
    let Some(ask) = asks.get(current) else {
        return (out, InlineKeyboard::new(Vec::new()));
    };
    out.push_str(&format!(
        "\n\n{}\n{}",
        bold(&format!("Frage {}/{}", current + 1, asks.len())),
        escape(&ask.text)
    ));
    for (index, option) in ask.options.iter().enumerate() {
        let mark = match ask.recommended == Some(index) {
            true => format!(" {RECOMMENDED_MARK}"),
            false => String::new(),
        };
        out.push_str(&format!("\n{}. {}{mark}", index + 1, bold(&option.label)));
        if let Some(detail) = option.detail.as_deref().filter(|d| !d.trim().is_empty()) {
            out.push_str(&format!(" – {}", escape(detail)));
        }
    }
    let how = match (ask.options.is_empty(), ask.multi_select) {
        (true, _) => "Antworte mit Text oder Sprache.",
        (false, true) => {
            "Kreuze an, was passt, und tippe „Fertig“ – oder antworte frei mit Text oder Sprache."
        }
        (false, false) => "Tippe eine Antwort oder antworte frei mit Text oder Sprache.",
    };
    out.push_str(&format!("\n\n{}", italic(how)));
    (out, ask_keyboard(draft_id, draft, questionnaire, current))
}

/// Die Knöpfe einer Rückfrage: je Möglichkeit einer (bei Mehrfachauswahl mit Häkchen und
/// „Fertig“), darunter Überspringen, Zurück (ab Frage 2) und Abbrechen
fn ask_keyboard(
    draft_id: &str,
    draft: &IdeaDraft,
    questionnaire: &Questionnaire,
    current: usize,
) -> InlineKeyboard {
    let ask = &draft.questions[current];
    let id = draft_id.to_string();
    let mut rows: Vec<Vec<InlineButton>> = ask
        .options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let star = match ask.recommended == Some(index) {
                true => format!("{RECOMMENDED_MARK} "),
                false => String::new(),
            };
            let button = match ask.multi_select {
                true => {
                    let tick = match questionnaire.ticked.contains(&index) {
                        true => "☑",
                        false => "☐",
                    };
                    InlineButton::callback(
                        format!("{tick} {star}{}", option.label),
                        CallbackAction::IdeaAskToggle(id.clone(), current, index).encode(),
                    )
                }
                false => InlineButton::callback(
                    format!("{star}{}", option.label),
                    CallbackAction::IdeaAskChoose(id.clone(), current, index).encode(),
                ),
            };
            vec![button]
        })
        .collect();
    if ask.multi_select {
        rows.push(vec![InlineButton::callback(
            "Fertig",
            CallbackAction::IdeaAskDone(id.clone(), current).encode(),
        )]);
    }
    let mut last = vec![InlineButton::callback(
        "Überspringen",
        CallbackAction::IdeaAskSkip(id.clone(), current).encode(),
    )];
    if current > 0 {
        last.push(InlineButton::callback(
            "Zurück",
            CallbackAction::IdeaAskBack(id.clone(), current).encode(),
        ));
    }
    last.push(InlineButton::callback(
        "Abbrechen",
        CallbackAction::IdeaAskCancel(id).encode(),
    ));
    rows.push(last);
    InlineKeyboard::new(rows)
}

/// Die Nachricht, wenn alle Rückfragen beantwortet sind: die Antworten zum Nachlesen
pub fn asks_done_message(draft: &IdeaDraft, questionnaire: &Questionnaire) -> String {
    format!(
        "{}\n\n{}\n\n{}",
        escape(&draft.summary),
        answered_lines(&draft.questions, questionnaire, None),
        italic("Danke – ich arbeite die Antworten ein.")
    )
}

/// Der Knopf, der eine gescheiterte Verfeinerungsrunde mit denselben Antworten neu startet
pub fn retry_keyboard(draft_id: &str, questions: usize) -> InlineKeyboard {
    InlineKeyboard::row(vec![
        InlineButton::callback(
            "Nochmal",
            CallbackAction::IdeaAskDone(draft_id.to_string(), questions).encode(),
        ),
        InlineButton::callback(
            "Verwerfen",
            CallbackAction::IdeaDrop(draft_id.to_string()).encode(),
        ),
    ])
}

/// „✓ Frage: Antwort“ je beantworteter Frage; `except` lässt die aktuelle aus
fn answered_lines(
    asks: &[IdeaAsk],
    questionnaire: &Questionnaire,
    except: Option<usize>,
) -> String {
    asks.iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != except)
        .filter_map(|(index, ask)| {
            questionnaire
                .answer_text(asks, index)
                .map(|answer| format!("✓ {}: {}", escape(&ask.text), escape(&answer)))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Was der Prüfer am Entwurf zurechtgebogen hat; leer, wenn alles passte
pub fn dropped_message(dropped: &[String]) -> Option<String> {
    if dropped.is_empty() {
        return None;
    }
    let mut out = italic("Dabei zurechtgebogen:");
    for hint in dropped {
        out.push_str(&format!("\n• {}", escape(hint)));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use nexus_core::telegram::idea::{IdeaDuplicate, IdeaRelated, IdeaTargetRef};

    use super::*;

    /// Ein Entwurf als Notiz auf Board 3
    fn draft() -> IdeaDraft {
        IdeaDraft {
            summary: "Der Cache der Board-Ansicht läuft nie ab.".into(),
            target: IdeaTargetRef {
                kind: IdeaKind::Note,
                board_id: 3,
                ticket_id: None,
            },
            title: "Cache der Board-Ansicht läuft nie ab".into(),
            details: "Gesehen in der Spaltenansicht; vermutlich fehlt die Invalidierung.".into(),
            related: Vec::new(),
            duplicates: Vec::new(),
            questions: Vec::new(),
            confidence: 0.8,
            transcript: "Der Cache läuft nie ab.".into(),
            rounds: 1,
        }
    }

    /// Die Beschriftungen und Daten der Knöpfe
    fn buttons(keyboard: &InlineKeyboard) -> Vec<(String, String)> {
        keyboard
            .inline_keyboard
            .iter()
            .flatten()
            .map(|button| {
                (
                    button.text.clone(),
                    button.callback_data.clone().unwrap_or_default(),
                )
            })
            .collect()
    }

    #[test]
    fn die_bestaetigung_nennt_ziel_titel_und_details() {
        let text = draft_message(&draft(), Some("Sprint"));

        assert!(text.contains("<b>Notiz auf Board 3 „Sprint“</b>"), "{text}");
        assert!(
            text.contains("Cache der Board-Ansicht läuft nie ab"),
            "{text}"
        );
        assert!(text.contains("fehlt die Invalidierung"), "{text}");
        assert!(text.contains("<i>Runde 1</i>"), "{text}");
        assert!(
            !text.contains("Bevor ich es anlege"),
            "ohne Rückfragen kein Vorspann: {text}"
        );
    }

    #[test]
    fn rueckfragen_stehen_oben_und_bezuege_mit_dubletten_unten() {
        let draft = IdeaDraft {
            questions: vec![IdeaAsk::open("Auch für SQL Server?")],
            related: vec![IdeaRelated {
                ticket_id: "NXS-0042".into(),
                why: "baut dieselbe Ansicht".into(),
            }],
            duplicates: vec![IdeaDuplicate {
                id: "7".into(),
                why: "Notiz beschreibt denselben Cache".into(),
            }],
            ..draft()
        };

        let text = draft_message(&draft, None);

        assert!(
            text.starts_with("<b>Bevor ich es anlege:</b>\n• Auch für SQL Server?"),
            "{text}"
        );
        assert!(
            text.contains("<b>Bezüge</b>\n• NXS-0042 – baut dieselbe Ansicht"),
            "{text}"
        );
        assert!(
            text.contains("<b>Vielleicht schon da</b>\n• 7 – Notiz beschreibt denselben Cache"),
            "{text}"
        );
        assert!(
            text.contains("<b>Notiz auf Board 3</b>"),
            "ohne Name nur die Nummer: {text}"
        );
    }

    #[test]
    fn lange_details_werden_gekuerzt() {
        let draft = IdeaDraft {
            details: "a".repeat(DETAILS_PREVIEW + 10),
            ..draft()
        };

        let text = draft_message(&draft, None);

        assert!(
            text.contains(&format!("{}…", "a".repeat(DETAILS_PREVIEW))),
            "gekürzt"
        );
        assert!(!text.contains(&"a".repeat(DETAILS_PREVIEW + 1)));
    }

    #[test]
    fn die_knoepfe_bieten_anlegen_umschalten_korrigieren_und_verwerfen() {
        let keyboard = draft_keyboard("d7", &draft(), 2, true);

        assert_eq!(
            buttons(&keyboard),
            vec![
                ("Anlegen".into(), "icr:d7".into()),
                ("Als Ticket".into(), "itg:d7:t".into()),
                ("Anderes Board".into(), "ibl:d7".into()),
                ("Korrigieren".into(), "ied:d7".into()),
                ("Verwerfen".into(), "idp:d7".into()),
            ]
        );
    }

    #[test]
    fn mit_einem_board_fehlt_die_board_wahl_und_ohne_runden_das_korrigieren() {
        let keyboard = draft_keyboard("d7", &draft(), 1, false);

        assert_eq!(
            buttons(&keyboard),
            vec![
                ("Anlegen".into(), "icr:d7".into()),
                ("Als Ticket".into(), "itg:d7:t".into()),
                ("Verwerfen".into(), "idp:d7".into()),
            ]
        );
    }

    #[test]
    fn ein_kommentar_laesst_sich_nicht_umschalten() {
        let draft = IdeaDraft {
            target: IdeaTargetRef {
                kind: IdeaKind::Comment,
                board_id: 3,
                ticket_id: Some("NXS-0042".into()),
            },
            ..draft()
        };

        let text = draft_message(&draft, Some("Sprint"));
        let keyboard = draft_keyboard("d7", &draft, 2, true);

        assert!(text.contains("<b>Kommentar an NXS-0042</b>"), "{text}");
        assert_eq!(
            buttons(&keyboard),
            vec![
                ("Anlegen".into(), "icr:d7".into()),
                ("Korrigieren".into(), "ied:d7".into()),
                ("Verwerfen".into(), "idp:d7".into()),
            ]
        );
    }

    #[test]
    fn die_board_wahl_laesst_das_aktuelle_board_weg() {
        let boards = vec![
            BoardChoice {
                id: 3,
                name: "Sprint".into(),
            },
            BoardChoice {
                id: 7,
                name: "Ideen".into(),
            },
        ];

        let (text, keyboard) = board_list("d7", &boards, 3);

        assert_eq!(text, "<b>Auf welches Board?</b>");
        assert_eq!(
            buttons(&keyboard),
            vec![
                ("Ideen".into(), "ibd:d7:7".into()),
                ("Abbrechen".into(), "x".into()),
            ]
        );
    }

    #[test]
    fn eine_zweite_idee_fragt_nach_dem_alten_entwurf() {
        let (text, keyboard) = replace_question("d7", &draft());

        assert!(text.contains("Es liegt noch ein Entwurf hier"), "{text}");
        assert!(
            text.contains("Cache der Board-Ansicht läuft nie ab"),
            "{text}"
        );
        assert_eq!(
            buttons(&keyboard),
            vec![
                ("Alten verwerfen".into(), "irp:d7".into()),
                ("Abbrechen".into(), "x".into()),
            ]
        );
    }

    #[test]
    fn der_korrekturdialog_nennt_die_rueckfragen_in_der_richtigen_zahl() {
        assert!(correction_message(0).contains("Was soll am Entwurf anders sein?"));
        assert!(correction_message(1).contains("die Rückfrage"));
        assert!(correction_message(3).contains("die Rückfragen"));
        assert!(correction_message(0).contains("/abbrechen"));
    }

    #[test]
    fn das_ergebnis_nennt_nummer_und_titel() {
        assert_eq!(
            created_message("Notiz angelegt:", "7", "Cache läuft nie ab"),
            "✅ Notiz angelegt: 7\nCache läuft nie ab"
        );
    }

    /// Ein Entwurf mit drei Rückfragen: Wahl mit Empfehlung, Mehrfachauswahl, frei
    fn mit_fragen() -> IdeaDraft {
        use nexus_core::telegram::idea::IdeaAskOption;
        let option = |label: &str, detail: Option<&str>| IdeaAskOption {
            label: label.into(),
            detail: detail.map(str::to_string),
        };
        IdeaDraft {
            summary: "Compose-Datei für einen STT-Dienst.".into(),
            questions: vec![
                IdeaAsk {
                    options: vec![
                        option("faster-whisper", Some("schnell auf CPU")),
                        option("Vosk", None),
                    ],
                    recommended: Some(0),
                    ..IdeaAsk::open("Welche Engine?")
                },
                IdeaAsk {
                    options: vec![option("Deutsch", None), option("Englisch", None)],
                    multi_select: true,
                    ..IdeaAsk::open("Welche Sprachen?")
                },
                IdeaAsk::open("Wo soll die Datei liegen?"),
            ],
            ..draft()
        }
    }

    #[test]
    fn die_erste_rueckfrage_zeigt_moeglichkeiten_empfehlung_und_knoepfe() {
        let draft = mit_fragen();
        let (text, keyboard) = ask_message("d7", &draft, &Questionnaire::new(3));

        assert!(
            text.starts_with("Compose-Datei für einen STT-Dienst."),
            "{text}"
        );
        assert!(text.contains("<b>Frage 1/3</b>\nWelche Engine?"), "{text}");
        assert!(
            text.contains("1. <b>faster-whisper</b> ★ – schnell auf CPU"),
            "{text}"
        );
        assert!(text.contains("2. <b>Vosk</b>\n"), "{text}");
        assert!(!text.contains("✓"), "noch nichts beantwortet: {text}");
        assert_eq!(
            buttons(&keyboard),
            vec![
                ("★ faster-whisper".into(), "iqa:d7:0:0".into()),
                ("Vosk".into(), "iqa:d7:0:1".into()),
                ("Überspringen".into(), "iqs:d7:0".into()),
                ("Abbrechen".into(), "iqx:d7".into()),
            ]
        );
    }

    #[test]
    fn die_mehrfachauswahl_zeigt_haekchen_fertig_und_zurueck() {
        let draft = mit_fragen();
        let mut bogen = Questionnaire::new(3);
        bogen.choose(&draft.questions, 0, 1).unwrap();
        bogen.toggle(&draft.questions, 1, 1).unwrap();

        let (text, keyboard) = ask_message("d7", &draft, &bogen);

        assert!(text.contains("✓ Welche Engine?: Vosk"), "{text}");
        assert!(text.contains("<b>Frage 2/3</b>"), "{text}");
        assert_eq!(
            buttons(&keyboard),
            vec![
                ("☐ Deutsch".into(), "iqt:d7:1:0".into()),
                ("☑ Englisch".into(), "iqt:d7:1:1".into()),
                ("Fertig".into(), "iqd:d7:1".into()),
                ("Überspringen".into(), "iqs:d7:1".into()),
                ("Zurück".into(), "iqb:d7:1".into()),
                ("Abbrechen".into(), "iqx:d7".into()),
            ]
        );
    }

    #[test]
    fn eine_freie_rueckfrage_hat_nur_die_zusatzknoepfe() {
        let draft = mit_fragen();
        let mut bogen = Questionnaire::new(3);
        bogen.skip(&draft.questions, 0).unwrap();
        bogen.toggle(&draft.questions, 1, 0).unwrap();
        bogen.confirm(&draft.questions, 1).unwrap();

        let (text, keyboard) = ask_message("d7", &draft, &bogen);

        assert!(text.contains("Antworte mit Text oder Sprache."), "{text}");
        assert!(text.contains("✓ Welche Sprachen?: Deutsch"), "{text}");
        assert_eq!(
            buttons(&keyboard)
                .into_iter()
                .map(|(label, _)| label)
                .collect::<Vec<_>>(),
            ["Überspringen", "Zurück", "Abbrechen"]
        );
    }

    #[test]
    fn nach_der_letzten_antwort_stehen_alle_antworten_da() {
        let draft = mit_fragen();
        let mut bogen = Questionnaire::new(3);
        bogen.choose(&draft.questions, 0, 0).unwrap();
        bogen.skip(&draft.questions, 1).unwrap();
        bogen.free(&draft.questions, "deploy/").unwrap();

        let text = asks_done_message(&draft, &bogen);

        assert!(text.contains("✓ Welche Engine?: faster-whisper"), "{text}");
        assert!(text.contains("✓ Welche Sprachen?: keine Vorgabe"), "{text}");
        assert!(
            text.contains("✓ Wo soll die Datei liegen?: deploy/"),
            "{text}"
        );
        assert_eq!(
            buttons(&retry_keyboard("d7", 3)),
            vec![
                ("Nochmal".into(), "iqd:d7:3".into()),
                ("Verwerfen".into(), "idp:d7".into()),
            ]
        );
    }

    #[test]
    fn hinweise_des_pruefers_kommen_nur_wenn_es_welche_gibt() {
        assert_eq!(dropped_message(&[]), None);
        assert_eq!(
            dropped_message(&["Titel auf 120 Zeichen gekürzt.".to_string()]),
            Some("<i>Dabei zurechtgebogen:</i>\n• Titel auf 120 Zeichen gekürzt.".into())
        );
    }
}
