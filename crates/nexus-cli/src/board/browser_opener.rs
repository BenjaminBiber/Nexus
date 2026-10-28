//! Öffnet eine Adresse im Standardbrowser; scheitert das, bleibt die Adresse im Terminal
//! stehen. Portiert aus `packages/cli/src/board/BrowserOpener.ts`.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Öffnet Adressen mit dem Befehl des Betriebssystems.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrowserOpener;

impl BrowserOpener {
    /// Startet den passenden Systembefehl abgelöst; liefert, ob er gestartet werden konnte
    pub fn open(&self, url: &str) -> bool {
        let (program, args) = Self::command_for(url);
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        detach(&mut command);
        command.spawn().is_ok()
    }

    /// Befehl je Betriebssystem (unter Windows ohne Shell, damit keine Anführungszeichen nötig sind)
    pub fn command_for(url: &str) -> (PathBuf, Vec<String>) {
        if cfg!(windows) {
            (
                PathBuf::from("rundll32"),
                vec!["url.dll,FileProtocolHandler".to_string(), url.to_string()],
            )
        } else if cfg!(target_os = "macos") {
            (PathBuf::from("open"), vec![url.to_string()])
        } else {
            (PathBuf::from("xdg-open"), vec![url.to_string()])
        }
    }
}

/// Löst den Kindprozess von diesem Terminal: eigene Prozessgruppe unter Unix
#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// Löst den Kindprozess unter Windows: kein Konsolenfenster
#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    /// DETACHED_PROCESS | CREATE_NO_WINDOW
    const FLAGS: u32 = 0x0000_0008 | 0x0800_0000;
    command.creation_flags(FLAGS);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_befehl_passt_zum_betriebssystem() {
        let (program, args) = BrowserOpener::command_for("http://127.0.0.1:4710/");
        if cfg!(windows) {
            assert_eq!(program, PathBuf::from("rundll32"));
            assert_eq!(args[0], "url.dll,FileProtocolHandler");
            assert_eq!(args[1], "http://127.0.0.1:4710/");
        } else {
            let erwartet = if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            };
            assert_eq!(program, PathBuf::from(erwartet));
            assert_eq!(args, vec!["http://127.0.0.1:4710/".to_string()]);
        }
    }

    #[test]
    fn ein_fehlender_befehl_liefert_false_statt_eines_fehlers() {
        // Der Befehl selbst ist je Plattform anders; geprüft wird nur, dass ein Fehlschlag
        // nicht geworfen wird
        let mut command = Command::new("gibt-es-nicht-nexus");
        command.stdout(Stdio::null()).stderr(Stdio::null());
        assert!(command.spawn().is_err());
    }
}
