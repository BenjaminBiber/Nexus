//! Das Modell einer Meldung; portiert aus
//! `packages/server/src/notifications/ToastModel.ts`.

use std::path::PathBuf;

pub use crate::notices::ToastKind;

/// Alles, was die Meldung zeigt; Texte sind fertig formuliert, Adressen vollständig.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastModel {
    pub kind: ToastKind,
    pub title: String,
    pub line1: String,
    pub line2: String,
    /// Adresse, die ein Klick oder „Board öffnen“ aufruft
    pub open_url: String,
}

/// Bildpfade und Voreinstellungen für die Windows-Meldung
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastOptions {
    pub assets_dir: PathBuf,
    pub hero_images: bool,
}
