//! `nexus serve`: der Server im Vordergrund.
//!
//! Reihenfolge ist wichtig: Erst wird der Port gebunden – ist er belegt, fällt das sofort auf,
//! bevor etwas anderes hochkommt. Danach beantwortet der Server Anfragen und schreibt
//! `server.json` mit seiner Prozessnummer und dem Port. Beim Beenden geht es rückwärts:
//! Server anhalten, `server.json` löschen.

use std::path::{Path, PathBuf};

use nexus_core::Result;
use nexus_platform::Platform;
use nexus_server::config::{ServerConfig, ServerConfigLoader};
use nexus_server::log::ServerLog;
use serde::Serialize;

use crate::context::CliContext;

/// Was `nexus serve` an Optionen kennt
#[derive(Debug, Clone, Default)]
pub struct ServeOptions {
    pub port: Option<String>,
    pub web_dir: Option<PathBuf>,
}

/// Das Ergebnis von `nexus serve` (`--json`): der Port, auf dem gehört wird
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServeResult {
    port: u16,
}

/// Führt `serve` aus; die Funktion kehrt erst zurück, wenn der Server beendet ist
pub fn run(context: &CliContext, options: &ServeOptions) -> Result<u8> {
    let config = load_config(options)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(nexus_core::Error::unexpected)?;
    runtime.block_on(serve(context, config))
}

/// Einstellungen aus Optionen und Umgebung; Port und Ordner gelten wie beim Server selbst
/// (`--port`, `NEXUS_PORT`, `HARNESS_PORT`, 4710)
fn load_config(options: &ServeOptions) -> Result<ServerConfig> {
    let mut args: Vec<String> = Vec::new();
    if let Some(port) = &options.port {
        args.push("--port".to_string());
        args.push(port.clone());
    }
    if let Some(dir) = &options.web_dir {
        args.push("--web-dir".to_string());
        args.push(dir.to_string_lossy().to_string());
    }
    let mut config = ServerConfigLoader::load_from_process(&args)?;
    // Ohne Angabe liegt die gebaute Oberfläche neben dem Programm (so legt die Paketierung sie ab)
    if config.web_dir.is_none() {
        config.web_dir = web_dir_beside_exe();
    }
    Ok(config)
}

/// Der eigentliche Ablauf; alles davon braucht eine Laufzeit
async fn serve(context: &CliContext, mut config: ServerConfig) -> Result<u8> {
    let log = ServerLog::new(&config.home);
    // Zuerst der Port: ist er belegt, endet der Befehl, bevor etwas gestartet ist
    let listener = nexus_server::bind(&config.host, config.port)
        .await
        .inspect_err(|error| log.write(&error.to_string()))?;
    config.port = listener
        .local_addr()
        .map_err(nexus_core::Error::unexpected)?
        .port();

    let board = nexus_server::board_from_config(config.home.clone());
    let platform = platform(&config);
    let server = nexus_server::serve_on(listener, config, board, platform).await?;
    context.printer().result(
        &ServeResult { port: server.port },
        &[format!("Server läuft auf http://127.0.0.1:{}", server.port)],
    );

    nexus_server::shutdown_signal().await;
    server.stop().await;
    Ok(0)
}

/// Ordner des laufenden Programms
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

/// Die gebaute Oberfläche neben dem Programm, falls es sie gibt. Gesucht wird in der
/// Reihenfolge der drei Lagen: neben dem Programm (Paketierung der App), eine Ebene höher
/// (Plugin: `bin/nexus` und `web/`) und im Benutzerordner (`nexus install`: `bin/nexus`
/// und `app/web/`).
fn web_dir_beside_exe() -> Option<PathBuf> {
    let dir = exe_dir()?;
    [
        dir.join("web"),
        dir.join("..").join("web"),
        dir.join("..").join("app").join("web"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_dir())
}

/// Systemfunktionen zu diesen Einstellungen: im Vertragsmodus die des Vertrags, sonst die
/// dieses Rechners
fn platform(config: &ServerConfig) -> Platform {
    match config.is_contract() {
        true => Platform::contract(),
        false => Platform::local(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nimmt_den_port_aus_der_option() {
        let options = ServeOptions {
            port: Some("4812".to_string()),
            ..ServeOptions::default()
        };
        assert_eq!(load_config(&options).unwrap().port, 4812);
    }

    #[test]
    fn ein_kaputter_port_wird_abgelehnt() {
        let options = ServeOptions {
            port: Some("abc".to_string()),
            ..ServeOptions::default()
        };
        let error = load_config(&options).unwrap_err();
        assert_eq!(error.code(), Some("port-invalid"));
    }

    #[test]
    fn nimmt_den_ordner_der_oberflaeche_aus_der_option() {
        let options = ServeOptions {
            web_dir: Some(PathBuf::from("/eigen/web")),
            ..ServeOptions::default()
        };
        assert_eq!(
            load_config(&options).unwrap().web_dir.as_deref(),
            Some(Path::new("/eigen/web"))
        );
    }
}
