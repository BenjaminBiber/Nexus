//! Öffnet eine Datei im Explorer (markiert) oder in VS Code; portiert aus
//! `packages/server/src/docs/FileOpener.ts`.
//!
//! Der Server läuft auf dem Rechner der Person: Explorer und VS Code erscheinen dort. Nur
//! unter Windows. VS Code startet über den Befehl `code` (vom Installer in den PATH gelegt);
//! fehlt er, kommt `open-unavailable`.
//!
//! Die reinen Teile (Aufrufargumente, Fehlermeldungen) stehen hier für jedes System, damit
//! sie überall geprüft werden können; nur [`WindowsFileOpener`] startet wirklich einen
//! Prozess und hängt deshalb an `#[cfg(windows)]`.

use crate::{OpenTarget, PlatformError};

/// Name des Programms in der Fehlermeldung
pub fn program_name(target: OpenTarget) -> &'static str {
    match target {
        OpenTarget::Explorer => "Der Explorer",
        OpenTarget::Vscode => "VS Code (Befehl „code“)",
    }
}

/// Befehl und Argumente des Ziels. Der Explorer bekommt sein Argument **wörtlich**
/// (`/select,<datei>` als ein Stück, wie `windowsVerbatimArguments` in TypeScript), VS Code
/// läuft über `cmd /c`, weil `code` eine Batchdatei ist.
pub fn command_line(file: &str, target: OpenTarget) -> (&'static str, Vec<String>) {
    match target {
        OpenTarget::Explorer => ("explorer.exe", vec![format!("/select,{file}")]),
        OpenTarget::Vscode => (
            "cmd",
            vec!["/c".into(), "code".into(), format!("\"{file}\"")],
        ),
    }
}

/// „… ließ sich nicht öffnen (Grund).“ – wortgleich zu heute
pub fn unavailable(target: OpenTarget, detail: &str) -> PlatformError {
    PlatformError::new(
        "open-unavailable",
        format!(
            "{} ließ sich nicht öffnen ({detail}).",
            program_name(target)
        ),
    )
}

/// „Dateien öffnen gibt es nur unter Windows.“
pub fn only_on_windows() -> PlatformError {
    PlatformError::new(
        "open-unavailable",
        "Dateien öffnen gibt es nur unter Windows.",
    )
}

#[cfg(windows)]
pub use windows_opener::WindowsFileOpener;

/// Die Umsetzung mit `explorer.exe` und `cmd /c code`; nur unter Windows gebaut
#[cfg(windows)]
mod windows_opener {
    use std::path::Path;

    use async_trait::async_trait;

    use super::{command_line, unavailable};
    use crate::{FileOpener, OpenTarget, Result};

    /// Kindprozess ohne Konsolenfenster und ohne Bindung an die Konsole des Servers
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    /// Öffnet Dateien im Explorer oder in VS Code
    #[derive(Debug, Clone, Copy, Default)]
    pub struct WindowsFileOpener;

    impl WindowsFileOpener {
        /// Neuer Öffner (hält keinen Zustand)
        pub fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl FileOpener for WindowsFileOpener {
        async fn open(&self, file: &Path, target: OpenTarget) -> Result<()> {
            let (program, args) = command_line(&file.to_string_lossy(), target);
            let mut command = tokio::process::Command::new(program);
            command
                .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            // Wörtliche Argumente: der Explorer will `/select,C:\mit Leerzeichen\x.md` als ein
            // Stück ohne Anführungszeichen, und `cmd /c code "<datei>"` bringt seine eigenen mit
            for arg in &args {
                command.raw_arg(arg);
            }
            let mut child = command
                .spawn()
                .map_err(|error| unavailable(target, &error.to_string()))?;
            if target == OpenTarget::Explorer {
                // Der Explorer endet auch beim Erfolg oft mit Code 1, deshalb zählt nur, dass
                // er startet
                return Ok(());
            }
            let status = child
                .wait()
                .await
                .map_err(|error| unavailable(target, &error.to_string()))?;
            if status.success() {
                return Ok(());
            }
            let code = status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unbekannt".into());
            Err(unavailable(target, &format!("Code {code}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruft_den_explorer_mit_select_und_vs_code_uber_cmd_auf() {
        assert_eq!(
            command_line(r"C:\repo\docs\a.md", OpenTarget::Explorer),
            (
                "explorer.exe",
                vec![r"/select,C:\repo\docs\a.md".to_string()]
            )
        );
        assert_eq!(
            command_line(r"C:\repo\mit Leerzeichen\a.md", OpenTarget::Vscode),
            (
                "cmd",
                vec![
                    "/c".to_string(),
                    "code".to_string(),
                    "\"C:\\repo\\mit Leerzeichen\\a.md\"".to_string(),
                ]
            )
        );
    }

    #[test]
    fn nennt_das_programm_in_der_fehlermeldung() {
        let explorer = unavailable(OpenTarget::Explorer, "ENOENT");
        assert_eq!(explorer.code, "open-unavailable");
        assert_eq!(
            explorer.message,
            "Der Explorer ließ sich nicht öffnen (ENOENT)."
        );

        let code = unavailable(OpenTarget::Vscode, "Code 9009");
        assert_eq!(
            code.message,
            "VS Code (Befehl „code“) ließ sich nicht öffnen (Code 9009)."
        );
    }

    #[test]
    fn meldet_auf_anderen_systemen_dass_es_nur_unter_windows_geht() {
        let error = only_on_windows();
        assert_eq!(error.code, "open-unavailable");
        assert_eq!(error.message, "Dateien öffnen gibt es nur unter Windows.");
    }
}
