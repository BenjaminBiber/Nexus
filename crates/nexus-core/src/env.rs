//! Umgebungsvariablen von Nexus mit Rückfall auf die alten Namen: `NEXUS_<NAME>` gilt,
//! `HARNESS_<NAME>` zählt noch während der Übergangszeit. Beide Namen stehen hier an einer
//! Stelle, damit CLI, Hooks und Server dieselbe Reihenfolge nehmen.

use std::collections::HashMap;

/// Präfix der neuen Namen
pub const NEXUS_PREFIX: &str = "NEXUS_";

/// Präfix der alten Namen (Rückfall)
pub const HARNESS_PREFIX: &str = "HARNESS_";

/// Namen, die ein Benutzer selbst setzt und zu denen ein Hinweis erscheint, solange der alte
/// Name benutzt wird. `SESSION` und `TICKET` setzt der Hook selbst, dazu kommt kein Hinweis.
pub const CONFIG_NAMES: [&str; 2] = ["HOME", "PORT"];

/// Beide Namen einer Variable: zuerst der neue, dann der alte
pub fn names(name: &str) -> [String; 2] {
    [
        format!("{NEXUS_PREFIX}{name}"),
        format!("{HARNESS_PREFIX}{name}"),
    ]
}

/// Wert aus der Umgebung des Prozesses: `NEXUS_<NAME>`, sonst `HARNESS_<NAME>`; leere Werte
/// zählen wie keine
pub fn var(name: &str) -> Option<String> {
    from(name, |full| std::env::var(full).ok())
}

/// Wert aus einer gelesenen Umgebung (Hooks, Tests)
pub fn var_in(env: &HashMap<String, String>, name: &str) -> Option<String> {
    from(name, |full| env.get(full).cloned())
}

/// Name, unter dem der Wert gefunden wurde; `None`, wenn keiner gesetzt ist
pub fn used_name_in(env: &HashMap<String, String>, name: &str) -> Option<String> {
    names(name)
        .into_iter()
        .find(|full| env.get(full).is_some_and(|value| !value.is_empty()))
}

/// Wert über eine frei wählbare Suche
fn from(name: &str, lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    names(name)
        .iter()
        .find_map(|full| lookup(full).filter(|value| !value.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nennt_beide_namen_in_der_reihenfolge_neu_vor_alt() {
        assert_eq!(names("PORT"), ["NEXUS_PORT", "HARNESS_PORT"]);
    }

    #[test]
    fn nexus_variable_hat_vorrang_vor_harness_variable() {
        let env = HashMap::from([
            ("NEXUS_PORT".to_string(), "4800".to_string()),
            ("HARNESS_PORT".to_string(), "4710".to_string()),
        ]);
        assert_eq!(var_in(&env, "PORT").as_deref(), Some("4800"));
        assert_eq!(used_name_in(&env, "PORT").as_deref(), Some("NEXUS_PORT"));
    }

    #[test]
    fn ohne_nexus_variable_zahlt_die_harness_variable() {
        let env = HashMap::from([("HARNESS_PORT".to_string(), "4710".to_string())]);
        assert_eq!(var_in(&env, "PORT").as_deref(), Some("4710"));
        assert_eq!(used_name_in(&env, "PORT").as_deref(), Some("HARNESS_PORT"));
    }

    #[test]
    fn leere_werte_zahlen_wie_keine() {
        let env = HashMap::from([
            ("NEXUS_PORT".to_string(), String::new()),
            ("HARNESS_PORT".to_string(), "4710".to_string()),
        ]);
        assert_eq!(var_in(&env, "PORT").as_deref(), Some("4710"));

        let leer = HashMap::from([("HARNESS_PORT".to_string(), String::new())]);
        assert_eq!(var_in(&leer, "PORT"), None);
        assert_eq!(used_name_in(&leer, "PORT"), None);
    }

    #[test]
    fn ohne_beide_variablen_gibt_es_keinen_wert() {
        assert_eq!(var_in(&HashMap::new(), "DEBUG"), None);
    }

    #[test]
    fn nennt_die_namen_mit_hinweis() {
        assert_eq!(CONFIG_NAMES, ["HOME", "PORT"]);
    }
}
