//! Die Umsetzungen der Systemfunktionen für Windows.
//!
//! Die Module stehen auf jedem System zur Verfügung, damit ihre reinen Teile (Kodierung des
//! Skripts, Aufrufargumente, Fehlermeldungen) überall geprüft werden können; nur die Typen,
//! die wirklich einen Windows-Prozess starten, hängen an `#[cfg(windows)]`.

pub mod file_opener;
pub mod folder_picker;
