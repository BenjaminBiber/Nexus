//! Der Fragebogen zu den Rückfragen eines Ideen-Entwurfs: die Fragen kommen einzeln, jede wird
//! per Knopf (eine oder mehrere Möglichkeiten), frei per Text oder durch Überspringen
//! beantwortet; „Zurück“ öffnet die vorige Frage wieder.
//!
//! Reine Logik ohne Telegram: der Bot hält den Stand am Entwurf (`drafts.json`) und zeigt
//! ihn an. Jeder Knopf trägt die Nummer seiner Frage; passt sie nicht zur aktuellen, ist der
//! Knopf überholt und ändert nichts. Sind alle Fragen beantwortet, macht
//! [`Questionnaire::answers_text`] daraus die Antwort für die Verfeinerungsrunde.

use serde::{Deserialize, Serialize};

use crate::telegram::idea::IdeaAsk;

/// Die Antwort auf eine Rückfrage
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum AskAnswer {
    /// Gewählte Möglichkeiten (Indizes, aufsteigend)
    Options(Vec<usize>),
    /// Freie Antwort per Text oder Sprache
    Free(String),
    /// Übersprungen – Claude entscheidet
    Skipped,
}

/// Was nach einer Antwort kommt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskStep {
    /// Die nächste (oder nach „Zurück“ die vorige) Frage ist dran
    Next,
    /// Alle Fragen sind beantwortet
    Done,
}

/// Warum ein Schritt nichts geändert hat
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskError {
    /// Der Knopf gehört zu einer anderen als der aktuellen Frage (oder alles ist beantwortet)
    Stale,
    /// Diese Möglichkeit gibt es an der Frage nicht
    UnknownOption,
    /// „Fertig“ ohne angekreuzte Möglichkeit
    NothingTicked,
    /// Freie Antwort ohne Text
    Empty,
}

/// Der Stand eines Fragebogens
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Questionnaire {
    /// Die Frage, die gerade dran ist (0-basiert); gleich der Zahl der Fragen, wenn alle
    /// beantwortet sind
    pub current: usize,
    /// Je Frage die Antwort, soweit gegeben
    pub answers: Vec<Option<AskAnswer>>,
    /// Bei Mehrfachauswahl: die angekreuzten Möglichkeiten der aktuellen Frage
    pub ticked: Vec<usize>,
}

impl Questionnaire {
    /// Ein neuer Fragebogen über so viele Fragen
    pub fn new(count: usize) -> Self {
        Self {
            current: 0,
            answers: vec![None; count],
            ticked: Vec::new(),
        }
    }

    /// Sind alle Fragen beantwortet?
    pub fn is_done(&self) -> bool {
        self.current >= self.answers.len()
    }

    /// Eine Möglichkeit wählen (Frage ohne Mehrfachauswahl)
    pub fn choose(
        &mut self,
        asks: &[IdeaAsk],
        question: usize,
        option: usize,
    ) -> Result<AskStep, AskError> {
        let ask = self.at(asks, question)?;
        if option >= ask.options.len() {
            return Err(AskError::UnknownOption);
        }
        Ok(self.answer(asks, AskAnswer::Options(vec![option])))
    }

    /// Eine Möglichkeit an- oder abhaken (Mehrfachauswahl); die Frage bleibt dran
    pub fn toggle(
        &mut self,
        asks: &[IdeaAsk],
        question: usize,
        option: usize,
    ) -> Result<(), AskError> {
        let ask = self.at(asks, question)?;
        if option >= ask.options.len() {
            return Err(AskError::UnknownOption);
        }
        match self.ticked.iter().position(|found| *found == option) {
            Some(index) => {
                self.ticked.remove(index);
            }
            None => {
                self.ticked.push(option);
                self.ticked.sort_unstable();
            }
        }
        Ok(())
    }

    /// „Fertig“ bei Mehrfachauswahl: die Häkchen werden die Antwort
    pub fn confirm(&mut self, asks: &[IdeaAsk], question: usize) -> Result<AskStep, AskError> {
        self.at(asks, question)?;
        if self.ticked.is_empty() {
            return Err(AskError::NothingTicked);
        }
        let ticked = std::mem::take(&mut self.ticked);
        Ok(self.answer(asks, AskAnswer::Options(ticked)))
    }

    /// Eine freie Antwort auf die aktuelle Frage
    pub fn free(&mut self, asks: &[IdeaAsk], text: &str) -> Result<AskStep, AskError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(AskError::Empty);
        }
        let current = self.current;
        self.at(asks, current)?;
        Ok(self.answer(asks, AskAnswer::Free(text.to_string())))
    }

    /// Die aktuelle Frage überspringen: Claude entscheidet
    pub fn skip(&mut self, asks: &[IdeaAsk], question: usize) -> Result<AskStep, AskError> {
        self.at(asks, question)?;
        Ok(self.answer(asks, AskAnswer::Skipped))
    }

    /// Zur vorigen Frage zurück; ihre Antwort bleibt stehen, bis sie neu beantwortet ist
    pub fn back(&mut self, asks: &[IdeaAsk], question: usize) -> Result<(), AskError> {
        self.at(asks, question)?;
        if self.current == 0 {
            return Err(AskError::Stale);
        }
        self.enter(asks, self.current - 1);
        Ok(())
    }

    /// Die Antwort auf diese Frage in Worten (für die Anzeige und den Prompt)
    pub fn answer_text(&self, asks: &[IdeaAsk], question: usize) -> Option<String> {
        let ask = asks.get(question)?;
        Some(match self.answers.get(question)?.as_ref()? {
            AskAnswer::Options(chosen) => chosen
                .iter()
                .filter_map(|index| ask.options.get(*index))
                .map(|option| option.label.clone())
                .collect::<Vec<_>>()
                .join(", "),
            AskAnswer::Free(text) => text.clone(),
            AskAnswer::Skipped => SKIPPED.to_string(),
        })
    }

    /// Alle Antworten als Liste „Frage → Antwort“ für die Verfeinerungsrunde
    pub fn answers_text(&self, asks: &[IdeaAsk]) -> String {
        let mut out = String::from("Antworten auf die Rückfragen:");
        for (index, ask) in asks.iter().enumerate() {
            let answer = self
                .answer_text(asks, index)
                .unwrap_or_else(|| SKIPPED.to_string());
            out.push_str(&format!("\n- {} → {answer}", ask.text));
        }
        out
    }

    /// Die Frage, wenn der Knopf zur aktuellen gehört
    fn at<'a>(&self, asks: &'a [IdeaAsk], question: usize) -> Result<&'a IdeaAsk, AskError> {
        if question != self.current || self.is_done() {
            return Err(AskError::Stale);
        }
        asks.get(question).ok_or(AskError::Stale)
    }

    /// Speichert die Antwort auf die aktuelle Frage und geht weiter
    fn answer(&mut self, asks: &[IdeaAsk], answer: AskAnswer) -> AskStep {
        if let Some(slot) = self.answers.get_mut(self.current) {
            *slot = Some(answer);
        }
        self.enter(asks, self.current + 1);
        match self.is_done() {
            true => AskStep::Done,
            false => AskStep::Next,
        }
    }

    /// Macht diese Frage zur aktuellen; bei Mehrfachauswahl stehen die Häkchen einer früheren
    /// Antwort wieder da
    fn enter(&mut self, asks: &[IdeaAsk], question: usize) {
        self.current = question;
        self.ticked = match (asks.get(question), self.answers.get(question)) {
            (Some(ask), Some(Some(AskAnswer::Options(chosen)))) if ask.multi_select => {
                chosen.clone()
            }
            _ => Vec::new(),
        };
    }
}

/// So steht eine übersprungene Frage in der Anzeige und im Prompt
pub const SKIPPED: &str = "keine Vorgabe – entscheide selbst";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telegram::idea::IdeaAskOption;

    /// Eine Frage mit diesen Möglichkeiten
    fn ask(text: &str, options: &[&str], multi_select: bool) -> IdeaAsk {
        IdeaAsk {
            options: options
                .iter()
                .map(|label| IdeaAskOption {
                    label: label.to_string(),
                    detail: None,
                })
                .collect(),
            multi_select,
            ..IdeaAsk::open(text)
        }
    }

    /// Engine (Wahl), Sprachen (Mehrfachauswahl), Ort (frei)
    fn asks() -> Vec<IdeaAsk> {
        vec![
            ask(
                "Welche Engine?",
                &["faster-whisper", "Vosk", "anderes"],
                false,
            ),
            ask("Welche Sprachen?", &["Deutsch", "Englisch"], true),
            ask("Wo soll die Datei liegen?", &[], false),
        ]
    }

    #[test]
    fn geht_die_fragen_nacheinander_durch() {
        let asks = asks();
        let mut bogen = Questionnaire::new(asks.len());

        assert_eq!(bogen.choose(&asks, 0, 0), Ok(AskStep::Next));
        assert_eq!(bogen.current, 1);
        assert_eq!(bogen.toggle(&asks, 1, 1), Ok(()));
        assert_eq!(bogen.toggle(&asks, 1, 0), Ok(()));
        assert_eq!(bogen.ticked, [0, 1]);
        assert_eq!(bogen.confirm(&asks, 1), Ok(AskStep::Next));
        assert!(bogen.ticked.is_empty());
        assert_eq!(bogen.free(&asks, "  deploy/  "), Ok(AskStep::Done));
        assert!(bogen.is_done());

        assert_eq!(
            bogen.answers_text(&asks),
            "Antworten auf die Rückfragen:\n\
             - Welche Engine? → faster-whisper\n\
             - Welche Sprachen? → Deutsch, Englisch\n\
             - Wo soll die Datei liegen? → deploy/"
        );
    }

    #[test]
    fn ueberspringen_laesst_claude_entscheiden() {
        let asks = asks();
        let mut bogen = Questionnaire::new(asks.len());

        bogen.skip(&asks, 0).unwrap();

        assert_eq!(bogen.answer_text(&asks, 0).as_deref(), Some(SKIPPED));
        assert_eq!(bogen.answer_text(&asks, 1), None, "noch nicht beantwortet");
    }

    #[test]
    fn ein_knopf_einer_anderen_frage_aendert_nichts() {
        let asks = asks();
        let mut bogen = Questionnaire::new(asks.len());
        bogen.choose(&asks, 0, 1).unwrap();

        assert_eq!(bogen.choose(&asks, 0, 0), Err(AskError::Stale));
        assert_eq!(bogen.skip(&asks, 2), Err(AskError::Stale));
        assert_eq!(bogen.answer_text(&asks, 0).as_deref(), Some("Vosk"));
        assert_eq!(bogen.current, 1);
    }

    #[test]
    fn unbekannte_moeglichkeit_und_fertig_ohne_haekchen_werden_abgelehnt() {
        let asks = asks();
        let mut bogen = Questionnaire::new(asks.len());

        assert_eq!(bogen.choose(&asks, 0, 7), Err(AskError::UnknownOption));
        bogen.choose(&asks, 0, 0).unwrap();
        assert_eq!(bogen.confirm(&asks, 1), Err(AskError::NothingTicked));
        assert_eq!(bogen.toggle(&asks, 1, 5), Err(AskError::UnknownOption));
        assert_eq!(bogen.free(&asks, "   "), Err(AskError::Empty));
        assert_eq!(bogen.current, 1);
    }

    #[test]
    fn zurueck_oeffnet_die_vorige_frage_mit_ihren_haekchen() {
        let asks = asks();
        let mut bogen = Questionnaire::new(asks.len());

        assert_eq!(
            bogen.back(&asks, 0),
            Err(AskError::Stale),
            "vor Frage 1 gibt es nichts"
        );
        bogen.choose(&asks, 0, 0).unwrap();
        bogen.toggle(&asks, 1, 1).unwrap();
        bogen.confirm(&asks, 1).unwrap();

        bogen.back(&asks, 2).unwrap();

        assert_eq!(bogen.current, 1);
        assert_eq!(bogen.ticked, [1], "die frühere Wahl ist wieder angekreuzt");
        bogen.back(&asks, 1).unwrap();
        assert_eq!(bogen.current, 0);
        assert!(bogen.ticked.is_empty());
        assert_eq!(bogen.choose(&asks, 0, 2), Ok(AskStep::Next));
        assert_eq!(bogen.answer_text(&asks, 0).as_deref(), Some("anderes"));
        assert_eq!(
            bogen.answer_text(&asks, 1).as_deref(),
            Some("Englisch"),
            "spätere Antworten bleiben"
        );
    }

    #[test]
    fn nach_der_letzten_frage_ist_jeder_knopf_ueberholt() {
        let asks = vec![ask("Nur eine?", &["ja", "nein"], false)];
        let mut bogen = Questionnaire::new(1);
        assert_eq!(bogen.choose(&asks, 0, 0), Ok(AskStep::Done));

        assert_eq!(bogen.choose(&asks, 0, 1), Err(AskError::Stale));
        assert_eq!(bogen.free(&asks, "doch nein"), Err(AskError::Stale));
        assert_eq!(bogen.back(&asks, 1), Err(AskError::Stale));
    }

    #[test]
    fn der_stand_uebersteht_die_rundreise_durch_json() {
        let asks = asks();
        let mut bogen = Questionnaire::new(asks.len());
        bogen.free(&asks, "faster-whisper large").unwrap();
        bogen.toggle(&asks, 1, 0).unwrap();

        let json = serde_json::to_value(&bogen).unwrap();
        let zurueck: Questionnaire = serde_json::from_value(json).unwrap();

        assert_eq!(zurueck, bogen);
    }
}
