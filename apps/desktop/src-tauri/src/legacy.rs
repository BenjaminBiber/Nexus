// Leitet beim Start die alten Startdateien in ~/.harness/bin auf das nexus neben der App um.
// Die Desktop-Installation läuft nicht über `nexus install`; ohne diesen Schritt startet `harness`
// weiter die alte Node-Fassung mit ihren eigenen Daten.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use nexus_core::install::launcher_writer::{LauncherWriter, BINARY_NAME};

use crate::server::harness_home;

// Wählt alten Startordner und Ziel-Binary; None heißt „nicht umleiten“. Bei gesetztem
// NEXUS_HOME/HARNESS_HOME oder wenn der Nexus-Ordner selbst ~/.harness ist, bleibt alles, wie es ist.
pub fn umleitung(
    env: &HashMap<String, String>,
    benutzer: &Path,
    home: &Path,
    exe_dir: &Path,
) -> Option<(PathBuf, PathBuf)> {
    let gesetzt = ["NEXUS_HOME", "HARNESS_HOME"]
        .iter()
        .any(|name| env.get(*name).is_some_and(|v| !v.trim().is_empty()));
    let alt = benutzer.join(".harness");
    if gesetzt || home == alt {
        return None;
    }
    Some((alt.join("bin"), exe_dir.join(BINARY_NAME)))
}

// Führt die Umleitung aus; ein Fehler wird nur geloggt und hält den Start nie auf
pub fn init() {
    let env: HashMap<String, String> = std::env::vars().collect();
    let Some(benutzer) = dirs::home_dir() else {
        return;
    };
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    else {
        return;
    };
    let Some((alt_bin, binary)) = umleitung(&env, &benutzer, &harness_home(&env), &exe_dir) else {
        return;
    };
    if !binary.is_file() {
        log::warn!("{} fehlt, alte Startdateien bleiben", binary.display());
        return;
    }
    match LauncherWriter::new().redirect_node_launchers(&alt_bin, &binary) {
        Ok(true) => log::info!(
            "Startdateien in {} leiten auf {} um",
            alt_bin.display(),
            binary.display()
        ),
        Ok(false) => {}
        Err(err) => log::warn!(
            "Startdateien in {} nicht umgeleitet: {err}",
            alt_bin.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn leitet_den_alten_startordner_auf_das_binary_neben_der_app_um() {
        let benutzer = Path::new("/u");
        let auswahl = umleitung(
            &env(&[]),
            benutzer,
            &benutzer.join(".nexus"),
            Path::new("/app"),
        );
        assert_eq!(
            auswahl,
            Some((
                benutzer.join(".harness").join("bin"),
                Path::new("/app").join(BINARY_NAME)
            ))
        );
    }

    #[test]
    fn bei_gesetztem_nexus_home_wird_nicht_umgeleitet() {
        let benutzer = Path::new("/u");
        for name in ["NEXUS_HOME", "HARNESS_HOME"] {
            let auswahl = umleitung(
                &env(&[(name, "/eigener-ordner")]),
                benutzer,
                Path::new("/eigener-ordner"),
                Path::new("/app"),
            );
            assert_eq!(auswahl, None, "{name}");
        }
    }

    #[test]
    fn ist_der_nexus_ordner_noch_harness_wird_nicht_umgeleitet() {
        let benutzer = Path::new("/u");
        let auswahl = umleitung(
            &env(&[]),
            benutzer,
            &benutzer.join(".harness"),
            Path::new("/app"),
        );
        assert_eq!(auswahl, None);
    }
}
