//! Entwicklungs-Binary des Rust-Servers: startet ihn mit `--port`, `--upstream` und
//! `--web-dir`.
//!
//! Später startet die Tauri-App denselben Server im eigenen Prozess; dieses Binary gibt es,
//! damit der Vertrag und die Handarbeit ihn ohne die App starten können:
//!
//! ```sh
//! cargo run -p nexus-server -- --port 4710 --upstream http://127.0.0.1:4711
//! ```

use std::process::ExitCode;

use nexus_platform::Platform;
use nexus_server::config::ServerConfigLoader;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut config = match ServerConfigLoader::load_from_process(&args) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    // Ohne Angabe liegt die gebaute Oberfläche im Repo neben dem Serverpaket
    if config.web_dir.is_none() {
        config.web_dir = Some(repo_web_dir());
    }
    let board = nexus_server::board_from_config(config.home.clone());
    let server = match nexus_server::serve(config, board, Platform::unavailable()).await {
        Ok(server) => server,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "Nexus-Server auf http://127.0.0.1:{} (Benutzerordner {})",
        server.port,
        server.state.home.dir().display()
    );
    nexus_server::shutdown_signal().await;
    server.stop().await;
    ExitCode::SUCCESS
}

/// `<Repo>/packages/web/dist`, aus dem Ordner dieses Crates heraus gerechnet
fn repo_web_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("packages")
        .join("web")
        .join("dist")
}
