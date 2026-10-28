//! Portiert `packages/core/src/git/PathKey.ts`.

use std::path::{Component, Path, PathBuf};

/// Vergleichbare Schreibweise eines Ordners: Pfade aus Git, aus der Registry und vom
/// Dispatcher können denselben Ordner unterschiedlich nennen (unter Windows 8.3-Kurzname
/// wie FLORIA~1.PAL, Groß-/Kleinschreibung, Rückstriche).
#[derive(Debug, Clone, Copy)]
pub struct PathKey {
    windows: bool,
}

impl Default for PathKey {
    fn default() -> Self {
        Self::new()
    }
}

impl PathKey {
    /// Schlüssel für die laufende Plattform
    pub fn new() -> Self {
        Self {
            windows: cfg!(windows),
        }
    }

    /// Schlüssel für eine ausdrücklich genannte Plattform (für Tests)
    pub fn for_platform(windows: bool) -> Self {
        Self { windows }
    }

    /// Schlüssel: aufgelöst, bei vorhandenem Ordner kanonisch (lange Namen),
    /// Schrägstriche, ohne Endstrich, unter Windows klein
    pub fn of(&self, path: impl AsRef<Path>) -> String {
        let mut full = absolute(path.as_ref());
        // Ordner gibt es (noch) nicht: aufgelöster Pfad genügt
        if let Ok(real) = std::fs::canonicalize(&full) {
            full = strip_verbatim(real);
        }
        let slashed = full.to_string_lossy().replace('\\', "/");
        let trimmed = slashed.trim_end_matches('/');
        // Wurzel bleibt Wurzel: „/“ darf nicht zum leeren Schlüssel werden
        let key = if trimmed.is_empty() {
            &slashed
        } else {
            trimmed
        };
        if self.windows {
            key.to_lowercase()
        } else {
            key.to_string()
        }
    }

    /// Gleicher Ordner?
    pub fn same(&self, a: impl AsRef<Path>, b: impl AsRef<Path>) -> bool {
        self.of(a) == self.of(b)
    }
}

/// Absoluter Pfad mit aufgelösten „.“ und „..“, wie `path.resolve` in Node (nur
/// rechnerisch, ohne das Dateisystem zu fragen)
fn absolute(path: &Path) -> PathBuf {
    let base = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().unwrap_or_default()
    };
    let mut result = base;
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

/// Entfernt die Windows-Schreibweise `\\?\`, die `canonicalize` voranstellt
fn strip_verbatim(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy().into_owned();
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setzt_kurznamen_ruckstriche_und_endstrich_mit_der_kanonischen_schreibweise_gleich() {
        let keys = PathKey::new();
        let temp = std::env::temp_dir();
        // Der Temp-Ordner heißt unter Windows oft mit 8.3-Kurznamen, unter macOS zeigt
        // /var auf /private/var; canonicalize liefert den langen, aufgelösten Namen
        let real = std::fs::canonicalize(&temp).unwrap();

        assert!(keys.same(&temp, &real));
        assert!(keys.same(
            format!("{}/", temp.to_string_lossy()),
            temp.to_string_lossy().replace('\\', "/")
        ));
    }

    #[test]
    fn ignoriert_gross_kleinschreibung_nur_unter_windows() {
        assert!(PathKey::for_platform(true).same("/nicht-da/Ordner", "/NICHT-DA/ordner"));
        assert!(!PathKey::for_platform(false).same("/nicht-da/Ordner", "/NICHT-DA/ordner"));
    }

    #[test]
    fn lost_nicht_vorhandene_pfade_nur_auf() {
        let temp = std::env::temp_dir();
        let missing = temp.join("gibt-es-nicht").join("..").join("auch-nicht");

        assert_eq!(
            PathKey::for_platform(false).of(missing),
            temp.join("auch-nicht")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_string()
        );
    }
}
