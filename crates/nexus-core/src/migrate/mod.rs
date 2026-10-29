//! Umzug der Daten vom alten Ordner `~/.harness` in den neuen Ordner `~/.nexus`. Der alte
//! Ordner bleibt dabei unverändert liegen, damit die alte Fassung weiter läuft.

pub mod migrator;

pub use migrator::{
    MigrateDeps, MigrateReport, MigrateStatus, Migrator, MIGRATED_FROM_FILE, SKIP_DIRS, SKIP_FILES,
};
