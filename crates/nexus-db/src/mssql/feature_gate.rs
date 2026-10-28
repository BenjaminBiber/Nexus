//! Sperre für Zugriffe, die eine noch nicht eingespielte Erweiterung brauchen; portiert aus
//! `packages/db/src/MssqlFeatureGate.ts`.

use nexus_core::board::BoardProvider;
use nexus_core::{HarnessError, Result};

use crate::schema_version_guard::SchemaVersionGuard;

/// Sperrt genau die Zugriffe, die eine noch nicht eingespielte Erweiterung brauchen. Alles
/// andere am Board läuft weiter, damit eine laufende Agenten-Session ihren Meldeweg behält.
#[derive(Debug, Clone, Default)]
pub struct MssqlFeatureGate {
    missing_features: Vec<String>,
    database: String,
}

impl MssqlFeatureGate {
    /// Gate über den Erweiterungen, die der Datenbank fehlen
    pub fn new(missing_features: Vec<String>, database: impl Into<String>) -> Self {
        Self {
            missing_features,
            database: database.into(),
        }
    }

    /// Meldet `board-schema-outdated` mit Hinweis auf das Skript, sobald eine der verlangten
    /// Erweiterungen fehlt. Geprüft wird in der Reihenfolge der Angabe – die Aufrufer nennen
    /// sie in der Reihenfolge der Skripte.
    pub fn require(&self, features: &[&str]) -> Result<()> {
        let Some(fehlend) = features
            .iter()
            .find(|feature| self.missing_features.iter().any(|fehlt| fehlt == *feature))
        else {
            return Ok(());
        };
        Err(HarnessError::new(
            "board-schema-outdated",
            SchemaVersionGuard::feature_message(fehlend, &self.database, BoardProvider::Mssql),
        )
        .into())
    }

    /// Ob die Erweiterung da ist (für Zugriffe, die die Quelle lieber auslassen als zu scheitern)
    pub fn has(&self, feature: &str) -> bool {
        !self.missing_features.iter().any(|fehlt| fehlt == feature)
    }

    /// Die Datenbank, auf die sich die Meldungen beziehen
    pub fn database(&self) -> &str {
        &self.database
    }
}
