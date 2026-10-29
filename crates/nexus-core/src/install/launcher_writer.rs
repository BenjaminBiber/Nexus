//! Schreibt die Startdateien `harness.cmd` (cmd und PowerShell) und `harness` (Bash, Git Bash,
//! macOS, Linux). Beide rufen jetzt das kopierte Binary `nexus` auf, nicht mehr
//! `node <home>/app/lib/cli.mjs`, und melden über `NEXUS_INVOKED_AS` den alten Namen, damit
//! `nexus` einmal darauf hinweist. Portiert aus `packages/core/src/install/LauncherWriter.ts`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Name des Binaries im Startordner (unter Windows mit `.exe`)
pub const BINARY_NAME: &str = if cfg!(windows) { "nexus.exe" } else { "nexus" };

/// Namen der Startdateien des alten Programmnamens
pub const LAUNCHER_NAMES: [&str; 2] = ["harness.cmd", "harness"];

/// Variable, an der `nexus` den alten Programmnamen erkennt
const INVOKED_AS_ENV: &str = "NEXUS_INVOKED_AS";

/// Wert, unter dem der alte Name gemeldet wird
const INVOKED_AS_HARNESS: &str = "harness";

/// Legt die Startdateien an.
#[derive(Debug, Clone, Copy, Default)]
pub struct LauncherWriter;

impl LauncherWriter {
    /// Neuer Schreiber (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Legt beide Startdateien im Ordner an; sie rufen das Binary daneben auf
    pub fn write(&self, bin_dir: &Path) -> Result<Vec<PathBuf>> {
        self.write_all(bin_dir, "\"%~dp0nexus.exe\"", "\"$(dirname \"$0\")/nexus\"")
    }

    /// Ersetzt die Startdateien im alten Ordner `~/.harness/bin` durch Weiterleitungen auf das
    /// neue Binary. So startet ein alter PATH-Eintrag nicht mehr die alte Fassung auf den
    /// alten Daten. Nur die Startdateien werden angefasst, keine Daten.
    pub fn write_forwarders(&self, bin_dir: &Path, binary: &Path) -> Result<Vec<PathBuf>> {
        let target = binary.display().to_string();
        let windows_target = format!("\"{}\"", target.replace('/', "\\"));
        self.write_all(bin_dir, &windows_target, &format!("\"{target}\""))
    }

    /// Schreibt beide Startdateien mit dem jeweils aufzurufenden Binary
    fn write_all(&self, bin_dir: &Path, windows: &str, unix: &str) -> Result<Vec<PathBuf>> {
        fs::create_dir_all(bin_dir).map_err(Error::unexpected)?;
        let cmd = bin_dir.join(LAUNCHER_NAMES[0]);
        let sh = bin_dir.join(LAUNCHER_NAMES[1]);
        // CRLF wie bisher, damit cmd die Datei auf jedem Windows liest
        let cmd_text = format!(
            "@echo off\r\nsetlocal\r\nset \"{INVOKED_AS_ENV}={INVOKED_AS_HARNESS}\"\r\n{windows} %*\r\n"
        );
        let sh_text = format!(
            "#!/bin/sh\n{INVOKED_AS_ENV}={INVOKED_AS_HARNESS}\nexport {INVOKED_AS_ENV}\nexec {unix} \"$@\"\n"
        );
        fs::write(&cmd, cmd_text).map_err(Error::unexpected)?;
        fs::write(&sh, sh_text).map_err(Error::unexpected)?;
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

        assert_eq!(
            launchers,
            vec![bin.join("harness.cmd"), bin.join("harness")]
        );
        let cmd = fs::read_to_string(bin.join("harness.cmd")).unwrap();
        assert!(cmd.contains("\"%~dp0nexus.exe\" %*"), "{cmd}");
        let sh = fs::read_to_string(bin.join("harness")).unwrap();
        assert!(
            sh.contains("exec \"$(dirname \"$0\")/nexus\" \"$@\""),
            "{sh}"
        );
        assert!(!sh.contains('\r'), "{sh}");
    }

    #[test]
    fn die_startdateien_melden_den_alten_programmnamen() {
        let dir = tempfile::Builder::new()
            .prefix("harness-launcher-")
            .tempdir()
            .unwrap();

        LauncherWriter::new().write(dir.path()).unwrap();

        let sh = fs::read_to_string(dir.path().join("harness")).unwrap();
        assert!(sh.contains("NEXUS_INVOKED_AS=harness"), "{sh}");
        assert!(sh.contains("export NEXUS_INVOKED_AS"), "{sh}");
        let cmd = fs::read_to_string(dir.path().join("harness.cmd")).unwrap();
        assert!(cmd.contains("set \"NEXUS_INVOKED_AS=harness\""), "{cmd}");
        // `setlocal` hält die Variable im Skript, sie bleibt nicht in der Shell des Benutzers
        assert!(cmd.contains("setlocal"), "{cmd}");
    }

    #[test]
    fn die_weiterleitungen_zeigen_auf_das_binary_im_neuen_ordner() {
        let dir = tempfile::Builder::new()
            .prefix("harness-launcher-")
            .tempdir()
            .unwrap();
        let alt = dir.path().join("alt").join("bin");
        let binary = dir.path().join("neu").join("bin").join(BINARY_NAME);

        let launchers = LauncherWriter::new()
            .write_forwarders(&alt, &binary)
            .unwrap();

        assert_eq!(
            launchers,
            vec![alt.join("harness.cmd"), alt.join("harness")]
        );
        let sh = fs::read_to_string(alt.join("harness")).unwrap();
        assert!(
            sh.contains(&format!("exec \"{}\" \"$@\"", binary.display())),
            "{sh}"
        );
        assert!(sh.contains("NEXUS_INVOKED_AS=harness"), "{sh}");
        let cmd = fs::read_to_string(alt.join("harness.cmd")).unwrap();
        assert!(
            cmd.contains(&binary.display().to_string().replace('/', "\\")),
            "{cmd}"
        );
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
