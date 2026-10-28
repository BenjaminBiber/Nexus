//! Trägt einen Ordner dauerhaft in den PATH des Benutzers ein (nur Benutzerebene, nie
//! systemweit). Portiert aus `packages/core/src/install/UserPath.ts`.

use std::path::Path;

use crate::error::Result;

/// Eintrag im PATH des Benutzers
pub trait UserPath {
    /// Liefert true, wenn der Eintrag neu hinzugefügt wurde, false, wenn er schon vorhanden war
    fn add(&self, dir: &Path) -> Result<bool>;
}
