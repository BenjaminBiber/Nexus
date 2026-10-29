//! Erzeugt aus markierten Stellen oder einem ganzen Dokument per Claude geprüfte
//! Ticket-Entwürfe (ohne sie anzulegen); portiert aus dem Claude-Teil von
//! `packages/server/src/docs/TicketDraftService.ts` (`create`, `cancel`, Prompts, `items`).
//!
//! Die gespeicherten Entwürfe gehören [`crate::docs::SavedTicketDrafts`]; das Ergebnis der
//! Erzeugung wird dort abgelegt, damit sich der Dialog später wieder öffnen lässt.

use std::collections::HashMap;
use std::sync::Mutex;

use nexus_core::clock::Clock;
use nexus_core::docs::markdown_sections::split_lines;
use nexus_core::docs::ticket_draft_validator::DraftDoc;
use nexus_core::docs::{TicketDraftValidator, MAX_DOC_DRAFTS, MAX_DRAFTS};
use nexus_core::{HarnessError, HarnessHome, Result};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::dispatch::claude_runner::ClaudeRunner;
use crate::dispatch::result_envelope::{excerpt, ClaudeResultEnvelope};
use crate::docs::doc_draft_store::TicketDraftResult;
use crate::docs::docs_claude::{config, items, numbered, run_input};
use crate::docs::docs_root_resolver::DocsSource;
use crate::docs::docs_service::DocsService;
use crate::docs::ticket_drafts_saved::SavedTicketDrafts;

/// Höchstens so viele Zeichen der markierten Stelle gehen in den Prompt
const MAX_QUOTE_CHARS: usize = 2000;
/// Höchstens so viele Zeichen des Hinweises gehen in den Prompt
const MAX_NOTE_CHARS: usize = 2000;

/// Markierte Stelle im Dokument, aus der Tickets entstehen sollen
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TicketDraftSelection {
    pub quote: Option<String>,
    pub heading: Option<String>,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
}

/// Der Körper der Erzeugungsroute
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TicketDraftRequest {
    pub path: Option<String>,
    pub selections: Vec<TicketDraftSelection>,
    /// `true` zerlegt das ganze Dokument (ohne Markierungen) in granulare Tickets
    pub whole: bool,
    pub note: Option<String>,
}

/// Lässt Claude Ticket-Entwürfe erzeugen; je Projekt läuft höchstens eine Erzeugung.
#[derive(Debug)]
pub struct TicketDraftService {
    home: HarnessHome,
    /// Laufende Erzeugung je Projekt; über ihr Abbruch-Signal lässt sie sich beenden
    running: Mutex<HashMap<String, CancellationToken>>,
    validator: TicketDraftValidator,
    clock: Clock,
}

impl TicketDraftService {
    /// Dienst im Benutzerordner (Einstellung und Arbeitsordner der Analyse)
    pub fn new(home: HarnessHome) -> Self {
        Self::with_clock(home, Clock::system())
    }

    /// Wie [`TicketDraftService::new`], mit eigener Uhr (Tests)
    pub fn with_clock(home: HarnessHome, clock: Clock) -> Self {
        Self {
            home,
            running: Mutex::new(HashMap::new()),
            validator: TicketDraftValidator::new(),
            clock,
        }
    }

    /// Erzeugt die Entwürfe zu einem Dokument und merkt sie sich; je Projekt läuft höchstens
    /// eine Erzeugung
    pub async fn create(
        &self,
        claude: &dyn ClaudeRunner,
        docs: &DocsService,
        saved: &SavedTicketDrafts,
        source: &DocsSource,
        project_id: &str,
        input: &TicketDraftRequest,
    ) -> Result<TicketDraftResult> {
        if !input.whole && input.selections.is_empty() {
            return Err(HarnessError::new(
                "invalid-input",
                "Bitte mindestens eine Stelle markieren.",
            )
            .into());
        }
        let doc = docs.read(source, input.path.as_deref())?;
        let cancel = self.start(project_id)?;
        let made = self.work(claude, input, &doc.content, &cancel).await;
        self.finish(project_id, &cancel);
        let made = made?;
        // Gemerkt, damit sich die Entwürfe später (auch nach Neuladen oder ohne offene Seite)
        // wieder öffnen lassen
        saved
            .put(docs, source, input.path.as_deref(), &made)
            .await?;
        Ok(made)
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
                "Für dieses Projekt läuft schon eine Erzeugung.",
            )
            .with_details(serde_json::json!({ "reason": "drafts-running" }))
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
        input: &TicketDraftRequest,
        content: &str,
        cancel: &CancellationToken,
    ) -> Result<TicketDraftResult> {
        let cfg = config(&self.home)?;
        let prompt = if input.whole {
            self.whole_prompt(content, input.note.as_deref())
        } else {
            self.prompt(content, &input.selections, input.note.as_deref())
        };
        let run = run_input(&self.home, &cfg, prompt, Some(cancel.clone()))?;
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
                "Claude hat die Entwürfe nicht innerhalb von 5 Minuten geliefert.",
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
        let max = if input.whole {
            MAX_DOC_DRAFTS
        } else {
            MAX_DRAFTS
        };
        let lines = split_lines(content).len() as i64;
        let check = self
            .validator
            .check(&items(&answer.text)?, DraftDoc::with_max(lines, max))?;
        Ok(TicketDraftResult {
            drafts: check.drafts,
            dropped: check.dropped,
            cost_usd: answer.cost_usd,
            at: self.clock.iso(),
        })
    }

    /// Arbeitsanweisung für das ganze Dokument: granular zerlegen, bei Plänen entlang der Tasks
    fn whole_prompt(&self, content: &str, note: Option<&str>) -> String {
        let mut zeilen: Vec<String> = vec![
            "Du zerlegst ein ganzes Dokument (Spec oder Implementierungsplan) in granulare, einzeln umsetzbare Tickets.".into(),
            String::new(),
            "Regeln:".into(),
            "- Decke das ganze Dokument ab: lass nichts Umsetzbares aus und erfinde nichts dazu.".into(),
            "- Ist es ein Plan mit Tasks oder nummerierten Schritten, wird in der Regel jeder Task ein Ticket, in der Reihenfolge des Plans; seine Schritte kommen in „steps“.".into(),
            "- Ist es eine Spec, schneide entlang der Abschnitte in kleine, für sich lieferbare Stücke.".into(),
            "- Jedes Ticket ist für sich umsetzbar und test-first prüfbar; die Abnahme nennt, woran man das Ergebnis sieht.".into(),
            format!("- Höchstens {MAX_DOC_DRAFTS} Tickets. Reine Hintergrund- oder Begründungsabschnitte werden kein eigenes Ticket."),
            "- „dependsOn“ sind ausschließlich 0-basierte Indizes in diese Liste; keine Kreise, kein Selbstbezug. Setze sie, wo ein Ticket auf einem früheren aufbaut.".into(),
            "- „source“ nennt Überschrift und Zeilenbereich des Abschnitts, aus dem das Ticket stammt (Zeilennummern aus dem Dokument unten).".into(),
            "- Antworte deutsch und ausschließlich mit einem JSON-Array, ohne Markdown und ohne Werkzeuge.".into(),
            String::new(),
            "Jedes Element:".into(),
            "{\"title\": string, \"description\": string, \"acceptance\": string, \"steps\": string[], \"dependsOn\": number[], \"labels\": string[], \"source\": {\"heading\": string, \"startLine\": number, \"endLine\": number}}".into(),
        ];
        zeilen.extend(note_part(note));
        zeilen.push(String::new());
        zeilen.push("## Dokument (mit Zeilennummern)".into());
        zeilen.push(numbered(content));
        zeilen.join("\n")
    }

    /// Arbeitsanweisung: ganzes Dokument mit Zeilennummern, die markierten Stellen, Regeln und
    /// Antwortformat
    fn prompt(
        &self,
        content: &str,
        selections: &[TicketDraftSelection],
        note: Option<&str>,
    ) -> String {
        let marked: Vec<String> = selections
            .iter()
            .enumerate()
            .map(|(index, selection)| {
                let heading = selection
                    .heading
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let start = selection.start_line.filter(|line| *line != 0).unwrap_or(1);
                let end = selection
                    .end_line
                    .filter(|line| *line != 0)
                    .unwrap_or(start);
                [
                    format!("### Stelle {}", index + 1),
                    format!(
                        "Überschrift: {}",
                        if heading.is_empty() {
                            "(keine)".to_string()
                        } else {
                            heading
                        }
                    ),
                    format!("Zeilen: {start}–{end}"),
                    cut(
                        selection.quote.as_deref().unwrap_or_default(),
                        MAX_QUOTE_CHARS,
                    ),
                ]
                .join("\n")
            })
            .collect();
        let mut zeilen: Vec<String> = vec![
            "Du zerlegst einen Abschnitt einer Spec in einzeln umsetzbare Tickets.".into(),
            String::new(),
            "Regeln:".into(),
            "- Arbeite nur zu den unten markierten Stellen, nicht zum übrigen Dokument.".into(),
            "- Jedes Ticket ist für sich umsetzbar und test-first prüfbar; die Abnahme nennt, woran man das Ergebnis sieht.".into(),
            format!("- Höchstens {MAX_DRAFTS} Tickets. Lieber wenige gute als viele kleine Schnipsel."),
            "- „dependsOn“ sind ausschließlich 0-basierte Indizes in diese Liste; keine Kreise, kein Selbstbezug.".into(),
            "- „source“ nennt Überschrift und Zeilenbereich der markierten Stelle, aus der das Ticket stammt (Zeilennummern aus dem Dokument unten).".into(),
            "- Antworte deutsch und ausschließlich mit einem JSON-Array, ohne Markdown und ohne Werkzeuge.".into(),
            String::new(),
            "Jedes Element:".into(),
            "{\"title\": string, \"description\": string, \"acceptance\": string, \"steps\": string[], \"dependsOn\": number[], \"labels\": string[], \"source\": {\"heading\": string, \"startLine\": number, \"endLine\": number}}".into(),
            String::new(),
            "## Markierte Stellen".into(),
        ];
        zeilen.extend(marked);
        zeilen.extend(note_part(note));
        zeilen.push(String::new());
        zeilen.push("## Dokument (mit Zeilennummern)".into());
        zeilen.push(numbered(content));
        zeilen.join("\n")
    }
}

/// Der Hinweis des Nutzers als eigener Abschnitt; ohne Hinweis bleibt er weg
fn note_part(note: Option<&str>) -> Vec<String> {
    let note = note.unwrap_or_default().trim();
    if note.is_empty() {
        return Vec::new();
    }
    vec![
        String::new(),
        "## Hinweis des Nutzers".into(),
        cut(note, MAX_NOTE_CHARS),
    ]
}

/// Kürzt auf so viele Zeichen (wie `slice` in TypeScript, dort nach UTF-16-Einheiten gezählt;
/// für Prompts genügt die Zeichenzahl)
fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Liest den Körper der Erzeugungsroute; unbrauchbare Markierungen fallen weg wie in
/// TypeScript (`filter((s) => s && typeof s === 'object')`)
pub fn valid_request(body: &Value) -> TicketDraftRequest {
    let selections = body
        .get("selections")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| item.is_object())
                .map(|item| TicketDraftSelection {
                    quote: item
                        .get("quote")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    heading: item
                        .get("heading")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    start_line: item.get("startLine").and_then(Value::as_i64),
                    end_line: item.get("endLine").and_then(Value::as_i64),
                })
                .collect()
        })
        .unwrap_or_default();
    TicketDraftRequest {
        path: body.get("path").and_then(Value::as_str).map(str::to_string),
        selections,
        whole: body.get("whole") == Some(&Value::Bool(true)),
        note: body.get("note").and_then(Value::as_str).map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dienst mit einem Benutzerordner, der nicht gebraucht wird (nur Prompts)
    fn service() -> TicketDraftService {
        TicketDraftService::new(HarnessHome::at(std::path::Path::new("h")))
    }

    #[test]
    fn nennt_im_prompt_des_ganzen_dokuments_die_zeilennummern_und_keine_markierungen() {
        let prompt = service().whole_prompt("eins\nzwei", None);

        assert!(prompt.contains("ganzes Dokument"));
        assert!(!prompt.contains("## Markierte Stellen"));
        assert!(prompt.contains("2\tzwei"));
        assert!(!prompt.contains("## Hinweis des Nutzers"));
    }

    #[test]
    fn nennt_markierte_stellen_mit_uberschrift_zeilen_und_zitat() {
        let prompt = service().prompt(
            "eins\nzwei",
            &[
                TicketDraftSelection {
                    quote: Some("Eine Quelle.".into()),
                    heading: Some(" Ziel ".into()),
                    start_line: Some(3),
                    end_line: Some(5),
                },
                TicketDraftSelection::default(),
            ],
            Some("  bitte klein schneiden  "),
        );

        assert!(prompt.contains("### Stelle 1\nÜberschrift: Ziel\nZeilen: 3–5\nEine Quelle."));
        assert!(prompt.contains("### Stelle 2\nÜberschrift: (keine)\nZeilen: 1–1"));
        assert!(prompt.contains("## Hinweis des Nutzers\nbitte klein schneiden"));
        assert!(prompt.contains("1\teins"));
    }

    #[test]
    fn liest_den_korper_und_wirft_unbrauchbare_markierungen_weg() {
        let request = valid_request(&serde_json::json!({
            "path": "a.md", "whole": "ja", "note": "x",
            "selections": [{ "heading": "Ziel", "startLine": 3, "endLine": 5 }, 7, null],
        }));

        assert_eq!(request.path.as_deref(), Some("a.md"));
        assert!(!request.whole, "nur true zählt als whole");
        assert_eq!(request.selections.len(), 1);
        assert_eq!(request.selections[0].start_line, Some(3));
    }
}
