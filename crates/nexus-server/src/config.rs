//! Einstellungen des Servers und ihr Lader; portiert aus
//! `packages/server/src/config/ServerConfig.ts` und `ServerConfigLoader.ts`.

use std::collections::HashMap;
use std::path::PathBuf;

use nexus_core::clock::Clock;
use nexus_core::registry::DEFAULT_SERVER_PORT;
use nexus_core::{HarnessError, HarnessHome, Result};

use crate::groups::RouteGroups;

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
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Port, auf dem gehört wird (0 = freien Port wählen)
    pub port: u16,
    /// Host; immer [`HOST`]
    pub host: String,
    /// Benutzerordner mit `server.json`, `server.log`, `board.yaml` …
    pub home: HarnessHome,
    /// Ordner der gebauten Oberfläche; `None`, wenn es keinen gibt
    pub web_dir: Option<PathBuf>,
    /// Version in `GET /api/health`
    pub version: String,
    /// Node-Server, an den alles geht, was Rust noch nicht beantwortet; `None` = kein Proxy
    pub upstream: Option<String>,
    /// Routengruppen, die trotz Rust-Umsetzung zum Node-Server gehen (Notschalter)
    pub proxy_groups: RouteGroups,
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
    /// Uhr des Servers; Tests setzen eine feste, damit Zeiträume nachrechenbar sind
    pub clock: Clock,
}

/// Version, die den Vertragsmodus kennzeichnet (`NEXUS_SERVER_VERSION=contract`)
pub const CONTRACT_VERSION: &str = "contract";

impl ServerConfig {
    /// Läuft der Server im Vertragsmodus? Dort gibt es keinen Ordnerdialog, kein Öffnen von
    /// Dateien und keine Systemmeldungen
    pub fn is_contract(&self) -> bool {
        self.version == CONTRACT_VERSION
    }

    /// Trägt den Node-Server dahinter ein und lässt damit den Notschalter
    /// `NEXUS_PROXY_GROUPS` gelten. Nötig, weil der Port des Sidecars erst nach dem Laden
    /// feststeht, der Notschalter aber nur mit Upstream wirkt: ohne ihn gäbe es niemanden,
    /// der die abgeschaltete Gruppe stattdessen beantwortet.
    pub fn set_upstream(&mut self, url: String, env: &dyn Fn(&str) -> Option<String>) {
        self.upstream = Some(url);
        self.proxy_groups = RouteGroups::from_env(value(env, &["NEXUS_PROXY_GROUPS"]).as_deref());
    }

    /// Einstellungen mit den Standardwerten für einen Benutzerordner (Tests und Aufrufer,
    /// die Port und Ordner schon kennen)
    pub fn new(port: u16, home: HarnessHome) -> Self {
        Self {
            port,
            host: HOST.to_string(),
            home,
            web_dir: None,
            version: default_version(),
            upstream: None,
            proxy_groups: RouteGroups::none(),
            heartbeat_ms: DEFAULT_HEARTBEAT_MS,
            db_poll_ms: DEFAULT_DB_POLL_MS,
            docs_poll_ms: DEFAULT_DOCS_POLL_MS,
            db_question_poll_ms: DEFAULT_DB_QUESTION_POLL_MS,
            dispatch_tick_ms: DEFAULT_DISPATCH_TICK_MS,
            clock: Clock::system(),
        }
    }
}

/// Version, die `GET /api/health` meldet: die des Workspace
fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
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
        let upstream = argument(args, "--upstream")
            .or_else(|| value(env, &["NEXUS_UPSTREAM"]))
            .filter(|url| !url.is_empty());
        let mut config = ServerConfig {
            port: port(args, env)?,
            web_dir,
            version: value(env, &["NEXUS_SERVER_VERSION", "HARNESS_BUNDLED_VERSION"])
                .filter(|version| !version.is_empty())
                .unwrap_or_else(default_version),
            ..ServerConfig::new(0, home)
        };
        if let Some(upstream) = upstream {
            config.set_upstream(upstream, env);
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
    use crate::groups::RouteGroup;

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
    fn nimmt_benutzerordner_weboberflache_und_upstream_mit_ruckfall() {
        let config = load(
            &[],
            &[
                ("NEXUS_HOME", "/n/home"),
                ("HARNESS_HOME", "/h/home"),
                ("HARNESS_WEB_DIR", "/h/web"),
                ("NEXUS_UPSTREAM", "http://127.0.0.1:4711"),
            ],
        )
        .unwrap();
        assert_eq!(config.home.dir(), std::path::Path::new("/n/home"));
        assert_eq!(
            config.web_dir.as_deref(),
            Some(std::path::Path::new("/h/web"))
        );
        assert_eq!(config.upstream.as_deref(), Some("http://127.0.0.1:4711"));
        assert_eq!(config.host, HOST);

        let nur_harness = load(&[], &[("HARNESS_HOME", "/h/home")]).unwrap();
        assert_eq!(nur_harness.home.dir(), std::path::Path::new("/h/home"));

        let argumente = load(
            &["--web-dir", "/a/web", "--upstream", "http://127.0.0.1:9"],
            &[("NEXUS_WEB_DIR", "/n/web")],
        )
        .unwrap();
        assert_eq!(
            argumente.web_dir.as_deref(),
            Some(std::path::Path::new("/a/web"))
        );
        assert_eq!(argumente.upstream.as_deref(), Some("http://127.0.0.1:9"));

        let ohne = load(&[], &[]).unwrap();
        assert_eq!(ohne.web_dir, None);
        assert_eq!(ohne.upstream, None);
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
    fn der_notschalter_der_routengruppen_wirkt_nur_mit_upstream() {
        let mit = load(
            &["--upstream", "http://127.0.0.1:4711"],
            &[("NEXUS_PROXY_GROUPS", "basis,docs")],
        )
        .unwrap();
        assert!(mit.proxy_groups.contains(RouteGroup::Basis));
        assert!(mit.proxy_groups.contains(RouteGroup::Docs));

        let ohne = load(&[], &[("NEXUS_PROXY_GROUPS", "basis")]).unwrap();
        assert!(!ohne.proxy_groups.contains(RouteGroup::Basis));
    }

    #[test]
    fn ein_nachtraglicher_upstream_lasst_den_notschalter_gelten() {
        // So startet `nexus serve`: der Port des Node-Servers steht erst nach dem Laden fest
        let mut config = load(&[], &[("NEXUS_PROXY_GROUPS", "db-tickets")]).unwrap();
        assert!(config.proxy_groups.enabled(RouteGroup::DbTickets));

        let umgebung = |name: &str| match name {
            "NEXUS_PROXY_GROUPS" => Some("db-tickets".to_string()),
            _ => None,
        };
        config.set_upstream("http://127.0.0.1:4711".to_string(), &umgebung);
        assert_eq!(config.upstream.as_deref(), Some("http://127.0.0.1:4711"));
        assert!(config.proxy_groups.contains(RouteGroup::DbTickets));
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
