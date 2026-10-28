//! `nexus customize …`: Anpassungen des Clones prüfen, kopieren, zurücksetzen, anwenden und
//! die Übersicht anzeigen. Ersetzt `scripts/customize.mjs` und die Aufrufe von
//! `CustomizationApplier` und `OverviewFormatter` in `build-plugin.mjs` und `setup.mjs`.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use nexus_core::clock::Clock;
use nexus_core::customization::{
    BaseSnapshot, CloneWorkflowTools, CustomizationApplier, CustomizationCopier,
    CustomizationEntry, CustomizationInput, CustomizationOverview, CustomizationPaths,
    CustomizeArgs, CustomizeCommand, EntryKind, EntryState, ExtensionPoint, OverviewFile,
    OverviewFormatter, Problem, WorkflowState, DEFAULT_MAX_DIFF_LINES,
};
use nexus_core::git::GitPaths;
use nexus_core::{Error, Result};

use crate::context::CliContext;

/// Ordner mit den mitgelieferten Skills und Output-Styles im Clone
const MANIFEST: [&str; 3] = ["packages", "plugin", "manifest"];

/// Datei, aus der die Fassung des Harness kommt
const VERSION_FILE: [&str; 3] = ["packages", "server", "package.json"];

/// Gemeinsame Optionen und der Unterbefehl von `nexus customize`
#[derive(Debug, Args)]
pub struct CustomizeCli {
    #[arg(
        long,
        value_name = "ordner",
        help = "Clone des Harness (Standard: Wurzel des Git-Repos im aktuellen Ordner)"
    )]
    pub clone: Option<PathBuf>,
    #[arg(
        long,
        value_name = "ordner",
        help = "Ordner der Anpassungen (Standard: HARNESS_CUSTOM_DIR, sonst <clone>/custom)"
    )]
    pub custom: Option<PathBuf>,
    // Nur für `scripts/customize.mjs`: dort heißt der Aufruf „npm run customize -- …“
    #[arg(long = "aufruf", value_name = "text", hide = true)]
    pub aufruf: Option<String>,
    #[command(subcommand)]
    pub command: Option<CustomizeCommands>,
}

/// Die Unterbefehle; `check`, `copy` und `reset` lesen ihre Argumente wie
/// `npm run customize -- …` und melden Aufruffehler als `customize-usage`
#[derive(Debug, Subcommand)]
pub enum CustomizeCommands {
    /// Anpassungen auf den Standard anwenden (für den Bau des Plugins)
    #[command(
        name = "apply",
        about = "Anpassungen prüfen und über ein gebautes Plugin legen",
        disable_help_flag = true
    )]
    Apply {
        #[arg(
            long,
            value_name = "ordner",
            required = true,
            help = "Ordner des Standards (skills/ und output-styles/)"
        )]
        standard: PathBuf,
        #[arg(
            long,
            value_name = "ordner",
            required = true,
            help = "Ordner des gebauten Plugins, der überlagert wird"
        )]
        out: PathBuf,
        #[arg(
            long,
            value_name = "fassung",
            help = "Fassung des Harness für die Übersicht (Standard: aus packages/server/package.json)"
        )]
        version: Option<String>,
    },

    /// Die Übersicht des letzten Anwendens anzeigen (für `setup`)
    #[command(
        name = "overview",
        about = "Übersicht der Anpassungen aus customizations/overview.json anzeigen",
        disable_help_flag = true
    )]
    Overview {
        #[arg(
            long,
            value_name = "datei",
            help = "Datei der Übersicht (Standard: customizations/overview.json im Harness-Ordner)"
        )]
        file: Option<PathBuf>,
    },

    /// `check`, `copy …` und `reset …` samt ihren Argumenten
    #[command(external_subcommand)]
    Weitere(Vec<String>),
}

/// Führt `nexus customize …` aus
pub fn run(context: &CliContext, cli: CustomizeCli) -> Result<u8> {
    let clone = clone_dir(context, cli.clone.as_deref());
    let custom = custom_dir(cli.custom.as_deref(), &clone);
    match cli.command {
        Some(CustomizeCommands::Apply {
            standard,
            out,
            version,
        }) => apply(context, &clone, &custom, &standard, &out, version),
        Some(CustomizeCommands::Overview { file }) => overview(context, file.as_deref()),
        Some(CustomizeCommands::Weitere(args)) => {
            weitere(context, &clone, &custom, &args, cli.aufruf.as_deref())
        }
        // Ohne Unterbefehl gilt dasselbe wie bei einem leeren Aufruf
        None => weitere(context, &clone, &custom, &[], cli.aufruf.as_deref()),
    }
}

/// `check`, `copy` und `reset`
fn weitere(
    context: &CliContext,
    clone: &Path,
    custom: &Path,
    args: &[String],
    aufruf: Option<&str>,
) -> Result<u8> {
    let parser = match aufruf {
        Some(text) => CustomizeArgs::with_usage(usage(text)),
        None => CustomizeArgs::new(),
    };
    let standard = manifest_dir(clone);
    let paths = CustomizationPaths::new(custom, &standard);
    match parser.parse(args)? {
        CustomizeCommand::Check => check(context, clone, custom, &standard),
        CustomizeCommand::Copy { target, rebase } => {
            let message = CustomizationCopier::new(&paths).copy(&target, rebase)?;
            meldung(context, &message);
            Ok(0)
        }
        CustomizeCommand::Reset { target } => {
            let message = CustomizationCopier::new(&paths).reset(&target)?;
            meldung(context, &message);
            Ok(0)
        }
    }
}

/// `customize check`: prüft alles wie `setup`, ohne zu schreiben, und zeigt die Diffs vollständig
fn check(context: &CliContext, clone: &Path, custom: &Path, standard: &Path) -> Result<u8> {
    let tools = CloneWorkflowTools::new(clone);
    let input = CustomizationInput {
        clone_dir: clone.to_path_buf(),
        custom_dir: custom.to_path_buf(),
        standard_dir: standard.to_path_buf(),
        harness_version: harness_version(clone),
        tools: &tools,
        clock: Clock::system(),
    };
    let result = CustomizationApplier::new().check(&input);
    let text = OverviewFormatter::new().format(&result.overview, None);
    context.printer().result(&result.overview, &zeilen(&text));
    Ok(0)
}

/// `customize apply`: wendet die Anpassungen auf ein gebautes Plugin an; ein Fehler darf den
/// Bau nie abbrechen (FR-015), deshalb Warnung, Rückfall-Übersicht und Rückgabecode 0
fn apply(
    context: &CliContext,
    clone: &Path,
    custom: &Path,
    standard: &Path,
    out: &Path,
    version: Option<String>,
) -> Result<u8> {
    let harness_version = version.unwrap_or_else(|| harness_version(clone));
    let tools = CloneWorkflowTools::new(clone);
    let input = CustomizationInput {
        clone_dir: clone.to_path_buf(),
        custom_dir: custom.to_path_buf(),
        standard_dir: standard.to_path_buf(),
        harness_version: harness_version.clone(),
        tools: &tools,
        clock: Clock::system(),
    };
    let printer = context.printer();
    match CustomizationApplier::new().apply(&input, out) {
        Ok(overview) => {
            printer.result(
                &overview,
                &zeilen(&format!(
                    "Anpassungen angewendet ({} Einträge).",
                    overview.entries.len()
                )),
            );
            Ok(0)
        }
        Err(error) => {
            let message = error.to_string();
            printer.warn(&format!(
                "Anpassungen ließen sich nicht anwenden: {message}. Es gilt der Standard."
            ));
            let overview = fallback_overview(clone, custom, &harness_version, &message);
            write_fallback(out, &overview)?;
            printer.result(&overview, &[]);
            Ok(0)
        }
    }
}

/// `customize overview`: zeigt die Übersicht des letzten Anwendens
fn overview(context: &CliContext, file: Option<&Path>) -> Result<u8> {
    let printer = context.printer();
    let overview = match file {
        Some(file) => OverviewFile::read_file(file),
        None => OverviewFile::new(context.home()).read(),
    };
    let Some(overview) = overview else {
        printer
            .warn("Keine Übersicht der Anpassungen gefunden (customizations/overview.json fehlt).");
        return Ok(0);
    };
    let text = OverviewFormatter::new().format(&overview, Some(DEFAULT_MAX_DIFF_LINES));
    printer.result(&overview, &zeilen(&text));
    Ok(0)
}

/// Übersicht mit Rückfall auf den Standard, wenn das Anwenden scheiterte
/// (wie `writeFallbackOverview` in `build-plugin.mjs`)
fn fallback_overview(
    clone: &Path,
    custom: &Path,
    harness_version: &str,
    message: &str,
) -> CustomizationOverview {
    let problem = Problem::new(
        "apply-failed",
        format!("Anpassungen ließen sich nicht anwenden: {message}"),
    );
    let entry = CustomizationEntry {
        state: EntryState::Fallback,
        problems: vec![problem],
        ..CustomizationEntry::new(ExtensionPoint::Workflow, "workflow", EntryKind::Replace)
    };
    CustomizationOverview {
        version: 1,
        created_at: Clock::system().iso(),
        clone_dir: clone.to_string_lossy().into_owned(),
        custom_dir: custom.to_string_lossy().into_owned(),
        harness_version: harness_version.to_string(),
        workflow: WorkflowState::Fallback,
        entries: vec![entry],
    }
}

/// Schreibt die Rückfall-Übersicht; der alte Ordner wird dabei vollständig ersetzt
fn write_fallback(out: &Path, overview: &CustomizationOverview) -> Result<()> {
    let dir = out.join("customizations");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(Error::unexpected)?;
    }
    std::fs::create_dir_all(&dir).map_err(Error::unexpected)?;
    std::fs::write(
        dir.join("overview.json"),
        BaseSnapshot::format_json(overview)?,
    )
    .map_err(Error::unexpected)
}

/// Meldung eines Vorgangs ausgeben (im JSON-Modus als Objekt)
fn meldung(context: &CliContext, message: &str) {
    context
        .printer()
        .result(&serde_json::json!({ "message": message }), &zeilen(message));
}

/// Ein mehrzeiliger Text als Liste von Zeilen für den Drucker
fn zeilen(text: &str) -> Vec<String> {
    text.split('\n').map(ToString::to_string).collect()
}

/// Aufruftext für Fehlermeldungen, aus dem Namen des Aufrufs
fn usage(aufruf: &str) -> String {
    format!("Aufruf: {aufruf} check | copy workflow|skill <name>|output-style <name> [--rebase] | reset workflow|skill <name>|output-style <name>")
}

/// Clone des Harness: `--clone`, sonst die Wurzel des Git-Repos, sonst der Projektordner
fn clone_dir(context: &CliContext, option: Option<&Path>) -> PathBuf {
    if let Some(dir) = option {
        return dir.to_path_buf();
    }
    let dir = context.project_dir();
    GitPaths::new()
        .top_level(dir)
        .map(PathBuf::from)
        .unwrap_or_else(|_| dir.to_path_buf())
}

/// Ordner der Anpassungen: `--custom`, sonst `HARNESS_CUSTOM_DIR`, sonst `<clone>/custom`
fn custom_dir(option: Option<&Path>, clone: &Path) -> PathBuf {
    if let Some(dir) = option {
        return dir.to_path_buf();
    }
    match std::env::var("HARNESS_CUSTOM_DIR") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => clone.join("custom"),
    }
}

/// Ordner des Standards im Clone (`packages/plugin/manifest`)
fn manifest_dir(clone: &Path) -> PathBuf {
    MANIFEST
        .iter()
        .fold(clone.to_path_buf(), |dir, teil| dir.join(teil))
}

/// Fassung des Harness aus `packages/server/package.json`; fehlt sie, bleibt sie leer
fn harness_version(clone: &Path) -> String {
    let file = VERSION_FILE
        .iter()
        .fold(clone.to_path_buf(), |dir, teil| dir.join(teil));
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json["version"].as_str().map(ToString::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_aufruftext_nennt_den_aufrufer() {
        assert!(usage("npm run customize --").starts_with("Aufruf: npm run customize -- check |"));
    }

    #[test]
    fn der_ordner_der_anpassungen_kommt_aus_der_option_oder_dem_clone() {
        let clone = Path::new("/c");
        assert_eq!(
            custom_dir(Some(Path::new("/woanders")), clone),
            PathBuf::from("/woanders")
        );
        // Ohne Option und ohne HARNESS_CUSTOM_DIR gilt <clone>/custom
        if std::env::var("HARNESS_CUSTOM_DIR").is_err() {
            assert_eq!(custom_dir(None, clone), clone.join("custom"));
        }
    }

    #[test]
    fn der_standard_liegt_im_manifest_des_clones() {
        assert_eq!(
            manifest_dir(Path::new("/c")),
            Path::new("/c")
                .join("packages")
                .join("plugin")
                .join("manifest")
        );
    }

    #[test]
    fn die_ruckfall_ubersicht_nennt_den_grund() {
        let overview =
            fallback_overview(Path::new("/c"), Path::new("/c/custom"), "0.2.0", "kaputt");
        assert_eq!(overview.workflow, WorkflowState::Fallback);
        assert_eq!(overview.entries.len(), 1);
        assert_eq!(overview.entries[0].problems[0].code, "apply-failed");
        assert_eq!(
            overview.entries[0].problems[0].message,
            "Anpassungen ließen sich nicht anwenden: kaputt"
        );
    }
}
