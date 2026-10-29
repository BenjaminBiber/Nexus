//! Gemeinsames der drei Docs-Dienste, die Claude rufen: Aufrufargumente, Umgebung, Zeitlimit,
//! das Schneiden des JSON-Arrays aus der Antwort und das Dokument mit Zeilennummern.
//!
//! Die Stücke stehen in TypeScript wortgleich in `TicketDraftService.ts`,
//! `DocQuestionService.ts` und `DocRevisionService.ts`; hier stehen sie einmal.

use std::collections::BTreeMap;
use std::time::Duration;

use nexus_core::dispatch::{ClaudeInvocation, DispatchConfig, DispatchConfigStore};
use nexus_core::docs::markdown_sections::split_lines;
use nexus_core::{HarnessError, HarnessHome, Result};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::dispatch::claude_process::ClaudeRunInput;
use crate::dispatch::claude_runner::analysis_dir;
use crate::dispatch::result_envelope::excerpt;

/// Fünf Minuten je Lauf (wie `TIMEOUT_MS` in TypeScript)
pub const TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Die Erzeugung liest nur den Prompt – keine Shell, keine Dateien, kein Web
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

/// Dispatcher-Einstellung für Befehl, Modell und Umgebung; ohne Einstellung die Vorgaben
pub fn config(home: &HarnessHome) -> Result<DispatchConfig> {
    Ok(DispatchConfigStore::new(home.clone()).read()?.config)
}

/// Aufrufargumente: ein Durchgang, Antwort als JSON, keine schreibenden Werkzeuge
pub fn args(cfg: &DispatchConfig) -> Vec<String> {
    let mut args: Vec<String> = ["-p", "--output-format", "json", "--max-turns", "1"]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    if let Some(model) = &cfg.claude.model {
        args.push("--model".to_string());
        args.push(model.clone());
    }
    args.push("--permission-mode".to_string());
    args.push("default".to_string());
    args.push("--disallowedTools".to_string());
    args.extend(DENIED_TOOLS.iter().map(|tool| tool.to_string()));
    args
}

/// Ein Lauf im leeren Arbeitsordner der Analyse: Befehl und Umgebung aus der Einstellung,
/// Anweisung über stdin, Zeitlimit fünf Minuten
pub fn run_input(
    home: &HarnessHome,
    cfg: &DispatchConfig,
    prompt: String,
    cancel: Option<CancellationToken>,
) -> Result<ClaudeRunInput> {
    let env: BTreeMap<String, String> = std::env::vars().collect();
    Ok(ClaudeRunInput {
        command: cfg.claude.command.clone(),
        args: args(cfg),
        prompt,
        cwd: analysis_dir(home)?,
        env: ClaudeInvocation::new().env(&env, cfg, None, None),
        timeout: TIMEOUT,
        cancel,
    })
}

/// JSON-Array aus dem Antworttext schneiden; nichts Lesbares ergibt „analysis-failed“
pub fn items(text: &str) -> Result<Value> {
    let start = text.find('[');
    let end = text.rfind(']');
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

/// Dokument mit vorangestellten Zeilennummern, damit Claude die Quelle nennen kann
pub fn numbered(content: &str) -> String {
    split_lines(content)
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{}\t{line}", index + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::dispatch::DispatchConfigNormalizer;

    /// Einstellung mit Modell
    fn cfg(model: Option<&str>) -> DispatchConfig {
        let raw = match model {
            Some(model) => serde_json::json!({ "claude": { "model": model } }),
            None => serde_json::json!({ "claude": { "model": null } }),
        };
        DispatchConfigNormalizer::new().normalize(&raw).unwrap()
    }

    #[test]
    fn nennt_einen_durchgang_json_und_die_verbotenen_werkzeuge() {
        let args = args(&cfg(Some("opus")));

        assert_eq!(
            args[..5],
            ["-p", "--output-format", "json", "--max-turns", "1"]
        );
        assert!(args.windows(2).any(|paar| paar == ["--model", "opus"]));
        assert!(args
            .windows(2)
            .any(|paar| paar == ["--permission-mode", "default"]));
        assert!(args
            .windows(2)
            .any(|paar| paar == ["--disallowedTools", "Bash"]));
        assert_eq!(args.last().map(String::as_str), Some("Task"));
    }

    #[test]
    fn laesst_das_modell_weg_wenn_keines_eingestellt_ist() {
        assert!(!args(&cfg(None)).contains(&"--model".to_string()));
    }

    #[test]
    fn schneidet_das_array_aus_der_antwort_und_meldet_unlesbares() {
        assert_eq!(
            items("Hier: [{\"a\":1}] – fertig").unwrap(),
            serde_json::json!([{ "a": 1 }])
        );
        let error = items("Ich würde drei Tickets vorschlagen.").unwrap_err();
        assert_eq!(error.code(), Some("analysis-failed"));
        assert!(error.to_string().contains("nicht lesbar"));
        assert_eq!(
            items("[kaputt").unwrap_err().code(),
            Some("analysis-failed")
        );
    }

    #[test]
    fn stellt_jeder_zeile_ihre_nummer_voran() {
        assert_eq!(numbered("eins\nzwei"), "1\teins\n2\tzwei");
    }
}
