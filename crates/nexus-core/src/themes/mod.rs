//! Theme-Pakete (`.nexus`): das Manifest eines Themes und seine Prüfung, dazu sicheres
//! Entpacken, Installieren, Auflisten und Entfernen unter `<NEXUS_HOME>/themes/<id>/`.

pub mod archive;
pub mod manifest;
pub mod package;

pub use archive::{
    check_archive_entry_path, content_type_for, extract_entry, ArchiveEntryError, MAX_ENTRY_PATH,
};
pub use manifest::{
    host_allowed, is_valid_theme_id, parse_manifest, validate_manifest, ColorScheme, ManifestError,
    ManifestErrors, ThemeManifest, ThemePermissions,
};
pub use package::{
    install_theme, install_theme_file, list_themes, pack_theme, remove_theme, InstalledTheme,
    PackedTheme, ThemePackageError,
};
