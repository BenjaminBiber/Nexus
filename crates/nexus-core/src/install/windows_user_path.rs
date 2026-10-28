//! PATH-Eintrag unter Windows: Benutzer-Umgebungsvariable `Path` in der Registry, gesetzt über
//! dasselbe PowerShell-Skript wie in `packages/core/src/install/WindowsUserPath.ts`. Kein
//! direkter Registry-Zugriff, damit `%VAR%`-Einträge unverändert bleiben.

use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::error::{HarnessError, Result};
use crate::install::user_path::UserPath;

/// Zeilen des PowerShell-Skripts; der Ordner kommt über die Umgebungsvariable
/// `HARNESS_BIN_DIR`, damit kein Quoting nötig ist.
const SCRIPT_LINES: &[&str] = &[
    "$d = $env:HARNESS_BIN_DIR",
    "$k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)",
    "$p = [string]$k.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)",
    "$items = @($p -split ';' | Where-Object { $_ -ne '' })",
    "if ($items -contains $d) { 'present'; exit 0 }",
    "$k.SetValue('Path', (($items + $d) -join ';'), [Microsoft.Win32.RegistryValueKind]::ExpandString)",
    // Löscht eine nicht vorhandene Variable; löst dabei die Änderungsmeldung an neue Terminals aus
    "[Environment]::SetEnvironmentVariable('HARNESS_PATH_REFRESH', $null, 'User')",
    "'added'",
];

/// Zeitlimit des Aufrufs (wie `timeout: 20000` in TypeScript)
const TIMEOUT: Duration = Duration::from_secs(20);

/// Abstand zwischen zwei Blicken darauf, ob PowerShell fertig ist
const POLL: Duration = Duration::from_millis(50);

/// Trägt den Ordner in den Benutzer-PATH der Registry ein.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsUserPath;

impl WindowsUserPath {
    /// Neuer Eintrag (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Das Skript als eine Zeile, genau wie in TypeScript mit `; ` verbunden
    fn script() -> String {
        SCRIPT_LINES.join("; ")
    }
}

impl UserPath for WindowsUserPath {
    /// Fügt den Ordner hinzu, falls er fehlt
    fn add(&self, dir: &Path) -> Result<bool> {
        let failed = |message: String| HarnessError::new("path-register-failed", message);
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &Self::script(),
            ])
            .env("HARNESS_BIN_DIR", dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // CREATE_NO_WINDOW: sonst blitzt beim Einrichten ein Konsolenfenster auf
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(crate::process::CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .map_err(|error| failed(format!("Der PATH konnte nicht ergänzt werden: {error}")))?;
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if started.elapsed() < TIMEOUT => sleep(POLL),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait_with_output();
                    return Err(failed(
                        "Der PATH konnte nicht ergänzt werden: PowerShell antwortet nicht."
                            .to_string(),
                    )
                    .into());
                }
                Err(error) => {
                    return Err(
                        failed(format!("Der PATH konnte nicht ergänzt werden: {error}")).into(),
                    )
                }
            }
        }
        let output = child
            .wait_with_output()
            .map_err(|error| failed(format!("Der PATH konnte nicht ergänzt werden: {error}")))?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(failed(format!("Der PATH konnte nicht ergänzt werden: {message}")).into());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim() == "added")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn das_skript_ist_wortgleich_zu_typescript() {
        let script = WindowsUserPath::script();
        assert!(
            script.starts_with("$d = $env:HARNESS_BIN_DIR; "),
            "{script}"
        );
        assert!(script.contains("DoNotExpandEnvironmentNames"), "{script}");
        assert!(
            script.contains("if ($items -contains $d) { 'present'; exit 0 }"),
            "{script}"
        );
        assert!(script.ends_with("'added'"), "{script}");
    }

    #[test]
    fn liest_den_benutzer_path_ohne_ihn_zu_andern() {
        // Der echte PATH des Testbenutzers darf sich nicht ändern, deshalb läuft hier nur der
        // lesende Teil des Skripts: er zeigt, dass PowerShell startet und die Registry liest.
        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "$k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true); \
                 [string]$k.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames); \
                 'gelesen'",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("gelesen"));
    }
}
