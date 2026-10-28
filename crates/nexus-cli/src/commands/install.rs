//! `harness install`: macht `harness` im normalen Terminal verfügbar (kopiert das Binary und
//! die Startdateien nach `~/.harness`, ergänzt den PATH). Portiert aus
//! `packages/cli/src/commands/InstallCommand.ts`.

use nexus_core::install::{platform_user_path, CliInstaller, InstallOptions, InstallResult};
use nexus_core::{HarnessError, Result};

use crate::context::CliContext;

/// Ablauf: gebündeltes Plugin erkennen, installieren, nächste Schritte nennen
pub fn run(context: &CliContext, register_path: bool) -> Result<u8> {
    let exe = std::env::current_exe().map_err(nexus_core::Error::unexpected)?;
    let Some(root) = CliInstaller::bundle_root(&exe) else {
        return Err(HarnessError::new(
            "install-not-bundled",
            "harness install funktioniert nur aus dem gebauten Plugin oder einem Release-Archiv.",
        )
        .into());
    };
    let user_path = platform_user_path();
    let installer = CliInstaller::new(context.home(), user_path.as_ref());
    let result = installer.install(&root, InstallOptions { register_path })?;
    let lines = lines(&result, register_path);
    context.printer().result(&result, &lines);
    Ok(0)
}

/// Ausgabe für Menschen
fn lines(result: &InstallResult, register_path: bool) -> Vec<String> {
    let mut lines = vec![
        format!("Installiert: {}", result.app_dir),
        format!("Startdateien: {}", result.bin_dir),
        format!("Regeln: {}", result.rules_dir),
    ];
    if !register_path {
        lines.push("PATH nicht geändert (--no-path).".to_string());
    } else if result.path_added {
        lines.push(
            "PATH ergänzt. Bitte ein neues Terminal öffnen, dann funktioniert `harness` überall."
                .to_string(),
        );
    } else {
        lines.push(
            "PATH war schon eingerichtet. Falls `harness` fehlt: neues Terminal öffnen."
                .to_string(),
        );
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ergebnis einer Installation
    fn ergebnis(path_added: bool, path_skipped: bool) -> InstallResult {
        InstallResult {
            bin_dir: "/h/bin".to_string(),
            app_dir: "/h/app".to_string(),
            rules_dir: "/h/rules".to_string(),
            launchers: vec!["/h/bin/harness".to_string()],
            path_added,
            path_skipped,
        }
    }

    #[test]
    fn nennt_kopie_startdateien_und_regeln() {
        let zeilen = lines(&ergebnis(true, false), true);
        assert_eq!(
            zeilen[..3],
            [
                "Installiert: /h/app".to_string(),
                "Startdateien: /h/bin".to_string(),
                "Regeln: /h/rules".to_string()
            ]
        );
    }

    #[test]
    fn sagt_was_mit_dem_path_passiert_ist() {
        assert_eq!(
            lines(&ergebnis(true, false), true)[3],
            "PATH ergänzt. Bitte ein neues Terminal öffnen, dann funktioniert `harness` überall."
        );
        assert_eq!(
            lines(&ergebnis(false, false), true)[3],
            "PATH war schon eingerichtet. Falls `harness` fehlt: neues Terminal öffnen."
        );
        assert_eq!(
            lines(&ergebnis(false, true), false)[3],
            "PATH nicht geändert (--no-path)."
        );
    }
}
