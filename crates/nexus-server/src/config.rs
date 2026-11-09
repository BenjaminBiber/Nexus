//! Einstellungen des Servers und ihr Lader; portiert aus
//! `packages/server/src/config/ServerConfig.ts` und `ServerConfigLoader.ts`.

use std::collections::HashMap;
use std::path::PathBuf;

use nexus_core::clock::Clock;
use nexus_core::registry::{AppToken, DEFAULT_SERVER_PORT};
use nexus_core::{HarnessError, HarnessHome, Result};

/// Host, auf dem der Server hört – immer nur die eigene Maschine
pub const HOST: &str = "127.0.0.1";

/// Abstand der Herzschlag-Meldungen im Ereignisstrom
pub const DEFAULT_HEARTBEAT_MS: u64 = 15_000;
/// Abstand der Abfrage des Änderungskennwerts der Board-Datenbank
pub const DEFAULT_DB_POLL_MS: u64 = 2_500;
/// Takt des Dokument-Watchers
pub const DEFAULT_DOCS_POLL_MS: u64 = 10_000;
/// Abstand der Abfrage neuer Fragen für Systemmeldungen (0 = aus)
pub const DEFAULT_DB_QUESTION_POLL_MS: u64 = 10_000;
/// Takt des Dispatchers (0 = aus)
pub const DEFAULT_DISPATCH_TICK_MS: u64 = 60_000;

/// Einstellungen des Servers
#[derive(Clone)]
pub struct ServerConfig {
    /// Port, auf dem gehört wird (0 = freien Port wählen)
    pub port: u16,
    /// Host; immer [`HOST`]
    pub host: String,
    /// Benutzerordner mit `server.json`, `server.log`, `board.yaml` …
    pub home: HarnessHome,
    /// Ordner der gebauten Oberfläche; `None`, wenn es keinen gibt
    pub web_dir: Option<PathBuf>,
    /// Ordner mit den Bildern der Systemmeldung (`logo-app.png` …)
    pub toast_dir: PathBuf,
    /// Version in `GET /api/health`
    pub version: String,
    /// Abstand der Herzschlag-Meldungen in Millisekunden
    pub heartbeat_ms: u64,
    /// Abstand der Abfrage des Änderungskennwerts in Millisekunden
    pub db_poll_ms: u64,
    /// Takt des Dokument-Watchers in Millisekunden
    pub docs_poll_ms: u64,
    /// Abstand der Abfrage neuer Fragen in Millisekunden (0 = aus)
    pub db_question_poll_ms: u64,
    /// Takt des Dispatchers in Millisekunden (0 = aus)
    pub dispatch_tick_ms: u64,
    /// Start der Worker eines Laufs; ohne Angabe [`crate::dispatch::ProcessWorkerLauncher`]
    /// (Tests setzen hier Attrappen ein)
    pub dispatch_launcher: Option<std::sync::Arc<dyn crate::dispatch::WorkerLauncher>>,
    /// Uhr des Servers; Tests setzen eine feste, damit Zeiträume nachrechenbar sind
    pub clock: Clock,
    /// Start-Token dieses Laufs: damit weist sich die Oberfläche der Desktop-App aus. Es
    /// entsteht bei jedem Start neu und landet in `server.json`.
    pub app_token: AppToken,
    /// Herkunft der Oberfläche im Entwicklungsmodus der Desktop-App (Vite, z. B.
    /// `http://localhost:5173`); gilt wie die der App nur mit Start-Token
    pub dev_origin: Option<String>,
    /// Mit der Desktop-App geliefertes Claude-Code-Plugin (`<Ressourcen>/plugin/nexus`); bei
    /// `nexus serve` `None`
    pub plugin_bundle: Option<PathBuf>,
    /// Umgebung der Einrichtung (Claude-Kommandozeile, Claude-Ordner); Tests setzen Attrappen
    pub setup: crate::setup::SetupContext,
    /// Anmeldung, Plugin und Limits der Claude-Accounts (Tests: ohne echten Schlüsselbund und
    /// ohne Anthropic)
    pub account_usage: Option<std::sync::Arc<crate::accounts::AccountUsageService>>,
    /// Startet `claude auth login` für die Anmeldung eines Accounts (Tests: Attrappe)
    pub account_login_spawner: Option<crate::accounts::LoginSpawner>,
    /// Richtet das Nexus-Plugin in Account-Ordnern ein (Tests: ohne echtes `claude plugin`)
    pub account_plugins: Option<std::sync::Arc<crate::accounts::AccountPluginInstaller>>,
    /// Claude-Accounts des Dispatchers
    pub dispatch_accounts: DispatchAccounts,
    /// Ordner der MCACC-Profile für die Vorschläge in `GET /api/accounts`; ohne Angabe
    /// `~/.mcacc/profiles`
    pub mcacc_dir: Option<PathBuf>,
    /// Zugriff auf die Bot-API von Telegram; ohne Angabe über reqwest (Tests setzen hier eine
    /// Attrappe ein, damit nie wirklich bei Telegram angefragt wird)
    pub telegram_api: Option<crate::telegram::TelegramApiFactory>,
    /// Der Dienst, der die Ideen des Bots versteht; ohne Angabe einer mit der Umgebung des
    /// Servers (Tests geben ihm die Umgebung ihres Claude-Ersatzes mit)
    pub idea_interpreter: Option<std::sync::Arc<crate::telegram::IdeaInterpreter>>,
    /// Größenlimit je Antwort des Asset-Proxys für Themes
    pub theme_fetch_max_bytes: u64,
}

/// Woher der Dispatcher seine Claude-Accounts nimmt
#[derive(Clone, Default)]
pub enum DispatchAccounts {
    /// Aus `~/.nexus/accounts.json` (Betrieb)
    #[default]
    Stored,
    /// Ohne Account-Wahl: jeder Lauf mit der Umgebung des Servers (Vertragsmodus, Tests)
    Off,
    /// Ein eigener Pool (Tests)
    Pool(std::sync::Arc<dyn crate::dispatch::AccountPool>),
}

impl std::fmt::Debug for DispatchAccounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Stored => "Stored",
            Self::Off => "Off",
            Self::Pool(_) => "Pool",
        };
        f.write_str(name)
    }
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("port", &self.port)
            .field("host", &self.host)
            .field("home", &self.home)
            .field("version", &self.version)
            .field("dispatch_accounts", &self.dispatch_accounts)
            .finish_non_exhaustive()
    }
}

/// Version, die den Vertragsmodus kennzeichnet (`NEXUS_SERVER_VERSION=contract`)
pub const CONTRACT_VERSION: &str = "contract";

impl ServerConfig {
    /// Läuft der Server im Vertragsmodus? Dort gibt es keinen Ordnerdialog, kein Öffnen von
    /// Dateien und keine Systemmeldungen
    pub fn is_contract(&self) -> bool {
        self.version == CONTRACT_VERSION
    }

    /// Einstellungen mit den Standardwerten für einen Benutzerordner (Tests und Aufrufer,
    /// die Port und Ordner schon kennen)
    pub fn new(port: u16, home: HarnessHome) -> Self {
        Self {
            port,
            host: HOST.to_string(),
            home,
            web_dir: None,
            toast_dir: default_toast_dir(),
            version: default_version(),
            heartbeat_ms: DEFAULT_HEARTBEAT_MS,
            db_poll_ms: DEFAULT_DB_POLL_MS,
            docs_poll_ms: DEFAULT_DOCS_POLL_MS,
            db_question_poll_ms: DEFAULT_DB_QUESTION_POLL_MS,
            dispatch_tick_ms: DEFAULT_DISPATCH_TICK_MS,
            dispatch_launcher: None,
            clock: Clock::system(),
            app_token: AppToken::new(),
            dev_origin: None,
            plugin_bundle: None,
            setup: crate::setup::SetupContext::from_process_env(),
            account_usage: None,
            account_login_spawner: None,
            account_plugins: None,
            dispatch_accounts: DispatchAccounts::default(),
            mcacc_dir: None,
            telegram_api: None,
            idea_interpreter: None,
            theme_fetch_max_bytes: crate::theme_proxy::DEFAULT_MAX_BYTES,
        }
    }
}

/// Version, die `GET /api/health` meldet: die des Workspace
fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Name des Bildes, an dem ein Bildordner zu erkennen ist
const TOAST_MARKER: &str = "logo-app.png";

/// Bilder der Systemmeldung: `toast/` neben dem Programm (gebündelte App), sonst der
/// Ordner im Crate (nur Entwicklung). `NEXUS_TOAST_DIR` geht beidem vor.
fn default_toast_dir() -> PathBuf {
    let next_to_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("toast")));
    let in_crate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/toast");
    next_to_exe
        .filter(|dir| dir.join(TOAST_MARKER).is_file())
        .unwrap_or(in_crate)
}

/// Liest die Einstellungen aus Argumenten und Umgebung.
///
/// Jeder Wert kommt zuerst aus dem Argument, dann aus der Nexus-Variable und zuletzt aus der
/// alten Harness-Variable – so startet der Rust-Server in einer Umgebung, die noch für den
/// Node-Server eingerichtet ist.
#[derive(Debug, Clone, Default)]
pub struct ServerConfigLoader;

impl ServerConfigLoader {
    /// Einstellungen aus den Argumenten (ohne Programmnamen) und der Prozessumgebung
    pub fn load_from_process(args: &[String]) -> Result<ServerConfig> {
        Self::load(args, &|name| std::env::var(name).ok())
    }

    /// Einstellungen aus Argumenten und einer frei wählbaren Umgebung (Tests)
    pub fn load(args: &[String], env: &dyn Fn(&str) -> Option<String>) -> Result<ServerConfig> {
        // `HarnessHome::from_env` kennt die Reihenfolge `NEXUS_HOME`, `HARNESS_HOME`, `~/.nexus`,
        // `~/.harness` selbst
        let home = HarnessHome::from_env(env);
        let web_dir = argument(args, "--web-dir")
            .or_else(|| value(env, &["NEXUS_WEB_DIR", "HARNESS_WEB_DIR"]))
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from);
        let toast_dir = value(env, &["NEXUS_TOAST_DIR", "HARNESS_TOAST_DIR"])
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from);
        let mut config = ServerConfig {
            port: port(args, env)?,
            web_dir,
            version: value(env, &["NEXUS_SERVER_VERSION", "HARNESS_BUNDLED_VERSION"])
                .filter(|version| !version.is_empty())
                .unwrap_or_else(default_version),
            // Der Vertragstest drückt die Takte, damit seine Szenarien in Sekunden fertig
            // sind – wie `contract/servers/ts-server.mjs` beim Node-Server
            heartbeat_ms: millis(env, "CONTRACT_HEARTBEAT_MS", DEFAULT_HEARTBEAT_MS),
            db_poll_ms: millis(env, "CONTRACT_POLL_MS", DEFAULT_DB_POLL_MS),
            docs_poll_ms: millis(env, "CONTRACT_POLL_MS", DEFAULT_DOCS_POLL_MS),
            // 0 schaltet den Takt ab; der Vertragstest drückt ihn, damit seine Szenarien in
            // Sekunden fertig sind
            dispatch_tick_ms: count(env, &["NEXUS_DISPATCH_TICK_MS"], DEFAULT_DISPATCH_TICK_MS),
            // 0 schaltet die Fragen-Beobachtung ab und ist deshalb ein erlaubter Wert
            db_question_poll_ms: count(
                env,
                &["NEXUS_DB_QUESTION_POLL_MS", "HARNESS_DB_QUESTION_POLL_MS"],
                DEFAULT_DB_QUESTION_POLL_MS,
            ),
            ..ServerConfig::new(0, home)
        };
        if let Some(dir) = toast_dir {
            config.toast_dir = dir;
        }
        // Im Vertragsmodus gibt es keine Meldungen über neue Fragen und keinen Takt des
        // Dispatchers von selbst (siehe contract/README.md)
        if config.is_contract() {
            config.db_question_poll_ms = 0;
            config.dispatch_tick_ms = 0;
        }
        Ok(config)
    }
}

/// Port aus `--port`, sonst `NEXUS_PORT`, sonst `HARNESS_PORT`, sonst dem Standard
fn port(args: &[String], env: &dyn Fn(&str) -> Option<String>) -> Result<u16> {
    let raw = match argument(args, "--port").or_else(|| value(env, &["NEXUS_PORT", "HARNESS_PORT"]))
    {
        Some(raw) => raw,
        None => return Ok(DEFAULT_SERVER_PORT),
    };
    raw.parse::<u16>().map_err(|_| {
        HarnessError::new(
            "port-invalid",
            format!("Der Port „{raw}“ ist ungültig. Erlaubt sind Zahlen von 0 bis 65535."),
        )
        .into()
    })
}

/// Takt in Millisekunden aus der Umgebung; ohne oder mit unbrauchbarem Wert der Standard
fn millis(env: &dyn Fn(&str) -> Option<String>, name: &str, default: u64) -> u64 {
    env(name)
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

/// Ganze Zahl ab 0 aus der ersten gesetzten Variable; ohne oder mit unbrauchbarem Wert der
/// Standard
fn count(env: &dyn Fn(&str) -> Option<String>, names: &[&str], default: u64) -> u64 {
    value(env, names)
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

/// Wert eines Schalters mit Wert (`--port 4710`)
fn argument(args: &[String], name: &str) -> Option<String> {
    let at = args.iter().position(|arg| arg == name)?;
    args.get(at + 1).cloned()
}

/// Erster gesetzter Wert der Umgebungsvariablen in der Reihenfolge ihrer Namen
fn value(env: &dyn Fn(&str) -> Option<String>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| env(name))
}

/// Umgebung aus einer Tabelle (Tests)
pub fn env_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lädt mit Argumenten und einer Tabelle als Umgebung
    fn load(args: &[&str], env: &[(&str, &str)]) -> Result<ServerConfig> {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
        let map = env_map(env);
        ServerConfigLoader::load(&args, &|name| map.get(name).cloned())
    }

    #[test]
    fn nimmt_den_port_aus_dem_argument_sonst_aus_der_umgebung_sonst_den_standard() {
        assert_eq!(
            load(&["--port", "5000"], &[("NEXUS_PORT", "6000")])
                .unwrap()
                .port,
            5000
        );
        assert_eq!(load(&[], &[("NEXUS_PORT", "6000")]).unwrap().port, 6000);
        assert_eq!(load(&[], &[("HARNESS_PORT", "6001")]).unwrap().port, 6001);
        assert_eq!(load(&[], &[]).unwrap().port, DEFAULT_SERVER_PORT);
        assert_eq!(load(&["--port", "0"], &[]).unwrap().port, 0);
    }

    #[test]
    fn meldet_einen_ungultigen_port_wortgleich_wie_heute() {
        // Ein leerer Wert ist ungültig; `Number('')` in JavaScript ergäbe 0, das ist ein
        // Zufall der Sprache und kein gewolltes Verhalten
        for raw in ["abc", "-1", "65536", "4710.5", ""] {
            let error = load(&["--port", raw], &[]).unwrap_err();
            let harness = error.harness().unwrap_or_else(|| panic!("{raw}"));
            assert_eq!(harness.code, "port-invalid", "{raw}");
            assert_eq!(
                harness.message,
                format!("Der Port „{raw}“ ist ungültig. Erlaubt sind Zahlen von 0 bis 65535.")
            );
        }
    }

    #[test]
    fn nimmt_benutzerordner_und_weboberflache_mit_ruckfall() {
        let config = load(
            &[],
            &[
                ("NEXUS_HOME", "/n/home"),
                ("HARNESS_HOME", "/h/home"),
                ("HARNESS_WEB_DIR", "/h/web"),
            ],
        )
        .unwrap();
        assert_eq!(config.home.dir(), std::path::Path::new("/n/home"));
        assert_eq!(
            config.web_dir.as_deref(),
            Some(std::path::Path::new("/h/web"))
        );
        assert_eq!(config.host, HOST);

        let nur_harness = load(&[], &[("HARNESS_HOME", "/h/home")]).unwrap();
        assert_eq!(nur_harness.home.dir(), std::path::Path::new("/h/home"));

        let argumente = load(&["--web-dir", "/a/web"], &[("NEXUS_WEB_DIR", "/n/web")]).unwrap();
        assert_eq!(
            argumente.web_dir.as_deref(),
            Some(std::path::Path::new("/a/web"))
        );

        let ohne = load(&[], &[]).unwrap();
        assert_eq!(ohne.web_dir, None);
    }

    #[test]
    fn nimmt_die_version_aus_der_umgebung_sonst_die_des_workspace() {
        assert_eq!(
            load(&[], &[("NEXUS_SERVER_VERSION", "contract")])
                .unwrap()
                .version,
            "contract"
        );
        assert_eq!(
            load(&[], &[("HARNESS_BUNDLED_VERSION", "9.9.9")])
                .unwrap()
                .version,
            "9.9.9"
        );
        assert_eq!(load(&[], &[]).unwrap().version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn nimmt_den_abstand_der_fragen_beobachtung_aus_der_umgebung_und_laesst_0_zu() {
        assert_eq!(
            load(&[], &[("HARNESS_DB_QUESTION_POLL_MS", "0")])
                .unwrap()
                .db_question_poll_ms,
            0
        );
        assert_eq!(
            load(&[], &[("NEXUS_DB_QUESTION_POLL_MS", "500")])
                .unwrap()
                .db_question_poll_ms,
            500
        );
        // Unbrauchbare Werte fallen auf den Standard zurück, wie bei den übrigen Takten
        for raw in ["", "abc", "-1"] {
            assert_eq!(
                load(&[], &[("HARNESS_DB_QUESTION_POLL_MS", raw)])
                    .unwrap()
                    .db_question_poll_ms,
                DEFAULT_DB_QUESTION_POLL_MS,
                "{raw}"
            );
        }
    }

    #[test]
    fn im_vertragsmodus_bleibt_die_fragen_beobachtung_aus() {
        let config = load(
            &[],
            &[
                ("NEXUS_SERVER_VERSION", CONTRACT_VERSION),
                ("HARNESS_DB_QUESTION_POLL_MS", "500"),
            ],
        )
        .unwrap();
        assert!(config.is_contract());
        assert_eq!(config.db_question_poll_ms, 0);
        assert_eq!(config.dispatch_tick_ms, 0);
    }

    #[test]
    fn nimmt_den_takt_des_dispatchers_aus_der_umgebung_und_laesst_0_zu() {
        assert_eq!(
            load(&[], &[("NEXUS_DISPATCH_TICK_MS", "0")])
                .unwrap()
                .dispatch_tick_ms,
            0
        );
        assert_eq!(
            load(&[], &[("NEXUS_DISPATCH_TICK_MS", "500")])
                .unwrap()
                .dispatch_tick_ms,
            500
        );
        for raw in ["", "abc", "-1"] {
            assert_eq!(
                load(&[], &[("NEXUS_DISPATCH_TICK_MS", raw)])
                    .unwrap()
                    .dispatch_tick_ms,
                DEFAULT_DISPATCH_TICK_MS,
                "{raw}"
            );
        }
    }

    #[test]
    fn nimmt_den_bildordner_aus_der_umgebung_sonst_den_des_crates() {
        assert_eq!(
            load(&[], &[("NEXUS_TOAST_DIR", "/n/toast")])
                .unwrap()
                .toast_dir,
            std::path::Path::new("/n/toast")
        );
        assert_eq!(
            load(&[], &[("HARNESS_TOAST_DIR", "/h/toast")])
                .unwrap()
                .toast_dir,
            std::path::Path::new("/h/toast")
        );
        // Ohne Angabe steht dort ein Ordner mit dem Symbol der App
        let ohne = load(&[], &[]).unwrap();
        assert!(
            ohne.toast_dir.join(TOAST_MARKER).is_file(),
            "{}",
            ohne.toast_dir.display()
        );
    }

    #[test]
    fn bringt_die_taktwerte_mit_standardwerten_mit() {
        let config = load(&[], &[]).unwrap();
        assert_eq!(config.heartbeat_ms, DEFAULT_HEARTBEAT_MS);
        assert_eq!(config.db_poll_ms, DEFAULT_DB_POLL_MS);
        assert_eq!(config.docs_poll_ms, DEFAULT_DOCS_POLL_MS);
        assert_eq!(config.db_question_poll_ms, DEFAULT_DB_QUESTION_POLL_MS);
        assert_eq!(config.dispatch_tick_ms, DEFAULT_DISPATCH_TICK_MS);
    }
}
