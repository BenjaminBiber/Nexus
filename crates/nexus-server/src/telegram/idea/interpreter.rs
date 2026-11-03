//! Versteht eine hingeworfene Idee: sammelt den Kontext der eigenen Boards, lässt Claude
//! daraus einen Entwurf machen und prüft die Antwort im Kern.
//!
//! Der Claude-Aufruf ist derselbe wie bei den Docs-Diensten
//! ([`crate::docs::docs_claude`]): ein Durchgang, JSON-Ausgabe, keine schreibenden Werkzeuge,
//! Account mit der meisten freien Kapazität, fünf Minuten Zeitlimit. Je Chat läuft höchstens
//! ein Lauf; ein zweiter ist ein Konflikt und der laufende lässt sich abbrechen.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use nexus_core::board::basics::{NoteState, TicketStatus};
use nexus_core::board::note_similarity::{note_title, NoteSimilarity};
use nexus_core::board::question_service::BoardQuestionListFilter;
use nexus_core::board::ticket_service::TicketListFilter;
use nexus_core::board::types::Note;
use nexus_core::questions::QuestionState;
use nexus_core::telegram::idea::{
    build_prompt, IdeaBoard, IdeaContext, IdeaDraft, IdeaDraftCheck, IdeaDraftValidator, IdeaNote,
    IdeaQuestion, IdeaRefinement, IdeaTicket,
};
use nexus_core::{HarnessError, HarnessHome, Result};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::claude_runner::ClaudeRunner;
use crate::dispatch::result_envelope::{excerpt, ClaudeResultEnvelope};
use crate::docs::docs_claude::{config, run_input_with_env, TIMEOUT};

/// Was ein Lauf braucht: der Chat, die neue Nachricht, das aktive Board und – in einer
/// Verfeinerungsrunde – der Entwurf, auf den die Nachricht antwortet
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IdeaRequest {
    /// Chat des Bots; je Chat läuft höchstens ein Lauf
    pub chat_id: i64,
    /// Die neue Nachricht: die erste Idee oder die Antwort auf den offenen Entwurf
    pub input: String,
    /// Board, auf dem der Benutzer gerade arbeitet (steht im Kontext vorn)
    pub active_board_id: Option<i32>,
    /// Bisheriger Entwurf; gesetzt = Verfeinerungsrunde
    pub previous: Option<IdeaDraft>,
}

/// Sammelt den Board-Kontext und lässt Claude daraus einen Entwurf machen.
#[derive(Debug)]
pub struct IdeaInterpreter {
    home: HarnessHome,
    /// Umgebung, die der Lauf erbt (im Betrieb die des Servers)
    env: BTreeMap<String, String>,
    /// Laufender Lauf je Chat; über sein Abbruch-Signal lässt er sich beenden
    running: Mutex<HashMap<i64, CancellationToken>>,
    /// Zeitlimit eines Laufs (im Betrieb [`TIMEOUT`])
    timeout: Duration,
    validator: IdeaDraftValidator,
}

impl IdeaInterpreter {
    /// Interpreter im Benutzerordner (Einstellung und Arbeitsordner der Analyse) mit der
    /// Umgebung des Servers
    pub fn new(home: HarnessHome) -> Self {
        Self::with_env(home, std::env::vars().collect())
    }

    /// Wie [`IdeaInterpreter::new`], mit eigener Umgebung für den Lauf (Tests)
    pub fn with_env(home: HarnessHome, env: BTreeMap<String, String>) -> Self {
        Self {
            home,
            env,
            running: Mutex::new(HashMap::new()),
            timeout: TIMEOUT,
            validator: IdeaDraftValidator::new(),
        }
    }

    /// Derselbe Interpreter mit einem kürzeren Zeitlimit (Tests des Zeitablaufs)
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Versteht die Idee: Kontext sammeln, Claude rufen, Antwort prüfen. Je Chat läuft
    /// höchstens ein Lauf.
    pub async fn interpret(
        &self,
        claude: &dyn ClaudeRunner,
        boards: &BoardRuntimeHolder,
        request: &IdeaRequest,
    ) -> Result<IdeaDraft> {
        self.interpret_checked(claude, boards, request)
            .await
            .map(|check| check.draft)
    }

    /// Wie [`IdeaInterpreter::interpret`], liefert dazu die Hinweise des Prüfers (was er am
    /// Entwurf zurechtgebogen hat)
    pub async fn interpret_checked(
        &self,
        claude: &dyn ClaudeRunner,
        boards: &BoardRuntimeHolder,
        request: &IdeaRequest,
    ) -> Result<IdeaDraftCheck> {
        if request.input.trim().is_empty() {
            return Err(HarnessError::new("invalid-input", "Die Idee ist leer.").into());
        }
        // Für die ähnlichen Notizen zählt alles, was bisher gesagt wurde
        let context = self
            .context(boards, request.active_board_id, &self.story(request))
            .await?;
        let cancel = self.start(request.chat_id)?;
        let made = self.work(claude, &context, request, &cancel).await;
        self.finish(request.chat_id, &cancel);
        made
    }

    /// Bricht den laufenden Lauf des Chats ab; `false`, wenn keiner läuft
    pub fn cancel(&self, chat_id: i64) -> bool {
        let mut running = self.running.lock().expect("laufende Ideen");
        match running.remove(&chat_id) {
            Some(cancel) => {
                cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// Der Kontext der eigenen Boards: Boards mit Repo-Pfad, offene Tickets, offene Notizen
    /// (ähnliche gekennzeichnet) und offene Fragen
    pub async fn context(
        &self,
        boards: &BoardRuntimeHolder,
        active_board_id: Option<i32>,
        input: &str,
    ) -> Result<IdeaContext> {
        let input = input.to_string();
        boards
            .run(|backend| async move {
                let me = backend.actor(None).user_id;
                let services = backend.services();
                let own = services.boards.owned(&me).await?;
                let board_ids: Vec<i32> = own.iter().map(|found| found.board.id).collect();
                let boards: Vec<IdeaBoard> = own
                    .iter()
                    .map(|found| IdeaBoard {
                        id: found.board.id,
                        name: found.board.name.clone(),
                        repo_path: found.board.repo_path.clone(),
                    })
                    .collect();

                // Nur Nummer, Titel, Status und Labels – die langen Texte bleiben weg
                let tickets: Vec<IdeaTicket> = services
                    .tickets
                    .list(&TicketListFilter {
                        board_ids: board_ids.clone(),
                        without_texts: true,
                        ..TicketListFilter::default()
                    })
                    .await?
                    .into_iter()
                    .filter(|view| {
                        !matches!(
                            view.ticket.status,
                            TicketStatus::Done | TicketStatus::NotNeeded
                        )
                    })
                    .map(|view| IdeaTicket {
                        id: view.ticket.id.clone(),
                        board_id: view.ticket.board_id,
                        title: view.ticket.title.clone(),
                        status: view.ticket.status,
                        labels: view.ticket.labels.clone(),
                        updated_at: view.ticket.updated_at.clone(),
                    })
                    .collect();

                let mut open: Vec<Note> = Vec::new();
                for board_id in &board_ids {
                    open.extend(
                        services
                            .notes
                            .list(*board_id, Some(&[NoteState::Open]), None)
                            .await?,
                    );
                }
                // Dieselbe Erkennung, mit der `harness board note add` Dubletten findet
                let similar: HashSet<i32> = NoteSimilarity::similar(&input, &open)
                    .iter()
                    .map(|note| note.id)
                    .collect();
                let notes: Vec<IdeaNote> = open
                    .iter()
                    .map(|note| IdeaNote {
                        id: note.id,
                        board_id: note.board_id,
                        title: note_title(note),
                        similar: similar.contains(&note.id),
                    })
                    .collect();

                // Fragen hängen am Projekt, nicht am Board: einmal je Projekt der eigenen Boards
                let keys: Vec<String> = {
                    let all = services.projects.list().await?;
                    let project_ids: Vec<i32> = own
                        .iter()
                        .map(|found| found.board.project_id)
                        .fold(Vec::new(), |mut ids, id| {
                            if !ids.contains(&id) {
                                ids.push(id);
                            }
                            ids
                        });
                    project_ids
                        .iter()
                        .filter_map(|id| {
                            all.iter()
                                .find(|project| project.id == *id)
                                .map(|project| project.key.clone())
                        })
                        .collect()
                };
                let mut questions: Vec<IdeaQuestion> = Vec::new();
                for key in &keys {
                    let list = services
                        .questions
                        .list(
                            key,
                            &BoardQuestionListFilter {
                                state: Some(QuestionState::Open),
                                ..BoardQuestionListFilter::default()
                            },
                            Some(&me),
                        )
                        .await?;
                    questions.extend(list.questions.into_iter().map(|item| IdeaQuestion {
                        id: item.view.question.id.clone(),
                        title: item.view.question.title.clone(),
                    }));
                }

                Ok(IdeaContext {
                    boards,
                    active_board_id: active_board_id.filter(|id| board_ids.contains(id)),
                    tickets,
                    notes,
                    questions,
                })
            })
            .await
    }

    /// Der Lauf selbst: Claude rufen und die Antwort prüfen
    async fn work(
        &self,
        claude: &dyn ClaudeRunner,
        context: &IdeaContext,
        request: &IdeaRequest,
        cancel: &CancellationToken,
    ) -> Result<IdeaDraftCheck> {
        let cfg = config(&self.home)?;
        let input = request.input.trim();
        let prompt = match &request.previous {
            Some(previous) => build_prompt(
                context,
                &previous.transcript,
                Some(IdeaRefinement {
                    draft: previous,
                    answer: input,
                }),
            ),
            None => build_prompt(context, input, None),
        };
        let mut run = run_input_with_env(
            &self.home,
            &cfg,
            prompt,
            Some(cancel.clone()),
            self.env.clone(),
        )
        .await?;
        run.timeout = self.timeout;
        let result = claude.run(run).await;
        if cancel.is_cancelled() {
            return Err(
                HarnessError::new("idea-cancelled", "Das Verstehen wurde abgebrochen.").into(),
            );
        }
        if result.code == Some(127) {
            return Err(HarnessError::new(
                "analysis-failed",
                format!("Claude-Befehl „{}“ nicht gefunden.", cfg.claude.command),
            )
            .into());
        }
        if result.timed_out {
            return Err(HarnessError::new(
                "analysis-failed",
                format!(
                    "Claude hat die Idee nicht innerhalb von {} verstanden.",
                    span(self.timeout)
                ),
            )
            .into());
        }
        if result.code != Some(0) {
            return Err(HarnessError::new(
                "analysis-failed",
                format!(
                    "Claude endete mit Exit-Code {}: {}",
                    result
                        .code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "null".to_string()),
                    excerpt(&result.output)
                ),
            )
            .into());
        }
        let answer = ClaudeResultEnvelope::new().parse(&result.output)?;
        let rounds = request
            .previous
            .as_ref()
            .map_or(1, |previous| previous.rounds + 1);
        self.validator.check(
            &object(&answer.text)?,
            context,
            &self.story(request),
            rounds,
        )
    }

    /// Alles, was der Benutzer bisher gesagt hat: in der ersten Runde die Idee, später das
    /// bisherige Transkript und die neue Antwort
    fn story(&self, request: &IdeaRequest) -> String {
        let input = request.input.trim();
        match &request.previous {
            Some(previous) => format!("{}\n{input}", previous.transcript.trim()),
            None => input.to_string(),
        }
    }

    /// Merkt den laufenden Lauf; läuft im Chat schon einer, ist das ein Konflikt
    fn start(&self, chat_id: i64) -> Result<CancellationToken> {
        let mut running = self.running.lock().expect("laufende Ideen");
        if running.contains_key(&chat_id) {
            return Err(
                HarnessError::new("conflict", "Für diesen Chat läuft schon ein Lauf.")
                    .with_details(serde_json::json!({ "reason": "idea-running" }))
                    .into(),
            );
        }
        let cancel = CancellationToken::new();
        running.insert(chat_id, cancel.clone());
        Ok(cancel)
    }

    /// Gibt den Chat wieder frei – aber nur, wenn noch der eigene Lauf eingetragen ist
    fn finish(&self, chat_id: i64, cancel: &CancellationToken) {
        let mut running = self.running.lock().expect("laufende Ideen");
        if running.get(&chat_id).is_some_and(|eigen| eigen == cancel) {
            running.remove(&chat_id);
        }
    }
}

/// Das Zeitlimit für die Fehlermeldung: ganze Minuten, darunter Sekunden
fn span(timeout: Duration) -> String {
    let seconds = timeout.as_secs();
    let (count, unit) = if seconds >= 60 && seconds.is_multiple_of(60) {
        (seconds / 60, ("einer Minute", "Minuten"))
    } else {
        (seconds, ("einer Sekunde", "Sekunden"))
    };
    if count == 1 {
        unit.0.to_string()
    } else {
        format!("{count} {}", unit.1)
    }
}

/// JSON-Objekt aus dem Antworttext schneiden (wie `items` in [`crate::docs::docs_claude`],
/// nur mit geschweiften Klammern); nichts Lesbares ergibt „analysis-failed“
pub fn object(text: &str) -> Result<Value> {
    let start = text.find('{');
    let end = text.rfind('}');
    if let (Some(start), Some(end)) = (start, end) {
        if end > start {
            if let Ok(parsed) = serde_json::from_str::<Value>(&text[start..=end]) {
                return Ok(parsed);
            }
        }
    }
    Err(HarnessError::new(
        "analysis-failed",
        format!("Antwort nicht lesbar: {}", excerpt(text)),
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nennt_das_zeitlimit_in_minuten_oder_sekunden() {
        assert_eq!(span(TIMEOUT), "5 Minuten");
        assert_eq!(span(Duration::from_secs(60)), "einer Minute");
        assert_eq!(span(Duration::from_secs(2)), "2 Sekunden");
        assert_eq!(span(Duration::from_secs(1)), "einer Sekunde");
    }

    #[test]
    fn schneidet_das_objekt_aus_der_antwort_und_meldet_unlesbares() {
        assert_eq!(
            object("Hier: {\"a\":1} – fertig").unwrap(),
            serde_json::json!({ "a": 1 })
        );
        let error = object("Ich habe die Idee verstanden.").unwrap_err();
        assert_eq!(error.code(), Some("analysis-failed"));
        assert!(error.to_string().contains("nicht lesbar"));
        assert_eq!(
            object("{kaputt").unwrap_err().code(),
            Some("analysis-failed")
        );
    }

    #[test]
    fn nimmt_je_chat_nur_einen_lauf_und_gibt_ihn_nach_dem_abbruch_frei() {
        let interpreter =
            IdeaInterpreter::with_env(HarnessHome::at(std::path::Path::new("h")), BTreeMap::new());

        let cancel = interpreter.start(7).expect("erster Lauf");
        assert_eq!(
            interpreter.start(7).unwrap_err().code(),
            Some("conflict"),
            "ein zweiter Lauf im selben Chat"
        );
        // Ein anderer Chat darf parallel laufen
        assert!(interpreter.start(8).is_ok());

        assert!(interpreter.cancel(7));
        assert!(cancel.is_cancelled());
        assert!(!interpreter.cancel(7));
        assert!(interpreter.start(7).is_ok());
    }

    #[test]
    fn setzt_die_erzaehlung_aus_transkript_und_neuer_antwort_zusammen() {
        let interpreter =
            IdeaInterpreter::with_env(HarnessHome::at(std::path::Path::new("h")), BTreeMap::new());
        let request = IdeaRequest {
            chat_id: 1,
            input: "  Der Cache läuft nie ab.  ".into(),
            ..IdeaRequest::default()
        };

        assert_eq!(interpreter.story(&request), "Der Cache läuft nie ab.");

        let previous = IdeaDraft {
            summary: "x".into(),
            target: nexus_core::telegram::idea::IdeaTargetRef {
                kind: nexus_core::telegram::idea::IdeaKind::Note,
                board_id: 1,
                ticket_id: None,
            },
            title: "x".into(),
            details: String::new(),
            related: Vec::new(),
            duplicates: Vec::new(),
            questions: Vec::new(),
            confidence: 0.5,
            transcript: "Der Cache läuft nie ab.".into(),
            rounds: 1,
        };
        let request = IdeaRequest {
            chat_id: 1,
            input: " Ja, auch für SQL Server. ".into(),
            previous: Some(previous),
            ..IdeaRequest::default()
        };

        assert_eq!(
            interpreter.story(&request),
            "Der Cache läuft nie ab.\nJa, auch für SQL Server."
        );
    }
}
