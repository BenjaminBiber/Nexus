//! Argumente und Umgebung für `claude -p`; portiert aus
//! `packages/core/src/dispatch/ClaudeInvocation.ts`. Die Arbeitsanweisung kommt über stdin.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dispatch::config::{
    planning_denied_tools, DispatchConfig, RunSettings, PLANNING_TOOLS, RESEARCH_DENIED_TOOLS,
    RESEARCH_PERMISSION_MODE, RESEARCH_TOOLS,
};

/// Vorgabe des Bash-Zeitlimits eines Werkzeugaufrufs in Millisekunden (15 min)
const BASH_DEFAULT_TIMEOUT_MS: i64 = 15 * 60_000;
/// Höchstes Bash-Zeitlimit eines Werkzeugaufrufs in Millisekunden (30 min)
const BASH_MAX_TIMEOUT_MS: i64 = 30 * 60_000;

/// Art eines Laufs, wie der Lauf-Speicher sie kennt (`RunKind`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub enum RunKind {
    #[default]
    Work,
    Merge,
    Research,
    Plan,
}

impl RunKind {
    /// Name der Art wie im JSON der Lauf-Datei
    pub fn as_str(&self) -> &'static str {
        match self {
            RunKind::Work => "work",
            RunKind::Merge => "merge",
            RunKind::Research => "research",
            RunKind::Plan => "plan",
        }
    }
}

/// Baut Argumentliste und Umgebung des Workers.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeInvocation;

impl ClaudeInvocation {
    /// Neuer Aufruf-Bauer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Argumentliste. `--disallowedTools` und `--allowedTools` nehmen beide alle folgenden Werte
    /// bis zur nächsten Option, daher stehen sie am Ende (Verbote vor den Erlaubnissen, die Liste
    /// der Erlaubnisse zuletzt). Eine Recherche übernimmt aus `claude.*` nur den Aufruf: fester
    /// Rechte-Modus „default“, keine extraArgs, nur `RESEARCH_TOOLS`, dazu die Verbote. Eine
    /// Planung genauso, mit `PLANNING_TOOLS` und den Verboten der Planung.
    pub fn args(
        &self,
        cfg: &DispatchConfig,
        run: &RunSettings,
        extra_tools: &[String],
        kind: RunKind,
    ) -> Vec<String> {
        let c = &cfg.claude;
        let research = kind == RunKind::Research || kind == RunKind::Plan;
        let base: Vec<String> = match kind {
            RunKind::Research => RESEARCH_TOOLS.iter().map(|t| t.to_string()).collect(),
            RunKind::Plan => PLANNING_TOOLS.iter().map(|t| t.to_string()).collect(),
            _ => c.allowed_tools.clone(),
        };
        let denied: Vec<String> = if kind == RunKind::Plan {
            planning_denied_tools()
                .into_iter()
                .map(|t| t.to_string())
                .collect()
        } else {
            RESEARCH_DENIED_TOOLS
                .iter()
                .map(|t| t.to_string())
                .collect()
        };
        let mut tools = base.clone();
        tools.extend(
            extra_tools
                .iter()
                .filter(|tool| !base.contains(tool))
                .cloned(),
        );
        let mode = if research {
            Some(RESEARCH_PERMISSION_MODE.to_string())
        } else {
            c.permission_mode.clone()
        };

        let mut args: Vec<String> = vec![
            "-p".to_string(),
            // stream-json schreibt jedes Ereignis sofort, so wächst das Protokoll, während Claude
            // arbeitet; ohne Terminal verlangt es --verbose
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--max-turns".to_string(),
            run.max_turns.to_string(),
        ];
        if let Some(model) = &run.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }
        if let Some(effort) = &run.effort {
            args.push("--effort".to_string());
            args.push(effort.clone());
        }
        if let Some(mode) = mode {
            args.push("--permission-mode".to_string());
            args.push(mode);
        }
        if !research {
            args.extend(
                c.extra_args
                    .iter()
                    .filter(|arg| arg.as_str() != "--verbose")
                    .cloned(),
            );
        }
        if research {
            args.push("--disallowedTools".to_string());
            args.extend(denied);
        }
        if !tools.is_empty() {
            args.push("--allowedTools".to_string());
            args.extend(tools);
        }
        args
    }

    /// Umgebung des Workers (`git_bash`: Pfad der Git Bash unter Windows, sonst `None`): ohne
    /// API-Key (Abrechnung über die Anmeldung), ohne fremde Session (auch keine
    /// Claude-Code-Kennung), mit `HARNESS_TICKET` (ohne Ticket, z. B. Recherche: ohne)
    pub fn env(
        &self,
        env: &BTreeMap<String, String>,
        cfg: &DispatchConfig,
        ticket_id: Option<&str>,
        git_bash: Option<&str>,
    ) -> BTreeMap<String, String> {
        let mut out = env.clone();
        if cfg.claude.use_subscription {
            out.remove("ANTHROPIC_API_KEY");
            out.remove("ANTHROPIC_AUTH_TOKEN");
        }
        // Der Server kann aus einer (Claude-)Session gestartet sein; der Worker ist eine eigene
        out.remove("HARNESS_SESSION");
        out.remove("CLAUDECODE");
        out.remove("CLAUDE_CODE_ENTRYPOINT");
        match ticket_id {
            Some(ticket_id) => {
                out.insert("HARNESS_TICKET".to_string(), ticket_id.to_string());
            }
            None => {
                out.remove("HARNESS_TICKET");
            }
        }
        // Längere Bash-Zeitlimits (sonst 2 bzw. 10 min): ein Befehl darüber landet im Hintergrund,
        // und mit `claude -p` endet der Lauf, bevor er fertig ist. Eigene Werte aus der Umgebung
        // des Servers bleiben
        out.entry("BASH_DEFAULT_TIMEOUT_MS".to_string())
            .or_insert_with(|| BASH_DEFAULT_TIMEOUT_MS.to_string());
        out.entry("BASH_MAX_TIMEOUT_MS".to_string())
            .or_insert_with(|| BASH_MAX_TIMEOUT_MS.to_string());
        // Unter Windows nutzt Claude Code sonst das PowerShell-Tool, für das die Bash-Regeln
        // (`Bash(harness:*)`) nicht gelten
        if let Some(git_bash) = git_bash {
            out.insert(
                "CLAUDE_CODE_USE_POWERSHELL_TOOL".to_string(),
                "0".to_string(),
            );
            out.insert(
                "CLAUDE_CODE_GIT_BASH_PATH".to_string(),
                git_bash.to_string(),
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;

    /// Einstellung mit knapper Tool-Liste und doppeltem --verbose
    fn cfg() -> DispatchConfig {
        DispatchConfigNormalizer::new()
            .normalize(&json!({
                "claude": { "allowedTools": ["Read", "Bash(harness:*)"], "extraArgs": ["--verbose"] }
            }))
            .unwrap()
    }

    /// Lauf-Einstellungen eines Standardlaufs
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
        }
    }

    /// Umgebung aus Paaren
    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    /// Einstellung, die einem Lauf alles erlauben würde
    fn risky() -> DispatchConfig {
        DispatchConfigNormalizer::new()
            .normalize(&json!({
                "claude": {
                    "permissionMode": "bypassPermissions",
                    "extraArgs": ["--dangerously-skip-permissions"],
                    "allowedTools": ["Bash(*)"],
                }
            }))
            .unwrap()
    }

    /// Stelle eines Arguments
    fn index_of(args: &[String], value: &str) -> usize {
        args.iter().position(|arg| arg == value).unwrap()
    }

    #[test]
    fn recherche_fester_modus_keine_extra_args_nur_recherche_tools() {
        let risky = risky();
        let args = ClaudeInvocation::new().args(&risky, &run(), &[], RunKind::Research);
        assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions"));
        assert!(!args.iter().any(|a| a == "Bash(*)"));
        let mode = index_of(&args, "--permission-mode");
        assert_eq!(args[mode + 1], "default");
        assert_eq!(args.iter().filter(|a| *a == "--permission-mode").count(), 1);
        let deny = index_of(&args, "--disallowedTools");
        let allow = index_of(&args, "--allowedTools");
        assert!(deny > mode);
        assert!(allow > deny);
        // Keine Liste darf eine Option schlucken: zwischen den Listen und danach stehen nur Werte
        // ohne führendes „-“
        assert!(args[deny + 1..allow].iter().all(|a| !a.starts_with('-')));
        assert!(args[allow + 1..].iter().all(|a| !a.starts_with('-')));
        assert_eq!(
            args[deny + 1..allow],
            RESEARCH_DENIED_TOOLS.map(String::from)
        );
        assert_eq!(args[allow + 1..], RESEARCH_TOOLS.map(String::from));
        assert!(!RESEARCH_TOOLS.contains(&"Write"));
        assert!(!RESEARCH_TOOLS.contains(&"Edit"));
        assert!(RESEARCH_TOOLS.contains(&"Write(./research.json)"));
        assert!(RESEARCH_TOOLS.contains(&"Edit(./research.json)"));
        assert!(!RESEARCH_DENIED_TOOLS.contains(&"Write"));
        let harness: Vec<&&str> = RESEARCH_TOOLS
            .iter()
            .filter(|tool| tool.starts_with("Bash(harness"))
            .collect();
        assert_eq!(
            harness,
            [
                &"Bash(harness board question show:*)",
                &"Bash(harness board question research:*)",
                &"Bash(harness board show:*)",
                &"Bash(harness board list:*)",
            ]
        );
        // Jede Regel gilt auch für den neuen Namen der CLI
        let nexus: Vec<&&str> = RESEARCH_TOOLS
            .iter()
            .filter(|tool| tool.starts_with("Bash(nexus"))
            .collect();
        assert_eq!(
            nexus,
            [
                &"Bash(nexus board question show:*)",
                &"Bash(nexus board question research:*)",
                &"Bash(nexus board show:*)",
                &"Bash(nexus board list:*)",
            ]
        );
    }

    #[test]
    fn setzt_harness_ticket_nur_mit_ticket() {
        let cfg = cfg();
        let invocation = ClaudeInvocation::new();
        let ohne = invocation.env(&env(&[("HARNESS_TICKET", "ARC-0001")]), &cfg, None, None);
        assert!(!ohne.contains_key("HARNESS_TICKET"));
        let mit = invocation.env(&env(&[]), &cfg, Some("ARC-0002"), None);
        assert_eq!(mit["HARNESS_TICKET"], "ARC-0002");
    }

    #[test]
    fn baut_die_argumente_mit_stream_json_und_allowed_tools_zuletzt() {
        let cfg = cfg();
        let invocation = ClaudeInvocation::new();
        assert_eq!(
            invocation.args(&cfg, &run(), &[], RunKind::Work),
            [
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--max-turns",
                "80",
                "--model",
                "opus",
                "--effort",
                "high",
                "--permission-mode",
                "acceptEdits",
                "--allowedTools",
                "Read",
                "Bash(harness:*)",
            ]
        );
        let extra = DispatchConfigNormalizer::new()
            .normalize(&json!({ "claude": { "extraArgs": ["--add-dir", "x"] } }))
            .unwrap();
        let args = invocation.args(&extra, &run(), &[], RunKind::Work);
        assert_eq!(
            args[..4],
            ["-p", "--output-format", "stream-json", "--verbose"]
        );
        assert!(args.iter().any(|a| a == "--add-dir"));
        assert!(args.iter().any(|a| a == "x"));
        for kind in [RunKind::Merge, RunKind::Research] {
            let args = invocation.args(&cfg, &run(), &[], kind);
            assert_eq!(
                args[..4],
                ["-p", "--output-format", "stream-json", "--verbose"]
            );
        }
    }

    #[test]
    fn lasst_fehlendes_modell_und_effort_weg_und_erganzt_werkzeuge() {
        let cfg = cfg();
        let ohne = RunSettings {
            model: None,
            effort: None,
            ..run()
        };
        let args = ClaudeInvocation::new().args(
            &cfg,
            &ohne,
            &["Bash(git merge:*)".to_string()],
            RunKind::Work,
        );
        assert!(!args.iter().any(|a| a == "--model"));
        assert!(!args.iter().any(|a| a == "--effort"));
        assert_eq!(
            args[args.len() - 4..],
            [
                "--allowedTools",
                "Read",
                "Bash(harness:*)",
                "Bash(git merge:*)"
            ]
        );
    }

    #[test]
    fn entfernt_api_keys_und_session_variablen_und_setzt_harness_ticket() {
        let cfg = cfg();
        let out = ClaudeInvocation::new().env(
            &env(&[
                ("PATH", "p"),
                ("ANTHROPIC_API_KEY", "k"),
                ("ANTHROPIC_AUTH_TOKEN", "t"),
                ("HARNESS_SESSION", "s"),
                ("CLAUDECODE", "1"),
                ("CLAUDE_CODE_ENTRYPOINT", "cli"),
            ]),
            &cfg,
            Some("ARC-0001"),
            None,
        );
        assert_eq!(
            out,
            env(&[
                ("PATH", "p"),
                ("HARNESS_TICKET", "ARC-0001"),
                ("BASH_DEFAULT_TIMEOUT_MS", "900000"),
                ("BASH_MAX_TIMEOUT_MS", "1800000"),
            ])
        );
        let keep = DispatchConfigNormalizer::new()
            .normalize(&json!({ "claude": { "useSubscription": false } }))
            .unwrap();
        let behalten = ClaudeInvocation::new().env(
            &env(&[("ANTHROPIC_API_KEY", "k")]),
            &keep,
            Some("ARC-0001"),
            None,
        );
        assert_eq!(behalten["ANTHROPIC_API_KEY"], "k");
    }

    #[test]
    fn setzt_mit_gefundener_git_bash_das_powershell_tool_aus() {
        let cfg = cfg();
        let bash = "C:/G/bin/bash.exe";
        let out =
            ClaudeInvocation::new().env(&env(&[("PATH", "p")]), &cfg, Some("ARC-0001"), Some(bash));
        assert_eq!(
            out,
            env(&[
                ("PATH", "p"),
                ("HARNESS_TICKET", "ARC-0001"),
                ("BASH_DEFAULT_TIMEOUT_MS", "900000"),
                ("BASH_MAX_TIMEOUT_MS", "1800000"),
                ("CLAUDE_CODE_USE_POWERSHELL_TOOL", "0"),
                ("CLAUDE_CODE_GIT_BASH_PATH", bash),
            ])
        );
        assert_eq!(
            ClaudeInvocation::new().env(&env(&[("PATH", "p")]), &cfg, Some("ARC-0001"), None),
            env(&[
                ("PATH", "p"),
                ("HARNESS_TICKET", "ARC-0001"),
                ("BASH_DEFAULT_TIMEOUT_MS", "900000"),
                ("BASH_MAX_TIMEOUT_MS", "1800000"),
            ])
        );
    }

    #[test]
    fn setzt_langere_bash_zeitlimits_eigene_werte_bleiben() {
        let cfg = cfg();
        let out = ClaudeInvocation::new().env(
            &env(&[("BASH_MAX_TIMEOUT_MS", "60000")]),
            &cfg,
            Some("ARC-0001"),
            None,
        );
        assert_eq!(out["BASH_DEFAULT_TIMEOUT_MS"], "900000");
        assert_eq!(out["BASH_MAX_TIMEOUT_MS"], "60000");
    }

    #[test]
    fn ubernimmt_git_bash_path_aus_der_einstellung() {
        let normalizer = DispatchConfigNormalizer::new();
        assert_eq!(cfg().claude.git_bash_path, None);
        assert_eq!(
            normalizer
                .normalize(&json!({ "claude": { "gitBashPath": " C:/x/bash.exe " } }))
                .unwrap()
                .claude
                .git_bash_path
                .as_deref(),
            Some("C:/x/bash.exe")
        );
        assert_eq!(
            normalizer
                .normalize(&json!({ "claude": { "gitBashPath": "" } }))
                .unwrap()
                .claude
                .git_bash_path,
            None
        );
    }

    #[test]
    fn planung_fester_modus_nur_planungs_tools_und_verbote_der_statusanderung() {
        let risky = risky();
        let args = ClaudeInvocation::new().args(&risky, &run(), &[], RunKind::Plan);
        assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions"));
        assert!(!args.iter().any(|a| a == "Bash(*)"));
        let mode = index_of(&args, "--permission-mode");
        assert_eq!(args[mode + 1], "default");
        let deny = index_of(&args, "--disallowedTools");
        let allow = index_of(&args, "--allowedTools");
        let denied: Vec<String> = planning_denied_tools()
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(args[deny + 1..allow], denied[..]);
        assert_eq!(args[allow + 1..], PLANNING_TOOLS.map(String::from));
        for tool in RESEARCH_DENIED_TOOLS {
            assert!(denied.iter().any(|d| d == tool), "{tool}");
        }
        for tool in [
            "Bash(harness board move:*)",
            "Bash(harness board set:*)",
            "Bash(harness board claim:*)",
            "Bash(nexus board move:*)",
            "Bash(nexus board dispatch:*)",
        ] {
            assert!(denied.iter().any(|d| d == tool), "{tool}");
        }
        for tool in [
            "Write(./plan.json)",
            "Bash(harness board plan-json:*)",
            "Bash(harness board question add:*)",
        ] {
            assert!(PLANNING_TOOLS.contains(&tool), "{tool}");
        }
        assert!(!PLANNING_TOOLS.contains(&"Write"));
        assert!(!PLANNING_TOOLS.contains(&"Bash(harness:*)"));
        assert!(!PLANNING_TOOLS.contains(&"Bash(nexus:*)"));
    }
}
