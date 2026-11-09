//! Theme-Pakete (`.nexus`): das Manifest eines Themes und seine Prüfung. Entpacken und
//! Installieren folgen später; hier nur Typ und Regeln.

pub mod manifest;

pub use manifest::{
    parse_manifest, validate_manifest, ColorScheme, ManifestError, ManifestErrors, ThemeManifest,
    ThemePermissions,
};
