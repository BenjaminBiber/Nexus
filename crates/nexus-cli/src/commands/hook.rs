//! `harness hook <ereignis>`: wird von den Hooks des Plugins aufgerufen; blockiert nie und
//! bleibt still. Portiert aus `packages/cli/src/commands/HookCommand.ts`.

use std::collections::HashMap;
use std::io::Write;
use std::time::Duration;

use nexus_core::sessions::{HookKind, HOOK_KINDS};
use nexus_core::Result;

use crate::hooks::{HookRunner, RuntimeOpener, StdinReader};
use crate::server::ServerLauncher;

/// Frist, nach der der Hook in jedem Fall endet (`DEFAULT_DEADLINE_MS` in TypeScript)
const DEFAULT_DEADLINE_MS: u64 = 3000;

/// Zeit, die nach der Board-Arbeit für Umgebung, Ausgabe und Meldung übrig bleibt
const BOARD_RESERVE_MS: u64 = 1000;

/// So viel Zeit bekommt die Board-DB mindestens
const MIN_BOARD_BUDGET_MS: u64 = 500;

/// Umgebungsvariable mit der Frist des Hooks (ohne Präfix; `NEXUS_…` vor `HARNESS_…`)
const DEADLINE_ENV: &str = "HOOK_DEADLINE_MS";

/// Beschreibung von `hook` in der Hilfe; die Liste ist [`HOOK_KINDS`]
pub fn about() -> String {
    let kinds: Vec<&str> = HOOK_KINDS.iter().map(|kind| kind.as_str()).collect();
    format!(
        "Wird von Claude Code aufgerufen ({}); liest die Hook-Meldung von der Standardeingabe.",
        kinds.join(", ")
    )
}

/// Beispiel unter der Hilfe von `hook`
pub const EXAMPLE: &str =
    "\nBeispiel:\n  echo '{\"session_id\":\"abc\",\"cwd\":\".\"}' | harness hook session-start";

/// Liest die Eingabe, führt den Hook aus und gibt höchstens den Kontext aus; endet spätestens
/// nach der Frist (3 s). Ein unbekanntes Ereignis endet still mit Code 0.
pub fn run(ereignis: &str) -> Result<u8> {
    let env: HashMap<String, String> = std::env::vars().collect();
    let limit = nexus_core::env::var_in(&env, DEADLINE_ENV)
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|ms| *ms >= 1)
        .unwrap_or(DEFAULT_DEADLINE_MS);
    // Ein Wächter beendet den Prozess nach der Frist; hängt etwas (DB, Netz), endet der Hook
    // trotzdem still mit Code 0
    watch(Duration::from_millis(limit));
    let Some(kind) = HookKind::from_str_opt(ereignis) else {
        return Ok(0);
    };
    let input = StdinReader::read_default();
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(&input) else {
        return Ok(0);
    };
    // Die Board-DB bekommt die Frist abzüglich einer Sekunde, damit Umgebung, Ausgabe und
    // Meldung sicher noch durchkommen
    let budget = limit
        .saturating_sub(BOARD_RESERVE_MS)
        .max(MIN_BOARD_BUDGET_MS);
    let runner = HookRunner::new(
        RuntimeOpener::new(budget),
        ServerLauncher::from_process_env(),
        Duration::from_millis(budget),
    );
    runner.run(kind, &payload, &env, &mut |output| {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{output}");
        let _ = out.flush();
    });
    Ok(0)
}

/// Beendet den Prozess nach der Frist mit Code 0
fn watch(limit: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(limit);
        std::process::exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nennt_alle_sieben_ereignisse_in_der_beschreibung() {
        let text = about();
        for kind in HOOK_KINDS {
            assert!(text.contains(kind.as_str()), "{text}");
        }
    }

    #[test]
    fn die_board_db_bekommt_die_frist_ohne_die_reserve_mindestens_aber_500_ms() {
        let budget = |limit: u64| {
            limit
                .saturating_sub(BOARD_RESERVE_MS)
                .max(MIN_BOARD_BUDGET_MS)
        };
        assert_eq!(budget(DEFAULT_DEADLINE_MS), 2000);
        assert_eq!(budget(1200), 500);
        assert_eq!(budget(60_000), 59_000);
    }
}
