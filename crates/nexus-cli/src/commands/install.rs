//! `nexus install`: macht `nexus` im normalen Terminal verfügbar (kopiert das Binary und
//! die Startdateien nach `~/.nexus`, ergänzt den PATH) und richtet das Plugin bei Claude Code
//! ein. Portiert aus `packages/cli/src/commands/InstallCommand.ts` und `installPlugin()` in
//! `scripts/setup.mjs`.

use std::path::{Path, PathBuf};

use nexus_core::install::{
    platform_user_path, ClaudePluginInstall, CliInstaller, InstallOptions, InstallResult,
    PluginInstallResult, SystemClaudeCli,
};
use nexus_core::registry::{harness_dir, process_lookup, HarnessHome, HomeSource};
use nexus_core::{HarnessError, Result};
use serde::Serialize;

use crate::context::CliContext;

/// Was `install` tun soll
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// PATH des Benutzers ergänzen (Standard: ja; `--no-path` schaltet es ab)
    pub register_path: bool,
    /// Plugin bei Claude Code einrichten (Standard: ja; `--no-plugin` schaltet es ab)
    pub register_plugin: bool,
    /// Ordner mit `.claude-plugin/marketplace.json` (Standard: über dem gebauten Plugin)
    pub marketplace: Option<PathBuf>,
}

/// Ergebnis von `install` als JSON: die Kopie im Benutzerordner und das Plugin
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    #[serde(flatten)]
    install: InstallResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    plugin: Option<PluginInstallResult>,
}

/// Der alte Benutzerordner, dessen Startdateien auf das neue Binary zeigen sollen – oder `None`,
/// wenn nichts umzuleiten ist. Zeigt `NEXUS_HOME` auf einen eigenen Ordner (Probe- oder Testlauf),
/// bleibt `~/.harness/bin` unangetastet: ein Lauf auf einem Wegwerf-Ordner darf die Startdateien
/// des Rechners nicht auf sich umbiegen und den Befehl `harness` unbrauchbar machen.
fn legacy_home(home: &HarnessHome) -> Option<PathBuf> {
    match home.source() {
        HomeSource::EnvNexus => None,
        _ => Some(harness_dir(process_lookup)),
    }
}

/// Ablauf: gebündeltes Plugin erkennen, installieren, Plugin anmelden, nächste Schritte nennen
pub fn run(context: &CliContext, options: &Options) -> Result<u8> {
    let exe = std::env::current_exe().map_err(nexus_core::Error::unexpected)?;
    let Some(root) = CliInstaller::bundle_root(&exe) else {
        return Err(HarnessError::new(
            "install-not-bundled",
            "nexus install funktioniert nur aus dem gebauten Plugin oder einem Release-Archiv.",
        )
        .into());
    };
    let user_path = platform_user_path();
    let home = context.home();
    let mut installer = CliInstaller::new(home.clone(), user_path.as_ref());
    if let Some(legacy) = legacy_home(&home) {
        installer = installer.with_legacy_home(legacy);
    }
    let install = installer.install(
        &root,
        InstallOptions {
            register_path: options.register_path,
        },
    )?;
    let mut lines = lines(&install, options.register_path);
    let plugin = plugin(options, &root, &mut lines);
    let output = Output { install, plugin };
    context.printer().result(&output, &lines);
    Ok(0)
}

/// Richtet das Plugin bei Claude Code ein und ergänzt die Ausgabe; `--no-plugin` lässt es aus
fn plugin(options: &Options, root: &Path, lines: &mut Vec<String>) -> Option<PluginInstallResult> {
    if !options.register_plugin {
        lines.push("Plugin in Claude Code nicht angemeldet (--no-plugin).".to_string());
        return None;
    }
    let dir = match options
        .marketplace
        .clone()
        .or_else(|| ClaudePluginInstall::marketplace_dir(root))
    {
        Some(dir) => dir,
        None => {
            lines.push(
                "Kein Marktplatz gefunden (.claude-plugin/marketplace.json fehlt über dem Plugin). Bitte mit --marketplace <ordner> angeben."
                    .to_string(),
            );
            return None;
        }
    };
    let claude = SystemClaudeCli::new();
    let result = ClaudePluginInstall::from_process_env(&claude).install(&dir);
    lines.extend(plugin_lines(&result, &dir));
    Some(result)
}

/// Ausgabe zur Einrichtung des Plugins
fn plugin_lines(result: &PluginInstallResult, dir: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    if result.installed {
        lines.push(format!(
            "Plugin in Claude Code eingerichtet (Marktplatz {}).",
            dir.display()
        ));
        if !result.updated_projects.is_empty() {
            lines.push(format!(
                "Projekt-Einträge aktualisiert: {}",
                result.updated_projects.join(", ")
            ));
        }
        if result.legacy_disabled {
            lines.push(
                "Das alte Plugin ai-harness@ai-harness-local ist auf Benutzerebene ausgeschaltet (nicht deinstalliert)."
                    .to_string(),
            );
        }
        lines.push(
            "Auf Benutzerebene ausgeschaltet; `nexus init` schaltet es je Projekt ein.".to_string(),
        );
    }
    lines.extend(result.hints.iter().cloned());
    lines
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
            "PATH ergänzt. Bitte ein neues Terminal öffnen, dann funktioniert `nexus` überall."
                .to_string(),
        );
    } else {
        lines.push(
            "PATH war schon eingerichtet. Falls `nexus` fehlt: neues Terminal öffnen.".to_string(),
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
            "PATH ergänzt. Bitte ein neues Terminal öffnen, dann funktioniert `nexus` überall."
        );
        assert_eq!(
            lines(&ergebnis(false, false), true)[3],
            "PATH war schon eingerichtet. Falls `nexus` fehlt: neues Terminal öffnen."
        );
        assert_eq!(
            lines(&ergebnis(false, true), false)[3],
            "PATH nicht geändert (--no-path)."
        );
    }

    #[test]
    fn nennt_marktplatz_projekte_und_das_alte_plugin() {
        let result = PluginInstallResult {
            available: true,
            marketplace_added: true,
            installed: true,
            updated_projects: vec!["/p".to_string()],
            disabled_for_user: true,
            legacy_disabled: true,
            hints: Vec::new(),
        };

        let zeilen = plugin_lines(&result, Path::new("/clone"));

        assert!(
            zeilen[0].starts_with("Plugin in Claude Code eingerichtet"),
            "{zeilen:?}"
        );
        assert_eq!(zeilen[1], "Projekt-Einträge aktualisiert: /p");
        assert!(
            zeilen[2].contains("ai-harness@ai-harness-local"),
            "{zeilen:?}"
        );
        assert!(zeilen[3].contains("nexus init"), "{zeilen:?}");
    }

    #[test]
    fn gibt_ohne_claude_nur_die_hinweise_aus() {
        let result = PluginInstallResult {
            hints: vec!["Claude Code wurde nicht gefunden.".to_string()],
            ..PluginInstallResult::default()
        };

        assert_eq!(
            plugin_lines(&result, Path::new("/clone")),
            ["Claude Code wurde nicht gefunden."]
        );
    }

    #[test]
    fn mit_eigenem_nexus_home_bleibt_der_alte_startordner_unberuhrt() {
        let home = HarnessHome::with_source("/tmp/probe", HomeSource::EnvNexus);

        assert_eq!(legacy_home(&home), None);
    }

    #[test]
    fn im_ueblichen_benutzerordner_werden_die_alten_startdateien_umgeleitet() {
        for quelle in [
            HomeSource::Nexus,
            HomeSource::HarnessFallback,
            HomeSource::New,
            HomeSource::EnvHarness,
        ] {
            let home = HarnessHome::with_source("/u/.nexus", quelle);
            assert_eq!(
                legacy_home(&home),
                Some(harness_dir(process_lookup)),
                "{quelle:?}"
            );
        }
    }

    #[test]
    fn mit_no_plugin_bleibt_claude_code_unberuhrt() {
        let mut zeilen = Vec::new();
        let options = Options {
            register_path: true,
            register_plugin: false,
            marketplace: None,
        };

        assert!(plugin(&options, Path::new("/plugin"), &mut zeilen).is_none());
        assert_eq!(
            zeilen,
            ["Plugin in Claude Code nicht angemeldet (--no-plugin)."]
        );
    }
}
