//! Arbeitsordner der Claude-Analyse (`~/.nexus/dispatch/analysis`); portiert aus
//! `packages/core/src/dispatch/optimization/AnalysisWorkspace.ts`.
//!
//! Der Ordner ist absichtlich leer, damit Claude dort kein Projekt vorfindet.

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::registry::HarnessHome;

/// Arbeitsordner der Claude-Analyse.
#[derive(Debug, Clone)]
pub struct AnalysisWorkspace {
    home: HarnessHome,
}

impl AnalysisWorkspace {
    /// Arbeitsordner im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self { home }
    }

    /// Legt den Ordner an, falls er fehlt, und liefert seinen Pfad
    pub fn dir(&self) -> Result<PathBuf> {
        let dir = self.home.dispatch_analysis_dir();
        std::fs::create_dir_all(&dir).map_err(Error::unexpected)?;
        Ok(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legt_den_leeren_arbeitsordner_an() {
        let dir = tempfile::Builder::new()
            .prefix("harness-analysis-")
            .tempdir()
            .unwrap();
        let workspace = AnalysisWorkspace::new(HarnessHome::at(dir.path()));
        let ordner = workspace.dir().unwrap();
        assert_eq!(ordner, dir.path().join("dispatch").join("analysis"));
        assert!(ordner.is_dir());
        assert_eq!(std::fs::read_dir(&ordner).unwrap().count(), 0);
    }
}
