// Einstieg der Desktop-App: Plugins, Zustand und Fenster einrichten.
pub fn run() {
    let builder = tauri::Builder::default();
    // Folge-Tickets hängen hier je Funktion genau eine Zeile an (server::init, tray::init, …)
    builder
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("Nexus konnte nicht starten");
}
