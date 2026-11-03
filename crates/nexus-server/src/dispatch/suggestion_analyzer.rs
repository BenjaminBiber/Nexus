//! Claude-Analyse der Optimierungs-Vorschläge: verdichtete Kennzahlen an `claude -p`, Antwort
//! als JSON-Array prüfen und merken. Portiert aus
//! `packages/server/src/dispatch/SuggestionAnalyzer.ts`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use nexus_core::clock::Clock;
use nexus_core::dispatch::optimization::{
    suggestion_id, AnalysisWorkspace, ConfigPatchOp, ConfigPatcher, StoredAnalysis, Suggestion,
    SuggestionKind, SuggestionSource, SuggestionTarget,
};
use nexus_core::dispatch::{
    ClaudeInvocation, DispatchConfig, DispatchConfigNormalizer, DispatchConfigRead,
};
use nexus_core::{HarnessError, HarnessHome, Result};
use serde_json::{json, Value};

use crate::dispatch::account_pool::side_run_account;
use crate::dispatch::claude_process::ClaudeRunInput;
use crate::dispatch::claude_runner::ClaudeRunner;
use crate::dispatch::result_envelope::{excerpt, ClaudeResultEnvelope};
use crate::dispatch::suggestion_service::{
    DispatchSuggestionService, SuggestionContext, SuggestionListDto,
};

/// Zeitlimit einer Analyse
const TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// So viele Antworten von Claude werden höchstens zu Vorschlägen
const MAX_SUGGESTIONS: usize = 10;
/// Die Analyse liest nur den Prompt – keine Shell, keine Dateien, kein Web
const DENIED_TOOLS: [&str; 8] = [
    "Bash",
    "Edit",
    "Write",
    "MultiEdit",
    "NotebookEdit",
    "WebFetch",
    "WebSearch",
    "Task",
];

/// Startet die Claude-Analyse und merkt ihr Ergebnis.
pub struct SuggestionAnalyzer {
    service: Arc<DispatchSuggestionService>,
    claude: Arc<dyn ClaudeRunner>,
    workspace: AnalysisWorkspace,
    home: HarnessHome,
    envelope: ClaudeResultEnvelope,
    patcher: ConfigPatcher,
    normalizer: DispatchConfigNormalizer,
    invocation: ClaudeInvocation,
    clock: Clock,
    /// Es läuft höchstens eine Analyse
    running: AtomicBool,
}

impl SuggestionAnalyzer {
    /// Analyse zum Dienst der Vorschläge und diesem Start von Claude
    pub fn new(
        service: Arc<DispatchSuggestionService>,
        claude: Arc<dyn ClaudeRunner>,
        home: HarnessHome,
        clock: Clock,
    ) -> Self {
        Self {
            service,
            claude,
            workspace: AnalysisWorkspace::new(home.clone()),
            home,
            envelope: ClaudeResultEnvelope::new(),
            patcher: ConfigPatcher::new(),
            normalizer: DispatchConfigNormalizer::new(),
            invocation: ClaudeInvocation::new(),
            clock,
            running: AtomicBool::new(false),
        }
    }

    /// Startet eine Analyse (nur eine gleichzeitig) und liefert die neu berechnete Liste
    pub async fn analyze(&self) -> Result<SuggestionListDto> {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(
                HarnessError::new("conflict", "Es läuft schon eine Analyse.")
                    .with_details(json!({ "reason": "analysis-running" }))
                    .into(),
            );
        }
        let result = self.run().await;
        self.running.store(false, Ordering::SeqCst);
        result
    }

    /// Eine Analyse von Anfang bis Ende
    async fn run(&self) -> Result<SuggestionListDto> {
        let context = self.service.context().await?;
        let cfg = context.read.config.clone();
        let cwd = self.workspace.dir()?;
        // Läufe außerhalb des Dispatchers nehmen den Account mit der meisten freien Kapazität
        let account = side_run_account(&self.home, &cfg, self.clock.now()).await;
        let mut env = self.invocation.env(
            &process_env(),
            &cfg,
            None,
            None,
            account.config_dir.as_deref(),
        );
        self.invocation
            .apply_oauth_token(&mut env, &cfg, account.oauth_token.as_deref());
        let result = self
            .claude
            .run(ClaudeRunInput {
                command: cfg.claude.command.clone(),
                args: self.args(&cfg),
                prompt: self.prompt(&context)?,
                cwd,
                env,
                timeout: TIMEOUT,
                cancel: None,
            })
            .await;
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
                "Claude hat die Analyse nicht innerhalb von 5 Minuten beendet.",
            )
            .into());
        }
        if result.code != Some(0) {
            return Err(HarnessError::new(
                "analysis-failed",
                format!(
                    "Claude endete mit Exit-Code {}: {}",
                    match result.code {
                        Some(code) => code.to_string(),
                        None => "null".to_string(),
                    },
                    excerpt(&result.output)
                ),
            )
            .into());
        }
        let parsed = self.envelope.parse(&result.output)?;
        let (suggestions, skipped) = self.suggestions(&parsed.text, &context)?;
        self.service.save_analysis(StoredAnalysis {
            at: self
                .clock
                .now()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            config_hash: context.config_hash.clone(),
            cost_usd: parsed.cost_usd,
            suggestions,
            skipped: Some(skipped),
        })?;
        self.service.list().await
    }

    /// Aufrufargumente: ein Durchgang, Antwort als JSON, keine schreibenden Werkzeuge
    fn args(&self, cfg: &DispatchConfig) -> Vec<String> {
        let mut args: Vec<String> = ["-p", "--output-format", "json", "--max-turns", "1"]
            .iter()
            .map(|arg| arg.to_string())
            .collect();
        if let Some(model) = cfg
            .claude
            .model
            .as_deref()
            .filter(|model| !model.is_empty())
        {
            args.push("--model".to_string());
            args.push(model.to_string());
        }
        args.push("--permission-mode".to_string());
        args.push("default".to_string());
        args.push("--disallowedTools".to_string());
        args.extend(DENIED_TOOLS.iter().map(|tool| tool.to_string()));
        args
    }

    /// Arbeitsanweisung: Erklärung des Dispatchers, Antwortformat und die verdichteten Daten
    fn prompt(&self, context: &SuggestionContext) -> Result<String> {
        let c = &context.read.config;
        let data = json!({
            "config": {
                "profiles": c.profiles,
                "defaultProfile": c.default_profile,
                "rules": c.rules,
                "ladder": c.ladder,
                "escalate": c.escalate,
                "maxAttempts": c.max_attempts,
            },
            "facts": context.facts,
            "existing": context
                .advice
                .suggestions
                .iter()
                .map(|suggestion| json!({ "title": suggestion.title, "patch": suggestion.patch }))
                .collect::<Vec<Value>>(),
        });
        let data = serde_json::to_string_pretty(&data).map_err(nexus_core::Error::unexpected)?;
        Ok([
            "Du berätst bei der Einstellung eines Dispatchers, der Tickets von Claude Code abarbeiten lässt. Jedes Ticket startet auf einem Profil",
            "(Modell, Effort, maxTurns = Schritte, timeoutMinutes); scheitert ein Lauf, eskaliert der Dispatcher entlang der Leiter (ladder) zum nächsten Profil.",
            "Regeln (rules) wählen das Startprofil; ohne passende Regel gilt defaultProfile.",
            "",
            "Werte die Kennzahlen unten aus und schlage höchstens 5 Verbesserungen vor, die über die bereits vorhandenen Vorschläge (existing) hinausgehen.",
            "Begründe jeden Vorschlag mit Zahlen aus den Daten. Gibt es nichts zu verbessern, antworte mit [].",
            "",
            "Antworte ausschließlich mit einem JSON-Array, ohne Markdown und ohne Werkzeuge. Jedes Element:",
            "{\"title\": string, \"reason\": string, \"target\": {\"profile\"?: string, \"ruleIndex\"?: number}, \"patch\": [{\"path\": string, \"from\": any, \"to\": any}] | null}",
            "Erlaubte Pfade: profiles.<name>.maxTurns, profiles.<name>.timeoutMinutes, profiles.<name>.model, profiles.<name>.effort, defaultProfile,",
            "rules.<Index ab 0>.profile, escalation (die ganze Leiter als Liste). \"from\" ist der aktuelle Wert aus config. Ohne sichere Änderung: \"patch\": null.",
            "",
            "Daten:",
            &data,
        ]
        .join("\n"))
    }

    /// Antworttext → Vorschläge; ungültige Änderungen bleiben als Hinweis (`patch` null)
    /// erhalten, unbrauchbare Elemente werden gezählt
    fn suggestions(
        &self,
        text: &str,
        context: &SuggestionContext,
    ) -> Result<(Vec<Suggestion>, i64)> {
        let items = array_of(text).ok_or_else(|| {
            HarnessError::new(
                "analysis-failed",
                format!("Antwort nicht lesbar: {}", excerpt(text)),
            )
        })?;
        let mut skipped = items.len().saturating_sub(MAX_SUGGESTIONS) as i64;
        let mut suggestions: Vec<Suggestion> = vec![];
        for item in items.iter().take(MAX_SUGGESTIONS) {
            let title = item.get("title").and_then(Value::as_str);
            let Some(title) = title.filter(|title| !title.trim().is_empty()) else {
                skipped += 1;
                continue;
            };
            let patch = self.valid_patch(item.get("patch"), &context.read);
            let target = SuggestionTarget {
                profile: item
                    .get("target")
                    .and_then(|target| target.get("profile"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                rule_index: item
                    .get("target")
                    .and_then(|target| target.get("ruleIndex"))
                    .and_then(Value::as_f64)
                    .map(|index| index as i64),
            };
            let fingerprint = format!(
                "claude:{title}:{}",
                serde_json::to_string(&patch).unwrap_or_else(|_| "null".to_string())
            );
            suggestions.push(Suggestion {
                id: suggestion_id(&fingerprint),
                fingerprint,
                source: SuggestionSource::Claude,
                kind: SuggestionKind::Claude,
                title: title.trim().to_string(),
                reason: item
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                target,
                patch,
                weight: 0.0,
                evidence: None,
            });
        }
        Ok((suggestions, skipped))
    }

    /// Nur erlaubte Pfade mit stimmendem Ausgangswert, deren Ergebnis die Prüfung der
    /// Einstellung besteht; sonst `None` (Hinweis)
    fn valid_patch(
        &self,
        value: Option<&Value>,
        read: &DispatchConfigRead,
    ) -> Option<Vec<ConfigPatchOp>> {
        let items = value?.as_array()?;
        if items.is_empty() {
            return None;
        }
        let ops: Vec<ConfigPatchOp> = items
            .iter()
            .map(|item| serde_json::from_value::<ConfigPatchOp>(item.clone()).ok())
            .collect::<Option<Vec<ConfigPatchOp>>>()?;
        if !ops.iter().all(|op| self.patcher.allowed(&op.path)) {
            return None;
        }
        let patched = self.patcher.apply(&read.raw, &read.config, &ops).ok()?;
        self.normalizer.normalize(&patched).ok()?;
        Some(ops)
    }
}

/// Das erste JSON-Array im Antworttext (Claude schreibt gern noch etwas davor)
fn array_of(text: &str) -> Option<Vec<Value>> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    if end <= start {
        return None;
    }
    match serde_json::from_str::<Value>(&text[start..=end]) {
        Ok(Value::Array(items)) => Some(items),
        _ => None,
    }
}

/// Die Umgebung des Servers als Karte (Grundlage der Umgebung des Laufs)
fn process_env() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_das_json_array_aus_einer_antwort_mit_beiwerk() {
        assert_eq!(
            array_of("Hier: [{\"title\":\"A\"}] – fertig"),
            Some(vec![json!({ "title": "A" })])
        );
        assert_eq!(array_of("[]"), Some(vec![]));
        assert!(array_of("Ich schlage vor, mehr Schritte zu geben.").is_none());
        assert!(array_of("[kaputt").is_none());
    }
}
