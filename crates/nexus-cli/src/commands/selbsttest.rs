//! Selbsttest des Gerüsts: wirft einen erwartbaren Fehler, damit sich Rückgabecode 1 und das
//! JSON-Format `{"error":{"code","message"}}` von außen prüfen lassen. Nur in Debug-Builds.

use nexus_core::{HarnessError, Result};

use crate::context::CliContext;

/// Wirft einen `HarnessError` mit dem angegebenen Code und der angegebenen Meldung
pub fn fehler(_context: &CliContext, code: &str, message: &str) -> Result<u8> {
    Err(HarnessError::new(code, message).into())
}
