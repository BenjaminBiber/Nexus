//! Benutzerweiter Ordner von Nexus (`~/.nexus`, überschreibbar mit `NEXUS_HOME`). Während der
//! Übergangszeit zählen `HARNESS_HOME` und `~/.harness` noch als Rückfall; die Reihenfolge steht
//! genau so in `packages/core/src/registry/HarnessHome.ts`, damit Node-Server und Rust-CLI
//! denselben Ordner nehmen. Alle Pfade darin stehen an dieser einen Stelle.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Woher der Ordner kommt; `migrate` und die Hinweise der CLI entscheiden daran
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeSource {
    /// `NEXUS_HOME` war gesetzt
    EnvNexus,
    /// `HARNESS_HOME` war gesetzt (veraltet)
    EnvHarness,
    /// `~/.nexus` ist vorhanden
    Nexus,
    /// Nur `~/.harness` ist vorhanden: Rückfall, solange nicht migriert
    HarnessFallback,
    /// Weder noch: Neuinstallation in `~/.nexus`
    New,
}

impl HomeSource {
    /// Kennung für Ausgaben und JSON
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnvNexus => "env-nexus",
            Self::EnvHarness => "env-harness",
            Self::Nexus => "nexus",
            Self::HarnessFallback => "harness-fallback",
            Self::New => "new",
        }
    }
}

/// Name des Ordners von Nexus im Benutzerordner
pub const NEXUS_DIR: &str = ".nexus";

/// Name des alten Ordners des Harness im Benutzerordner (Rückfall der Übergangszeit)
pub const HARNESS_DIR: &str = ".harness";

/// Benutzerweiter Ordner von Nexus und alle Dateien darin
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusHome {
    dir: PathBuf,
    source: HomeSource,
}

/// Alter Name des Typs; die Aufrufer werden schrittweise umgestellt
pub type HarnessHome = NexusHome;

impl NexusHome {
    /// Ordner aus einer Umgebung (siehe [`NexusHome::from_env`])
    pub fn new(env: &HashMap<String, String>) -> Self {
        Self::from_env(|name| env.get(name).cloned())
    }

    /// Ordner aus der Umgebung des laufenden Prozesses
    pub fn from_process_env() -> Self {
        Self::from_env(|name| std::env::var(name).ok())
    }

    /// Ordner aus einer frei wählbaren Umgebung. Reihenfolge: `NEXUS_HOME`, `HARNESS_HOME`
    /// (veraltet), vorhandenes `~/.nexus`, vorhandenes `~/.harness` (Rückfall), sonst `~/.nexus`.
    /// Ein leerer Wert zählt wie keiner.
    pub fn from_env(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let value = |name| lookup(name).filter(|value: &String| !value.is_empty());
        if let Some(dir) = value("NEXUS_HOME") {
            return Self::with_source(dir, HomeSource::EnvNexus);
        }
        if let Some(dir) = value("HARNESS_HOME") {
            return Self::with_source(dir, HomeSource::EnvHarness);
        }
        Self::in_home_dir(&home_dir())
    }

    /// Ordner unter einem Benutzerordner: `.nexus`, sonst `.harness` als Rückfall, sonst
    /// `.nexus` als Neuinstallation
    pub fn in_home_dir(base: &Path) -> Self {
        let nexus = base.join(NEXUS_DIR);
        if nexus.is_dir() {
            return Self::with_source(nexus, HomeSource::Nexus);
        }
        let harness = base.join(HARNESS_DIR);
        if harness.is_dir() {
            return Self::with_source(harness, HomeSource::HarnessFallback);
        }
        Self::with_source(nexus, HomeSource::New)
    }

    /// Ordner an einem bekannten Pfad (Tests und Aufrufer, die ihn schon kennen)
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self::with_source(dir, HomeSource::Nexus)
    }

    /// Ordner an einem bekannten Pfad mit bekannter Herkunft
    pub fn with_source(dir: impl Into<PathBuf>, source: HomeSource) -> Self {
        Self {
            dir: dir.into(),
            source,
        }
    }

    /// Der Ordner selbst
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Woher der Ordner kommt
    pub fn source(&self) -> HomeSource {
        self.source
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

    /// Claude-Accounts des Dispatchers
    pub fn accounts_file(&self) -> PathBuf {
        self.dir.join("accounts.json")
    }

    /// Langzeit-Tokens der Accounts für die Läufe (`claude setup-token`); nur für den Benutzer
    pub fn account_tokens_file(&self) -> PathBuf {
        self.dir.join("account-tokens.json")
    }

    /// Konfigurationsordner neu angelegter Accounts (`<id>/`)
    pub fn accounts_dir(&self) -> PathBuf {
        self.dir.join("accounts")
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

    /// Angewendete Anpassungen (Workflow, Übersicht), geschrieben von `nexus install`
    pub fn customizations_dir(&self) -> PathBuf {
        self.dir.join("customizations")
    }

    /// Lokaler Marktplatz mit der Kopie des mitgelieferten Plugins (`plugin/nexus`); überlebt
    /// Updates der App, deren Ressourcenordner das nicht tut
    pub fn plugin_dir(&self) -> PathBuf {
        self.dir.join("plugin")
    }
}

/// Quelle eines Umzugs: `HARNESS_HOME`, sonst `~/.harness` – unabhängig davon, ob es den
/// Ordner gibt
pub fn harness_dir(lookup: impl Fn(&str) -> Option<String>) -> PathBuf {
    match lookup("HARNESS_HOME").filter(|value| !value.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => home_dir().join(HARNESS_DIR),
    }
}

/// Ziel eines Umzugs: `NEXUS_HOME`, sonst `~/.nexus` – unabhängig davon, ob es den Ordner gibt
pub fn nexus_dir(lookup: impl Fn(&str) -> Option<String>) -> PathBuf {
    match lookup("NEXUS_HOME").filter(|value| !value.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => home_dir().join(NEXUS_DIR),
    }
}

/// Die Umgebung dieses Prozesses als Suche für [`harness_dir`] und [`nexus_dir`]
pub fn process_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Benutzerordner des angemeldeten Benutzers, wie `os.homedir()` in Node
fn home_dir() -> PathBuf {
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Umgebung mit frei wählbaren Variablen
    fn env(paare: &[(&str, &str)]) -> HashMap<String, String> {
        paare
            .iter()
            .map(|(name, wert)| (name.to_string(), wert.to_string()))
            .collect()
    }

    #[test]
    fn nexus_home_hat_vorrang_vor_harness_home() {
        let home = NexusHome::new(&env(&[("NEXUS_HOME", "/x/n"), ("HARNESS_HOME", "/x/h")]));
        assert_eq!(home.dir(), Path::new("/x/n"));
        assert_eq!(home.source(), HomeSource::EnvNexus);
    }

    #[test]
    fn ohne_nexus_home_gilt_harness_home_als_veralteter_rueckfall() {
        let home = NexusHome::new(&env(&[("HARNESS_HOME", "/x/h")]));
        assert_eq!(home.dir(), Path::new("/x/h"));
        assert_eq!(home.source(), HomeSource::EnvHarness);
    }

    #[test]
    fn leere_werte_zahlen_wie_keine() {
        let home = NexusHome::new(&env(&[("NEXUS_HOME", ""), ("HARNESS_HOME", "/x/h")]));
        assert_eq!(home.dir(), Path::new("/x/h"));

        let leer = NexusHome::new(&env(&[("NEXUS_HOME", ""), ("HARNESS_HOME", "")]));
        assert_eq!(leer.source(), NexusHome::in_home_dir(&home_dir()).source());
    }

    #[test]
    fn ohne_variablen_gilt_punkt_nexus_im_benutzerordner() {
        let base = tempfile::tempdir().unwrap();
        fs::create_dir(base.path().join(".nexus")).unwrap();
        fs::create_dir(base.path().join(".harness")).unwrap();

        let home = NexusHome::in_home_dir(base.path());

        assert_eq!(home.dir(), base.path().join(".nexus"));
        assert_eq!(home.source(), HomeSource::Nexus);
    }

    #[test]
    fn ohne_punkt_nexus_fallt_es_auf_punkt_harness_zurueck() {
        let base = tempfile::tempdir().unwrap();
        fs::create_dir(base.path().join(".harness")).unwrap();

        let home = NexusHome::in_home_dir(base.path());

        assert_eq!(home.dir(), base.path().join(".harness"));
        assert_eq!(home.source(), HomeSource::HarnessFallback);
    }

    #[test]
    fn ohne_beide_ordner_ist_es_eine_neuinstallation_in_punkt_nexus() {
        let base = tempfile::tempdir().unwrap();

        let home = NexusHome::in_home_dir(base.path());

        assert_eq!(home.dir(), base.path().join(".nexus"));
        assert_eq!(home.source(), HomeSource::New);
    }

    #[test]
    fn quelle_und_ziel_eines_umzugs_kommen_aus_den_variablen() {
        let env = env(&[("NEXUS_HOME", "/x/n"), ("HARNESS_HOME", "/x/h")]);
        let lookup = |name: &str| env.get(name).cloned();
        assert_eq!(harness_dir(lookup), Path::new("/x/h"));
        assert_eq!(nexus_dir(lookup), Path::new("/x/n"));
    }

    #[test]
    fn ohne_variablen_liegen_quelle_und_ziel_im_benutzerordner() {
        let leer = |_: &str| None;
        assert_eq!(harness_dir(leer), home_dir().join(HARNESS_DIR));
        assert_eq!(nexus_dir(leer), home_dir().join(NEXUS_DIR));
    }

    #[test]
    fn die_herkunft_hat_eine_kennung_fuer_ausgaben() {
        assert_eq!(HomeSource::EnvNexus.as_str(), "env-nexus");
        assert_eq!(HomeSource::EnvHarness.as_str(), "env-harness");
        assert_eq!(HomeSource::Nexus.as_str(), "nexus");
        assert_eq!(HomeSource::HarnessFallback.as_str(), "harness-fallback");
        assert_eq!(HomeSource::New.as_str(), "new");
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
        assert_eq!(home.accounts_file(), Path::new("h").join("accounts.json"));
        assert_eq!(home.accounts_dir(), Path::new("h").join("accounts"));
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
