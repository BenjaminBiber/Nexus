// Einstieg der Desktop-App: Plugins, Zustand und Fenster einrichten.
mod http;
mod instance;
mod lifecycle;
mod server;
mod tray;
mod updater;
mod worker;

use tauri_plugin_window_state::StateFlags;

/// Start als Dispatch-Worker (`<app> dispatch-worker <lauf>`), falls die Argumente das sagen.
/// `true`: der Lauf ist abgearbeitet, die App startet nicht.
pub fn run_dispatch_worker() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(run_id) = worker::lauf_aus_argumenten(&args) else {
        return false;
    };
    worker::run(&run_id);
    true
}

pub fn run() {
    let builder = tauri::Builder::default()
        // Single-Instance muss das erste Plugin sein, damit der zweite Start sofort abbricht und
        // seine nexus://-Links an die laufende Instanz weitergibt
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            instance::focus_main(app, instance::deep_link_in_args(&argv));
        }))
        .plugin(tauri_plugin_deep_link::init())
        // Größe und Position bleiben erhalten, aber nicht die Sichtbarkeit:
        // ein ins Tray verstecktes Fenster soll beim nächsten Start wieder da sein
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .build(),
        )
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
        .plugin(tauri_plugin_opener::init())
        // Systemmeldungen der Oberfläche (bereite Sessions, Meldungen des Servers)
        .plugin(tauri_plugin_notification::init())
        // Selbstaktualisierung; geprüft wird aus updater::init, nicht aus dem Fenster
        .plugin(tauri_plugin_updater::Builder::new().build());
    // Folge-Tickets hängen hier je Funktion genau eine Zeile an (tray::init, …)
    let builder = builder
        .on_page_load(server::on_page_load)
        // Schließen versteckt das Fenster, solange es ein Tray-Symbol gibt
        .on_window_event(lifecycle::fenster_ereignis)
        // „Nexus beenden“ aus dem Menü der App geht über die Rückfrage
        .on_menu_event(lifecycle::menu_ereignis)
        .setup(|app| {
            lifecycle::init(app.handle());
            server::init(app.handle());
            tray::init(app.handle());
            instance::setup(app.handle());
            updater::init(app.handle());
            Ok(())
        });
    // Auf macOS ersetzt ein eigenes Menü das vorgefertigte „Quit“, damit Cmd+Q die Rückfrage auslöst
    #[cfg(target_os = "macos")]
    let builder = builder.menu(lifecycle::app_menu);
    builder
        .build(tauri::generate_context!())
        .expect("Nexus konnte nicht starten")
        .run(|app, event| {
            // Dock-Klick zeigt das Fenster, Cmd+Q fragt bei laufendem Dispatch nach
            lifecycle::lauf_ereignis(app, &event);
            // Beim Beenden nimmt die App den Server mit, den sie selbst gestartet hat
            if matches!(event, tauri::RunEvent::Exit) {
                server::shutdown(app);
            }
        });
}
