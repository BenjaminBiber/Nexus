//! Kopieren von Plugin-Ordnern in den Benutzerordner; gemeinsam für `nexus install` und die
//! Einrichtung aus der Desktop-App.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::registry::HarnessHome;

/// Legt Regeln und Anpassungen eines Plugin-Ordners frisch unter `~/.nexus/rules` und
/// `~/.nexus/customizations` ab: dem festen Importziel der Projekte und dem Ordner, den Hook und
/// Server lesen. Der Ordner des Plugins in Claude Code taugt dafür nicht, er wechselt mit jeder
/// Version.
pub fn copy_home_data(plugin_root: &Path, home: &HarnessHome) -> Result<()> {
    copy_fresh(&plugin_root.join("rules"), &home.rules_dir())?;
    copy_fresh(
        &plugin_root.join("customizations"),
        &home.customizations_dir(),
    )
}

/// Kopiert einen Ordner frisch an sein Ziel (ein vorhandenes Ziel wird ersetzt); fehlt die
/// Quelle oder ist sie schon das Ziel, passiert nichts
pub fn copy_fresh(source: &Path, target: &Path) -> Result<()> {
    if !source.is_dir() || is_same(source, target) {
        return Ok(());
    }
    remove_dir(target)?;
    copy_dir(source, target)
}

/// Löscht einen Ordner samt Inhalt, falls es ihn gibt
pub(crate) fn remove_dir(dir: &Path) -> Result<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::unexpected(error)),
    }
}

/// Kopiert einen Ordner samt Unterordnern (wie `cp -r`)
pub(crate) fn copy_dir(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).map_err(Error::unexpected)?;
    for entry in fs::read_dir(source).map_err(Error::unexpected)? {
        let entry = entry.map_err(Error::unexpected)?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(Error::unexpected)?;
        }
    }
    Ok(())
}

/// Quelle und Ziel gleich? (Aufruf aus der bereits installierten Kopie; Windows ohne
/// Groß-/Kleinschreibung)
pub(crate) fn is_same(a: &Path, b: &Path) -> bool {
    let norm = |path: &Path| {
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let text = absolute.to_string_lossy().into_owned();
        if cfg!(windows) {
            text.to_lowercase()
        } else {
            text
        }
    };
    norm(a) == norm(b)
}
