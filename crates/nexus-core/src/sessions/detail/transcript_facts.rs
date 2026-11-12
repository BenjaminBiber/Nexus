//! Kennzahlen einer Session aus ihrem Transcript: Quelle, Branch, Version, Titel, Tokens,
//! Kosten, Dauer, Kontextstand und Todo-Liste. Wird Zeile für Zeile gefüttert, damit der
//! Server bei wachsender Datei nur das Neue lesen muss.

use std::collections::HashMap;

use serde_json::Value;

use super::jsonl::{flag, number, text};
use super::types::{ContextUsage, TodoItem, TodoStatus, TokenTotals};
use crate::sessions::session::SessionClient;

/// Kontextgrenze der Modelle
const CONTEXT_LIMIT: i64 = 200_000;
/// Kontextgrenze der Modelle mit großem Kontext (`[1m]`)
const LARGE_CONTEXT_LIMIT: i64 = 1_000_000;

/// Die Kennzahlen zum aktuellen Stand
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TranscriptFacts {
    /// Aus dem `entrypoint` der ersten Nachrichtenzeile
    pub client: Option<SessionClient>,
    pub branch: Option<String>,
    pub version: Option<String>,
    pub title: Option<String>,
    /// Modell der letzten Antwort der Hauptsession
    pub model: Option<String>,
    pub tokens: TokenTotals,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<i64>,
    pub context: Option<ContextUsage>,
    pub todos: Vec<TodoItem>,
}

/// Sammelt die Kennzahlen Zeile für Zeile
#[derive(Debug, Clone, Default)]
pub struct FactsCollector {
    client: Option<SessionClient>,
    branch: Option<String>,
    version: Option<String>,
    title: Option<String>,
    model: Option<String>,
    /// Tokens je Antwort (`message.id`); Zeilen derselben Antwort wiederholen sie
    usage: HashMap<String, TokenTotals>,
    usage_sum: TokenTotals,
    /// Letzter Stand aus `cost-state`
    cost_tokens: Option<TokenTotals>,
    cost_usd: Option<f64>,
    duration_ms: Option<i64>,
    context: Option<ContextUsage>,
    /// Aufgaben mit ihrer Nummer (nur TaskCreate vergibt eine)
    todos: Vec<(Option<String>, TodoItem)>,
    /// Aufrufe von TodoWrite/TaskCreate/TaskUpdate ohne Ergebnis: Kennung → (Name, Eingabe)
    pending: HashMap<String, (String, Value)>,
}

impl FactsCollector {
    /// Neuer, leerer Stand
    pub fn new() -> Self {
        Self::default()
    }

    /// Verarbeitet eine Zeile des Transcripts
    pub fn feed(&mut self, value: &Value) {
        let side = flag(value, "isSidechain");
        if !side {
            if self.client.is_none() {
                if let Some(entrypoint) = text(value, "entrypoint") {
                    self.client = Some(SessionClient::from_entrypoint(entrypoint));
                }
            }
            if let Some(branch) = text(value, "gitBranch") {
                self.branch = Some(branch.to_string());
            }
            if let Some(version) = text(value, "version") {
                self.version = Some(version.to_string());
            }
        }
        match text(value, "type") {
            Some("custom-title") => {
                if let Some(title) = text(value, "customTitle") {
                    self.title = Some(title.to_string());
                }
            }
            Some("cost-state") => self.costs(value),
            Some("assistant") => self.assistant(value, side),
            Some("user") if !side => self.results(value),
            _ => {}
        }
    }

    /// Der aktuelle Stand
    pub fn facts(&self) -> TranscriptFacts {
        TranscriptFacts {
            client: self.client,
            branch: self.branch.clone(),
            version: self.version.clone(),
            title: self.title.clone(),
            model: self.model.clone(),
            tokens: self.cost_tokens.unwrap_or(self.usage_sum),
            cost_usd: self.cost_usd,
            duration_ms: self.duration_ms,
            context: self.context,
            todos: self.todos.iter().map(|(_, todo)| todo.clone()).collect(),
        }
    }

    /// Kosten, Dauer und Tokens aller Modelle; der letzte Stand gilt
    fn costs(&mut self, value: &Value) {
        if let Some(cost) = value.get("totalCostUSD").and_then(Value::as_f64) {
            self.cost_usd = Some(cost);
        }
        if let Some(duration) = number(value, "totalDuration") {
            self.duration_ms = Some(duration);
        }
        if let Some(models) = value.get("modelUsage").and_then(Value::as_object) {
            let mut sum = TokenTotals::default();
            for model in models.values() {
                sum.input += number(model, "inputTokens").unwrap_or(0);
                sum.output += number(model, "outputTokens").unwrap_or(0);
                sum.cache_read += number(model, "cacheReadInputTokens").unwrap_or(0);
                sum.cache_write += number(model, "cacheCreationInputTokens").unwrap_or(0);
            }
            self.cost_tokens = Some(sum);
        }
    }

    /// Tokens der Antwort, Kontextstand (nur Hauptverlauf) und offene Todo-Aufrufe
    fn assistant(&mut self, value: &Value, side: bool) {
        let message = value.get("message").unwrap_or(&Value::Null);
        if let Some(usage) = message.get("usage") {
            let tokens = TokenTotals {
                input: number(usage, "input_tokens").unwrap_or(0),
                output: number(usage, "output_tokens").unwrap_or(0),
                cache_read: number(usage, "cache_read_input_tokens").unwrap_or(0),
                cache_write: number(usage, "cache_creation_input_tokens").unwrap_or(0),
            };
            let key = text(message, "id")
                .or_else(|| text(value, "uuid"))
                .unwrap_or("")
                .to_string();
            let old = self.usage.insert(key, tokens).unwrap_or_default();
            self.usage_sum.input += tokens.input - old.input;
            self.usage_sum.output += tokens.output - old.output;
            self.usage_sum.cache_read += tokens.cache_read - old.cache_read;
            self.usage_sum.cache_write += tokens.cache_write - old.cache_write;
            let used = tokens.input + tokens.cache_read + tokens.cache_write;
            if !side && used > 0 {
                let large = text(message, "model").is_some_and(|model| model.contains("[1m]"))
                    || used > CONTEXT_LIMIT;
                self.context = Some(ContextUsage {
                    used_tokens: used,
                    limit_tokens: Some(if large {
                        LARGE_CONTEXT_LIMIT
                    } else {
                        CONTEXT_LIMIT
                    }),
                });
            }
        }
        if side {
            return;
        }
        // Platzhalter von Claude Code (z. B. bei abgebrochenen Antworten) zählen nicht als Modell
        if let Some(model) = text(message, "model").filter(|model| !model.starts_with('<')) {
            self.model = Some(model.to_string());
        }
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            return;
        };
        for block in blocks {
            let name = text(block, "name").unwrap_or("");
            if text(block, "type") == Some("tool_use")
                && matches!(name, "TodoWrite" | "TaskCreate" | "TaskUpdate")
            {
                if let Some(id) = text(block, "id") {
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    self.pending
                        .insert(id.to_string(), (name.to_string(), input));
                }
            }
        }
    }

    /// Wendet erfolgreiche Ergebnisse offener Todo-Aufrufe an
    fn results(&mut self, value: &Value) {
        let Some(parts) = value.pointer("/message/content").and_then(Value::as_array) else {
            return;
        };
        let details = value.get("toolUseResult").unwrap_or(&Value::Null);
        for part in parts {
            let Some(id) = text(part, "tool_use_id") else {
                continue;
            };
            let Some((name, input)) = self.pending.remove(id) else {
                continue;
            };
            if flag(part, "is_error") {
                continue;
            }
            match name.as_str() {
                "TodoWrite" => self.write_todos(&input),
                "TaskCreate" => self.create_task(&input, details),
                _ => self.update_task(&input),
            }
        }
    }

    /// TodoWrite ersetzt die ganze Liste
    fn write_todos(&mut self, input: &Value) {
        let Some(todos) = input.get("todos").and_then(Value::as_array) else {
            return;
        };
        self.todos = todos
            .iter()
            .filter_map(|todo| {
                Some((
                    None,
                    TodoItem {
                        content: text(todo, "content")?.to_string(),
                        status: text(todo, "status")
                            .and_then(TodoStatus::from_str_opt)
                            .unwrap_or(TodoStatus::Pending),
                    },
                ))
            })
            .collect();
    }

    /// TaskCreate hängt eine offene Aufgabe an; die Nummer steht im Ergebnis
    fn create_task(&mut self, input: &Value, details: &Value) {
        let subject = text(input, "subject")
            .or_else(|| details.pointer("/task/subject").and_then(Value::as_str));
        let Some(subject) = subject else {
            return;
        };
        let id = details.pointer("/task/id").and_then(id_text);
        self.todos.push((
            id,
            TodoItem {
                content: subject.to_string(),
                status: TodoStatus::Pending,
            },
        ));
    }

    /// TaskUpdate ändert Stand oder Titel; `deleted` entfernt die Aufgabe
    fn update_task(&mut self, input: &Value) {
        let Some(id) = input.get("taskId").and_then(id_text) else {
            return;
        };
        let Some(index) = self
            .todos
            .iter()
            .position(|(task, _)| task.as_deref() == Some(id.as_str()))
        else {
            return;
        };
        if text(input, "status") == Some("deleted") {
            self.todos.remove(index);
            return;
        }
        let todo = &mut self.todos[index].1;
        if let Some(status) = text(input, "status").and_then(TodoStatus::from_str_opt) {
            todo.status = status;
        }
        if let Some(subject) = text(input, "subject") {
            todo.content = subject.to_string();
        }
    }
}

/// Eine Nummer als Text (Text oder Zahl)
fn id_text(value: &Value) -> Option<String> {
    match value {
        Value::String(id) if !id.is_empty() => Some(id.clone()),
        Value::Number(id) => Some(id.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixtures;
    use super::*;

    /// Füttert alle lesbaren Zeilen
    fn collect(content: &str) -> TranscriptFacts {
        let mut collector = FactsCollector::new();
        for line in content.lines() {
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                collector.feed(&value);
            }
        }
        collector.facts()
    }

    #[test]
    fn liest_quelle_branch_version_und_titel() {
        let facts = collect(&fixtures::transcript());
        assert_eq!(facts.client, Some(SessionClient::ClaudeDesktop));
        assert_eq!(facts.branch.as_deref(), Some("feature/demo"));
        assert_eq!(facts.version.as_deref(), Some("2.1.0"));
        assert_eq!(facts.title.as_deref(), Some("Demo-Titel"));
    }

    #[test]
    fn nimmt_tokens_kosten_und_dauer_aus_dem_letzten_kostenstand() {
        let facts = collect(&fixtures::transcript());
        assert_eq!(
            facts.tokens,
            TokenTotals {
                input: 120,
                output: 203,
                cache_read: 2300,
                cache_write: 260
            }
        );
        assert_eq!(facts.cost_usd, Some(0.42));
        assert_eq!(facts.duration_ms, Some(65_000));
    }

    #[test]
    fn summiert_ohne_kostenstand_die_tokens_je_antwort_einmal() {
        let facts = collect(&fixtures::transcript_without_costs());
        assert_eq!(
            facts.tokens,
            TokenTotals {
                input: 125,
                output: 203,
                cache_read: 2360,
                cache_write: 260
            }
        );
        assert_eq!(facts.cost_usd, None);
        assert_eq!(facts.duration_ms, None);
    }

    #[test]
    fn kontext_der_letzten_antwort_des_hauptverlaufs() {
        let facts = collect(&fixtures::transcript());
        assert_eq!(
            facts.context,
            Some(ContextUsage {
                used_tokens: 1205,
                limit_tokens: Some(200_000)
            })
        );
        let large = fixtures::line(fixtures::assistant(
            "a-gross",
            "m-gross",
            serde_json::json!({ "type": "text", "text": "x" }),
            [1, 1, 250_000, 0],
        ));
        let facts = collect(&format!("{}{large}", fixtures::transcript()));
        assert_eq!(facts.context.unwrap().limit_tokens, Some(1_000_000));
    }

    #[test]
    fn baut_die_todo_liste_aus_todowrite_und_tasks() {
        let facts = collect(&fixtures::transcript());
        assert_eq!(
            facts.todos,
            vec![
                TodoItem {
                    content: "Plan lesen".into(),
                    status: TodoStatus::Completed
                },
                TodoItem {
                    content: "Umsetzen".into(),
                    status: TodoStatus::InProgress
                },
                TodoItem {
                    content: "Tests schreiben".into(),
                    status: TodoStatus::Completed
                },
            ]
        );
    }

    #[test]
    fn geloeschte_und_gescheiterte_aufgaben_zaehlen_nicht() {
        let lines = [
            fixtures::tool_use(
                "a1",
                "m1",
                "t1",
                "TaskCreate",
                serde_json::json!({ "subject": "Weg" }),
                [0; 4],
            ),
            fixtures::tool_result(
                "u1",
                "t1",
                serde_json::json!("ok"),
                false,
                serde_json::json!({ "task": { "id": 1 } }),
            ),
            fixtures::tool_use(
                "a2",
                "m2",
                "t2",
                "TaskCreate",
                serde_json::json!({ "subject": "Fehler" }),
                [0; 4],
            ),
            fixtures::tool_result(
                "u2",
                "t2",
                serde_json::json!("nein"),
                true,
                serde_json::json!("Error"),
            ),
            fixtures::tool_use(
                "a3",
                "m3",
                "t3",
                "TaskUpdate",
                serde_json::json!({ "taskId": "1", "status": "deleted" }),
                [0; 4],
            ),
            fixtures::tool_result(
                "u3",
                "t3",
                serde_json::json!("ok"),
                false,
                serde_json::json!({}),
            ),
        ];
        let content: String = lines.into_iter().map(fixtures::line).collect();
        assert!(collect(&content).todos.is_empty());
    }
}
