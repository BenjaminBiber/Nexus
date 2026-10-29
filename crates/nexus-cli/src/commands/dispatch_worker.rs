//! `nexus dispatch-worker <lauf>`: der losgelöste Worker eines Laufs des Dispatchers.
//! Portiert aus dem Schalter `--dispatch-worker` in `packages/server/src/main.ts`.
//!
//! Versteckter Befehl: ihn startet nur der Server selbst. Er endet immer mit Rückgabecode 0 –
//! wie der Lauf ausging, steht in `dispatch/runs/<lauf>.result.json`, nicht im Rückgabecode.

use std::collections::BTreeMap;

use nexus_core::{Error, HarnessHome, Result};
use nexus_server::dispatch::DispatchWorker;

use crate::context::CliContext;

/// Führt den Lauf aus; die Umgebung ist die, mit der der Server den Worker gestartet hat
pub fn run(_context: &CliContext, run_id: &str) -> Result<u8> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(Error::unexpected)?;
    let env: BTreeMap<String, String> = std::env::vars().collect();
    let worker = DispatchWorker::new(HarnessHome::from_process_env());
    // Fehler stehen im Ergebnis des Laufs; der Worker endet trotzdem ordentlich
    let _ = runtime.block_on(worker.run(run_id, env));
    Ok(0)
}
