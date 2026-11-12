//! Kompakte Kodierung der Knopf-Aktionen für Telegram-Callback-Daten (höchstens 64 Byte,
//! siehe [`MAX_CALLBACK_LEN`]).
//!
//! Format `<kurzcode>[:<feld>[:<feld>]]`, z. B. `acc:NXS-0042`, `qo:NXS-F0007:O2`. Jeder
//! Kurzcode steht für genau eine Aktion mit fester Zahl an Feldern; unbekannte Kurzcodes,
//! eine falsche Zahl an Feldern oder ein Feld, das nicht zum erwarteten Typ passt, ergeben
//! beim Dekodieren `None` – ein alter oder manipulierter Knopf löst dann nichts aus.

use crate::board::inbox::types::InboxKind;
use crate::telegram::config::IdeaTarget;

/// Höchstlänge von Telegram-Callback-Daten in Byte
pub const MAX_CALLBACK_LEN: usize = 64;

/// Eine per Knopf ausgelöste Aktion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackAction {
    /// Ticket aus der Abnahme übernehmen (verlangt die Sicherheitsfrage [`AcceptConfirm`])
    ///
    /// [`AcceptConfirm`]: CallbackAction::AcceptConfirm
    Accept(String),
    /// Übernehmen nach der Sicherheitsfrage wirklich ausführen
    AcceptConfirm(String),
    /// Ticket zurück an den Agenten geben
    Return(String),
    /// Gescheiterten Merge erneut versuchen (verlangt die Sicherheitsfrage [`MergeConfirm`])
    ///
    /// [`MergeConfirm`]: CallbackAction::MergeConfirm
    Merge(String),
    /// Erneuten Merge nach der Sicherheitsfrage wirklich ausführen
    MergeConfirm(String),
    /// Übersicht der geänderten Dateien eines Tickets
    Diff(String),
    /// Blockierungsgrund eines Tickets anzeigen
    BlockedReason(String),
    /// Kommentar zu einem Ticket schreiben (Folgedialog mit Text/Sprache)
    Comment(String),
    /// Ticket neu planen lassen
    Replan(String),
    /// Plan eines Tickets anzeigen
    Plan(String),
    /// Notiz in ein Ticket umwandeln
    NoteToTicket(i32),
    /// Notiz verwerfen
    NoteDrop(i32),
    /// Eine Option einer Frage wählen (verlangt die Sicherheitsfrage [`QuestionOptionConfirm`])
    ///
    /// [`QuestionOptionConfirm`]: CallbackAction::QuestionOptionConfirm
    QuestionOption(String, String),
    /// Gewählte Option nach der Sicherheitsfrage wirklich entscheiden
    QuestionOptionConfirm(String, String),
    /// Frage per Freitext entscheiden (Folgedialog)
    QuestionFree(String),
    /// Recherche zu einer Frage starten
    QuestionResearch(String),
    /// Frage verwerfen (verlangt eine Begründung, Folgedialog)
    QuestionDiscard(String),
    /// Ideen-Entwurf anlegen
    IdeaCreate(String),
    /// Ziel eines Ideen-Entwurfs umschalten (Notiz/Ticket)
    IdeaTarget(String, IdeaTarget),
    /// Andere Boards zur Auswahl anzeigen
    IdeaBoardList(String),
    /// Board eines Ideen-Entwurfs setzen
    IdeaBoard(String, i32),
    /// Ideen-Entwurf korrigieren (Folgedialog mit Text/Sprache)
    IdeaEdit(String),
    /// Ideen-Entwurf verwerfen
    IdeaDrop(String),
    /// Vorhandenen offenen Entwurf durch einen neuen ersetzen
    IdeaReplace(String),
    /// Rückfrage: Möglichkeit wählen (Entwurf, Frage, Möglichkeit)
    IdeaAskChoose(String, usize, usize),
    /// Rückfrage mit Mehrfachauswahl: Möglichkeit an-/abhaken (Entwurf, Frage, Möglichkeit)
    IdeaAskToggle(String, usize, usize),
    /// Rückfrage mit Mehrfachauswahl abschließen; mit der Fragenzahl als Nummer startet es die
    /// gescheiterte Verfeinerungsrunde neu
    IdeaAskDone(String, usize),
    /// Rückfrage überspringen
    IdeaAskSkip(String, usize),
    /// Zurück zur vorigen Rückfrage
    IdeaAskBack(String, usize),
    /// Rückfragen abbrechen und den Entwurf verwerfen
    IdeaAskCancel(String),
    /// Aktives Board umschalten
    Board(i32),
    /// Weitere Einträge einer Art aus „Meine Aufgaben“ (Seite ab 2)
    More(InboxKind, u32),
    /// Vorgang abbrechen, Knöpfe verschwinden ohne Aktion
    Cancel,
}

impl CallbackAction {
    /// Kurze Kodierung für `callback_data`; immer höchstens [`MAX_CALLBACK_LEN`] Byte
    pub fn encode(&self) -> String {
        match self {
            CallbackAction::Accept(ticket) => format!("acc:{ticket}"),
            CallbackAction::AcceptConfirm(ticket) => format!("accc:{ticket}"),
            CallbackAction::Return(ticket) => format!("ret:{ticket}"),
            CallbackAction::Merge(ticket) => format!("mrg:{ticket}"),
            CallbackAction::MergeConfirm(ticket) => format!("mrgc:{ticket}"),
            CallbackAction::Diff(ticket) => format!("dif:{ticket}"),
            CallbackAction::BlockedReason(ticket) => format!("blk:{ticket}"),
            CallbackAction::Comment(ticket) => format!("cmt:{ticket}"),
            CallbackAction::Replan(ticket) => format!("rpl:{ticket}"),
            CallbackAction::Plan(ticket) => format!("pln:{ticket}"),
            CallbackAction::NoteToTicket(note_id) => format!("ntt:{note_id}"),
            CallbackAction::NoteDrop(note_id) => format!("ndr:{note_id}"),
            CallbackAction::QuestionOption(question, option) => {
                format!("qo:{question}:{option}")
            }
            CallbackAction::QuestionOptionConfirm(question, option) => {
                format!("qoc:{question}:{option}")
            }
            CallbackAction::QuestionFree(question) => format!("qf:{question}"),
            CallbackAction::QuestionResearch(question) => format!("qr:{question}"),
            CallbackAction::QuestionDiscard(question) => format!("qd:{question}"),
            CallbackAction::IdeaCreate(draft) => format!("icr:{draft}"),
            CallbackAction::IdeaTarget(draft, kind) => {
                format!("itg:{draft}:{}", idea_target_code(*kind))
            }
            CallbackAction::IdeaBoardList(draft) => format!("ibl:{draft}"),
            CallbackAction::IdeaBoard(draft, board_id) => format!("ibd:{draft}:{board_id}"),
            CallbackAction::IdeaEdit(draft) => format!("ied:{draft}"),
            CallbackAction::IdeaDrop(draft) => format!("idp:{draft}"),
            CallbackAction::IdeaReplace(draft) => format!("irp:{draft}"),
            CallbackAction::IdeaAskChoose(draft, question, option) => {
                format!("iqa:{draft}:{question}:{option}")
            }
            CallbackAction::IdeaAskToggle(draft, question, option) => {
                format!("iqt:{draft}:{question}:{option}")
            }
            CallbackAction::IdeaAskDone(draft, question) => format!("iqd:{draft}:{question}"),
            CallbackAction::IdeaAskSkip(draft, question) => format!("iqs:{draft}:{question}"),
            CallbackAction::IdeaAskBack(draft, question) => format!("iqb:{draft}:{question}"),
            CallbackAction::IdeaAskCancel(draft) => format!("iqx:{draft}"),
            CallbackAction::Board(board_id) => format!("brd:{board_id}"),
            CallbackAction::More(kind, page) => format!("mo:{}:{page}", kind.as_str()),
            CallbackAction::Cancel => "x".to_string(),
        }
    }

    /// Zerlegt Callback-Daten; `None` bei unbekanntem Kurzcode, falscher Feldzahl oder einem
    /// Feld, das nicht zum erwarteten Typ passt
    pub fn decode(data: &str) -> Option<CallbackAction> {
        let mut felder = data.split(':');
        let code = felder.next()?;
        match code {
            "acc" => Some(CallbackAction::Accept(single(felder)?)),
            "accc" => Some(CallbackAction::AcceptConfirm(single(felder)?)),
            "ret" => Some(CallbackAction::Return(single(felder)?)),
            "mrg" => Some(CallbackAction::Merge(single(felder)?)),
            "mrgc" => Some(CallbackAction::MergeConfirm(single(felder)?)),
            "dif" => Some(CallbackAction::Diff(single(felder)?)),
            "blk" => Some(CallbackAction::BlockedReason(single(felder)?)),
            "cmt" => Some(CallbackAction::Comment(single(felder)?)),
            "rpl" => Some(CallbackAction::Replan(single(felder)?)),
            "pln" => Some(CallbackAction::Plan(single(felder)?)),
            "ntt" => Some(CallbackAction::NoteToTicket(single_i32(felder)?)),
            "ndr" => Some(CallbackAction::NoteDrop(single_i32(felder)?)),
            "qo" => {
                let (question, option) = pair(felder)?;
                Some(CallbackAction::QuestionOption(question, option))
            }
            "qoc" => {
                let (question, option) = pair(felder)?;
                Some(CallbackAction::QuestionOptionConfirm(question, option))
            }
            "qf" => Some(CallbackAction::QuestionFree(single(felder)?)),
            "qr" => Some(CallbackAction::QuestionResearch(single(felder)?)),
            "qd" => Some(CallbackAction::QuestionDiscard(single(felder)?)),
            "icr" => Some(CallbackAction::IdeaCreate(single(felder)?)),
            "itg" => {
                let (draft, kind) = pair(felder)?;
                Some(CallbackAction::IdeaTarget(
                    draft,
                    idea_target_from_code(&kind)?,
                ))
            }
            "ibl" => Some(CallbackAction::IdeaBoardList(single(felder)?)),
            "ibd" => {
                let (draft, board_id) = pair(felder)?;
                Some(CallbackAction::IdeaBoard(draft, board_id.parse().ok()?))
            }
            "ied" => Some(CallbackAction::IdeaEdit(single(felder)?)),
            "idp" => Some(CallbackAction::IdeaDrop(single(felder)?)),
            "irp" => Some(CallbackAction::IdeaReplace(single(felder)?)),
            "iqa" => {
                let (draft, question, option) = triple(felder)?;
                Some(CallbackAction::IdeaAskChoose(
                    draft,
                    question.parse().ok()?,
                    option.parse().ok()?,
                ))
            }
            "iqt" => {
                let (draft, question, option) = triple(felder)?;
                Some(CallbackAction::IdeaAskToggle(
                    draft,
                    question.parse().ok()?,
                    option.parse().ok()?,
                ))
            }
            "iqd" => {
                let (draft, question) = pair(felder)?;
                Some(CallbackAction::IdeaAskDone(draft, question.parse().ok()?))
            }
            "iqs" => {
                let (draft, question) = pair(felder)?;
                Some(CallbackAction::IdeaAskSkip(draft, question.parse().ok()?))
            }
            "iqb" => {
                let (draft, question) = pair(felder)?;
                Some(CallbackAction::IdeaAskBack(draft, question.parse().ok()?))
            }
            "iqx" => Some(CallbackAction::IdeaAskCancel(single(felder)?)),
            "brd" => Some(CallbackAction::Board(single_i32(felder)?)),
            "mo" => {
                let (kind, page) = pair(felder)?;
                Some(CallbackAction::More(
                    inbox_kind_from_str(&kind)?,
                    page.parse().ok()?,
                ))
            }
            "x" if felder.next().is_none() => Some(CallbackAction::Cancel),
            _ => None,
        }
    }
}

/// Genau ein restliches, nicht leeres Feld, sonst `None`
fn single(mut felder: std::str::Split<'_, char>) -> Option<String> {
    let wert = felder.next()?;
    if wert.is_empty() || felder.next().is_some() {
        return None;
    }
    Some(wert.to_string())
}

/// Genau ein restliches Feld als `i32`, sonst `None`
fn single_i32(felder: std::str::Split<'_, char>) -> Option<i32> {
    single(felder)?.parse().ok()
}

/// Genau zwei restliche, nicht leere Felder, sonst `None`
fn pair(mut felder: std::str::Split<'_, char>) -> Option<(String, String)> {
    let a = felder.next()?;
    let b = felder.next()?;
    if a.is_empty() || b.is_empty() || felder.next().is_some() {
        return None;
    }
    Some((a.to_string(), b.to_string()))
}

/// Genau drei restliche, nicht leere Felder, sonst `None`
fn triple(mut felder: std::str::Split<'_, char>) -> Option<(String, String, String)> {
    let a = felder.next()?;
    let b = felder.next()?;
    let c = felder.next()?;
    if a.is_empty() || b.is_empty() || c.is_empty() || felder.next().is_some() {
        return None;
    }
    Some((a.to_string(), b.to_string(), c.to_string()))
}

/// Kurzcode für das Ziel eines Ideen-Entwurfs
fn idea_target_code(kind: IdeaTarget) -> &'static str {
    match kind {
        IdeaTarget::Note => "n",
        IdeaTarget::Ticket => "t",
    }
}

/// Ziel eines Ideen-Entwurfs aus seinem Kurzcode
fn idea_target_from_code(code: &str) -> Option<IdeaTarget> {
    match code {
        "n" => Some(IdeaTarget::Note),
        "t" => Some(IdeaTarget::Ticket),
        _ => None,
    }
}

/// Art aus „Meine Aufgaben“ anhand ihrer Schreibweise in JSON (`as_str`)
fn inbox_kind_from_str(value: &str) -> Option<InboxKind> {
    InboxKind::ALL
        .into_iter()
        .find(|kind| kind.as_str() == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Längster Schlüssel (6 Zeichen) mit größter Nummer (`u32::MAX`, 10 Ziffern): 17 Zeichen
    const TICKET_MAX: &str = "ABCDEF-4294967295";
    /// Wie `TICKET_MAX`, mit zusätzlichem „F“: 18 Zeichen
    const QUESTION_MAX: &str = "ABCDEF-F4294967295";
    /// Vorgabe für Entwurfs-IDs laut Plan: kurz, z. B. 8 Zeichen
    const DRAFT_MAX: &str = "d1234567";
    /// Höchste Optionsnummer einer Recherche (`MAX_OPTIONS` in `questions::research_validator`)
    const OPTION_MAX: &str = "O5";

    /// Jede Variante mit den längsten vorkommenden IDs, für die Rundreise und die Längenprobe
    fn alle_mit_laengsten_ids() -> Vec<CallbackAction> {
        vec![
            CallbackAction::Accept(TICKET_MAX.into()),
            CallbackAction::AcceptConfirm(TICKET_MAX.into()),
            CallbackAction::Return(TICKET_MAX.into()),
            CallbackAction::Merge(TICKET_MAX.into()),
            CallbackAction::MergeConfirm(TICKET_MAX.into()),
            CallbackAction::Diff(TICKET_MAX.into()),
            CallbackAction::BlockedReason(TICKET_MAX.into()),
            CallbackAction::Comment(TICKET_MAX.into()),
            CallbackAction::Replan(TICKET_MAX.into()),
            CallbackAction::Plan(TICKET_MAX.into()),
            CallbackAction::NoteToTicket(i32::MAX),
            CallbackAction::NoteDrop(i32::MAX),
            CallbackAction::QuestionOption(QUESTION_MAX.into(), OPTION_MAX.into()),
            CallbackAction::QuestionOptionConfirm(QUESTION_MAX.into(), OPTION_MAX.into()),
            CallbackAction::QuestionFree(QUESTION_MAX.into()),
            CallbackAction::QuestionResearch(QUESTION_MAX.into()),
            CallbackAction::QuestionDiscard(QUESTION_MAX.into()),
            CallbackAction::IdeaCreate(DRAFT_MAX.into()),
            CallbackAction::IdeaTarget(DRAFT_MAX.into(), IdeaTarget::Note),
            CallbackAction::IdeaTarget(DRAFT_MAX.into(), IdeaTarget::Ticket),
            CallbackAction::IdeaBoardList(DRAFT_MAX.into()),
            CallbackAction::IdeaBoard(DRAFT_MAX.into(), i32::MAX),
            CallbackAction::IdeaEdit(DRAFT_MAX.into()),
            CallbackAction::IdeaDrop(DRAFT_MAX.into()),
            CallbackAction::IdeaReplace(DRAFT_MAX.into()),
            CallbackAction::IdeaAskChoose(DRAFT_MAX.into(), 2, 3),
            CallbackAction::IdeaAskToggle(DRAFT_MAX.into(), 2, 3),
            CallbackAction::IdeaAskDone(DRAFT_MAX.into(), 3),
            CallbackAction::IdeaAskSkip(DRAFT_MAX.into(), 2),
            CallbackAction::IdeaAskBack(DRAFT_MAX.into(), 2),
            CallbackAction::IdeaAskCancel(DRAFT_MAX.into()),
            CallbackAction::Board(i32::MAX),
            CallbackAction::More(InboxKind::Planning, u32::MAX),
            CallbackAction::Cancel,
        ]
    }

    #[test]
    fn kodiert_wie_im_plan_vorgegeben() {
        assert_eq!(
            CallbackAction::Accept("NXS-0042".into()).encode(),
            "acc:NXS-0042"
        );
        assert_eq!(
            CallbackAction::QuestionOption("NXS-F0007".into(), "O2".into()).encode(),
            "qo:NXS-F0007:O2"
        );
    }

    #[test]
    fn rundreise_mit_den_laengsten_ids_bleibt_unter_der_grenze() {
        for aktion in alle_mit_laengsten_ids() {
            let kodiert = aktion.encode();
            assert!(
                kodiert.len() <= MAX_CALLBACK_LEN,
                "{kodiert} ist {} Byte lang",
                kodiert.len()
            );
            assert_eq!(
                CallbackAction::decode(&kodiert),
                Some(aktion.clone()),
                "Rundreise für {kodiert}"
            );
        }
    }

    #[test]
    fn jede_art_aus_meine_aufgaben_kodiert_und_dekodiert() {
        for art in InboxKind::ALL {
            let aktion = CallbackAction::More(art, 3);
            assert_eq!(CallbackAction::decode(&aktion.encode()), Some(aktion));
        }
    }

    #[test]
    fn unbekannte_oder_kaputte_daten_ergeben_none() {
        for kaputt in [
            "",
            "quatsch",
            "quatsch:NXS-0042",
            "acc",
            "acc:",
            "acc:NXS-0042:zuviel",
            "ntt:keine-zahl",
            "ntt:",
            "qo:NXS-F0007",
            "qo:NXS-F0007:O2:zuviel",
            "itg:d1234567:falsch",
            "iqa:d1234567:0",
            "iqa:d1234567:x:1",
            "iqa:d1234567:0:1:zuviel",
            "iqd:d1234567:-1",
            "iqx:d1234567:0",
            "mo:kaputt:1",
            "mo:note:keine-zahl",
            "x:zuviel",
        ] {
            assert_eq!(CallbackAction::decode(kaputt), None, "{kaputt}");
        }
    }
}
