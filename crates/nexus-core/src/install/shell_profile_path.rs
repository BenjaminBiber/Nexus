//! PATH-Eintrag unter macOS und Linux: eine Zeile in den vorhandenen Shell-Profilen (sonst
//! `.profile`). Portiert aus `packages/core/src/install/ShellProfilePath.ts`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::install::user_path::UserPath;

/// Profile, die ergänzt werden, wenn es sie gibt
const PROFILES: &[&str] = &[".zshrc", ".bashrc", ".profile"];

/// Marke der angehängten Zeile
const MARKER: &str = "# ai-harness";

/// Hängt die PATH-Zeile an die Shell-Profile an.
#[derive(Debug, Clone)]
pub struct ShellProfilePath {
    home_dir: PathBuf,
}

impl Default for ShellProfilePath {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellProfilePath {
    /// Profile im Benutzerordner des angemeldeten Benutzers
    pub fn new() -> Self {
        #[allow(deprecated)]
        Self::at(std::env::home_dir().unwrap_or_default())
    }

    /// Profile in einem frei wählbaren Benutzerordner (Tests)
    pub fn at(home_dir: impl Into<PathBuf>) -> Self {
        Self {
            home_dir: home_dir.into(),
        }
    }

    /// Vorhandene Profile; gibt es keines, wird `.profile` neu angelegt
    fn targets(&self) -> Vec<PathBuf> {
        let existing: Vec<PathBuf> = PROFILES
            .iter()
            .map(|name| self.home_dir.join(name))
            .filter(|file| file.exists())
            .collect();
        if existing.is_empty() {
            vec![self.home_dir.join(".profile")]
        } else {
            existing
        }
    }
}

impl UserPath for ShellProfilePath {
    /// Hängt die Zeile an jedes passende Profil an, das den Ordner noch nicht kennt
    fn add(&self, dir: &Path) -> Result<bool> {
        let dir = dir.to_string_lossy().into_owned();
        let mut changed = false;
        for file in self.targets() {
            let text = fs::read_to_string(&file).unwrap_or_default();
            if text.contains(&dir) {
                continue;
            }
            let prefix = if text.is_empty() || text.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            let mut handle = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file)
                .map_err(Error::unexpected)?;
            handle
                .write_all(format!("{prefix}\n{MARKER}\nexport PATH=\"{dir}:$PATH\"\n").as_bytes())
                .map_err(Error::unexpected)?;
            changed = true;
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Temporärer Benutzerordner
    fn home() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("harness-profile-")
            .tempdir()
            .unwrap()
    }

    #[test]
    fn erganzt_vorhandene_profile_genau_einmal() {
        let home = home();
        fs::write(home.path().join(".bashrc"), "alias ll=\"ls -l\"").unwrap();
        let path = ShellProfilePath::at(home.path());

        assert!(path.add(Path::new("/home/x/.harness/bin")).unwrap());
        assert!(!path.add(Path::new("/home/x/.harness/bin")).unwrap());

        let text = fs::read_to_string(home.path().join(".bashrc")).unwrap();
        assert!(text.contains("alias ll=\"ls -l\"\n"), "{text}");
        assert_eq!(text.matches(".harness/bin").count(), 1, "{text}");
        assert!(!home.path().join(".profile").exists());
    }

    #[test]
    fn legt_profile_an_wenn_es_kein_profil_gibt() {
        let home = home();
        assert!(ShellProfilePath::at(home.path())
            .add(Path::new("/h/bin"))
            .unwrap());
        let text = fs::read_to_string(home.path().join(".profile")).unwrap();
        assert!(text.contains("export PATH=\"/h/bin:$PATH\""), "{text}");
        assert!(text.contains(MARKER), "{text}");
    }
}
