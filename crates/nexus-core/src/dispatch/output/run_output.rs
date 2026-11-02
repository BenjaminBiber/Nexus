//! Liest die Ausgabe von `claude -p --output-format stream-json` (früher `json`) und
//! formuliert Abbruchgründe; portiert aus `packages/core/src/dispatch/RunOutputReader.ts`.
//!
//! Die Abbruchtexte stehen so in `board.DispatchRuns.abortReason` und werden vom
//! [`AbortReasonClassifier`](crate::dispatch::AbortReasonClassifier) in Kategorien einsortiert –
//! beide Schreibweisen sind Vertrag und dürfen sich nicht ändern.

use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::Value;

use super::stream_formatter::StreamEventFormatter;
use super::text::{first_line, last_chars};
use crate::dispatch::{RunResult, RunSettings};

/// Nutzungslimit; bewusst grob, den Wortlaut bestimmt Claude Code (ältere Versionen:
/// „usage limit reached“, neuere: „You've hit your session limit · resets …“, auch
/// Wochen- und Modell-Limits)
static USAGE_LIMIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)usage limit|limit reached|rate.?limit|hit (?:your|the)\b[^.\n]{0,30}\blimit")
        .expect("festes Muster")
});
/// Volles Kontextfenster; ebenfalls bewusst grob
static CONTEXT_OVERFLOW: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)prompt is too long|context length|context window|input is too long|exceeds the maximum context",
    )
    .expect("festes Muster")
});

/// Höchstens so viele abgelehnte Werkzeugaufrufe stehen im Abbruchgrund
const MAX_DENIED: usize = 5;

/// Zusammenfassung des Ergebnisses eines Laufs
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputSummary {
    pub subtype: Option<String>,
    pub is_error: bool,
    pub text: String,
    /// Von Claude Code abgelehnte Werkzeugaufrufe, z. B. „PowerShell: harness board show
    /// ARC-0010“ (höchstens 5, ohne Doppelte)
    pub denied: Vec<String>,
}

/// Verbrauch eines Modells in einem Lauf
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModelUsage {
    pub input_tokens: Option<f64>,
    pub output_tokens: Option<f64>,
    pub cache_read_tokens: Option<f64>,
    pub cache_write_tokens: Option<f64>,
    pub cost_usd: Option<f64>,
}

/// Verbrauch eines Laufs laut Ergebnis-Ereignis; fehlende Felder sind `None`
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunUsage {
    pub input_tokens: Option<f64>,
    pub output_tokens: Option<f64>,
    pub cache_read_tokens: Option<f64>,
    pub cache_write_tokens: Option<f64>,
    pub cost_usd: Option<f64>,
    pub num_turns: Option<f64>,
    pub duration_ms: Option<f64>,
    /// Hauptmodell: der Eintrag in `modelUsage` mit den höchsten Kosten
    pub model: Option<String>,
    /// Verbrauch je Modell in der Reihenfolge des Ergebnis-Ereignisses
    pub model_usage: IndexMap<String, ModelUsage>,
}

/// Liest die Ausgabe eines Laufs und formuliert Abbruchgründe.
#[derive(Debug, Default)]
pub struct RunOutputReader;

impl RunOutputReader {
    pub fn new() -> Self {
        Self
    }

    /// Kosten, Tokens, Schritte, Dauer und Modelle aus dem letzten Ergebnis-Ereignis;
    /// `None` ohne Ergebnis
    pub fn usage(&self, output: &str) -> Option<RunUsage> {
        let result = result_object(output)?;
        let empty = Value::Object(serde_json::Map::new());
        let usage = result.get("usage").unwrap_or(&empty);
        let mut models: IndexMap<String, ModelUsage> = IndexMap::new();
        if let Some(raw) = result.get("modelUsage").and_then(Value::as_object) {
            for (name, value) in raw {
                if !value.is_object() {
                    continue;
                }
                models.insert(
                    name.clone(),
                    ModelUsage {
                        input_tokens: number(value, "inputTokens"),
                        output_tokens: number(value, "outputTokens"),
                        cache_read_tokens: number(value, "cacheReadInputTokens"),
                        cache_write_tokens: number(value, "cacheCreationInputTokens"),
                        cost_usd: number(value, "costUSD"),
                    },
                );
            }
        }
        // Hauptmodell ist das teuerste; bei gleichen Kosten bleibt das erste (stabil wie in JS)
        let model = models
            .iter()
            .fold(None::<(&String, f64)>, |best, (name, usage)| {
                let cost = usage.cost_usd.unwrap_or(-1.0);
                match best {
                    Some((_, found)) if found >= cost => best,
                    _ => Some((name, cost)),
                }
            })
            .map(|(name, _)| name.clone());
        Some(RunUsage {
            cost_usd: number_of(result.get("total_cost_usd")),
            input_tokens: number(usage, "input_tokens"),
            output_tokens: number(usage, "output_tokens"),
            cache_read_tokens: number(usage, "cache_read_input_tokens"),
            cache_write_tokens: number(usage, "cache_creation_input_tokens"),
            num_turns: number_of(result.get("num_turns")),
            duration_ms: number_of(result.get("duration_ms")),
            model,
            model_usage: models,
        })
    }

    /// Das Ergebnisobjekt (`type: result`), wenn es sich finden lässt; sonst der Text mit
    /// lesbaren statt roher Ereignisse
    pub fn summarize(&self, output: &str) -> OutputSummary {
        match result_object(output) {
            Some(result) => OutputSummary {
                subtype: result
                    .get("subtype")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                is_error: result.get("is_error") == Some(&Value::Bool(true)),
                text: result
                    .get("result")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                denied: denials(result.get("permission_denials")),
            },
            None => OutputSummary {
                text: self.readable(output.trim()),
                ..OutputSummary::default()
            },
        }
    }

    /// Was von einem Lauf für einen Kommentar gezeigt wird: der Ergebnistext, sonst die
    /// Ausgabe mit lesbaren Ereignissen
    pub fn shown(&self, output: &str, summary: &OutputSummary) -> String {
        if summary.text.is_empty() {
            self.readable(output)
        } else {
            summary.text.clone()
        }
    }

    /// Ausgabe mit lesbaren Zeilen statt der Ereignisse von stream-json; Zeilen ohne JSON
    /// bleiben
    pub fn readable(&self, output: &str) -> String {
        StreamEventFormatter::new().format(output.trim()).join("\n")
    }

    /// Warum ein Lauf als abgebrochen gilt; `fallback` ersetzt den Grund für einen Lauf ohne
    /// Fehler (Standard: Ticket ohne Ausgang)
    pub fn abort_reason(
        &self,
        result: &RunResult,
        summary: &OutputSummary,
        run: &RunSettings,
        fallback: Option<&str>,
    ) -> String {
        let base = self.base_reason(result, summary, run, fallback);
        if summary.denied.is_empty() {
            base
        } else {
            format!(
                "{base}; abgelehnte Werkzeugaufrufe: {} (erlaubte Tools in den Dispatcher-Einstellungen prüfen)",
                summary.denied.join(", ")
            )
        }
    }

    /// Abbruchgrund ohne die abgelehnten Aufrufe
    fn base_reason(
        &self,
        result: &RunResult,
        summary: &OutputSummary,
        run: &RunSettings,
        fallback: Option<&str>,
    ) -> String {
        if result.timed_out {
            return format!(
                "Zeitlimit von {} min erreicht, Lauf abgebrochen",
                run.timeout_minutes
            );
        }
        if summary.subtype.as_deref() == Some("error_max_turns") {
            return format!("Maximale Zahl an Schritten ({}) erreicht", run.max_turns);
        }
        let failed = result.code.is_some_and(|code| code != 0);
        if failed || summary.is_error {
            let text = if summary.text.is_empty() {
                self.tail(&result.output, 1500)
            } else {
                summary.text.clone()
            };
            if Self::looks_like_context_overflow(&text) {
                return format!("Kontext voll: {}", first_line(&text, 200));
            }
        }
        if let Some(code) = result.code.filter(|code| *code != 0) {
            return format!("Claude endete mit Exit-Code {code}");
        }
        if summary.is_error {
            return format!(
                "Claude meldet einen Fehler ({})",
                summary.subtype.as_deref().unwrap_or("unbekannt")
            );
        }
        fallback.unwrap_or(
            "Lauf endete ohne Abschluss – das Ticket steht weder auf Erledigt noch auf Blockiert und hat keine offene Frage",
        )
        .to_string()
    }

    /// Sieht der Text nach einem Nutzungslimit aus?
    pub fn looks_like_usage_limit(text: &str) -> bool {
        USAGE_LIMIT.is_match(text)
    }

    /// Sieht der Text nach einem vollen Kontextfenster aus?
    pub fn looks_like_context_overflow(text: &str) -> bool {
        CONTEXT_OVERFLOW.is_match(text)
    }

    /// Endete der Lauf an einem Nutzungslimit? Nur bei fehlerhaftem Ende (Exit-Code ≠ 0 oder
    /// `is_error`, kein Zeitlimit) und nur in stderr bzw. dem Fehlertext – der Ergebnistext
    /// eines erfolgreichen Laufs darf „rate limit“ enthalten
    pub fn hit_usage_limit(&self, result: &RunResult) -> bool {
        if result.timed_out {
            return false;
        }
        let summary = self.summarize(&result.output);
        if !summary.is_error && result.code.is_none_or(|code| code == 0) {
            return false;
        }
        Self::looks_like_usage_limit(&error_text(&result.output, &summary))
    }

    /// Wurde Claude Code nicht gefunden? (Exit-Code 127 des Workers bzw. Meldung
    /// „claude-not-found“)
    pub fn claude_missing(&self, result: &RunResult) -> bool {
        result.code == Some(127) || result.output.contains("claude-not-found")
    }

    /// Ende der Ausgabe für einen Kommentar; Codezäune werden entschärft
    pub fn tail(&self, text: &str, max: usize) -> String {
        let clean = text.replace("```", "'''");
        let clean = clean.trim();
        if clean.chars().count() > max {
            format!("…{}", last_chars(clean, max))
        } else {
            clean.to_string()
        }
    }
}

/// Letztes Ergebnisobjekt (`type: result`) der Ausgabe oder `None`
fn result_object(output: &str) -> Option<serde_json::Map<String, Value>> {
    let text = output.trim();
    let mut candidates: Vec<&str> = vec![text];
    candidates.extend(
        text.split('\n')
            .map(|line| line.trim_end_matches('\r'))
            .rev(),
    );
    for candidate in candidates {
        if !candidate.starts_with('{') {
            continue;
        }
        if let Ok(Value::Object(parsed)) = serde_json::from_str::<Value>(candidate) {
            if parsed.get("type").and_then(Value::as_str) == Some("result") {
                return Some(parsed);
            }
        }
    }
    None
}

/// Ist der Text das JSON-Ergebnisobjekt (`type: result`)?
fn is_result_object(text: &str) -> bool {
    text.starts_with('{')
        && serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|parsed| {
                parsed
                    .get("type")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .as_deref()
            == Some("result")
}

/// Fehlertext eines Laufs: alles außer den Ereignissen von stream-json (Claudes Text zählt
/// nicht) und dem Ergebnisobjekt, dazu dessen Text nur bei `is_error`
fn error_text(output: &str, summary: &OutputSummary) -> String {
    let text = output.trim();
    let own = if summary.is_error {
        summary.text.clone()
    } else {
        String::new()
    };
    if is_result_object(text) {
        return own;
    }
    let formatter = StreamEventFormatter::new();
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !is_result_object(line.trim()) && !formatter.is_event(line))
        .map(str::to_string)
        .collect();
    lines.push(own);
    lines.join("\n")
}

/// Abgelehnte Aufrufe aus `permission_denials` als kurze Texte (höchstens 5, ohne Doppelte)
fn denials(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let tool = item
            .get("tool_name")
            .and_then(Value::as_str)
            .unwrap_or("unbekannt");
        let command = item
            .get("tool_input")
            .and_then(|input| input.get("command"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|command| !command.is_empty());
        let text = match command {
            Some(command) => format!("{tool}: {}", super::text::take_chars(command, 120)),
            None => tool.to_string(),
        };
        if !out.contains(&text) {
            out.push(text);
        }
        if out.len() == MAX_DENIED {
            break;
        }
    }
    out
}

/// Eine endliche Zahl unter diesem Schlüssel, sonst `None` (wie `Number.isFinite` in JS)
fn number(value: &Value, key: &str) -> Option<f64> {
    number_of(value.get(key))
}

/// Dasselbe für einen schon gelesenen Wert
fn number_of(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reader() -> RunOutputReader {
        RunOutputReader::new()
    }

    /// Die Einstellungen des Laufs aus den TypeScript-Tests
    fn run() -> RunSettings {
        RunSettings {
            model: Some("opus".to_string()),
            effort: Some("high".to_string()),
            max_turns: 80,
            timeout_minutes: 90,
            profile: "standard".to_string(),
            why: String::new(),
            reason: None,
            config_hash: None,
            rule_index: None,
            escalated_from: None,
            allowed_accounts: None,
            accounts_from: None,
            account: None,
            account_why: None,
        }
    }

    fn result(code: Option<i64>, timed_out: bool, output: &str) -> RunResult {
        RunResult {
            code,
            timed_out,
            output: output.to_string(),
            ..RunResult::default()
        }
    }

    #[test]
    fn findet_das_ergebnisobjekt_von_output_format_json() {
        let out = r#"Log
{"type":"result","subtype":"error_max_turns","is_error":true,"result":"halb fertig"}"#;
        assert_eq!(
            reader().summarize(out),
            OutputSummary {
                subtype: Some("error_max_turns".to_string()),
                is_error: true,
                text: "halb fertig".to_string(),
                denied: Vec::new(),
            }
        );
        assert_eq!(
            reader().summarize("kein json"),
            OutputSummary {
                subtype: None,
                is_error: false,
                text: "kein json".to_string(),
                denied: Vec::new(),
            }
        );
    }

    #[test]
    fn liest_die_ausgabe_von_stream_json() {
        let lines = [
            json!({ "type": "system", "subtype": "init", "model": "opus" }).to_string(),
            json!({ "type": "assistant", "message": { "content": [{ "type": "text", "text": "Ich baue einen rate limit ein." }] } }).to_string(),
            json!({ "type": "result", "subtype": "success", "is_error": false, "result": "fertig", "permission_denials": [{ "tool_name": "Bash", "tool_input": { "command": "git push" } }] }).to_string(),
        ];
        let out = lines.join("\n");
        assert_eq!(
            reader().summarize(&out),
            OutputSummary {
                subtype: Some("success".to_string()),
                is_error: false,
                text: "fertig".to_string(),
                denied: vec!["Bash: git push".to_string()],
            }
        );
        // Ohne Ergebnis (z. B. Zeitlimit): die lesbaren Zeilen statt roher Ereignisse
        let cut = format!("{}\nstderr-Meldung", lines[..2].join("\n"));
        let expected =
            "Claude startet (opus)\nClaude: Ich baue einen rate limit ein.\nstderr-Meldung";
        assert_eq!(reader().summarize(&cut).text, expected);
        let summary = reader().summarize(&cut);
        assert_eq!(reader().shown(&cut, &summary), expected);
        // Leerer Ergebnistext: lesbare Zeilen statt roher Ereignisse
        let empty = [
            lines[1].clone(),
            json!({ "type": "result", "subtype": "success", "is_error": false, "result": "" })
                .to_string(),
        ]
        .join("\n");
        let summary = reader().summarize(&empty);
        assert_eq!(
            reader().shown(&empty, &summary),
            "Claude: Ich baue einen rate limit ein.\nfertig"
        );
    }

    #[test]
    fn erkennt_im_stream_json_ein_nutzungslimit_nur_in_stderr_bzw_im_fehlerergebnis() {
        let talk = json!({ "type": "assistant", "message": { "content": [{ "type": "text", "text": "usage limit reached?" }] } }).to_string();
        let ok =
            json!({ "type": "result", "subtype": "success", "is_error": false, "result": "ok" })
                .to_string();
        assert!(!reader().hit_usage_limit(&result(Some(1), false, &format!("{talk}\n{ok}"))));
        assert!(!reader().hit_usage_limit(&result(Some(1), false, &talk)));
        assert!(reader().hit_usage_limit(&result(
            Some(1),
            false,
            &format!("{talk}\nClaude AI usage limit reached|1760000000")
        )));
        let error = json!({ "type": "result", "subtype": "error_during_execution", "is_error": true, "result": "Usage limit reached" }).to_string();
        assert!(reader().hit_usage_limit(&result(Some(1), false, &format!("{talk}\n{error}"))));
    }

    #[test]
    fn nennt_den_abbruchgrund_in_fester_reihenfolge() {
        let none = OutputSummary::default();
        let reason = |result: RunResult, summary: OutputSummary| {
            reader().abort_reason(&result, &summary, &run(), None)
        };
        assert_eq!(
            reason(result(None, true, ""), none.clone()),
            "Zeitlimit von 90 min erreicht, Lauf abgebrochen"
        );
        assert_eq!(
            reason(
                result(Some(1), false, ""),
                OutputSummary {
                    subtype: Some("error_max_turns".to_string()),
                    ..none.clone()
                }
            ),
            "Maximale Zahl an Schritten (80) erreicht"
        );
        assert_eq!(
            reason(result(Some(2), false, ""), none.clone()),
            "Claude endete mit Exit-Code 2"
        );
        assert_eq!(
            reason(
                result(Some(0), false, ""),
                OutputSummary {
                    is_error: true,
                    subtype: Some("error_x".to_string()),
                    ..none.clone()
                }
            ),
            "Claude meldet einen Fehler (error_x)"
        );
        assert_eq!(
            reason(result(Some(0), false, ""), none),
            "Lauf endete ohne Abschluss – das Ticket steht weder auf Erledigt noch auf Blockiert und hat keine offene Frage"
        );
    }

    #[test]
    fn erkennt_ein_volles_kontextfenster_vor_exit_code_und_claude_fehler() {
        let context = OutputSummary {
            subtype: Some("error_during_execution".to_string()),
            is_error: true,
            text: "Prompt is too long: 215000 tokens > 200000 maximum".to_string(),
            denied: Vec::new(),
        };
        let reason = |result: RunResult, summary: &OutputSummary| {
            reader().abort_reason(&result, summary, &run(), None)
        };
        assert_eq!(
            reason(result(Some(1), false, ""), &context),
            "Kontext voll: Prompt is too long: 215000 tokens > 200000 maximum"
        );
        // Zeitlimit und Schrittlimit gewinnen weiter
        assert_eq!(
            reason(result(Some(1), true, ""), &context),
            "Zeitlimit von 90 min erreicht, Lauf abgebrochen"
        );
        assert_eq!(
            reason(
                result(Some(1), false, ""),
                &OutputSummary {
                    subtype: Some("error_max_turns".to_string()),
                    ..context.clone()
                }
            ),
            "Maximale Zahl an Schritten (80) erreicht"
        );
        // Ein normaler Fehler ohne das Muster bleibt „Claude meldet einen Fehler“
        assert_eq!(
            reason(
                result(Some(0), false, ""),
                &OutputSummary {
                    text: "irgendein Fehler".to_string(),
                    ..context
                }
            ),
            "Claude meldet einen Fehler (error_during_execution)"
        );
    }

    #[test]
    fn erkennt_volle_kontextfenster_grob() {
        assert!(RunOutputReader::looks_like_context_overflow(
            "Prompt is too long"
        ));
        assert!(RunOutputReader::looks_like_context_overflow(
            "context window exceeded"
        ));
        assert!(!RunOutputReader::looks_like_context_overflow("alles gut"));
    }

    #[test]
    fn liest_abgelehnte_werkzeugaufrufe_dedupliziert_und_hochstens_funf() {
        let denial = |tool: &str, command: Option<&str>| match command {
            Some(command) => json!({ "tool_name": tool, "tool_input": { "command": command } }),
            None => json!({ "tool_name": tool, "tool_input": {} }),
        };
        let out = json!({
            "type": "result", "subtype": "success", "is_error": false, "result": "",
            "permission_denials": [
                denial("PowerShell", Some("harness board show ARC-0010")),
                denial("PowerShell", Some("harness board show ARC-0010")),
                denial("Edit", None),
                denial("A", Some("1")),
                denial("B", Some("2")),
                denial("C", Some("3")),
                denial("D", Some("4")),
            ],
        })
        .to_string();
        let summary = reader().summarize(&out);
        assert_eq!(
            summary.denied,
            vec![
                "PowerShell: harness board show ARC-0010".to_string(),
                "Edit".to_string(),
                "A: 1".to_string(),
                "B: 2".to_string(),
                "C: 3".to_string(),
            ]
        );
        let one = OutputSummary {
            denied: summary.denied[..1].to_vec(),
            ..summary
        };
        assert_eq!(
            reader().abort_reason(&result(Some(0), false, &out), &one, &run(), None),
            "Lauf endete ohne Abschluss – das Ticket steht weder auf Erledigt noch auf Blockiert und hat keine offene Frage; abgelehnte Werkzeugaufrufe: PowerShell: harness board show ARC-0010 (erlaubte Tools in den Dispatcher-Einstellungen prüfen)"
        );
    }

    #[test]
    fn erkennt_nutzungslimits_grob() {
        assert!(RunOutputReader::looks_like_usage_limit(
            "Claude usage limit reached"
        ));
        assert!(RunOutputReader::looks_like_usage_limit("rate-limit hit"));
        assert!(!RunOutputReader::looks_like_usage_limit("alles gut"));
    }

    #[test]
    fn erkennt_die_limit_meldungen_neuerer_claude_code_versionen() {
        for text in [
            "You've hit your session limit · resets 12:10pm (Europe/Berlin)",
            "You've hit your weekly limit · resets Oct 23, 4pm",
            "You've hit your Opus limit · resets 6pm",
            "You've hit your limit · resets 3pm",
        ] {
            assert!(RunOutputReader::looks_like_usage_limit(text), "{text}");
        }
        // Wie im Protokoll: Exit-Code 1, die Meldung steht allein in der Ausgabe
        assert!(reader().hit_usage_limit(&result(
            Some(1),
            false,
            "You've hit your session limit · resets 12:10pm (Europe/Berlin)"
        )));
        assert!(!RunOutputReader::looks_like_usage_limit(
            "Test hit the breakpoint. Speicherlimit erhöht"
        ));
    }

    #[test]
    fn erkennt_ein_nutzungslimit_nur_bei_fehlerhaftem_ende_und_nur_im_fehlertext() {
        let ok = json!({ "type": "result", "subtype": "success", "is_error": false, "result": "rate limit der API eingebaut" }).to_string();
        assert!(!reader().hit_usage_limit(&result(Some(0), false, &ok)));
        // Exit ≠ 0, aber „rate limit“ steht nur im erfolgreichen Ergebnistext
        assert!(!reader().hit_usage_limit(&result(Some(1), false, &ok)));
        assert!(reader().hit_usage_limit(&result(
            Some(1),
            false,
            &format!("{ok}\nClaude usage limit reached")
        )));
        assert!(reader().hit_usage_limit(&result(Some(1), false, "Claude usage limit reached")));
        let error = json!({ "type": "result", "subtype": "error_during_execution", "is_error": true, "result": "Usage limit reached" }).to_string();
        assert!(reader().hit_usage_limit(&result(Some(0), false, &error)));
        assert!(!reader().hit_usage_limit(&result(None, true, "usage limit")));
    }

    #[test]
    fn erkennt_fehlendes_claude_code() {
        assert!(reader().claude_missing(&result(Some(127), false, "spawn claude ENOENT")));
        assert!(reader().claude_missing(&result(Some(1), false, "claude-not-found")));
        assert!(!reader().claude_missing(&result(Some(1), false, "x")));
    }

    #[test]
    fn kurzt_das_ende_und_entscharft_codezaune() {
        assert_eq!(reader().tail("```x```", 100), "'''x'''");
        assert_eq!(reader().tail("abcdef", 3), "…def");
    }

    #[test]
    fn liest_kosten_tokens_schritte_dauer_und_modell_aus_dem_ergebnis_ereignis() {
        let out = [
            json!({ "type": "system", "subtype": "init", "model": "sonnet" }).to_string(),
            single(),
        ]
        .join("\n");
        let usage = reader().usage(&out).expect("Ergebnis-Ereignis");
        assert_eq!(usage.cost_usd, Some(0.2369144));
        assert_eq!(usage.input_tokens, Some(18.0));
        assert_eq!(usage.output_tokens, Some(1782.0));
        assert_eq!(usage.cache_read_tokens, Some(349_024.0));
        assert_eq!(usage.cache_write_tokens, Some(46039.0));
        assert_eq!(usage.num_turns, Some(17.0));
        assert_eq!(usage.duration_ms, Some(241_000.0));
        assert_eq!(usage.model.as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(
            usage.model_usage.get("claude-sonnet-5-5"),
            Some(&ModelUsage {
                input_tokens: Some(18.0),
                output_tokens: Some(1782.0),
                cache_read_tokens: Some(349_024.0),
                cache_write_tokens: Some(46039.0),
                cost_usd: Some(0.2369144),
            })
        );
    }

    #[test]
    fn nimmt_als_hauptmodell_das_teuerste_aus_modelusage() {
        let usage = reader().usage(&mixed()).expect("Ergebnis-Ereignis");
        assert_eq!(usage.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(usage.cost_usd, Some(0.369613015));
        assert_eq!(
            usage.model_usage.keys().collect::<Vec<&String>>(),
            vec!["claude-haiku-5-5", "claude-opus-5-5"]
        );
    }

    #[test]
    fn liefert_none_ohne_ergebnis_ereignis_oder_bei_kaputtem_json() {
        assert_eq!(reader().usage(""), None);
        assert_eq!(
            reader()
                .usage(&json!({ "type": "assistant", "message": { "content": [] } }).to_string()),
            None
        );
        assert_eq!(reader().usage(r#"{"type":"result", kaputt"#), None);
    }

    #[test]
    fn setzt_fehlende_felder_auf_none_auch_bei_is_error() {
        let usage = reader()
            .usage(
                &json!({ "type": "result", "subtype": "error_during_execution", "is_error": true })
                    .to_string(),
            )
            .expect("Ergebnis-Ereignis");
        assert_eq!(usage, RunUsage::default());
    }

    /// Gekürzte echte Ergebniszeile aus einem Lauf-Protokoll vom 15.10. (ein Modell)
    fn single() -> String {
        json!({
            "type": "result", "subtype": "success", "is_error": false, "duration_ms": 241_000,
            "num_turns": 17, "result": "fertig", "total_cost_usd": 0.2369144,
            "usage": { "input_tokens": 18, "cache_creation_input_tokens": 46039, "cache_read_input_tokens": 349_024, "output_tokens": 1782 },
            "modelUsage": {
                "claude-sonnet-5-5": { "inputTokens": 18, "outputTokens": 1782, "cacheReadInputTokens": 349_024, "cacheCreationInputTokens": 46039, "costUSD": 0.2369144 },
            },
        })
        .to_string()
    }

    /// Gekürzte echte Ergebniszeile mit Opus und Haiku
    fn mixed() -> String {
        json!({
            "type": "result", "subtype": "success", "is_error": false, "duration_ms": 60000,
            "num_turns": 9, "result": "ok", "total_cost_usd": 0.369613015,
            "usage": { "input_tokens": 14, "cache_creation_input_tokens": 31384, "cache_read_input_tokens": 275_494, "output_tokens": 2939 },
            "modelUsage": {
                "claude-haiku-5-5": { "inputTokens": 6, "outputTokens": 830, "cacheReadInputTokens": 54024, "cacheCreationInputTokens": 29203, "costUSD": 0.004606215 },
                "claude-opus-5-5": { "inputTokens": 14, "outputTokens": 2939, "cacheReadInputTokens": 275_494, "cacheCreationInputTokens": 31384, "costUSD": 0.3650068 },
            },
        })
        .to_string()
    }
}
