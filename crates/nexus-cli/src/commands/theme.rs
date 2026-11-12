//! `nexus theme`: Theme-Pakete (`.nexus`) bauen (`pack`), installieren (`install`), auflisten
//! (`list`) und entfernen (`remove`).

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use nexus_core::themes::{
    install_theme_file, list_themes, pack_theme, remove_theme, ColorScheme, ThemePackageError,
};
use nexus_core::{Error, HarnessError, Result};

use crate::context::CliContext;

/// Beschreibung der Gruppe `theme`
pub const THEME_ABOUT: &str = "Theme-Pakete bauen und verwalten";

/// Beschreibung von `theme pack`
pub const PACK_ABOUT: &str = "Prüft einen Theme-Ordner und baut daraus ein .nexus-Paket (ohne --out: <id>-<version>.nexus im aktuellen Ordner).";
/// Beispiele unter der Hilfe von `theme pack`
pub const PACK_EXAMPLES: &str =
    "\nBeispiele:\n  nexus theme pack mein-theme\n  nexus theme pack mein-theme --out dist/mein-theme.nexus";

/// Beschreibung von `theme install`
pub const INSTALL_ABOUT: &str = "Installiert eine .nexus-Datei nach <NEXUS_HOME>/themes/<id>/ (eine vorhandene gleiche id wird ersetzt).";
/// Beispiele unter der Hilfe von `theme install`
pub const INSTALL_EXAMPLES: &str = "\nBeispiele:\n  nexus theme install mein-theme-1.0.0.nexus";

/// Beschreibung von `theme list`
pub const LIST_ABOUT: &str = "Listet die installierten Themes (id, Version, Name, Farbschema).";
/// Beispiele unter der Hilfe von `theme list`
pub const LIST_EXAMPLES: &str = "\nBeispiele:\n  nexus theme list";

/// Beschreibung von `theme remove`
pub const REMOVE_ABOUT: &str = "Entfernt ein installiertes Theme.";
/// Beispiele unter der Hilfe von `theme remove`
pub const REMOVE_EXAMPLES: &str = "\nBeispiele:\n  nexus theme remove mein-theme";

/// Hinweis, wenn ein installiertes Theme eigenen Code mitbringt
const CODE_HINT: &str = "Achtung: Das Theme bringt Code mit (main.js).";

/// Ordner und Ziel von `theme pack`
#[derive(Debug, Args)]
pub struct PackArgs {
    pub ordner: PathBuf,
    #[arg(
        long,
        value_name = "datei",
        help = "Zieldatei (Standard: <id>-<version>.nexus im aktuellen Ordner)"
    )]
    pub out: Option<PathBuf>,
}

/// Paketdatei von `theme install`
#[derive(Debug, Args)]
pub struct InstallArgs {
    pub datei: PathBuf,
}

/// Theme-ID von `theme remove`
#[derive(Debug, Args)]
pub struct RemoveArgs {
    pub id: String,
}

/// Die Unterbefehle von `theme`
#[derive(Debug, Subcommand)]
pub enum ThemeCommands {
    #[command(
        name = "pack",
        about = PACK_ABOUT,
        after_help = PACK_EXAMPLES,
        disable_help_flag = true
    )]
    Pack(PackArgs),

    #[command(
        name = "install",
        about = INSTALL_ABOUT,
        after_help = INSTALL_EXAMPLES,
        disable_help_flag = true
    )]
    Install(InstallArgs),

    #[command(
        name = "list",
        about = LIST_ABOUT,
        after_help = LIST_EXAMPLES,
        disable_help_flag = true
    )]
    List,

    #[command(
        name = "remove",
        about = REMOVE_ABOUT,
        after_help = REMOVE_EXAMPLES,
        disable_help_flag = true
    )]
    Remove(RemoveArgs),

    #[command(
        name = "help",
        about = "Hilfe zu einem Befehl anzeigen",
        disable_help_flag = true
    )]
    Help { befehl: Option<String> },
}

/// Führt einen Unterbefehl von `theme` aus
pub fn run(context: &CliContext, command: &ThemeCommands) -> Result<u8> {
    match command {
        ThemeCommands::Pack(args) => pack(&args.ordner, args.out.as_deref()),
        ThemeCommands::Install(args) => install(context, &args.datei),
        ThemeCommands::List => list(context),
        ThemeCommands::Remove(args) => remove(context, &args.id),
        ThemeCommands::Help { befehl } => crate::cli::group_help(&["theme"], befehl.as_deref()),
    }
}

/// Installiert die Datei unter `<NEXUS_HOME>/themes/<id>/`; eine vorhandene gleiche id wird
/// ersetzt. Bringt das Paket Code mit, steht ein deutlicher Hinweis in der Ausgabe.
fn install(context: &CliContext, datei: &Path) -> Result<u8> {
    if !datei.is_file() {
        return Err(HarnessError::new(
            "theme-pack-invalid",
            format!("Datei nicht gefunden: {}", datei.display()),
        )
        .into());
    }
    let home = context.home();
    let manifest = install_theme_file(&home, datei).map_err(theme_error)?;
    let mit_code = home
        .themes_dir()
        .join(&manifest.id)
        .join("main.js")
        .is_file();
    let mut lines = vec![format!(
        "Theme installiert: {} {} ({})",
        manifest.id, manifest.version, manifest.name
    )];
    if mit_code {
        lines.push(CODE_HINT.to_string());
    }
    context.printer().result(
        &serde_json::json!({
            "id": manifest.id,
            "version": manifest.version,
            "name": manifest.name,
            "hasCode": mit_code,
        }),
        &lines,
    );
    Ok(0)
}

/// Listet die installierten Themes nach id sortiert
fn list(context: &CliContext) -> Result<u8> {
    let themes = list_themes(&context.home()).map_err(theme_error)?;
    let lines: Vec<String> = if themes.is_empty() {
        vec!["Keine Themes installiert.".to_string()]
    } else {
        let breite = themes
            .iter()
            .map(|theme| theme.manifest.id.len())
            .max()
            .unwrap_or(0);
        let version = themes
            .iter()
            .map(|theme| theme.manifest.version.len())
            .max()
            .unwrap_or(0);
        themes
            .iter()
            .map(|theme| {
                let manifest = &theme.manifest;
                format!(
                    "{:<breite$}  {:<version$}  {}  ({})",
                    manifest.id,
                    manifest.version,
                    manifest.name,
                    color_scheme(manifest.color_scheme)
                )
            })
            .collect()
    };
    let data: Vec<_> = themes.iter().map(|theme| &theme.manifest).collect();
    context.printer().result(&data, &lines);
    Ok(0)
}

/// Entfernt den Ordner des Themes
fn remove(context: &CliContext, id: &str) -> Result<u8> {
    remove_theme(&context.home(), id).map_err(theme_error)?;
    context.printer().result(
        &serde_json::json!({ "id": id }),
        &[format!("Theme entfernt: {id}")],
    );
    Ok(0)
}

/// Farbschema wie im Manifest (`light`/`dark`)
fn color_scheme(scheme: ColorScheme) -> &'static str {
    match scheme {
        ColorScheme::Light => "light",
        ColorScheme::Dark => "dark",
    }
}

/// Baut das Paket und schreibt es über eine temporäre Datei im Zielordner, damit bei einem
/// Fehler nichts Halbes liegen bleibt
fn pack(ordner: &Path, out: Option<&Path>) -> Result<u8> {
    let paket = pack_theme(ordner).map_err(theme_error)?;
    let ziel = match out {
        Some(out) => out.to_path_buf(),
        None => PathBuf::from(format!(
            "{}-{}.nexus",
            paket.manifest.id, paket.manifest.version
        )),
    };
    let zielordner = match ziel.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&zielordner).map_err(Error::unexpected)?;
    let temp = tempfile::NamedTempFile::new_in(&zielordner).map_err(Error::unexpected)?;
    std::fs::write(temp.path(), &paket.bytes).map_err(Error::unexpected)?;
    temp.persist(&ziel)
        .map_err(|error| Error::unexpected(error.error))?;
    println!(
        "Paket gebaut: {} ({} {}, {} Dateien)",
        ziel.display(),
        paket.manifest.id,
        paket.manifest.version,
        paket.files
    );
    Ok(0)
}

/// Prüffehler des Pakets sind erwartbar: Meldung mit Datei und Grund, Rückgabecode 1
fn theme_error(error: ThemePackageError) -> Error {
    match error {
        ThemePackageError::Io(error) => Error::unexpected(error),
        andere => HarnessError::new("theme-pack-invalid", andere.to_string()).into(),
    }
}
