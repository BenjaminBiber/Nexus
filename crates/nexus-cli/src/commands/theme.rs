//! `nexus theme pack`: baut aus einem Theme-Ordner ein `.nexus`-Paket.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use nexus_core::themes::{pack_theme, ThemePackageError};
use nexus_core::{Error, HarnessError, Result};

/// Beschreibung der Gruppe `theme`
pub const THEME_ABOUT: &str = "Theme-Pakete bauen";

/// Beschreibung von `theme pack`
pub const PACK_ABOUT: &str = "Prüft einen Theme-Ordner und baut daraus ein .nexus-Paket (ohne --out: <id>-<version>.nexus im aktuellen Ordner).";
/// Beispiele unter der Hilfe von `theme pack`
pub const PACK_EXAMPLES: &str =
    "\nBeispiele:\n  nexus theme pack mein-theme\n  nexus theme pack mein-theme --out dist/mein-theme.nexus";

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
        name = "help",
        about = "Hilfe zu einem Befehl anzeigen",
        disable_help_flag = true
    )]
    Help { befehl: Option<String> },
}

/// Führt einen Unterbefehl von `theme` aus
pub fn run(command: &ThemeCommands) -> Result<u8> {
    match command {
        ThemeCommands::Pack(args) => pack(&args.ordner, args.out.as_deref()),
        ThemeCommands::Help { befehl } => crate::cli::group_help(&["theme"], befehl.as_deref()),
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
