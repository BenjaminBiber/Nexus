//! Benutzerweiter Ordner des Harness (`~/.harness`, überschreibbar mit `HARNESS_HOME`).
//! Alle Pfade darin stehen an dieser einen Stelle, genau wie in
//! `packages/core/src/registry/HarnessHome.ts`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Benutzerweiter Ordner des Harness und alle Dateien darin
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessHome {
    dir: PathBuf,
}

impl HarnessHome {
    /// Ordner aus einer Umgebung: `HARNESS_HOME`, sonst `.harness` im Benutzerordner
    pub fn new(env: &HashMap<String, String>) -> Self {
        Self::from_env(|name| env.get(name).cloned())
    }

    /// Ordner aus der Umgebung des laufenden Prozesses
    pub fn from_process_env() -> Self {
        Self::from_env(|name| std::env::var(name).ok())
    }

    /// Ordner aus einer frei wählbaren Umgebung; ein leerer `HARNESS_HOME` zählt wie keiner
    pub fn from_env(lookup: impl Fn(&str) -> Option<String>) -> Self {
        match lookup("HARNESS_HOME") {
            Some(value) if !value.is_empty() => Self::at(value),
            _ => Self::at(home_dir().join(".harness")),
        }
    }

    /// Ordner an einem bekannten Pfad (Tests und Aufrufer, die ihn schon kennen)
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Der Ordner selbst
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Liste der angemeldeten Projekte
    pub fn projects_file(&self) -> PathBuf {
        self.dir.join("projects.json")
    }

    /// Sperre der Projektliste
    pub fn projects_lock(&self) -> PathBuf {
        self.dir.join("projects.lock")
    }

    /// Prozessangaben des laufenden Servers
    pub fn server_file(&self) -> PathBuf {
        self.dir.join("server.json")
    }

    /// Protokoll des Servers
    pub fn log_file(&self) -> PathBuf {
        self.dir.join("server.log")
    }

    /// Bekannte Claude-Sessions
    pub fn sessions_file(&self) -> PathBuf {
        self.dir.join("sessions.json")
    }

    /// Favoriten des Benutzers
    pub fn favorites_file(&self) -> PathBuf {
        self.dir.join("favorites.json")
    }

    /// Benutzerweite Einstellungen
    pub fn settings_file(&self) -> PathBuf {
        self.dir.join("settings.json")
    }

    /// Sicherungen
    pub fn backups_dir(&self) -> PathBuf {
        self.dir.join("backups")
    }

    /// Verbindung zur Board-Datenbank
    pub fn board_config_file(&self) -> PathBuf {
        self.dir.join("board.yaml")
    }

    /// Standardpfad der SQLite-Datei des Boards (Anbieter `sqlite` ohne eigenes `file`)
    pub fn board_db_file(&self) -> PathBuf {
        self.dir.join("board.db")
    }

    /// Konfiguration des Dispatchers
    pub fn dispatch_config_file(&self) -> PathBuf {
        self.dir.join("dispatch.yaml")
    }

    /// Ausblendungen und letzte Claude-Analyse der Optimierungs-Vorschläge
    pub fn dispatch_suggestions_file(&self) -> PathBuf {
        self.dir.join("dispatch-suggestions.json")
    }

    /// Eigene Repo-Pfade der Boards (je Benutzer lokal, nicht in der gemeinsamen DB)
    pub fn board_repos_file(&self) -> PathBuf {
        self.dir.join("board-repos.yaml")
    }

    /// Eigene Reihenfolge von „Meine Aufgaben“ (je Benutzer lokal)
    pub fn inbox_order_file(&self) -> PathBuf {
        self.dir.join("inbox-order.json")
    }

    /// Ordner des Dispatchers
    pub fn dispatch_dir(&self) -> PathBuf {
        self.dir.join("dispatch")
    }

    /// Läufe des Dispatchers
    pub fn dispatch_runs_dir(&self) -> PathBuf {
        self.dispatch_dir().join("runs")
    }

    /// Zustand des Dispatchers
    pub fn dispatch_state_file(&self) -> PathBuf {
        self.dispatch_dir().join("state.json")
    }

    /// Review-Einträge vor dem Merge (`<Ticket-ID>.json`)
    pub fn dispatch_reviews_dir(&self) -> PathBuf {
        self.dispatch_dir().join("reviews")
    }

    /// Leerer Arbeitsordner der Claude-Analyse (Optimierungs-Vorschläge)
    pub fn dispatch_analysis_dir(&self) -> PathBuf {
        self.dispatch_dir().join("analysis")
    }

    /// Protokolle der Dispatcher-Läufe
    pub fn dispatch_logs_dir(&self) -> PathBuf {
        self.dir.join("dispatch-logs")
    }

    /// Zentrale Regeln, die die Projekte über ihre Rule importieren (z. B. `docs.md`)
    pub fn rules_dir(&self) -> PathBuf {
        self.dir.join("rules")
    }

    /// Angewendete Anpassungen (Workflow, Übersicht), geschrieben von `harness install`
    pub fn customizations_dir(&self) -> PathBuf {
        self.dir.join("customizations")
    }
}

/// Benutzerordner des angemeldeten Benutzers, wie `os.homedir()` in Node
fn home_dir() -> PathBuf {
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Umgebung mit gesetztem `HARNESS_HOME`
    fn env(value: &str) -> HashMap<String, String> {
        HashMap::from([("HARNESS_HOME".to_string(), value.to_string())])
    }

    #[test]
    fn nutzt_harness_home_sonst_punkt_harness_im_benutzerordner() {
        let gesetzt = HarnessHome::new(&env("/x/y"));
        assert_eq!(gesetzt.dir(), Path::new("/x/y"));

        let ohne = HarnessHome::new(&HashMap::new());
        assert!(ohne.dir().ends_with(".harness"), "{:?}", ohne.dir());
    }

    #[test]
    fn leeres_harness_home_zahlt_wie_keines() {
        let leer = HarnessHome::new(&env(""));
        assert!(leer.dir().ends_with(".harness"), "{:?}", leer.dir());
    }

    #[test]
    fn nennt_den_ordner_der_angewendeten_anpassungen() {
        let home = HarnessHome::at(Path::new("x").join("h"));
        assert_eq!(
            home.customizations_dir(),
            Path::new("x").join("h").join("customizations")
        );
    }

    #[test]
    fn nennt_die_standarddatei_der_board_datenbank() {
        let home = HarnessHome::at(Path::new("x").join("h"));
        assert_eq!(
            home.board_db_file(),
            Path::new("x").join("h").join("board.db")
        );
    }

    #[test]
    fn kennt_alle_dateien_und_ordner_des_benutzerordners() {
        let home = HarnessHome::at("h");
        assert_eq!(home.projects_file(), Path::new("h").join("projects.json"));
        assert_eq!(home.projects_lock(), Path::new("h").join("projects.lock"));
        assert_eq!(home.server_file(), Path::new("h").join("server.json"));
        assert_eq!(home.log_file(), Path::new("h").join("server.log"));
        assert_eq!(home.sessions_file(), Path::new("h").join("sessions.json"));
        assert_eq!(home.favorites_file(), Path::new("h").join("favorites.json"));
        assert_eq!(home.settings_file(), Path::new("h").join("settings.json"));
        assert_eq!(home.backups_dir(), Path::new("h").join("backups"));
        assert_eq!(home.board_config_file(), Path::new("h").join("board.yaml"));
        assert_eq!(
            home.dispatch_config_file(),
            Path::new("h").join("dispatch.yaml")
        );
        assert_eq!(
            home.dispatch_suggestions_file(),
            Path::new("h").join("dispatch-suggestions.json")
        );
        assert_eq!(
            home.board_repos_file(),
            Path::new("h").join("board-repos.yaml")
        );
        assert_eq!(
            home.inbox_order_file(),
            Path::new("h").join("inbox-order.json")
        );
        assert_eq!(home.dispatch_dir(), Path::new("h").join("dispatch"));
        assert_eq!(
            home.dispatch_runs_dir(),
            Path::new("h").join("dispatch").join("runs")
        );
        assert_eq!(
            home.dispatch_state_file(),
            Path::new("h").join("dispatch").join("state.json")
        );
        assert_eq!(
            home.dispatch_reviews_dir(),
            Path::new("h").join("dispatch").join("reviews")
        );
        assert_eq!(
            home.dispatch_analysis_dir(),
            Path::new("h").join("dispatch").join("analysis")
        );
        assert_eq!(
            home.dispatch_logs_dir(),
            Path::new("h").join("dispatch-logs")
        );
        assert_eq!(home.rules_dir(), Path::new("h").join("rules"));
    }
}
