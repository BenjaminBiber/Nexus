//! `nexus stop`: beendet den laufenden Harness-Server; `nexus board` startet ihn bei Bedarf neu.
//! Portiert aus `packages/cli/src/commands/StopCommand.ts`.

use nexus_core::install::{ServerStopper, StopDeps};
use nexus_core::process;
use nexus_core::Result;
use serde::Serialize;

use crate::context::CliContext;
use crate::server::{PortState, ServerLauncher};

/// Beschreibung von `stop` (wortgleich zu `StopCommand.ts`)
pub const STOP_ABOUT: &str =
    "Beendet den laufenden Harness-Server (nach einem Update, damit die neue Version startet).";

/// Beispiele unter der Hilfe von `stop`
pub const STOP_EXAMPLES: &str = "\nBeispiele:\n  harness stop\n  harness stop --restart\n\nDas Board startet mit `harness board` wieder.";

/// Das Ergebnis von `nexus stop` (`--json`)
#[derive(Debug, Serialize)]
struct StopResult {
    stopped: bool,
    restarted: bool,
}

/// Prüft den Port über den Health-Aufruf und beendet den Prozess mit SIGTERM
/// (unter Windows mit `taskkill`).
struct ProcessDeps {
    launcher: ServerLauncher,
}

impl StopDeps for ProcessDeps {
    fn is_ours(&self, port: u16) -> bool {
        self.launcher.probe(port) == PortState::Ours
    }

    fn kill(&self, pid: u32) {
        process::terminate(pid);
    }
}

/// Beendet den Server, startet ihn auf Wunsch wieder und meldet das Ergebnis
pub fn run(context: &CliContext, restart: bool) -> Result<u8> {
    let launcher = ServerLauncher::from_process_env();
    let mut stopper = ServerStopper::new(
        context.home(),
        ProcessDeps {
            launcher: launcher.clone(),
        },
    );
    let stopped = stopper.stop()?;
    let port = stopper.stopped_port();
    let restarted = restart && stopped && port.is_some();
    if let (true, Some(port)) = (restarted, port) {
        launcher.start(port)?;
    }
    let lines = if !stopped {
        vec!["Es läuft kein Server.".to_string()]
    } else if restarted {
        vec![format!(
            "Server neu gestartet auf Port {}.",
            port.unwrap_or_default()
        )]
    } else {
        vec!["Server beendet.".to_string()]
    };
    context
        .printer()
        .result(&StopResult { stopped, restarted }, &lines);
    Ok(0)
}
