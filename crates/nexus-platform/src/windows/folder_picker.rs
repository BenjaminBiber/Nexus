//! Ordnerdialog auf diesem Rechner; portiert aus
//! `packages/server/src/dbboard/FolderPicker.ts` und `FolderPickerScript.ts`.
//!
//! Der Server läuft auf dem Rechner der Person: der Ordnerdialog erscheint dort und liefert
//! den vollen Pfad, den ein Browser-Dateifeld aus Sicherheitsgründen nicht herausgibt. Nur
//! unter Windows (Windows PowerShell, Explorer-Dialog).
//!
//! Das Skript wird nicht als Datei abgelegt, sondern als `-EncodedCommand` übergeben: Base64
//! über die UTF-16LE-Bytes des Skripts, genau wie `Buffer.from(script, 'utf16le')` in
//! TypeScript. Der Startordner kommt über die Umgebung (`HARNESS_PICK_START`), damit nichts in
//! das Skript eingeschleust werden kann.

use std::time::Duration;

use crate::PlatformError;

/// Das PowerShell-Skript des Dialogs, Zeichen für Zeichen wie in TypeScript
pub const FOLDER_PICKER_SCRIPT: &str = include_str!("folder_picker.ps1");

/// Höchstens so lange wartet der Server auf eine Auswahl (der Dialog bleibt sonst offen
/// liegen)
pub const TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Das Skript als `-EncodedCommand`: Base64 über die UTF-16LE-Bytes
pub fn encoded_command(script: &str) -> String {
    let bytes: Vec<u8> = script
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    base64(&bytes)
}

/// Die Aufrufargumente von `powershell.exe` für dieses Skript
pub fn arguments(script: &str) -> Vec<String> {
    [
        "-NoProfile",
        "-NonInteractive",
        "-STA",
        "-ExecutionPolicy",
        "Bypass",
        "-EncodedCommand",
    ]
    .into_iter()
    .map(str::to_string)
    .chain(std::iter::once(encoded_command(script)))
    .collect()
}

/// „Die Ordnerauswahl ließ sich nicht öffnen“ mit dem Grund, wortgleich zu heute
pub fn unavailable(detail: &str) -> PlatformError {
    PlatformError::new(
        "folder-picker-unavailable",
        format!("Die Ordnerauswahl ließ sich nicht öffnen ({detail}); den Pfad bitte eintippen."),
    )
}

/// Base64 ohne weitere Abhängigkeit (Standardalphabet mit Auffüllung)
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let packed = group.iter().enumerate().fold(0u32, |packed, (at, byte)| {
            packed | (u32::from(*byte) << (16 - 8 * at))
        });
        for at in 0..4 {
            if at <= group.len() {
                let index = (packed >> (18 - 6 * at)) & 0b11_1111;
                out.push(char::from(ALPHABET[index as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(windows)]
pub use windows_picker::WindowsFolderPicker;

/// Die Umsetzung mit `powershell.exe`; nur unter Windows gebaut
#[cfg(windows)]
mod windows_picker {
    use async_trait::async_trait;

    use super::{arguments, unavailable, FOLDER_PICKER_SCRIPT, TIMEOUT};
    use crate::{FolderPicker, Result};

    /// Zeigt den Explorer-Ordnerdialog über Windows PowerShell
    #[derive(Debug, Clone, Copy, Default)]
    pub struct WindowsFolderPicker;

    impl WindowsFolderPicker {
        /// Neuer Dialog (hält keinen Zustand)
        pub fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl FolderPicker for WindowsFolderPicker {
        async fn pick(&self, start: Option<&str>) -> Result<Option<String>> {
            // Ausnahme vom Handbuch: hier ohne CREATE_NO_WINDOW. Der Dialog braucht ein
            // sichtbares Elternfenster, sonst erscheint er hinter dem Browser oder gar nicht
            // (TypeScript nutzt dafür `windowsHide: false`).
            let mut command = tokio::process::Command::new("powershell.exe");
            command
                .args(arguments(FOLDER_PICKER_SCRIPT))
                .env("HARNESS_PICK_START", start.unwrap_or(""))
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                // Nach dem Zeitlimit fällt das Kind weg und wird dabei beendet, damit kein
                // offener Dialog zurückbleibt
                .kill_on_drop(true);
            let child = command
                .spawn()
                .map_err(|error| unavailable(&error.to_string()))?;
            let output = match tokio::time::timeout(TIMEOUT, child.wait_with_output()).await {
                Ok(output) => output.map_err(|error| unavailable(&error.to_string()))?,
                Err(_) => return Err(unavailable("die Auswahl hat zu lange gedauert")),
            };
            if !output.status.success() {
                let code = output
                    .status
                    .code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unbekannt".into());
                return Err(unavailable(&format!("PowerShell endete mit Code {code}")));
            }
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(if path.is_empty() { None } else { Some(path) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kodiert_das_skript_als_base64_uber_utf_16le() {
        // Dieselben Werte wie `Buffer.from(text, 'utf16le').toString('base64')` in Node
        assert_eq!(encoded_command("A"), "QQA=");
        assert_eq!(encoded_command("AB"), "QQBCAA==");
        assert_eq!(encoded_command("ABC"), "QQBCAEMA");
        assert_eq!(encoded_command("ä"), "5AA=");
        assert_eq!(encoded_command(""), "");
    }

    #[test]
    fn ruft_powershell_mit_dem_kodierten_skript_auf() {
        let args = arguments(FOLDER_PICKER_SCRIPT);
        assert_eq!(
            args[..6],
            [
                "-NoProfile",
                "-NonInteractive",
                "-STA",
                "-ExecutionPolicy",
                "Bypass",
                "-EncodedCommand"
            ]
        );
        assert_eq!(args[6], encoded_command(FOLDER_PICKER_SCRIPT));
        assert!(args[6].len() > 1000, "das Skript fehlt");
    }

    #[test]
    fn das_skript_ist_das_der_typescript_fassung() {
        assert!(FOLDER_PICKER_SCRIPT.starts_with('\n'));
        assert!(FOLDER_PICKER_SCRIPT.contains("FOS_PICKFOLDERS"));
        assert!(FOLDER_PICKER_SCRIPT.contains("$env:HARNESS_PICK_START"));
        // Der Dialog schreibt den Pfad auf die Standardausgabe, sonst nichts
        assert!(FOLDER_PICKER_SCRIPT
            .trim_end()
            .ends_with("if ($path) { [Console]::Out.Write($path) }"));
        assert!(!FOLDER_PICKER_SCRIPT.contains('\r'), "CRLF im Skript");
    }

    #[test]
    fn nennt_den_grund_im_fehler_der_ordnerauswahl() {
        let error = unavailable("PowerShell endete mit Code 1");
        assert_eq!(error.code, "folder-picker-unavailable");
        assert_eq!(
            error.message,
            "Die Ordnerauswahl ließ sich nicht öffnen (PowerShell endete mit Code 1); den Pfad bitte eintippen."
        );
    }
}
