//! Liest per Claude die offenen Fragen eines Dokuments heraus und legt die gewählten als
//! Board-Fragen im Projekt des Boards an; portiert aus
//! `packages/server/src/docs/DocQuestionService.ts`.
//!
//! Die Entwürfe werden nicht gespeichert; ein einzelner Entwurf, der beim Anlegen nicht
//! durchkommt, wird übersprungen.

use std::collections::HashMap;
use std::sync::Mutex;

use nexus_core::actors::AuthorKind;
use nexus_core::board::board_service::BoardListFilter;
use nexus_core::board::question_service::{BoardQuestionWriteOptions, NewBoardQuestionInput};
use nexus_core::board::question_types::{BoardQuestionActor, MAX_QUESTION_TITLE_LENGTH};
use nexus_core::clock::Clock;
use nexus_core::docs::markdown_sections::split_lines;
use nexus_core::docs::TicketDraftSource;
use nexus_core::questions::decision_policy::DecisionInput;
use nexus_core::{HarnessError, HarnessHome, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::claude_runner::ClaudeRunner;
use crate::dispatch::result_envelope::{excerpt, ClaudeResultEnvelope};
use crate::docs::doc_ticket_service::DocTicketService;
use crate::docs::doc_ticket_source::format_source;
use crate::docs::docs_claude::{config, items, numbered, run_input};
use crate::docs::docs_root_resolver::DocsSource;
use crate::docs::docs_service::DocsService;

/// Mehr offene Fragen auf einmal sind nicht mehr überschaubar
pub const MAX_QUESTION_DRAFTS: usize = 20;

/// Höchstlänge des Anlasses (Spalte `board.Questions.Occasion`)
const MAX_OCCASION: usize = 200;

/// Entwurf einer Board-Frage aus einem Dokument
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionDraft {
    pub title: String,
    pub description: String,
    pub source: TicketDraftSource,
}

/// Die Entwürfe eines Laufs samt Kosten und Zeitpunkt
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionDraftResult {
    pub drafts: Vec<QuestionDraft>,
    pub dropped: Vec<String>,
    pub cost_usd: Option<f64>,
    pub at: String,
}

/// Angelegte Frage samt Platz des Entwurfs in der Eingabe; `answered` = gleich mit der Antwort
/// entschieden
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedDocQuestion {
    pub index: usize,
    pub id: String,
    pub answered: bool,
    /// Fehlercode, wenn die Frage angelegt wurde, das Entscheiden aber scheiterte (sie bleibt
    /// dann offen)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer_error: Option<String>,
}

/// Übersprungener Entwurf samt Grund
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedDocQuestion {
    pub index: usize,
    pub reason: String,
}

/// Ergebnis des Anlegens
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocQuestionResult {
    pub created: Vec<CreatedDocQuestion>,
    pub skipped: Vec<SkippedDocQuestion>,
}

/// Erzeugt Fragen-Entwürfe eines Dokuments und legt die gewählten als Board-Fragen an.
#[derive(Debug)]
pub struct DocQuestionService {
    home: HarnessHome,
    /// Laufende Erzeugung je Projekt; über ihr Abbruch-Signal lässt sie sich beenden
    running: Mutex<HashMap<String, CancellationToken>>,
    clock: Clock,
}

impl DocQuestionService {
    /// Dienst im Benutzerordner (Einstellung und Arbeitsordner der Analyse)
    pub fn new(home: HarnessHome) -> Self {
        Self::with_clock(home, Clock::system())
    }

    /// Wie [`DocQuestionService::new`], mit eigener Uhr (Tests)
    pub fn with_clock(home: HarnessHome, clock: Clock) -> Self {
        Self {
            home,
            running: Mutex::new(HashMap::new()),
            clock,
        }
    }

    /// Erzeugt die Fragen-Entwürfe zu einem Dokument; je Projekt läuft höchstens eine Erzeugung
    pub async fn drafts(
        &self,
        claude: &dyn ClaudeRunner,
        docs: &DocsService,
        source: &DocsSource,
        project_id: &str,
        path: Option<&str>,
    ) -> Result<QuestionDraftResult> {
        let doc = docs.read(source, path)?;
        let cancel = self.start(project_id)?;
        let result = self.work(claude, &doc.content, &cancel).await;
        self.finish(project_id, &cancel);
        result
    }

    /// Bricht die laufende Erzeugung des Projekts ab; `false`, wenn keine läuft
    pub fn cancel(&self, project_id: &str) -> bool {
        let mut running = self.running.lock().expect("laufende Erzeugungen");
        match running.remove(project_id) {
            Some(cancel) => {
                cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// Merkt die laufende Erzeugung; läuft schon eine, ist das ein Konflikt
    fn start(&self, project_id: &str) -> Result<CancellationToken> {
        let mut running = self.running.lock().expect("laufende Erzeugungen");
        if running.contains_key(project_id) {
            return Err(HarnessError::new(
                "conflict",
                "Für dieses Projekt läuft schon eine Erzeugung von Fragen.",
            )
            .with_details(serde_json::json!({ "reason": "question-drafts-running" }))
            .into());
        }
        let cancel = CancellationToken::new();
        running.insert(project_id.to_string(), cancel.clone());
        Ok(cancel)
    }

    /// Gibt das Projekt wieder frei – aber nur, wenn noch der eigene Lauf eingetragen ist
    fn finish(&self, project_id: &str, cancel: &CancellationToken) {
        let mut running = self.running.lock().expect("laufende Erzeugungen");
        if running.get(project_id).is_some_and(|eigen| eigen == cancel) {
            running.remove(project_id);
        }
    }

    /// Der Lauf selbst: Claude rufen und die Antwort prüfen
    async fn work(
        &self,
        claude: &dyn ClaudeRunner,
        content: &str,
        cancel: &CancellationToken,
    ) -> Result<QuestionDraftResult> {
        let cfg = config(&self.home)?;
        let run = run_input(&self.home, &cfg, prompt(content), Some(cancel.clone())).await?;
        let result = claude.run(run).await;
        if cancel.is_cancelled() {
            return Err(
                HarnessError::new("drafts-cancelled", "Die Erzeugung wurde abgebrochen.").into(),
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
                "Claude hat die Fragen nicht innerhalb von 5 Minuten geliefert.",
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
        let lines = split_lines(content).len() as i64;
        let (drafts, dropped) = check(&items(&answer.text)?, lines)?;
        Ok(QuestionDraftResult {
            drafts,
            dropped,
            cost_usd: answer.cost_usd,
            at: self.clock.iso(),
        })
    }

    /// Legt die Entwürfe der Reihe nach als Fragen an: Quelle am Ende der Beschreibung,
    /// Anlass = Dokument, verknüpft mit den Tickets des Dokuments, die aus derselben
    /// Überschrift entstanden sind; mit Antwort gleich entschieden
    pub async fn create(
        &self,
        holder: &BoardRuntimeHolder,
        docs: &DocsService,
        tickets: &DocTicketService,
        source: &DocsSource,
        path: Option<&str>,
        drafts: &[Value],
    ) -> Result<DocQuestionResult> {
        if drafts.is_empty() {
            return Err(HarnessError::new(
                "invalid-input",
                "Bitte mindestens eine Frage übergeben.",
            )
            .into());
        }
        let board_id = board_id(source)?;
        // wirft doc-not-found, wenn es den Pfad nicht gibt
        let doc = docs.read(source, path)?;
        let lines = split_lines(&doc.content).len() as i64;
        let project_key = project_key(holder, board_id).await?;
        let refs = tickets.list_for(holder, docs, source, path).await?;
        let occasion = cut(&format!("Dokument {}", doc.path), MAX_OCCASION);

        let mut created: Vec<CreatedDocQuestion> = Vec::new();
        let mut skipped: Vec<SkippedDocQuestion> = Vec::new();
        for (index, raw) in drafts.iter().enumerate() {
            let Some(draft) = valid(raw, lines) else {
                skipped.push(SkippedDocQuestion {
                    index,
                    reason: "draft-invalid".into(),
                });
                continue;
            };
            let heading = draft.source.heading.clone();
            let ticket_ids: Vec<String> = refs
                .iter()
                .filter(|found| {
                    !heading.is_empty() && found.heading.as_deref() == Some(heading.as_str())
                })
                .map(|found| found.id.clone())
                .collect();
            let input = NewBoardQuestionInput {
                title: draft.title.clone(),
                description: Some(format!(
                    "{}\n\n{}",
                    draft.description,
                    format_source(
                        &doc.path,
                        if heading.is_empty() {
                            "(ohne Überschrift)"
                        } else {
                            &heading
                        },
                        draft.source.start_line,
                        draft.source.end_line,
                    )
                )),
                occasion: Some(occasion.clone()),
                ticket_ids: Some(ticket_ids),
                ..NewBoardQuestionInput::default()
            };
            let key = project_key.clone();
            let result = holder
                .run(move |b| async move {
                    b.services()
                        .questions
                        .create(&key, &input, &actor(b.as_ref()))
                        .await
                })
                .await;
            match result {
                Ok(question) => {
                    let answer = text(raw.get("answer"));
                    created.push(
                        self.answer(
                            holder,
                            index,
                            &question.view.question.id,
                            question.view.question.rev,
                            &answer,
                        )
                        .await?,
                    );
                }
                // Nur Fachfehler des einzelnen Entwurfs werden übersprungen, alles andere kommt
                // als Fehler an
                Err(error)
                    if error.harness().is_some()
                        && !BoardRuntimeHolder::is_connection_error(&error) =>
                {
                    skipped.push(SkippedDocQuestion {
                        index,
                        reason: error.harness().expect("erwartbarer Fehler").code.clone(),
                    });
                }
                Err(error) => return Err(error),
            }
        }
        Ok(DocQuestionResult { created, skipped })
    }

    /// Entscheidet die eben angelegte Frage mit der Antwort aus dem Dialog; ohne Antwort bleibt
    /// sie offen (für die Recherche). Scheitert nur das Entscheiden, bleibt die Frage angelegt
    /// und offen, und der Fehlercode steht am Eintrag.
    async fn answer(
        &self,
        holder: &BoardRuntimeHolder,
        index: usize,
        id: &str,
        rev: i32,
        answer: &str,
    ) -> Result<CreatedDocQuestion> {
        if answer.is_empty() {
            return Ok(CreatedDocQuestion {
                index,
                id: id.to_string(),
                answered: false,
                answer_error: None,
            });
        }
        let input = DecisionInput {
            text: Some(answer.to_string()),
            ..DecisionInput::default()
        };
        let question_id = id.to_string();
        let result = holder
            .run(move |b| async move {
                b.services()
                    .questions
                    .answer(
                        &question_id,
                        &input,
                        &actor(b.as_ref()),
                        &BoardQuestionWriteOptions::rev(rev),
                    )
                    .await
            })
            .await;
        match result {
            Ok(_) => Ok(CreatedDocQuestion {
                index,
                id: id.to_string(),
                answered: true,
                answer_error: None,
            }),
            Err(error)
                if error.harness().is_some()
                    && !BoardRuntimeHolder::is_connection_error(&error) =>
            {
                Ok(CreatedDocQuestion {
                    index,
                    id: id.to_string(),
                    answered: false,
                    answer_error: Some(error.harness().expect("erwartbarer Fehler").code.clone()),
                })
            }
            Err(error) => Err(error),
        }
    }
}

/// Der Mensch der Oberfläche (wie beim Anlegen über die Fragen-Seite)
fn actor(backend: &dyn crate::board_runtime::BoardBackend) -> BoardQuestionActor {
    BoardQuestionActor {
        actor: backend.actor(None),
        kind: AuthorKind::Human,
        session: None,
    }
}

/// Projektschlüssel des Boards; Fragen gehören zum Projekt, nicht zum Board
async fn project_key(holder: &BoardRuntimeHolder, board_id: i32) -> Result<String> {
    let boards = holder
        .run(|b| async move { b.services().boards.list(&BoardListFilter::default()).await })
        .await?;
    boards
        .into_iter()
        .find(|board| board.board.id == board_id)
        .map(|board| board.project_key)
        .ok_or_else(|| {
            HarnessError::new(
                "board-not-found",
                format!("Board {board_id} gibt es nicht."),
            )
            .into()
        })
}

/// Board des Ordners; ohne Board oder Repo-Pfad lässt sich nichts anlegen
fn board_id(source: &DocsSource) -> Result<i32> {
    let Some(board_id) = source.board_id else {
        return Err(HarnessError::new(
            "no-board",
            "Zu diesem Ordner gibt es kein Board; Fragen lassen sich erst nach dem Verknüpfen anlegen.",
        )
        .into());
    };
    if source.root.is_none() {
        return Err(HarnessError::new(
            "no-repo-path",
            format!(
                "Das Board „{}“ hat keinen erreichbaren Repo-Pfad; ohne ihn gibt es keine Dokumente.",
                source.board_name.as_deref().unwrap_or_default()
            ),
        )
        .into());
    }
    Ok(board_id)
}

/// Prüft Claudes Antwort: Titel Pflicht, Zeilenbereich im Dokument, höchstens
/// [`MAX_QUESTION_DRAFTS`]
fn check(items: &Value, lines: i64) -> Result<(Vec<QuestionDraft>, Vec<String>)> {
    let Some(list) = items.as_array() else {
        return Err(
            HarnessError::new("analysis-failed", "Antwort ist keine Liste von Fragen.").into(),
        );
    };
    let mut dropped: Vec<String> = Vec::new();
    if list.len() > MAX_QUESTION_DRAFTS {
        dropped.push(format!(
            "Nur die ersten {MAX_QUESTION_DRAFTS} von {} Fragen übernommen.",
            list.len()
        ));
    }
    let mut drafts: Vec<QuestionDraft> = Vec::new();
    for (index, item) in list.iter().take(MAX_QUESTION_DRAFTS).enumerate() {
        match valid(item, lines) {
            Some(draft) => drafts.push(draft),
            None => dropped.push(format!(
                "Frage {} verworfen: Titel fehlt oder ist zu lang.",
                index + 1
            )),
        }
    }
    Ok((drafts, dropped))
}

/// Ein Entwurf mit Titel (bis [`MAX_QUESTION_TITLE_LENGTH`]) und auf das Dokument begrenzter
/// Quelle; sonst `None`
fn valid(raw: &Value, lines: i64) -> Option<QuestionDraft> {
    if !raw.is_object() {
        return None;
    }
    let title = text(raw.get("title"));
    if title.is_empty() || nexus_core::text::utf16_len(&title) > MAX_QUESTION_TITLE_LENGTH {
        return None;
    }
    let source = raw.get("source");
    let start = source
        .and_then(|source| source.get("startLine"))
        .and_then(Value::as_i64);
    let end = source
        .and_then(|source| source.get("endLine"))
        .and_then(Value::as_i64);
    // Nur ein Bereich, der im Dokument liegt, wird übernommen; sonst zählt die erste Zeile
    let (start_line, end_line) = match (start, end) {
        (Some(start), Some(end)) if start >= 1 && end >= start && start <= lines => {
            (start, end.min(lines))
        }
        _ => (1, 1),
    };
    Some(QuestionDraft {
        title,
        description: text(raw.get("description")),
        source: TicketDraftSource {
            heading: text(source.and_then(|source| source.get("heading"))),
            start_line,
            end_line,
        },
    })
}

/// Ein Textfeld, getrimmt; alles andere als eine Zeichenkette gilt als leer
fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

/// Kürzt auf so viele Zeichen (wie `slice` in TypeScript)
fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Arbeitsanweisung: offene, ungeklärte Punkte des ganzen Dokuments als eigenständige Fragen
fn prompt(content: &str) -> String {
    [
        "Du liest aus einem Dokument (Spec oder Implementierungsplan) die offenen, noch ungeklärten Fragen heraus.".to_string(),
        String::new(),
        "Regeln:".to_string(),
        "- Eine offene Frage ist ein Punkt, den das Dokument ausdrücklich offen lässt oder der vor der Umsetzung entschieden oder recherchiert werden muss:".to_string(),
        "  ein Abschnitt „Offene Fragen“, Markierungen wie TBD, TODO, „noch zu klären“, „?“, unentschiedene Alternativen oder fehlende Angaben.".to_string(),
        "- Erfinde keine Fragen zu Dingen, die das Dokument schon entscheidet. Findest du keine, antworte mit [].".to_string(),
        "- Der Titel ist die Frage selbst, kurz und für sich verständlich (endet mit „?“).".to_string(),
        "- Die Beschreibung gibt den nötigen Kontext aus dem Dokument und nennt, falls vorhanden, die erwogenen Möglichkeiten.".to_string(),
        format!("- Höchstens {MAX_QUESTION_DRAFTS} Fragen; fasse Zusammengehöriges zu einer Frage zusammen."),
        "- „source“ nennt Überschrift und Zeilenbereich der Stelle, an der die Frage steht (Zeilennummern aus dem Dokument unten).".to_string(),
        "- Antworte deutsch und ausschließlich mit einem JSON-Array, ohne Markdown und ohne Werkzeuge.".to_string(),
        String::new(),
        "Jedes Element:".to_string(),
        "{\"title\": string, \"description\": string, \"source\": {\"heading\": string, \"startLine\": number, \"endLine\": number}}".to_string(),
        String::new(),
        "## Dokument (mit Zeilennummern)".to_string(),
        numbered(content),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nennt_im_prompt_das_dokument_mit_zeilennummern() {
        let prompt = prompt("eins\nzwei");

        assert!(prompt.contains("offenen, noch ungeklärten Fragen"));
        assert!(prompt.contains("2\tzwei"));
    }

    #[test]
    fn verwirft_entwurfe_ohne_titel_und_begrenzt_den_zeilenbereich() {
        let items = serde_json::json!([
            { "title": " Welcher Speicher? ", "description": " Kontext ",
              "source": { "heading": "Ziel", "startLine": 3, "endLine": 5 } },
            { "title": "" },
            { "title": "Wie viele?", "source": { "heading": "Plan", "startLine": 99, "endLine": 120 } },
            { "title": "Bis wohin?", "source": { "heading": "Plan", "startLine": 3, "endLine": 120 } },
        ]);

        let (drafts, dropped) = check(&items, 10).unwrap();

        assert_eq!(
            drafts.iter().map(|d| d.title.as_str()).collect::<Vec<_>>(),
            ["Welcher Speicher?", "Wie viele?", "Bis wohin?"]
        );
        assert_eq!(drafts[0].description, "Kontext");
        assert_eq!(
            (drafts[0].source.start_line, drafts[0].source.end_line),
            (3, 5)
        );
        // Ein Zeilenbereich außerhalb des Dokuments wird zur ersten Zeile
        assert_eq!(
            (drafts[1].source.start_line, drafts[1].source.end_line),
            (1, 1)
        );
        // Ein zu weit reichendes Ende wird auf die letzte Zeile gekürzt
        assert_eq!(
            (drafts[2].source.start_line, drafts[2].source.end_line),
            (3, 10)
        );
        assert_eq!(dropped.len(), 1);
    }

    #[test]
    fn ubernimmt_hochstens_zwanzig_fragen() {
        let items = Value::Array(
            (0..25)
                .map(|nummer| serde_json::json!({ "title": format!("Frage {nummer}?") }))
                .collect(),
        );

        let (drafts, dropped) = check(&items, 10).unwrap();

        assert_eq!(drafts.len(), MAX_QUESTION_DRAFTS);
        assert_eq!(dropped[0], "Nur die ersten 20 von 25 Fragen übernommen.");
    }

    #[test]
    fn meldet_eine_antwort_die_keine_liste_ist() {
        let error = check(&serde_json::json!({ "a": 1 }), 10).unwrap_err();
        assert_eq!(error.code(), Some("analysis-failed"));
    }

    #[test]
    fn ubernimmt_keine_antwort_aus_claudes_entwurfen() {
        let draft = valid(
            &serde_json::json!({ "title": "Welcher Speicher?", "answer": "erfunden" }),
            10,
        )
        .expect("Entwurf");

        let json = serde_json::to_value(&draft).unwrap();
        assert!(json.get("answer").is_none());
    }
}
