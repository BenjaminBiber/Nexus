// Der Dispatch-Worker in der Programmdatei der App: `<app> dispatch-worker <lauf>`.
//
// Der Server startet die Worker seiner Läufe über `std::env::current_exe()`. Läuft der Server im
// Prozess der App, ist das die App selbst; sie muss den Aufruf deshalb vor `tauri::Builder`
// abfangen und darf dabei kein Fenster und keine NSApplication aufbauen – sonst stünde der Worker
// unter macOS im Dock und unter Windows in der Taskleiste.
use std::collections::BTreeMap;

use nexus_core::HarnessHome;
use nexus_server::dispatch::DispatchWorker;

// Unterbefehl, mit dem die Programmdatei zum Worker wird (SUBCOMMAND in worker_launcher.rs)
const SUBCOMMAND: &str = "dispatch-worker";

// Der Lauf, den dieser Start ausführen soll; None heißt „ganz normal die App starten“
pub fn lauf_aus_argumenten<S: AsRef<str>>(args: &[S]) -> Option<String> {
    let mut args = args.iter().map(AsRef::as_ref);
    if args.next()? != SUBCOMMAND {
        return None;
    }
    let lauf = args.next()?.trim();
    (!lauf.is_empty()).then(|| lauf.to_string())
}

// Führt den Lauf aus und endet; wie der Lauf ausging, steht in `dispatch/runs/<lauf>.result.json`
pub fn run(run_id: &str) {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    let env: BTreeMap<String, String> = std::env::vars().collect();
    let worker = DispatchWorker::new(HarnessHome::from_process_env());
    // Fehler stehen im Ergebnis des Laufs; der Worker endet trotzdem ordentlich
    let _ = runtime.block_on(worker.run(run_id, env));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erkennt_den_versteckten_unterbefehl_mit_lauf() {
        assert_eq!(
            lauf_aus_argumenten(&["dispatch-worker", "ARC-0001-x"]),
            Some("ARC-0001-x".to_string())
        );
    }

    #[test]
    fn alles_andere_startet_die_app() {
        assert_eq!(lauf_aus_argumenten::<&str>(&[]), None);
        assert_eq!(lauf_aus_argumenten(&["dispatch-worker"]), None);
        assert_eq!(lauf_aus_argumenten(&["dispatch-worker", "  "]), None);
        assert_eq!(lauf_aus_argumenten(&["nexus://open/ticket/ARC-0001"]), None);
        // Nur als erstes Argument; ein Deep-Link davor gehört der App
        assert_eq!(
            lauf_aus_argumenten(&["nexus://open", "dispatch-worker", "ARC-0001-x"]),
            None
        );
    }
}
