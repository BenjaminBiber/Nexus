//! Theme-Pakete (`.nexus`): das Manifest eines Themes und seine Prüfung, dazu sicheres
//! Entpacken, Installieren, Auflisten und Entfernen unter `<NEXUS_HOME>/themes/<id>/`.

pub mod manifest;
pub mod package;

pub use manifest::{
    host_allowed, is_valid_theme_id, parse_manifest, validate_manifest, ColorScheme, ManifestError,
    ManifestErrors, ThemeManifest, ThemePermissions,
};
pub use package::{
    install_theme, install_theme_file, list_themes, remove_theme, InstalledTheme, ThemePackageError,
};
