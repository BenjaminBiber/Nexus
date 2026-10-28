// Einstieg der Desktop-App: Plugins, Zustand und Fenster einrichten.
mod server;

pub fn run() {
    let builder = tauri::Builder::default()
        // Info genügt; auf Trace würden die Bibliotheken unter reqwest das Protokoll fluten
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        // Ordnerdialog der Oberfläche (Repo-Pfad eines Boards)
        .plugin(tauri_plugin_dialog::init())
        // Datei im Dateimanager zeigen, VS Code und den Standardbrowser öffnen
        .plugin(tauri_plugin_opener::init());
    // Folge-Tickets hängen hier je Funktion genau eine Zeile an (tray::init, …)
    builder
        .on_page_load(server::on_page_load)
        .setup(|app| {
            server::init(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Nexus konnte nicht starten")
        .run(|app, event| {
            // Beim Beenden nimmt die App den Server mit, den sie selbst gestartet hat
            if matches!(event, tauri::RunEvent::Exit) {
                server::shutdown(app);
            }
        });
}
