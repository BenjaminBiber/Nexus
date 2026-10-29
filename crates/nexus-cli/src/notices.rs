//! Hinweise der Übergangszeit auf stderr: veraltete `HARNESS_*`-Variablen und der alte
//! Programmname `harness`. Sie stehen nur einmal je Prozess und nie in einem Hook, damit die
//! Ausgabe für Claude Code unverändert bleibt.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use nexus_core::env;

/// Variable, die die Startdatei `harness` setzt
pub const INVOKED_AS_ENV: &str = "NEXUS_INVOKED_AS";

/// Wert, unter dem der alte Name gemeldet wird
pub const INVOKED_AS_HARNESS: &str = "harness";

/// Hinweis zum alten Programmnamen
const ALIAS_NOTICE: &str =
    "Hinweis: `harness` heißt jetzt `nexus`. Der alte Name funktioniert noch eine Weile.";

/// Schon ausgegeben? Die Hinweise stehen einmal je Prozess.
static DONE: AtomicBool = AtomicBool::new(false);

/// Gibt die Hinweise zur Umgebung dieses Prozesses einmalig auf stderr aus
pub fn print_once() {
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    let env: HashMap<String, String> = std::env::vars().collect();
    for line in lines(&env) {
        eprintln!("{line}");
    }
}

/// Die Hinweise zu dieser Umgebung in fester Reihenfolge: erst der Programmname, dann die
/// veralteten Variablen in der Reihenfolge von [`env::CONFIG_NAMES`]
pub fn lines(environment: &HashMap<String, String>) -> Vec<String> {
    let mut lines = Vec::new();
    if environment
        .get(INVOKED_AS_ENV)
        .is_some_and(|value| value == INVOKED_AS_HARNESS)
    {
        lines.push(ALIAS_NOTICE.to_string());
    }
    for name in env::CONFIG_NAMES {
        let old = format!("{}{name}", env::HARNESS_PREFIX);
        let new = format!("{}{name}", env::NEXUS_PREFIX);
        if env::used_name_in(environment, name).as_deref() == Some(old.as_str()) {
            lines.push(format!("{old} ist veraltet, bitte {new} verwenden."));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Umgebung aus Paaren
    fn env(paare: &[(&str, &str)]) -> HashMap<String, String> {
        paare
            .iter()
            .map(|(name, wert)| (name.to_string(), wert.to_string()))
            .collect()
    }

    #[test]
    fn ohne_alte_variablen_und_ohne_alten_namen_gibt_es_keinen_hinweis() {
        assert!(lines(&env(&[("NEXUS_HOME", "/n")])).is_empty());
    }

    #[test]
    fn ein_gesetztes_harness_home_erzeugt_einen_hinweis() {
        assert_eq!(
            lines(&env(&[("HARNESS_HOME", "/h")])),
            vec!["HARNESS_HOME ist veraltet, bitte NEXUS_HOME verwenden.".to_string()]
        );
    }

    #[test]
    fn mit_nexus_home_daneben_gibt_es_keinen_hinweis_zum_ordner() {
        assert!(lines(&env(&[("NEXUS_HOME", "/n"), ("HARNESS_HOME", "/h")])).is_empty());
    }

    #[test]
    fn auch_der_alte_port_wird_genannt() {
        assert_eq!(
            lines(&env(&[("HARNESS_PORT", "4710")])),
            vec!["HARNESS_PORT ist veraltet, bitte NEXUS_PORT verwenden.".to_string()]
        );
    }

    #[test]
    fn zu_session_und_ticket_kommt_kein_hinweis() {
        assert!(lines(&env(&[
            ("HARNESS_SESSION", "s-1"),
            ("HARNESS_TICKET", "ARC-0001")
        ]))
        .is_empty());
    }

    #[test]
    fn der_alte_programmname_wird_vor_den_variablen_genannt() {
        assert_eq!(
            lines(&env(&[
                ("NEXUS_INVOKED_AS", "harness"),
                ("HARNESS_HOME", "/h")
            ])),
            vec![
                ALIAS_NOTICE.to_string(),
                "HARNESS_HOME ist veraltet, bitte NEXUS_HOME verwenden.".to_string()
            ]
        );
    }

    #[test]
    fn ein_anderer_aufrufname_erzeugt_keinen_hinweis() {
        assert!(lines(&env(&[("NEXUS_INVOKED_AS", "nexus")])).is_empty());
    }
}
