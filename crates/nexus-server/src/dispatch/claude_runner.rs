//! Der Start von Claude hinter einer Schnittstelle; Gegenstück zu `ClaudeRunner` aus
//! `packages/server/src/dispatch/SuggestionAnalyzer.ts`.
//!
//! Die Docs-Dienste (Ticket-Entwürfe, Doc-Fragen, Umschreiben) rufen Claude nur über diesen
//! Trait auf. Im Server steht dahinter [`ClaudeProcess`], in Tests eine Attrappe – so startet
//! kein Test das echte `claude`.

use std::sync::Arc;

use async_trait::async_trait;

use super::claude_process::{ClaudeProcess, ClaudeRunInput, RunResult};

/// Startet Claude einmal und liefert das Ergebnis; ein Abbruch kommt über
/// [`ClaudeRunInput::cancel`]
#[async_trait]
pub trait ClaudeRunner: Send + Sync {
    /// Führt den Lauf aus; endet nie mit einem Fehler, alles steht im Ergebnis
    async fn run(&self, input: ClaudeRunInput) -> RunResult;
}

/// Der echte Start über [`ClaudeProcess`]; die Ausgabe interessiert niemanden (die Dienste
/// lesen nur die Antwort am Ende)
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessClaudeRunner;

#[async_trait]
impl ClaudeRunner for ProcessClaudeRunner {
    async fn run(&self, input: ClaudeRunInput) -> RunResult {
        ClaudeProcess::new().run(input, |_, _| {}).await
    }
}

/// Die Läufe, die der Zustand des Servers bereithält: einer für die Entwürfe (Tickets und
/// Fragen), einer für das Umschreiben, einer für die Analyse der Optimierungs-Vorschläge,
/// einer für die Ideen des Telegram-Bots. Getrennte Einträge, damit Tests sie einzeln
/// ersetzen können (wie `ticketDraftClaude`, `reviseClaude` und `suggestionClaude` in
/// `TestServer.ts`).
#[derive(Clone)]
pub struct ClaudeRunners {
    pub ticket_draft: Arc<dyn ClaudeRunner>,
    pub revise: Arc<dyn ClaudeRunner>,
    pub suggestion: Arc<dyn ClaudeRunner>,
    /// Versteht die Ideen aus dem Telegram-Bot
    pub idea: Arc<dyn ClaudeRunner>,
}

impl Default for ClaudeRunners {
    fn default() -> Self {
        Self {
            ticket_draft: Arc::new(ProcessClaudeRunner),
            revise: Arc::new(ProcessClaudeRunner),
            suggestion: Arc::new(ProcessClaudeRunner),
            idea: Arc::new(ProcessClaudeRunner),
        }
    }
}

/// Antwortet mit einem festen Ergebnis, ohne etwas zu starten: so verhält sich der Server im
/// Vertragsmodus (`contract/servers/ts-server.mjs` setzt dieselben Werte).
#[derive(Debug, Clone)]
pub struct FixedClaudeRunner {
    output: String,
}

impl FixedClaudeRunner {
    /// Läufer, dessen Antwort immer diesen Ergebnistext hat
    pub fn new(result: &str) -> Self {
        Self {
            output: serde_json::json!({
                "type": "result", "subtype": "success", "is_error": false,
                "result": result, "total_cost_usd": 0,
            })
            .to_string(),
        }
    }
}

#[async_trait]
impl ClaudeRunner for FixedClaudeRunner {
    async fn run(&self, _input: ClaudeRunInput) -> RunResult {
        RunResult {
            code: Some(0),
            timed_out: false,
            output: self.output.clone(),
        }
    }
}

impl ClaudeRunners {
    /// Die Läufe des Vertragsmodus: Entwürfe (Tickets und Fragen) und die Analyse sind leer,
    /// das Umschreiben liefert „Ersatztext“ – nie startet ein echtes Claude
    pub fn contract() -> Self {
        Self {
            ticket_draft: Arc::new(FixedClaudeRunner::new("[]")),
            revise: Arc::new(FixedClaudeRunner::new("Ersatztext")),
            suggestion: Arc::new(FixedClaudeRunner::new("[]")),
            // Der Vertrag fährt den Bot nicht; über HTTP ist der Ideen-Lauf nicht erreichbar
            idea: Arc::new(FixedClaudeRunner::new("{}")),
        }
    }
}
