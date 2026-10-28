//! `nexus serve`: der Rust-Server auf dem öffentlichen Port, der Node-Server als Kindprozess
//! dahinter.
//!
//! Reihenfolge ist wichtig: Erst wird der öffentliche Port gebunden – ist er belegt, fällt das
//! sofort auf und es bleibt kein Node-Prozess übrig. Dann startet der Node-Server auf einem
//! freien internen Port, danach beantwortet der Rust-Server Anfragen und schreibt `server.json`
//! mit seiner eigenen Prozessnummer und dem öffentlichen Port. Beim Beenden geht es rückwärts:
//! Server anhalten, `server.json` löschen, Node-Server abräumen.

use std::path::{Path, PathBuf};

use nexus_core::Result;
use nexus_platform::Platform;
use nexus_server::config::{ServerConfig, ServerConfigLoader};
use nexus_server::log::ServerLog;
use nexus_server::sidecar::{NodeSidecar, SidecarSpec};
use serde::Serialize;

use crate::context::CliContext;

/// Was `nexus serve` an Optionen kennt
#[derive(Debug, Clone, Default)]
pub struct ServeOptions {
    pub port: Option<String>,
    pub web_dir: Option<PathBuf>,
    pub node_sidecar: Option<String>,
    pub no_node: bool,
}

/// Das Ergebnis von `nexus serve` (`--json`): die Ports, auf denen beide Server hören
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServeResult {
    port: u16,
    node_port: Option<u16>,
}

/// Führt `serve` aus; die Funktion kehrt erst zurück, wenn der Server beendet ist
pub fn run(context: &CliContext, options: &ServeOptions) -> Result<u8> {
    let config = load_config(options)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(nexus_core::Error::unexpected)?;
    runtime.block_on(serve(context, options, config))
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
async fn serve(
    context: &CliContext,
    options: &ServeOptions,
    mut config: ServerConfig,
) -> Result<u8> {
    let log = ServerLog::new(&config.home);
    // Zuerst der öffentliche Port: ist er belegt, endet der Befehl, bevor etwas gestartet ist
    let listener = nexus_server::bind(&config.host, config.port)
        .await
        .inspect_err(|error| log.write(&error.to_string()))?;
    let public = listener
        .local_addr()
        .map_err(nexus_core::Error::unexpected)?
        .port();
    config.port = public;

    let sidecar = match options.no_node {
        true => None,
        false => {
            let spec = SidecarSpec::resolve(
                options.node_sidecar.as_deref(),
                &|name| std::env::var(name).ok(),
                exe_dir().as_deref(),
            )?;
            Some(NodeSidecar::start(&spec, &config.home, public).await?)
        }
    };
    if let Some(sidecar) = &sidecar {
        config.upstream = Some(format!("http://127.0.0.1:{}", sidecar.port));
    }

    let board = nexus_server::board_from_config(config.home.clone());
    let server =
        match nexus_server::serve_on(listener, config, board, Platform::unavailable()).await {
            Ok(server) => server,
            Err(error) => {
                // Der Node-Server darf nicht übrig bleiben, wenn der Rust-Server nicht hochkommt
                if let Some(sidecar) = sidecar {
                    sidecar.stop().await;
                }
                return Err(error);
            }
        };
    let node_port = sidecar.as_ref().map(|sidecar| sidecar.port);
    context.printer().result(
        &ServeResult {
            port: server.port,
            node_port,
        },
        &[match node_port {
            Some(port) => format!(
                "Server läuft auf http://127.0.0.1:{} (Node-Server auf Port {port})",
                server.port
            ),
            None => format!(
                "Server läuft auf http://127.0.0.1:{} (ohne Node-Server)",
                server.port
            ),
        }],
    );

    nexus_server::shutdown_signal().await;
    server.stop().await;
    if let Some(sidecar) = sidecar {
        sidecar.stop().await;
    }
    Ok(0)
}

/// Ordner des laufenden Programms
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

/// `web` neben dem Programm, falls es den Ordner gibt
fn web_dir_beside_exe() -> Option<PathBuf> {
    exe_dir()
        .map(|dir| dir.join("web"))
        .filter(|dir| dir.is_dir())
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
