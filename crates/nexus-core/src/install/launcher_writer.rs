//! Schreibt die Startdateien `harness.cmd` (cmd und PowerShell) und `harness` (Bash, Git Bash,
//! macOS, Linux). Beide rufen jetzt das kopierte Binary `nexus` daneben auf, nicht mehr
//! `node <home>/app/lib/cli.mjs`. Portiert aus `packages/core/src/install/LauncherWriter.ts`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Name des Binaries im Startordner (unter Windows mit `.exe`)
pub const BINARY_NAME: &str = if cfg!(windows) { "nexus.exe" } else { "nexus" };

/// Inhalt von `harness.cmd`: ruft das Binary im eigenen Ordner auf (CRLF wie bisher)
const CMD: &str = "@echo off\r\n\"%~dp0nexus.exe\" %*\r\n";

/// Inhalt von `harness`: ruft das Binary im eigenen Ordner auf
const SH: &str = "#!/bin/sh\nexec \"$(dirname \"$0\")/nexus\" \"$@\"\n";

/// Legt die beiden Startdateien an.
#[derive(Debug, Clone, Copy, Default)]
pub struct LauncherWriter;

impl LauncherWriter {
    /// Neuer Schreiber (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Legt beide Startdateien im Ordner an und liefert ihre Pfade
    pub fn write(&self, bin_dir: &Path) -> Result<Vec<PathBuf>> {
        fs::create_dir_all(bin_dir).map_err(Error::unexpected)?;
        let cmd = bin_dir.join("harness.cmd");
        let sh = bin_dir.join("harness");
        fs::write(&cmd, CMD).map_err(Error::unexpected)?;
        fs::write(&sh, SH).map_err(Error::unexpected)?;
        make_executable(&sh)?;
        Ok(vec![cmd, sh])
    }
}

/// Setzt das Ausführungsrecht (0o755); unter Windows gibt es nichts zu tun
fn make_executable(file: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(file, fs::Permissions::from_mode(0o755)).map_err(Error::unexpected)?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legt_beide_startdateien_an_die_das_binary_daneben_aufrufen() {
        let dir = tempfile::Builder::new()
            .prefix("harness-launcher-")
            .tempdir()
            .unwrap();
        let bin = dir.path().join("bin");

        let launchers = LauncherWriter::new().write(&bin).unwrap();

        assert_eq!(launchers, vec![bin.join("harness.cmd"), bin.join("harness")]);
        let cmd = fs::read_to_string(bin.join("harness.cmd")).unwrap();
        assert!(cmd.contains("\"%~dp0nexus.exe\" %*"), "{cmd}");
        let sh = fs::read_to_string(bin.join("harness")).unwrap();
        assert!(sh.contains("exec \"$(dirname \"$0\")/nexus\" \"$@\""), "{sh}");
        assert!(!sh.contains('\r'), "{sh}");
    }

    #[cfg(unix)]
    #[test]
    fn die_sh_startdatei_ist_ausfuhrbar() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::Builder::new()
            .prefix("harness-launcher-")
            .tempdir()
            .unwrap();
        LauncherWriter::new().write(dir.path()).unwrap();
        let mode = fs::metadata(dir.path().join("harness"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}
