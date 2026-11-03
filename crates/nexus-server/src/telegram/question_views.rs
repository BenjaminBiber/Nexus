//! Die Nachricht zu einer offenen Frage und ihre Knöpfe: je Option der neuesten Recherche
//! einer, dazu Freitext, „Recherche starten“ und „Verwerfen“.
//!
//! Wie in [`crate::telegram::views`] steht hier nur Formatierung über schon gelesenen Daten;
//! jeder fremde Text geht durch die Bausteine aus [`nexus_core::telegram::html`].

use chrono::{DateTime, Utc};
use nexus_core::board::inbox::types::InboxKind;
use nexus_core::board::question_types::{
    BoardQuestionDetail, BoardQuestionOption, BoardQuestionResearch,
};
use nexus_core::telegram::{bold, escape, CallbackAction};

use crate::telegram::action_views::{question, NOTHING_CHANGED};
use crate::telegram::api::{InlineButton, InlineKeyboard};
use crate::telegram::views::{shortened, since_text};

/// So viele Zeichen zeigt die Nachricht je Text (Beschreibung, Kontext der Recherche)
pub const QUESTION_TEXT_LENGTH: usize = 400;

/// So viele Zeichen zeigt eine Zeile zur Option (Titel, Vor- und Nachteile)
pub const OPTION_TEXT_LENGTH: usize = 160;

/// So viele Zeichen trägt die Beschriftung eines Options-Knopfes
pub const OPTION_BUTTON_LENGTH: usize = 40;

/// Marke an der empfohlenen Option
pub const RECOMMENDED: &str = "★ empfohlen";

/// Steht statt der Optionen, solange es keine Recherche gibt
pub const NO_RESEARCH: &str = "Noch keine Recherche – entscheide per Text oder starte eine.";

/// Antwort auf eine Entscheidung ohne Inhalt
pub const EMPTY_DECISION: &str = "Da war kein Text dabei – nichts entschieden.";

/// Antwort auf ein Verwerfen ohne Begründung; die Begründung ist Pflicht
pub const DISCARD_WITHOUT_REASON: &str =
    "Ohne Begründung lässt sich die Frage nicht verwerfen – nichts geändert.";

/// Hinweis an einem Knopf zu einer Frage, die nicht mehr offen ist
pub fn not_open(id: &str) -> String {
    format!("{id} ist nicht mehr offen{NOTHING_CHANGED}")
}

/// Hinweis an einem Knopf zu einer Option, die es in der neuesten Recherche nicht gibt
pub fn unknown_option(option: &str) -> String {
    format!("Die Option {option} steht nicht mehr zur Wahl{NOTHING_CHANGED}")
}

/// Die Nachricht zu einer offenen Frage samt ihren Knöpfen: Titel, Beschreibung und Kontext
/// gekürzt, dann je Option der neuesten Recherche eine Zeile und ein Knopf
pub fn question_message(
    detail: &BoardQuestionDetail,
    now: DateTime<Utc>,
) -> (String, InlineKeyboard) {
    let question = &detail.view.question;
    let mut out = bold(&format!(
        "{} · {}",
        InboxKind::Question.label(),
        question.id
    ));
    out.push_str(&format!("\n{}", escape(&question.title)));
    for text in [question.description.as_str()]
        .into_iter()
        .chain(latest(detail).map(|research| research.context.as_str()))
    {
        if !text.trim().is_empty() {
            out.push_str(&format!(
                "\n{}",
                escape(&shortened(text.trim(), QUESTION_TEXT_LENGTH))
            ));
        }
    }
    match latest(detail) {
        Some(research) if !research.options.is_empty() => {
            for option in &research.options {
                out.push_str(&option_lines(option));
            }
        }
        _ => out.push_str(&format!("\n{}", escape(NO_RESEARCH))),
    }
    out.push_str(&format!(
        "\n<i>{} · {}</i>",
        escape(&detail.view.project_key),
        since_text(&question.created_at, now)
    ));
    (out, question_keyboard(detail))
}

/// Die Knöpfe einer offenen Frage: je Option der neuesten Recherche einer, darunter Freitext,
/// Recherche und Verwerfen
pub fn question_keyboard(detail: &BoardQuestionDetail) -> InlineKeyboard {
    let id = &detail.view.question.id;
    let mut rows: Vec<Vec<InlineButton>> = options(detail)
        .iter()
        .map(|option| {
            vec![InlineButton::callback(
                option_button(option),
                CallbackAction::QuestionOption(id.clone(), option.id.clone()).encode(),
            )]
        })
        .collect();
    rows.push(vec![
        InlineButton::callback(
            "Eigener Text",
            CallbackAction::QuestionFree(id.clone()).encode(),
        ),
        InlineButton::callback(
            "Recherche starten",
            CallbackAction::QuestionResearch(id.clone()).encode(),
        ),
    ]);
    rows.push(vec![InlineButton::callback(
        "Verwerfen",
        CallbackAction::QuestionDiscard(id.clone()).encode(),
    )]);
    InlineKeyboard::new(rows)
}

/// Sicherheitsfrage vor der Entscheidung für eine Option samt „Ja“ und „Abbrechen“
pub fn option_question(id: &str, option: &BoardQuestionOption) -> (String, InlineKeyboard) {
    let (mut text, keyboard) = question(
        &format!("{id} wirklich mit {} entscheiden?", option.id),
        &format!("Ja, {}", option.id),
        CallbackAction::QuestionOptionConfirm(id.to_string(), option.id.clone()),
    );
    text.push_str(&format!(
        "\n{}",
        escape(&shortened(option.title.trim(), OPTION_TEXT_LENGTH))
    ));
    (text, keyboard)
}

/// Die Optionen der neuesten Recherche; ohne Recherche keine
pub fn options(detail: &BoardQuestionDetail) -> &[BoardQuestionOption] {
    latest(detail).map_or(&[], |research| research.options.as_slice())
}

/// Die neueste Recherche einer Frage; die Liste kommt in der Reihenfolge ihrer Entstehung
fn latest(detail: &BoardQuestionDetail) -> Option<&BoardQuestionResearch> {
    detail.research.last()
}

/// Die Zeilen einer Option: Nummer, Titel, Marke der Empfehlung, dann Vor- und Nachteile je
/// in einer Zeile
fn option_lines(option: &BoardQuestionOption) -> String {
    let mut out = format!(
        "\n{} {}",
        bold(&option.id),
        escape(&shortened(option.title.trim(), OPTION_TEXT_LENGTH))
    );
    if option.recommended {
        out.push_str(&format!(" {}", escape(RECOMMENDED)));
    }
    for (mark, texts) in [("+", &option.pros), ("−", &option.cons)] {
        let line = texts
            .iter()
            .map(|text| text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("; ");
        if !line.is_empty() {
            out.push_str(&format!(
                "\n{mark} {}",
                escape(&shortened(&line, OPTION_TEXT_LENGTH))
            ));
        }
    }
    out
}

/// Die Beschriftung des Knopfes einer Option; die empfohlene trägt den Stern vorn
fn option_button(option: &BoardQuestionOption) -> String {
    let star = match option.recommended {
        true => "★ ",
        false => "",
    };
    format!(
        "{star}{} · {}",
        option.id,
        shortened(option.title.trim(), OPTION_BUTTON_LENGTH)
    )
}

#[cfg(test)]
mod tests {
    use nexus_core::actors::AuthorKind;
    use nexus_core::board::question_types::{BoardQuestion, BoardQuestionView};
    use nexus_core::questions::QuestionState;

    use super::*;

    /// Ein Zeitpunkt für die Fußzeile
    fn now() -> DateTime<Utc> {
        "2026-11-03T12:34:56Z".parse().expect("Zeitpunkt")
    }

    /// Eine Option mit dieser Nummer
    fn option(id: &str, recommended: bool) -> BoardQuestionOption {
        BoardQuestionOption {
            id: id.into(),
            title: format!("Variante {id}"),
            text: "Langer Text".into(),
            pros: vec!["schnell".into(), "  ".into()],
            cons: vec!["teuer".into()],
            recommended,
        }
    }

    /// Eine offene Frage mit diesen Recherchen
    fn detail(research: Vec<Vec<BoardQuestionOption>>) -> BoardQuestionDetail {
        let question = BoardQuestion {
            id: "NXS-F0007".into(),
            project_id: 1,
            number: 7,
            title: "Welche Datenbank?".into(),
            description: "Es geht um <b>Last</b>".into(),
            state: QuestionState::Open,
            occasion: None,
            origin_kind: AuthorKind::Session,
            origin_name: "junior".into(),
            origin_session: None,
            tags: Vec::new(),
            ticket_ids: Vec::new(),
            depends_on: Vec::new(),
            decision: None,
            created_at: "2026-11-03T10:34:56Z".into(),
            created_by_id: "u".into(),
            updated_at: "2026-11-03T10:34:56Z".into(),
            rev: 1,
        };
        BoardQuestionDetail {
            research: research
                .into_iter()
                .enumerate()
                .map(|(index, options)| BoardQuestionResearch {
                    id: index as i64 + 1,
                    question_id: "NXS-F0007".into(),
                    at: "2026-11-03T11:00:00Z".into(),
                    by_id: "u".into(),
                    by_name: "junior".into(),
                    kind: AuthorKind::Session,
                    context: format!("Kontext {index}"),
                    options,
                })
                .collect(),
            comments: Vec::new(),
            history: Vec::new(),
            view: BoardQuestionView {
                question,
                project_key: "NXS".into(),
                tickets: Vec::new(),
                waiting_on: Vec::new(),
                ready: true,
                researched: true,
                option_count: 0,
            },
        }
    }

    #[test]
    fn die_nachricht_zeigt_titel_kontext_und_die_optionen_der_neuesten_recherche() {
        let frage = detail(vec![
            vec![option("O1", false)],
            vec![option("O2", false), option("O3", true)],
        ]);

        let (text, knoepfe) = question_message(&frage, now());

        assert!(text.starts_with("<b>Frage · NXS-F0007</b>"), "{text}");
        assert!(text.contains("\nWelche Datenbank?"), "{text}");
        assert!(
            text.contains("Es geht um &lt;b&gt;Last&lt;/b&gt;"),
            "{text}"
        );
        assert!(text.contains("Kontext 1"), "{text}");
        assert!(!text.contains("Kontext 0"), "nur die neueste: {text}");
        assert!(
            text.contains("<b>O2</b> Variante O2\n+ schnell\n− teuer"),
            "{text}"
        );
        assert!(
            text.contains(&format!("<b>O3</b> Variante O3 {RECOMMENDED}")),
            "{text}"
        );
        assert!(text.ends_with("<i>NXS · vor 2 h</i>"), "{text}");
        assert_eq!(
            knoepfe.inline_keyboard[0][0].callback_data.as_deref(),
            Some("qo:NXS-F0007:O2")
        );
        assert_eq!(knoepfe.inline_keyboard[0][0].text, "O2 · Variante O2");
        assert_eq!(knoepfe.inline_keyboard[1][0].text, "★ O3 · Variante O3");
    }

    #[test]
    fn ohne_recherche_bleiben_nur_text_recherche_und_verwerfen() {
        let (text, knoepfe) = question_message(&detail(Vec::new()), now());

        assert!(text.contains(NO_RESEARCH), "{text}");
        assert_eq!(
            knoepfe,
            InlineKeyboard::new(vec![
                vec![
                    InlineButton::callback("Eigener Text", "qf:NXS-F0007"),
                    InlineButton::callback("Recherche starten", "qr:NXS-F0007"),
                ],
                vec![InlineButton::callback("Verwerfen", "qd:NXS-F0007")],
            ])
        );
    }

    #[test]
    fn die_sicherheitsfrage_nennt_die_option_und_fragt_nach_ja() {
        let (text, knoepfe) = option_question("NXS-F0007", &option("O2", true));

        assert_eq!(
            text,
            "<b>NXS-F0007 wirklich mit O2 entscheiden?</b>\nVariante O2"
        );
        assert_eq!(
            knoepfe,
            InlineKeyboard::row(vec![
                InlineButton::callback("Ja, O2", "qoc:NXS-F0007:O2"),
                InlineButton::callback("Abbrechen", "x"),
            ])
        );
    }

    #[test]
    fn die_hinweise_nennen_die_frage_und_dass_nichts_geschehen_ist() {
        assert_eq!(
            not_open("NXS-F0007"),
            "NXS-F0007 ist nicht mehr offen – nichts geändert."
        );
        assert_eq!(
            unknown_option("O9"),
            "Die Option O9 steht nicht mehr zur Wahl – nichts geändert."
        );
    }
}
