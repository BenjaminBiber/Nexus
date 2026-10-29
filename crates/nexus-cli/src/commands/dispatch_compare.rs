//! `nexus dispatch-compare`: der Bericht des Schattenbetriebs (`NEXUS_DISPATCH=shadow`).
//!
//! Versteckter Befehl: ihn braucht nur, wer den Parallelbetrieb auswertet. Gelesen werden
//! allein die JSONL-Dateien unter `<Benutzerordner>/dispatch/shadow/`; weder Board noch
//! Server müssen laufen.

use nexus_core::dispatch::shadow::ShadowStore;
use nexus_core::Result;

use crate::context::CliContext;

/// Beschreibung von `dispatch-compare` auf der Hilfeseite
pub const COMPARE_ABOUT: &str =
    "Vergleicht die Entscheidungen von Node und Rust aus dem Schattenbetrieb (NEXUS_DISPATCH=shadow).";

/// Beispiele unter der Hilfe von `dispatch-compare`
pub const COMPARE_EXAMPLES: &str = "\nBeispiele:\n  nexus dispatch-compare\n  nexus dispatch-compare --since 2026-11-01\n  nexus dispatch-compare --since 2026-11-01 --until 2026-11-07 --json";

/// Hilfe der Option `--since`
pub const SINCE_HELP: &str = "Erster Tag des Zeitraums (JJJJ-MM-TT)";

/// Hilfe der Option `--until`
pub const UNTIL_HELP: &str = "Letzter Tag des Zeitraums (JJJJ-MM-TT)";

/// Was der Befehl ausgeben soll
#[derive(Debug, Default)]
pub struct CompareOptions {
    pub since: Option<String>,
    pub until: Option<String>,
    /// `--json` nach dem Befehl; vorn (`nexus --json dispatch-compare`) zählt genauso
    pub json: bool,
}

/// Schreibt den Bericht als Markdown oder, mit `--json`, als JSON
pub fn run(context: &CliContext, options: &CompareOptions) -> Result<u8> {
    let store = ShadowStore::new(context.home());
    let report = store.report(options.since.as_deref(), options.until.as_deref())?;
    let printer = if options.json {
        crate::output::Printer::new(true)
    } else {
        context.printer()
    };
    // Markdown ist mehrzeilig; der Drucker gibt jede Zeile einzeln aus
    let lines: Vec<String> = report.markdown().lines().map(str::to_string).collect();
    printer.result(&report, &lines);
    Ok(0)
}
